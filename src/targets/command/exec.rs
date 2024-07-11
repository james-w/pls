use std::sync::{Arc, Mutex};

use anyhow::Result;
use log::{debug, info};

use crate::cleanup::CleanupManager;
use crate::commands::{run_command, spawn_command_with_pidfile, stop_using_pidfile};
use crate::config::ExecCommand as ConfigExecCommand;
use crate::context::Context;
use crate::default::{default_optional, default_to};
use crate::outputs::OutputsManager;
use crate::target::create_metadata_dir;
use crate::target::{CommandInfo, Runnable, Startable, TargetInfo};

#[derive(Debug, Clone)]
pub struct ExecCommand {
    pub command: String,
    pub default_args: Option<String>,

    pub target_info: TargetInfo,
    pub command_info: CommandInfo,
}

impl ExecCommand {
    pub fn from_config(
        target_info: TargetInfo,
        command_info: CommandInfo,
        defn: &ConfigExecCommand,
        base: Option<&Self>,
    ) -> Self {
        ExecCommand {
            command: default_to!(defn, base, command),
            default_args: default_optional!(defn, base, default_args),
            target_info,
            command_info,
        }
    }
}

impl ExecCommand {
    pub fn resolve_command(
        &self,
        context: &Context,
        outputs: &OutputsManager,
        args: Vec<String>,
    ) -> Result<String> {
        debug!(
            "Resolving command <{}> for target <{}> with args <{:?}>",
            self.command, self.target_info.name, args
        );
        let resolved = context.resolve_substitutions_with_args(
            self.command.as_str(),
            &self.target_info.name,
            outputs,
            args,
            &self.default_args,
        )?;
        debug!("Resolved command to <{}>", resolved);
        Ok(resolved)
    }
}

impl Runnable for ExecCommand {
    fn run(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        // TODO: default_args
        let command = self.resolve_command(context, outputs, args)?;
        debug!(
            "Running target <{}> with command <{}>",
            self.target_info.name, command
        );
        info!("[{}] Running {}", self.target_info.name, command);
        // TODO: cwd
        run_command(command.as_str())
    }
}

impl Startable for ExecCommand {
    fn start(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        let taskrunner_dir = create_metadata_dir(self.target_info.name.to_string().as_str())?;

        let pid_path = taskrunner_dir.join("pid");
        let log_path = taskrunner_dir.join("log");
        // TODO: default_args
        let cmd = self.resolve_command(context, outputs, args)?;
        let log_start = || {
            info!("[{}] Starting {}", self.target_info.name, cmd);
        };
        spawn_command_with_pidfile(cmd.as_str(), &pid_path, &log_path, log_start)
    }

    fn stop(
        &self,
        _context: &Context,
        _outputs: &mut OutputsManager,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let taskrunner_dir = std::env::current_dir()?
            .join(".taskrunner")
            .join(self.target_info.name.to_string().as_str());

        let pid_path = taskrunner_dir.join("pid");
        debug!(
            "Searching for pid file for target <{}> at <{}>",
            self.target_info.name,
            pid_path.display()
        );
        let log_stop = || {
            info!("[{}] Stopping", self.target_info.name);
        };
        stop_using_pidfile(&pid_path, log_stop)
    }
}
