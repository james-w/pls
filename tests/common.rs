#![allow(dead_code)]

use std::process::Command;

use assert_cmd::prelude::*;
use assert_fs::prelude::*;

// Cross-platform command helpers for tests

/// Get a copy file command that works on both Unix and Windows
#[cfg(unix)]
pub fn copy_command(src: &str, dst: &str) -> String {
    format!("cp {} {}", src, dst)
}

#[cfg(windows)]
pub fn copy_command(src: &str, dst: &str) -> String {
    // On Windows, pls runs commands through cmd.exe /C automatically
    format!("copy {} {}", src, dst)
}

/// Get a command that prints the current working directory
#[cfg(unix)]
pub fn pwd_command() -> &'static str {
    "pwd"
}

#[cfg(windows)]
pub fn pwd_command() -> &'static str {
    // On Windows, pls runs commands through cmd.exe /C automatically
    "cd"
}

/// Get a command that lists environment variables
#[cfg(unix)]
pub fn env_command() -> &'static str {
    "env"
}

#[cfg(windows)]
pub fn env_command() -> &'static str {
    // On Windows, pls runs commands through cmd.exe /C automatically
    "set"
}

/// Get a shell command that echoes a message and sleeps
#[cfg(unix)]
pub fn echo_and_sleep(msg: &str, secs: u32) -> String {
    format!("bash -c 'echo {}; sleep {}'", msg, secs)
}

#[cfg(windows)]
pub fn echo_and_sleep(msg: &str, secs: u32) -> String {
    // On Windows, pls runs commands through cmd.exe /C automatically
    // ping localhost -n N waits approximately N-1 seconds
    format!("echo {} & ping localhost -n {} >nul", msg, secs + 1)
}

/// Get a shell command that echoes multiple lines and sleeps
#[cfg(unix)]
pub fn echo_lines_and_sleep(lines: &[&str], secs: u32) -> String {
    let echo_cmds: Vec<String> = lines.iter().map(|l| format!("echo {}", l)).collect();
    format!("bash -c '{}; sleep {}'", echo_cmds.join("; "), secs)
}

#[cfg(windows)]
pub fn echo_lines_and_sleep(lines: &[&str], secs: u32) -> String {
    // On Windows, pls runs commands through cmd.exe /C automatically
    let echo_cmds: Vec<String> = lines.iter().map(|l| format!("echo {}", l)).collect();
    format!(
        "{} & ping localhost -n {} >nul",
        echo_cmds.join(" & "),
        secs + 1
    )
}

/// Get a shell command that runs a loop echoing lines
#[cfg(unix)]
pub fn echo_loop_and_sleep(count: u32, secs: u32) -> String {
    format!(
        "bash -c 'i=1; while [ $i -le {} ]; do echo line$i; i=$((i+1)); done; sleep {}'",
        count, secs
    )
}

#[cfg(windows)]
pub fn echo_loop_and_sleep(count: u32, secs: u32) -> String {
    // On Windows, pls runs commands through cmd.exe /C automatically.
    // We need to echo all lines FIRST, then do the sleep, matching Unix behavior.
    // The parentheses ensure all echoes happen before the ping delay.
    format!(
        "(for /L %i in (1,1,{}) do @echo line%i) & ping localhost -n {} >nul",
        count,
        secs + 1
    )
}

/// Get a shell command for a long-running daemon (with date output)
#[cfg(unix)]
pub fn daemon_loop_command() -> &'static str {
    "bash -c 'i=0; while [ $i -lt 10 ]; do i=$((i+1)); date; sleep 1; done'"
}

#[cfg(windows)]
pub fn daemon_loop_command() -> &'static str {
    // On Windows, pls runs commands through cmd.exe /C automatically
    "for /L %i in (1,1,10) do @echo %TIME% & ping localhost -n 2 >nul"
}

/// Get a shell command that writes pwd to a file
#[cfg(unix)]
pub fn pwd_to_file_command(filename: &str) -> String {
    format!("sh -c 'pwd > {}'", filename)
}

#[cfg(windows)]
pub fn pwd_to_file_command(filename: &str) -> String {
    // On Windows, pls runs commands through cmd.exe /C automatically
    format!("cd > {}", filename)
}

pub struct TestContext {
    pub workdir: assert_fs::TempDir,
}

impl Default for TestContext {
    fn default() -> Self {
        Self {
            workdir: assert_fs::TempDir::new().unwrap(),
        }
    }
}

impl TestContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new test context with git initialized (for Go tests that need VCS)
    #[allow(dead_code)]
    pub fn new_with_git() -> Self {
        let context = Self::new();
        // Initialize git repo so Go's VCS stamping works
        Command::new("git")
            .args(["init"])
            .current_dir(context.workdir())
            .output()
            .expect("Failed to initialize git");
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(context.workdir())
            .output()
            .expect("Failed to set git user.email");
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(context.workdir())
            .output()
            .expect("Failed to set git user.name");
        context
    }

    pub fn workdir(&self) -> &std::path::Path {
        self.workdir.path()
    }

    pub fn write_config(&self, config_src: &str) {
        let config_path = self.workdir.child("pls.toml");
        config_path.write_str(config_src).unwrap();
    }

    pub fn add_context(&self, cmd: &mut Command) {
        cmd.arg("-C").arg(self.workdir());
    }

    pub fn get_command(&self) -> Command {
        let mut cmd = Command::cargo_bin("pls").unwrap();
        cmd.arg("--debug");
        self.add_context(&mut cmd);
        cmd
    }
}
