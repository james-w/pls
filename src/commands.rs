use std::fs::File;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use log::debug;
use nix::errno::Errno;

pub fn build_command(command: &str) -> Result<std::process::Command> {
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

pub fn is_process_alive(pid: nix::unistd::Pid) -> bool {
    match nix::sys::signal::kill(pid, None) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn send_signal(
    pid: nix::unistd::Pid,
    signal: nix::sys::signal::Signal,
) -> Result<()> {
    debug!("Sending <{}> to process <{}>", signal, pid);
    match nix::sys::signal::kill(pid, signal) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("Failed to send signal, got errno: {}", e)),
    }
}

pub fn stop_process(pid: nix::unistd::Pid) -> Result<()> {
    let mut signal = nix::sys::signal::SIGTERM;
    let start = std::time::Instant::now();
    send_signal(pid, signal).map_err(|e| {
        anyhow!(
            "Error sending kill signal to process <{}>: {}",
            pid, e
        )
    })?;
    while is_process_alive(pid) {
        if start.elapsed() > Duration::from_secs(10) {
            signal = nix::sys::signal::SIGKILL;
            send_signal(pid, signal).map_err(|e| {
                anyhow!(
                    "Error sending kill signal to process <{}>: {}",
                    pid, e
                )
            })?;
        }
        let status = nix::sys::wait::waitpid(pid, Some(nix::sys::wait::WaitPidFlag::WNOHANG))
            .map_or_else(
                |err| {
                    if err == Errno::ECHILD {
                        return Ok(None);
                    } else {
                        return Err(err);
                    }
                },
                |x| Ok(Some(x)),
            )
            .map_err(|e| {
                anyhow!(
                    "Error waiting for process {}: {}",
                    pid, e
                )
            })?;
        if let Some(status) = status {
            match status {
                nix::sys::wait::WaitStatus::Exited(_, _) => {
                    debug!("Process <{}> exited", pid);
                    break;
                }
                _ => {
                    let sleep_time = 100;
                    if start.elapsed().as_millis() % 1000 < (sleep_time as f64 * 1.5) as u128 {
                        debug!("Process <{}> still alive, sleeping", pid);
                    }
                    thread::sleep(Duration::from_millis(sleep_time));
                }
            }
        }
    }
    Ok(())
}

pub fn run_command(cmd: &str) -> Result<()> {
    let mut cmd = build_command(cmd)?;
    let status = cmd.status()?;
    if !status.success() {
        if let Some(code) = status.code() {
            return Err(anyhow!("Command failed with exit code: {}", code));
        } else {
            return Err(anyhow!(
                "Command terminated by a signal"
            ));
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
    pid_path: &std::path::PathBuf,
    log_path: &std::path::PathBuf,
    on_start: impl Fn() -> (),
) -> Result<()> {
    if pid_path.exists() {
        let pid_str = std::fs::read_to_string(&pid_path)?;
        debug!(
            "Found pid file at <{}>, with contents <{}>, checking if it is alive",
            pid_path.display(),
            pid_str.trim()
        );
        let pid = pid_str.trim().parse::<i32>()?;
        if is_process_alive(nix::unistd::Pid::from_raw(pid)) {
            return Err(anyhow!(
                "Daemon for is already running with pid <{}>",
                pid
            ));
        }
        debug!("Process with pid <{}> is not running, continuing", pid);
    }

    debug!("Creating log file at <{}>", log_path.display());
    let log = File::create(log_path)?;

    debug!("Starting daemon with command <{}>", cmd);
    on_start();
    // TODO: cwd
    let mut cmd = build_command(cmd)?;
    let child = cmd
        .stdout(log.try_clone()?)
        .stderr(log.try_clone()?)
        .spawn()?;
    debug!(
        "Started daemon for with pid <{}>, storing at <{}>",
        child.id(),
        pid_path.display()
    );
    std::fs::write(&pid_path, child.id().to_string())?;
    Ok(())
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

    #[test]
    fn test_build_command_splits() {
        let cmd = build_command("echo hello").unwrap();
        assert_eq!(cmd.get_program(), "echo");
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), &["hello"]);
    }

    #[test]
    fn test_is_process_alive() {
        let pid = nix::unistd::Pid::from_raw(std::process::id() as i32);
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_is_process_alive_on_dead_process() {
        let pid = nix::unistd::Pid::from_raw(-2);
        assert!(!is_process_alive(pid));
    }

    #[test]
    fn test_send_signal() {
        send_signal(
            nix::unistd::Pid::from_raw(std::process::id() as i32),
            nix::sys::signal::SIGWINCH,
        )
        .unwrap();
    }

    #[test]
    fn test_send_signal_on_dead_process() {
        assert!(send_signal(nix::unistd::Pid::from_raw(-2), nix::sys::signal::SIGWINCH).is_err());
    }

    #[test]
    fn test_stop_process() {
        let start = std::time::Instant::now();
        let child = build_command("sleep 4").unwrap().spawn().unwrap();
        let pid = nix::unistd::Pid::from_raw(child.id() as i32);
        assert!(is_process_alive(pid));
        stop_process(pid).unwrap();
        assert!(!is_process_alive(pid));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
