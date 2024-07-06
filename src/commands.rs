use std::fs::File;

use log::debug;
use nix::errno::Errno;

pub fn build_command(command: &str) -> Result<std::process::Command, Box<dyn std::error::Error>> {
    let mut split = shlex::Shlex::new(command);
    debug!("Split command <{}> into parts: <{}>", command, split.collect::<Vec<_>>().join(", "));
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

fn send_kill_signal(pid: nix::unistd::Pid) -> Result<(), Box<dyn std::error::Error>> {
    match nix::sys::signal::kill(pid, nix::sys::signal::SIGTERM) {
        Ok(_) => Ok(()),
        Err(e) => Err(Box::from(e)),
    }
}

pub fn stop_process(pid: nix::unistd::Pid) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: don't send signal on every loop
    // TODO: switch to SIGKILL after a timeout
    while is_process_alive(pid) {
        debug!("Sending SIGTERM to process <{}>", pid);
        send_kill_signal(pid).map_err(|e| Box::<dyn std::error::Error>::from(format!("Error sending kill signal to process <{}>: {}", pid, e)))?;
        nix::sys::wait::waitpid(pid, None).map_or_else(|err| if err == Errno::ECHILD { return Ok(None) } else { return Err(err) }, |x| Ok(Some(x))).map_err(|e| Box::<dyn std::error::Error>::from(format!("Error waiting for process {}: {}", pid, e)))?;
        break;
    }
    Ok(())
}

pub fn spawn_command_with_pidfile(cmd: &str, pid_path: &std::path::PathBuf, log_path: &std::path::PathBuf, on_start: impl Fn() -> ()) -> Result<(), Box<dyn std::error::Error>> {
    if pid_path.exists() {
        let pid_str = std::fs::read_to_string(&pid_path)?;
        debug!("Found pid file at <{}>, with contents <{}>, checking if it is alive", pid_path.display(), pid_str.trim());
        let pid = pid_str.trim().parse::<i32>()?;
        if is_process_alive(nix::unistd::Pid::from_raw(pid)) {
            return Err(Box::from(format!("Daemon for is already running with pid <{}>", pid)));
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
    debug!("Started daemon for with pid <{}>, storing at <{}>", child.id(), pid_path.display());
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
