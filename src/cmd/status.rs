use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::colors;
use crate::context::{CommandLookupResult, Context};
use crate::outputs::OutputsManager;
use crate::target::{StatusResult, Targetable};

#[derive(Parser, Debug)]
pub struct StatusCommand {
    /// The name of the target to get status for
    pub name: String,
}

impl Execute for StatusCommand {
    fn execute(
        &self,
        context: Context,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let mut outputs = OutputsManager::default();
        match context.get_target(self.name.as_str()) {
            CommandLookupResult::Found(target) => {
                let builder = target.as_startable();
                if let Some(builder) = builder {
                    match builder.status(&context, &mut outputs) {
                        Ok(StatusResult::Running(msg)) => {
                            println!("{} {}",
                                colors::target_prefix(&target.target_info().name.to_string()),
                                colors::success_msg(msg.as_str())
                            );
                            Ok(())
                        },
                        Ok(StatusResult::NotRunning()) => {
                            println!("{} {}",
                                colors::target_prefix(&target.target_info().name.to_string()),
                                colors::grey_msg("Not running")
                            );
                            Ok(())
                        },
                        Err(e) => Err(e),
                    }
                } else {
                    Err(anyhow!(
                        "Target <{}> is not startable",
                        self.name
                    ))
                }
            },
            CommandLookupResult::NotFound => {
                let suggestions = context.get_suggestions(&self.name);
                let mut error_msg = format!(
                    "Target <{}> not found in config file <{}>",
                    self.name, context.config_path
                );
                if !suggestions.is_empty() {
                    error_msg.push_str(&format!(
                        "\n\nDid you mean one of these?\n  {}",
                        suggestions.join("\n  ")
                    ));
                }
                Err(anyhow!(error_msg))
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
