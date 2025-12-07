use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use log::debug;
use nix::errno::Errno;
use nix::fcntl::{Flock, FlockArg};
use nix::sys::signal::{self, Signal};
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;

/// Raw process ID type for Unix
pub type RawPid = i32;

/// Check if a process is alive
pub fn is_process_alive(pid: super::ProcessId) -> bool {
    signal::kill(Pid::from_raw(pid.as_raw()), None).is_ok()
}

/// Stop a process, first trying SIGTERM then escalating to SIGKILL
pub fn stop_process(pid: super::ProcessId) -> Result<()> {
    let nix_pid = Pid::from_raw(pid.as_raw());
    let mut current_signal = Signal::SIGTERM;
    let start = std::time::Instant::now();

    send_signal(nix_pid, current_signal)
        .map_err(|e| anyhow!("Error sending kill signal to process <{}>: {}", pid, e))?;

    while is_process_alive(pid) {
        if start.elapsed() > Duration::from_secs(10) {
            current_signal = Signal::SIGKILL;
            send_signal(nix_pid, current_signal)
                .map_err(|e| anyhow!("Error sending kill signal to process <{}>: {}", pid, e))?;
        }
        let status = waitpid(nix_pid, Some(WaitPidFlag::WNOHANG))
            .map_or_else(
                |err| {
                    if err == Errno::ECHILD {
                        Ok(None)
                    } else {
                        Err(err)
                    }
                },
                |x| Ok(Some(x)),
            )
            .map_err(|e| anyhow!("Error waiting for process {}: {}", pid, e))?;

        if let Some(status) = status {
            match status {
                WaitStatus::Exited(_, _) => {
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

fn send_signal(pid: Pid, signal: Signal) -> Result<()> {
    debug!("Sending <{}> to process <{}>", signal, pid);
    match signal::kill(pid, signal) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("Failed to send signal, got errno: {}", e)),
    }
}

/// Inner type for file locking on Unix
pub struct FileLockInner {
    flock: Flock<File>,
}

impl Read for FileLockInner {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.flock.read(buf)
    }
}

impl Write for FileLockInner {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.flock.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.flock.flush()
    }
}

impl Seek for FileLockInner {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.flock.seek(pos)
    }
}

impl FileLockInner {
    pub fn set_len(&self, size: u64) -> std::io::Result<()> {
        self.flock.set_len(size)
    }
}

/// Try to acquire an exclusive non-blocking lock on a file
pub fn try_lock_file(file: File) -> Result<Option<FileLockInner>> {
    match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
        Ok(flock) => Ok(Some(FileLockInner { flock })),
        Err((_file, Errno::EWOULDBLOCK)) => Ok(None),
        Err((_file, e)) => Err(anyhow!("Failed to lock file: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ProcessId;

    #[test]
    fn test_is_process_alive() {
        let pid = ProcessId::from_raw(std::process::id() as i32);
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_is_process_alive_on_dead_process() {
        let pid = ProcessId::from_raw(-2);
        assert!(!is_process_alive(pid));
    }

    #[test]
    fn test_send_signal() {
        send_signal(Pid::from_raw(std::process::id() as i32), Signal::SIGWINCH).unwrap();
    }

    #[test]
    fn test_send_signal_on_dead_process() {
        assert!(send_signal(Pid::from_raw(-2), Signal::SIGWINCH).is_err());
    }
}
