use std::process::Command;

use assert_cmd::prelude::*;
use assert_fs::prelude::*;

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
