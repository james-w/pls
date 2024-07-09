use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::cleanup::CleanupManager;
use crate::context::Context;

pub trait Execute {
    fn execute(&self, context: Context, cleanup_manager: Arc<Mutex<CleanupManager>>) -> Result<()>;
}
