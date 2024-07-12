use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::context::Context;

#[derive(Parser, Debug)]
pub struct ListCommand {}

impl Execute for ListCommand {
    fn execute(&self, context: Context, _cleanup_manager: Arc<Mutex<CleanupManager>>) -> Result<()> {
        let mut targets = context.targets.iter().collect::<Vec<_>>();
        targets.sort_by(|a, b| a.0.cmp(b.0));
        for (name, target) in targets {
            println!("{} - {}", name, target.target_info().description.clone().unwrap_or_default());
        }
        Ok(())
    }
}

