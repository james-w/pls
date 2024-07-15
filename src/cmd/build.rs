use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::context::{CommandLookupResult, Context};
use crate::outputs::OutputsManager;
use crate::target::Targetable;

#[derive(Parser, Debug)]
pub struct BuildCommand {
    /// The name of the artifact to build
    pub artifact: String,
}

impl Execute for BuildCommand {
    fn execute(&self, context: Context, cleanup_manager: Arc<Mutex<CleanupManager>>) -> Result<()> {
        let mut outputs = OutputsManager::default();
        match context.get_target(self.artifact.as_str()) {
            CommandLookupResult::Found(target) => {
                let builder = target.as_buildable();
                if let Some(builder) = builder {
                    builder.build(&context, &mut outputs, cleanup_manager)
                } else {
                    Err(anyhow!(
                        "Target <{}> is not buildable, use the run command instead",
                        self.artifact
                    ))
                }
            }
            CommandLookupResult::NotFound => Err(anyhow!(
                "Target <{}> not found in config file <{}>",
                self.artifact,
                context.config_path
            )),
            CommandLookupResult::Duplicates(ref mut duplicates) => {
                duplicates.sort();
                Err(anyhow!(
                    "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                    self.artifact, duplicates.join(", ")
                ))
            }
        }
    }
}
