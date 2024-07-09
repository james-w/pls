use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::context::{CommandLookupResult, Context, OutputsManager};
use crate::runner::stop_target;

#[derive(Parser, Debug)]
pub struct StopCommand {
    /// The name of the target to stop
    pub target: String,
}

impl Execute for StopCommand {
    fn execute(
        &self,
        context: Context,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let mut outputs = OutputsManager::default();
        match context.get_command(self.target.as_str()) {
            CommandLookupResult::Found(target) => {
                stop_target(&target, &context, &mut outputs, cleanup_manager)
            },
            CommandLookupResult::NotFound => {
                Err(anyhow!(
                    "Target <{}> not found in config file <{}>",
                    self.target,
                    context.config_path
                ))
            },
            CommandLookupResult::Duplicates(duplicates) => {
                Err(anyhow!(
                    "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                    self.target, duplicates.join(", ")
                ))
            },
        }
    }
}


