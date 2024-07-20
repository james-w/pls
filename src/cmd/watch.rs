use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::Parser;
use log::{debug, error};

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::context::{CommandLookupResult, Context};
use crate::outputs::OutputsManager;
use crate::target::{Target, Targetable};
use crate::watch::WatchTrigger;

use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;

#[derive(Parser, Debug)]
pub struct WatchCommand {
    /// The name of the target to watch
    pub name: String,

    /// The arguments to pass to the command
    pub args: Vec<String>,
}

fn start_or_run(
    target: &Target,
    context: &Context,
    outputs: &mut OutputsManager,
    cleanup_manager: Arc<Mutex<CleanupManager>>,
    args: Vec<String>,
) -> Result<()> {
    match (
        target.as_runnable(),
        target.command_info().map(|c| c.daemon).unwrap_or(false),
        target.as_startable(),
    ) {
        (_, true, Some(starter)) => {
            if let Err(e) =
                starter.restart_no_deps(context, outputs, cleanup_manager.clone(), args.clone())
            {
                error!("Error: {:?}", e);
            }
            Ok(())
        }
        (Some(runner), _, _) => {
            if let Err(e) =
                runner.run_no_deps(context, outputs, cleanup_manager.clone(), args.clone())
            {
                error!("Error: {:?}", e);
            }
            Ok(())
        }
        _ => Err(anyhow!(
            "Target <{}> is not runnable or startable",
            target.target_info().name
        )),
    }
}

impl Execute for WatchCommand {
    fn execute(&self, context: Context, cleanup_manager: Arc<Mutex<CleanupManager>>) -> Result<()> {
        let mut outputs = OutputsManager::default();
        match context.get_target(self.name.as_str()) {
            CommandLookupResult::Found(target) => {
                let triggers = WatchTrigger::get_all(target, &context)?;
                debug!("Triggers: {:?}", triggers);
                let (tx, rx) = std::sync::mpsc::channel();

                let mut debouncer = new_debouncer(Duration::from_millis(250), tx)
                    .expect("Failed to create debouncer");
                if let Some(starter) = target.as_startable() {
                    starter.start(
                        &context,
                        &mut outputs,
                        cleanup_manager.clone(),
                        self.args.clone(),
                    )?
                }
                let watcher = debouncer.watcher();
                for path in WatchTrigger::find_minimal_watches(&triggers) {
                    debug!("Watching: {:?}", path);
                    watcher
                        .watch(Path::new(path.as_str()), RecursiveMode::Recursive)
                        .expect("Failed to watch directory");
                }
                for result in rx {
                    match result {
                        Ok(events) => {
                            events.iter().for_each(|event| {
                                debug!("Event: {:?}", event);
                            });
                            let paths = events.iter().map(|e| &e.path).collect::<Vec<_>>();
                            let abs = std::fs::canonicalize(".")?;
                            let relative = paths
                                .iter()
                                .map(|p| p.strip_prefix(&abs).unwrap_or(p))
                                .collect::<Vec<_>>();
                            let filtered = triggers
                                .iter()
                                .filter(|trigger| trigger.matches(&relative))
                                .collect::<Vec<_>>();
                            for trigger in filtered {
                                debug!(
                                    "Triggered by <{:?}> for <{}>",
                                    paths,
                                    trigger.target.target_info().name
                                );
                                start_or_run(
                                    trigger.target,
                                    &context,
                                    &mut outputs,
                                    cleanup_manager.clone(),
                                    vec![],
                                )?;
                                for target in trigger.and_then.iter() {
                                    // TODO: args
                                    start_or_run(
                                        target,
                                        &context,
                                        &mut outputs,
                                        cleanup_manager.clone(),
                                        vec![],
                                    )?;
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error: {:?}", e);
                        }
                    }
                }
                Ok(())
            }
            CommandLookupResult::NotFound => Err(anyhow!(
                "Target <{}> not found in config file <{}>",
                self.name,
                context.config_path
            )),
            CommandLookupResult::Duplicates(ref mut duplicates) => {
                duplicates.sort();
                Err(anyhow!(
                    "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                    self.name, duplicates.join(", ")
                ))
            }
        }
    }
}
