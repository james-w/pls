use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem::zeroed;
use std::os::windows::io::AsRawHandle;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use log::debug;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_LOCK_VIOLATION, HANDLE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{
    LockFileEx, UnlockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, TerminateProcess, WaitForSingleObject,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};

/// Raw process ID type for Windows
pub type RawPid = u32;

const STILL_ACTIVE: u32 = 259;

/// Check if a process is alive
pub fn is_process_alive(pid: super::ProcessId) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid.as_raw());
        if handle.is_null() {
            return false;
        }

        let mut exit_code: u32 = 0;
        let result = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);

        result != 0 && exit_code == STILL_ACTIVE
    }
}

/// Stop a process by terminating it and its child processes
/// Uses taskkill /T /F to terminate the entire process tree
pub fn stop_process(pid: super::ProcessId) -> Result<()> {
    // First check if process is even alive
    if !is_process_alive(pid) {
        debug!("Process <{}> already exited", pid);
        return Ok(());
    }

    // Use taskkill /T /F to terminate the entire process tree
    // /T = terminate child processes
    // /F = force termination
    debug!("Terminating process tree for <{}>", pid);
    let output = std::process::Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .output();

    match output {
        Ok(output) => {
            if !output.status.success() {
                // taskkill may fail if process already exited
                if !is_process_alive(pid) {
                    debug!("Process <{}> already exited", pid);
                    return Ok(());
                }
                let stderr = String::from_utf8_lossy(&output.stderr);
                debug!("taskkill stderr: {}", stderr);
            }
        }
        Err(e) => {
            debug!("taskkill failed: {}", e);
            // Fall back to TerminateProcess
            return terminate_single_process(pid);
        }
    }

    // Wait for process to exit
    let start = std::time::Instant::now();
    while is_process_alive(pid) {
        if start.elapsed() > Duration::from_secs(5) {
            debug!("Timeout waiting for process <{}> to exit", pid);
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    debug!("Process <{}> exited", pid);
    Ok(())
}

/// Terminate a single process (fallback if taskkill is unavailable)
fn terminate_single_process(pid: super::ProcessId) -> Result<()> {
    unsafe {
        let handle = OpenProcess(
            PROCESS_TERMINATE | PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid.as_raw(),
        );
        if handle.is_null() {
            let error = GetLastError();
            if !is_process_alive(pid) {
                return Ok(());
            }
            return Err(anyhow!(
                "Failed to open process {}: error code {}",
                pid,
                error
            ));
        }

        debug!("Terminating process <{}>", pid);
        if TerminateProcess(handle, 1) == 0 {
            let error = GetLastError();
            CloseHandle(handle);
            if !is_process_alive(pid) {
                return Ok(());
            }
            return Err(anyhow!(
                "Failed to terminate process {}: error code {}",
                pid,
                error
            ));
        }

        // Wait for process to exit
        let start = std::time::Instant::now();
        loop {
            let wait_result = WaitForSingleObject(handle, 100);
            if wait_result == WAIT_OBJECT_0 {
                debug!("Process <{}> exited", pid);
                break;
            }
            if start.elapsed() > Duration::from_secs(5) {
                debug!("Timeout waiting for process <{}> to exit", pid);
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        CloseHandle(handle);
        Ok(())
    }
}

/// Inner type for file locking on Windows
pub struct FileLockInner {
    file: File,
    handle: HANDLE,
}

impl Read for FileLockInner {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }
}

impl Write for FileLockInner {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

impl Seek for FileLockInner {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos)
    }
}

impl FileLockInner {
    pub fn set_len(&self, size: u64) -> std::io::Result<()> {
        self.file.set_len(size)
    }
}

impl Drop for FileLockInner {
    fn drop(&mut self) {
        unsafe {
            let mut overlapped: windows_sys::Win32::System::IO::OVERLAPPED = zeroed();
            UnlockFileEx(self.handle, 0, u32::MAX, u32::MAX, &mut overlapped);
        }
    }
}

/// Try to acquire an exclusive non-blocking lock on a file
pub fn try_lock_file(file: File) -> Result<Option<FileLockInner>> {
    unsafe {
        let handle = file.as_raw_handle() as HANDLE;
        let mut overlapped: windows_sys::Win32::System::IO::OVERLAPPED = zeroed();

        let result = LockFileEx(
            handle,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            u32::MAX,
            u32::MAX,
            &mut overlapped,
        );

        if result == 0 {
            let error = GetLastError();
            if error == ERROR_LOCK_VIOLATION {
                // Lock is held by another process
                return Ok(None);
            }
            return Err(anyhow!("Failed to lock file: error code {}", error));
        }

        Ok(Some(FileLockInner { file, handle }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::ProcessId;

    #[test]
    fn test_is_process_alive() {
        let pid = ProcessId::from_raw(std::process::id());
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_is_process_alive_on_dead_process() {
        // Use an invalid PID
        let pid = ProcessId::from_raw(u32::MAX);
        assert!(!is_process_alive(pid));
    }
}
