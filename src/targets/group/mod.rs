use std::sync::{Arc, Mutex};

use anyhow::Result;
use log::debug;

use crate::cleanup::CleanupManager;
use crate::context::Context;
use crate::outputs::OutputsManager;
use crate::target::{run_required, Runnable, TargetInfo, Targetable};

#[derive(Clone, Debug)]
pub struct Group {
    pub target_info: TargetInfo,
}

impl Runnable for Group {
    fn run(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        if !args.is_empty() {
            return Err(anyhow::anyhow!("Groups do not accept arguments"));
        }
        debug!(
            "Running group <{}>, with definition <{:?}>",
            self.target_info.name, self
        );
        // Groups execute their dependencies
        run_required(&self.target_info, context, outputs, cleanup_manager)?;
        Ok(())
    }
}

impl Targetable for Group {
    fn as_runnable(&self) -> Option<&dyn Runnable> {
        Some(self)
    }
}
