use std::sync::{Arc, Mutex};

use anyhow::Result;
use log::{debug, info};
use validator::Validate;

use crate::cleanup::CleanupManager;
use crate::commands::run_command;
use crate::config::ExecArtifact as ConfigExecArtifact;
use crate::context::Context;
use crate::default::default_to;
use crate::outputs::OutputsManager;
use crate::target::{ArtifactInfo, Buildable, TargetInfo};

#[derive(Debug, Clone, Validate)]
pub struct ExecArtifact {
    #[validate(length(min = 1))]
    pub command: String,

    #[validate(nested)]
    pub artifact_info: ArtifactInfo,
    #[validate(nested)]
    pub target_info: TargetInfo,
}

impl ExecArtifact {
    pub fn from_config(
        target_info: TargetInfo,
        artifact_info: ArtifactInfo,
        defn: &ConfigExecArtifact,
        base: Option<&Self>,
    ) -> Self {
        Self {
            target_info,
            artifact_info,
            command: default_to!(defn, base, command),
        }
    }
}

impl Buildable for ExecArtifact {
    fn build(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        debug!(
            "Building exec artifact for target <{}> with definition <{:?}>",
            self.target_info.name, self
        );
        let cmd = context.resolve_substitutions(
            self.command.as_str(),
            &self.target_info.name,
            outputs,
        )?;
        debug!(
            "Building exec artifact for target <{}> with command <{}>",
            self.target_info.name, cmd
        );
        info!("[{}] Building with command {}", self.target_info.name, cmd);
        run_command(&cmd)
    }
}
