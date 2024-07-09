use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::context::{CommandLookupResult, Context, OutputsManager};
use crate::runner::build_target;

#[derive(Parser, Debug)]
pub struct BuildCommand {
    /// The name of the artifact to build
    pub artifact: String,
}

impl Execute for BuildCommand {
    fn execute(
        &self,
        context: Context,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let mut outputs = OutputsManager::default();
        match context.get_command(self.artifact.as_str()) {
            CommandLookupResult::Found(target) => {
                build_target(&target.clone(), &context, &mut outputs, cleanup_manager)
            },
            CommandLookupResult::NotFound => {
                Err(anyhow!(
                    "Target <{}> not found in config file <{}>",
                    self.artifact,
                    context.config_path
                ))
            },
            CommandLookupResult::Duplicates(duplicates) => {
                Err(anyhow!(
                    "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                    self.artifact, duplicates.join(", ")
                ))
            },
        }
    }
}
