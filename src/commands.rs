#[cfg(unix)]
use std::fs::File;
use std::io::{Seek, SeekFrom};

use anyhow::{anyhow, Result};
use log::debug;

use crate::platform::{self, FileLock, ProcessId, RawPid};

pub fn build_command(command: &str) -> Result<std::process::Command> {
    build_command_with_env(command, &[], None)
}

pub fn build_command_with_env(
    command: &str,
    env: &[String],
    dir: Option<&std::path::Path>,
) -> Result<std::process::Command> {
    let mut cmd = build_command_platform(command)?;
    for env_v in env {
        let split = env_v.split_once('=');
        if let Some((key, val)) = split {
            cmd.env(key, val);
        } else {
            debug!("Setting env var <{}> to <>", env_v);
            cmd.env(env_v, "");
        }
    }
    if let Some(dir) = dir {
        debug!("Setting working directory to <{}>", dir.display());
        cmd.current_dir(dir);
    }
    Ok(cmd)
}

#[cfg(unix)]
fn build_command_platform(command: &str) -> Result<std::process::Command> {
    let mut split = shlex::Shlex::new(command);
    debug!(
        "Split command <{}> into parts: <{}>",
        command,
        split.collect::<Vec<_>>().join(", ")
    );
    split = shlex::Shlex::new(command);
    if let Some(cmd) = split.next() {
        let cmd = std::process::Command::new(cmd);
        Ok(split.fold(cmd, |mut cmd, arg| {
            cmd.arg(arg);
            cmd
        }))
    } else {
        Err(anyhow!("Command <{}> is empty", command))
    }
}

#[cfg(windows)]
fn build_command_platform(command: &str) -> Result<std::process::Command> {
    // On Windows, we run commands through cmd.exe to handle shell builtins
    // like echo, cd, dir, etc. which are not standalone executables.
    debug!("Running command through cmd.exe: <{}>", command);
    let mut cmd = std::process::Command::new("cmd.exe");
    cmd.args(["/C", command]);
    Ok(cmd)
}

/// Spawn a daemon process that runs independently of the parent.
/// On Windows, we use CreateProcessW directly with bInheritHandles=FALSE and
/// DETACHED_PROCESS | CREATE_NO_WINDOW flags to fully detach the child process.
/// We use shell redirection (>> log 2>&1) to capture output to the log file.
#[cfg(windows)]
fn spawn_daemon(
    command: &str,
    env: &[String],
    dir: Option<&std::path::Path>,
    log_path: &std::path::Path,
) -> Result<DaemonChild> {
    use std::ffi::OsStr;
    use std::fs::OpenOptions;
    use std::iter::once;
    use std::mem::zeroed;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
    };

    // CREATE_NO_WINDOW: Don't create a console window for the child process
    // Note: We don't use DETACHED_PROCESS because cmd.exe is a console app and
    // DETACHED_PROCESS can cause it to create a visible console window.
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // Create the log file first so it exists even before the daemon writes to it.
    // This is important for the `logs` command to find the file.
    debug!("Creating log file at <{}>", log_path.display());
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)?;

    // Wrap the command to redirect stdout and stderr to the log file using shell redirection.
    // The "2>&1" redirects stderr to stdout, and ">>" appends to the log file.
    // Note: We don't quote the path because cmd.exe has issues with quoted paths in redirection.
    // This means paths with spaces won't work, but pls metadata paths typically don't have spaces.
    // IMPORTANT: We wrap the command in parentheses so that redirection applies to ALL commands
    // in a chain (e.g., "echo a & echo b" needs "(echo a & echo b) >> file" to capture both).
    let log_path_str = log_path.to_string_lossy();
    let wrapped_command = format!("({}) >> {} 2>&1", command, log_path_str);
    debug!(
        "Starting daemon with wrapped command: <{}>",
        wrapped_command
    );

    // Build environment block if we have custom env vars
    // Format: VAR1=VALUE1\0VAR2=VALUE2\0\0
    let env_block: Option<Vec<u16>> = if env.is_empty() {
        None
    } else {
        // Get current environment and add our vars
        let mut env_strings: Vec<String> = std::env::vars()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        for env_v in env {
            if let Some((key, val)) = env_v.split_once('=') {
                // Remove existing var with same key if present
                env_strings.retain(|s| !s.starts_with(&format!("{}=", key)));
                env_strings.push(format!("{}={}", key, val));
            } else {
                env_strings.retain(|s| !s.starts_with(&format!("{}=", env_v)));
                env_strings.push(format!("{}=", env_v));
            }
        }

        // Convert to wide string block
        let mut block: Vec<u16> = Vec::new();
        for s in env_strings {
            block.extend(OsStr::new(&s).encode_wide());
            block.push(0); // Null terminator for each string
        }
        block.push(0); // Double null terminator at end
        Some(block)
    };

    // Build command line: cmd.exe /C "command"
    let cmd_line = format!("cmd.exe /C \"{}\"", wrapped_command);
    let mut cmd_line_wide: Vec<u16> = OsStr::new(&cmd_line).encode_wide().chain(once(0)).collect();

    // Convert working directory to wide string if specified
    let dir_wide: Option<Vec<u16>> = dir.map(|d| {
        OsStr::new(d.as_os_str())
            .encode_wide()
            .chain(once(0))
            .collect()
    });

    unsafe {
        let mut si: STARTUPINFOW = zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

        let mut pi: PROCESS_INFORMATION = zeroed();

        let result = CreateProcessW(
            ptr::null(),                // lpApplicationName
            cmd_line_wide.as_mut_ptr(), // lpCommandLine
            ptr::null_mut(),            // lpProcessAttributes
            ptr::null_mut(),            // lpThreadAttributes
            0,                          // bInheritHandles = FALSE (key!)
            CREATE_NO_WINDOW,           // dwCreationFlags
            env_block
                .as_ref()
                .map_or(ptr::null(), |b| b.as_ptr() as *const _), // lpEnvironment
            dir_wide.as_ref().map_or(ptr::null(), |d| d.as_ptr()), // lpCurrentDirectory
            &si,                        // lpStartupInfo
            &mut pi,                    // lpProcessInformation
        );

        if result == 0 {
            let error = windows_sys::Win32::Foundation::GetLastError();
            return Err(anyhow!("CreateProcessW failed with error code {}", error));
        }

        let pid = pi.dwProcessId;
        debug!("Started daemon with PID <{}>", pid);

        // Close the thread handle, we don't need it
        CloseHandle(pi.hThread);
        // Close the process handle too - we don't need to wait on it
        CloseHandle(pi.hProcess);

        Ok(DaemonChild { pid })
    }
}

/// A minimal wrapper for daemon child process info on Windows.
/// We don't keep handles open since we fully detach the process.
#[cfg(windows)]
pub struct DaemonChild {
    pid: u32,
}

#[cfg(windows)]
impl DaemonChild {
    pub fn id(&self) -> u32 {
        self.pid
    }
}

/// Spawn a daemon process that runs independently of the parent.
/// On Unix, just spawns normally with stdout/stderr redirected to log files.
#[cfg(unix)]
fn spawn_daemon(
    command: &str,
    env: &[String],
    dir: Option<&std::path::Path>,
    log_path: &std::path::Path,
) -> Result<std::process::Child> {
    let log = File::create(log_path)?;
    let mut cmd = build_command_with_env(command, env, dir)?;
    let child = cmd.stdout(log.try_clone()?).stderr(log).spawn()?;
    Ok(child)
}

pub fn is_process_alive(pid: ProcessId) -> bool {
    platform::is_process_alive(pid)
}

pub fn stop_process(pid: ProcessId) -> Result<()> {
    platform::stop_process(pid)
}

pub fn run_command(cmd: &str) -> Result<()> {
    run_command_with_env(cmd, &[], None)
}

pub fn run_command_with_env(
    cmd: &str,
    env: &[String],
    dir: Option<&std::path::Path>,
) -> Result<()> {
    let mut cmd = build_command_with_env(cmd, env, dir)?;
    let status = cmd.status()?;
    if !status.success() {
        if let Some(code) = status.code() {
            return Err(anyhow!("Command failed with exit code: {}", code));
        } else {
            return Err(anyhow!("Command terminated by a signal"));
        }
    }
    Ok(())
}

/*
pub fn run_command_with_cleanup(cmd: &str, cleanup_manager: Arc<Mutex<CleanupManager>>) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = build_command(cmd)?;
    let mut child = cmd.spawn()?;
    let id = child.id();
    cleanup_manager.lock().unwrap().push_cleanup(move || {
        if let Err(e) = stop_process(nix::unistd::Pid::from_raw(id as i32)) {
            warn!("Error stopping child process: {}", e);
        }
    });
    let status = child.wait()?;
    cleanup_manager.lock().unwrap().pop_cleanup();
    if !status.success() {
        return Err(Box::from(format!(
            "Command failed with exit code: {}",
            status.code().unwrap()
        )));
    }
    Ok(())
}
*/

pub fn spawn_command_with_pidfile(
    cmd: &str,
    env: &[String],
    pid_path: &std::path::Path,
    log_path: &std::path::Path,
    dir: Option<&std::path::Path>,
    on_start: impl Fn(),
    idempotent: bool,
) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};

    // Open/create PID file with locking for idempotent operation
    if idempotent {
        let pid_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(pid_path)?;

        // Try to acquire exclusive lock
        match FileLock::try_lock(pid_file)? {
            Some(mut flock) => {
                // Got the lock - check if there's an existing PID
                let mut existing_pid = String::new();
                flock.read_to_string(&mut existing_pid)?;

                if !existing_pid.is_empty() {
                    let existing_pid_str = existing_pid.trim();
                    debug!(
                        "Found existing pid <{}> in locked file, checking if alive",
                        existing_pid_str
                    );
                    if let Ok(pid) = existing_pid_str.parse::<RawPid>() {
                        if is_process_alive(ProcessId::from_raw(pid)) {
                            debug!("Daemon is already running with pid <{}>", pid);
                            // Daemon is running, unlock and return success
                            return Ok(());
                        }
                        debug!(
                            "Process with pid <{}> is not running, will start new daemon",
                            pid
                        );
                    }
                }

                // No running daemon, proceed to start
                debug!("Starting daemon with command <{}>", cmd);
                on_start();
                let child = spawn_daemon(cmd, env, dir, log_path)?;
                debug!(
                    "Started daemon with pid <{}>, storing at <{}>",
                    child.id(),
                    pid_path.display()
                );

                // Write PID while holding lock
                flock.set_len(0)?; // Truncate file
                flock.seek(SeekFrom::Start(0))?; // Seek to start after truncate
                flock.write_all(child.id().to_string().as_bytes())?;
                flock.flush()?;

                // Lock is automatically released when flock is dropped
                Ok(())
            }
            None => {
                // Someone else holds the lock = daemon is starting or running
                debug!("PID file is locked by another process, daemon is already starting/running");
                Ok(())
            }
        }
    } else {
        // Non-idempotent mode: existing behavior (error if already running)
        if pid_path.exists() {
            let pid_str = std::fs::read_to_string(pid_path)?;
            debug!(
                "Found pid file at <{}>, with contents <{}>, checking if it is alive",
                pid_path.display(),
                pid_str.trim()
            );
            let pid = pid_str.trim().parse::<RawPid>()?;
            if is_process_alive(ProcessId::from_raw(pid)) {
                return Err(anyhow!("Daemon for is already running with pid <{}>", pid));
            }
            debug!("Process with pid <{}> is not running, continuing", pid);
        }

        debug!("Starting daemon with command <{}>", cmd);
        on_start();
        let child = spawn_daemon(cmd, env, dir, log_path)?;
        debug!(
            "Started daemon for with pid <{}>, storing at <{}>",
            child.id(),
            pid_path.display()
        );
        std::fs::write(pid_path, child.id().to_string())?;
        Ok(())
    }
}

pub fn stop_using_pidfile(pid_path: &std::path::Path, on_stop: impl Fn()) -> Result<()> {
    let mut pid_str = std::fs::read_to_string(pid_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => anyhow!("Task not running"),
        _ => anyhow!(
            "Error reading pid file for target at <{}>: {}",
            pid_path.display(),
            e
        ),
    })?;
    pid_str = pid_str.trim().to_string();
    debug!(
        "Found pid <{}> for target at <{}>",
        pid_str,
        pid_path.display()
    );

    let pid = ProcessId::from_raw(pid_str.parse::<RawPid>()?);
    if is_process_alive(pid) {
        on_stop();
        stop_process(pid)?;
    } else {
        debug!("Process with pid <{}> is no longer alive", pid,);
    }
    debug!("Removing pid file at <{}>", pid_path.display());
    std::fs::remove_file(pid_path)?;
    Ok(())
}

pub fn status_using_pidfile(pid_path: &std::path::Path) -> Result<Option<String>> {
    let pid_str = std::fs::read_to_string(pid_path);
    match pid_str {
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => Ok(None),
            _ => Err(anyhow!(
                "Error reading pid file for target at <{}>: {}",
                pid_path.display(),
                e
            )),
        },
        Ok(pid_str) => {
            let pid_str = pid_str.trim().to_string();
            debug!(
                "Found pid <{}> for target at <{}>",
                pid_str,
                pid_path.display()
            );

            let pid = ProcessId::from_raw(pid_str.parse::<RawPid>()?);
            if is_process_alive(pid) {
                Ok(Some(format!("Process running with pid <{}>", pid)))
            } else {
                Ok(None)
            }
        }
    }
}

/*
use daemonize::{Daemonize, Outcome};
        let daemonize = Daemonize::new()
            .pid_file(pid_path)
            .chown_pid_file(true)
            .working_directory(std::env::current_dir()?)
            .stdout(log.try_clone()?)
            .stderr(log);

        match daemonize.execute() {
            Outcome::Parent(Ok(_)) => {
                println!("Started daemon for target <{}>", target.name);
                Ok(())
            }
            Outcome::Parent(Err(e)) => {
                Err(Box::from(format!("Error starting daemon for target <{}>: {}", target.name, e)))
            }
            Outcome::Child(Ok(_)) => {
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(target.command.as_str())
                    .status()?;
                if !status.success() {
                    return Err(Box::from(format!("Command failed with exit code: {}", status.code().unwrap())));
                }
                Ok(())
            }
            Outcome::Child(Err(e)) => {
                Err(Box::from(format!("Error starting daemon for target <{}>: {}", target.name, e)))
            }
        }
*/

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    #[cfg(unix)]
    fn test_build_command_splits() {
        let cmd = build_command("echo hello").unwrap();
        assert_eq!(cmd.get_program(), "echo");
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), &["hello"]);
    }

    #[test]
    #[cfg(windows)]
    fn test_build_command_uses_cmd() {
        let cmd = build_command("echo hello").unwrap();
        assert_eq!(cmd.get_program(), "cmd.exe");
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), &["/C", "echo hello"]);
    }

    #[test]
    fn test_is_process_alive() {
        #[cfg(unix)]
        let pid = ProcessId::from_raw(std::process::id() as i32);
        #[cfg(windows)]
        let pid = ProcessId::from_raw(std::process::id());
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_is_process_alive_on_dead_process() {
        #[cfg(unix)]
        let pid = ProcessId::from_raw(-2);
        #[cfg(windows)]
        let pid = ProcessId::from_raw(u32::MAX);
        assert!(!is_process_alive(pid));
    }

    #[test]
    fn test_stop_process() {
        let start = std::time::Instant::now();

        #[cfg(unix)]
        let cmd = "sleep 4";
        #[cfg(windows)]
        let cmd = "powershell -Command \"Start-Sleep -Seconds 4\"";

        #[allow(clippy::zombie_processes)]
        let child = build_command(cmd).unwrap().spawn().unwrap();

        #[cfg(unix)]
        let pid = ProcessId::from_raw(child.id() as i32);
        #[cfg(windows)]
        let pid = ProcessId::from_raw(child.id());

        assert!(is_process_alive(pid));
        stop_process(pid).unwrap();
        assert!(!is_process_alive(pid));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
