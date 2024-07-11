use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::context::{CommandLookupResult, Context};
use crate::outputs::OutputsManager;
use crate::target::Targetable;

#[derive(Parser, Debug)]
pub struct StopCommand {
    /// The name of the target to stop
    pub target: String,
}

impl Execute for StopCommand {
    fn execute(&self, context: Context, cleanup_manager: Arc<Mutex<CleanupManager>>) -> Result<()> {
        let mut outputs = OutputsManager::default();
        match context.get_target(self.target.as_str()) {
            CommandLookupResult::Found(target) => {
                let builder = target.as_startable();
                if let Some(builder) = builder {
                    builder.stop(&context, &mut outputs, cleanup_manager)
                } else {
                    Err(anyhow!(
                        "Target <{}> is not stopable",
                        self.target
                    ))
                }
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
