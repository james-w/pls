use std::fs::File;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use log::{debug, warn};
use nix::errno::Errno;

use crate::{context::Context, cleanup::CleanupManager};

pub fn escape_string(s: &str) -> Result<String, shlex::QuoteError> {
    Ok(shlex::try_quote(s)?.to_string())
}

pub fn prepend_argument_if_set(arg: &str, value: &Option<&str>) -> Result<String, shlex::QuoteError> {
    prepend_arguments_if_set(arg, &value.map(|v| vec![v]))
}

pub fn prepend_arguments_if_set(
    arg: &str,
    value: &Option<Vec<&str>>,
) -> Result<String, shlex::QuoteError> {
    value.as_ref().map_or_else(
        || Ok("".to_string()),
        |v| {
            v.iter()
                .map(|e| escape_string(e).map(|e| format!("{} {}", arg, e)))
                .collect::<Result<Vec<_>, _>>()
                .map(|vs| vs.join(" "))
        },
    )
}

pub fn escape_and_prepend(
    target_name: &str,
    context: &Context,
    arg: &str,
    value: &Option<String>,
) -> Result<String, shlex::QuoteError> {
    if let Some(v) = value {
        prepend_argument_if_set(
            arg,
            &Some(
                context
                    .resolve_substitutions(v.as_ref(), target_name)
                    .as_str(),
            ),
        )
    } else {
        Ok("".to_string())
    }
}

pub fn escape_and_prepend_vec(
    target_name: &str,
    context: &Context,
    arg: &str,
    value: &Option<Vec<String>>,
) -> Result<String, shlex::QuoteError> {
    if let Some(v) = value {
        let resolved = v.iter()
                    .map(|ref e| context.resolve_substitutions(e, target_name))
                    .collect::<Vec<_>>();
        prepend_arguments_if_set(
            arg,
            &Some(resolved.iter().map(|e| e.as_str()).collect()),
        )
    } else {
        Ok("".to_string())
    }
}

pub fn build_command(command: &str) -> Result<std::process::Command, Box<dyn std::error::Error>> {
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
        Err(Box::from(format!("Command <{}> is empty", command)))
    }
}

pub fn is_process_alive(pid: nix::unistd::Pid) -> bool {
    match nix::sys::signal::kill(pid, None) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn send_signal(pid: nix::unistd::Pid, signal: nix::sys::signal::Signal) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Sending <{}> to process <{}>", signal, pid);
    match nix::sys::signal::kill(pid, signal) {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::from(e)),
    }
}

pub fn stop_process(pid: nix::unistd::Pid) -> Result<(), Box<dyn std::error::Error>> {
    let mut signal = nix::sys::signal::SIGTERM;
    let start = std::time::Instant::now();
    send_signal(pid, signal).map_err(|e| {
        Box::<dyn std::error::Error>::from(format!(
            "Error sending kill signal to process <{}>: {}",
            pid, e
        ))
    })?;
    while is_process_alive(pid) {
        if start.elapsed() > Duration::from_secs(10) {
            signal = nix::sys::signal::SIGKILL;
            send_signal(pid, signal).map_err(|e| {
                Box::<dyn std::error::Error>::from(format!(
                    "Error sending kill signal to process <{}>: {}",
                    pid, e
                ))
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
                Box::<dyn std::error::Error>::from(format!(
                    "Error waiting for process {}: {}",
                    pid, e
                ))
            })?;
        if let Some(status) = status {
            match status {
                nix::sys::wait::WaitStatus::Exited(_, _) => {
                    debug!("Process <{}> exited", pid);
                    break;
                }
                _ => {
                    debug!("Process <{}> still alive, sleeping", pid);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }
    Ok(())
}

pub fn run_command(cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = build_command(cmd)?;
    let status = cmd.status()?;
    if !status.success() {
        return Err(Box::from(format!(
            "Command failed with exit code: {}",
            status.code().unwrap()
        )));
    }
    Ok(())
}

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

pub fn spawn_command_with_pidfile(
    cmd: &str,
    pid_path: &std::path::PathBuf,
    log_path: &std::path::PathBuf,
    on_start: impl Fn() -> (),
) -> Result<(), Box<dyn std::error::Error>> {
    if pid_path.exists() {
        let pid_str = std::fs::read_to_string(&pid_path)?;
        debug!(
            "Found pid file at <{}>, with contents <{}>, checking if it is alive",
            pid_path.display(),
            pid_str.trim()
        );
        let pid = pid_str.trim().parse::<i32>()?;
        if is_process_alive(nix::unistd::Pid::from_raw(pid)) {
            return Err(Box::from(format!(
                "Daemon for is already running with pid <{}>",
                pid
            )));
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
