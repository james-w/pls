#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};

use anyhow::Result;

/// Platform-specific process ID type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessId(RawPid);

impl ProcessId {
    pub fn from_raw(pid: RawPid) -> Self {
        ProcessId(pid)
    }

    pub fn as_raw(&self) -> RawPid {
        self.0
    }
}

impl std::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A cross-platform file lock wrapper that provides exclusive locking
/// with non-blocking try-lock semantics.
pub struct FileLock {
    inner: FileLockInner,
}

impl FileLock {
    /// Try to acquire an exclusive lock on the file.
    /// Returns Ok(Some(lock)) if successful, Ok(None) if the file is already locked,
    /// or Err if there was an error.
    pub fn try_lock(file: File) -> Result<Option<Self>> {
        match try_lock_file(file) {
            Ok(Some(inner)) => Ok(Some(FileLock { inner })),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Read for FileLock {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for FileLock {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl Seek for FileLock {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(pos)
    }
}

impl FileLock {
    pub fn set_len(&self, size: u64) -> std::io::Result<()> {
        self.inner.set_len(size)
    }
}
