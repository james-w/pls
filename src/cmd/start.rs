use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::context::{CommandLookupResult, Context, OutputsManager};
use crate::runner::start_target;

#[derive(Parser, Debug)]
pub struct StartCommand {
    /// The name of the target to start
    pub name: String,

    /// The arguments to pass to the command
    pub args: Vec<String>,
}

impl Execute for StartCommand {
    fn execute(
        &self,
        context: Context,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let mut outputs = OutputsManager::default();
        match context.get_command(self.name.as_str()) {
            CommandLookupResult::Found(target) => {
                start_target(
                    &target,
                    &context,
                    &mut outputs,
                    cleanup_manager,
                    self.args.clone(),
                )
            },
            CommandLookupResult::NotFound => {
                Err(anyhow!(
                    "Target <{}> not found in config file <{}>",
                    self.name,
                    context.config_path
                ))
            },
            CommandLookupResult::Duplicates(duplicates) => {
                Err(anyhow!(
                    "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                    self.name, duplicates.join(", ")
                ))
            },
        }
    }
}
