use std::sync::{Arc, Mutex};

use anyhow::Result;
use log::{debug, info};
use validator::Validate;

use crate::cleanup::CleanupManager;
use crate::commands::run_command_with_env;
use crate::config::ExecArtifact as ConfigExecArtifact;
use crate::context::Context;
use crate::default::default_to;
use crate::outputs::OutputsManager;
use crate::target::{ArtifactInfo, Buildable, TargetInfo};

#[derive(Debug, Clone, Validate)]
pub struct ExecArtifact {
    #[validate(length(min = 1))]
    pub command: String,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Vec<String>,

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
        let mut env = vec![];
        if let Some(base) = base {
            env.extend(base.env.clone());
        }
        env.extend(defn.env.clone().unwrap_or_default());
        Self {
            target_info,
            artifact_info,
            command: default_to!(defn, base, command),
            env,
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
        let env = self
            .env
            .iter()
            .map(|s| context.resolve_substitutions(s, &self.target_info.name, outputs))
            .collect::<Result<Vec<String>>>()?;
        debug!(
            "Building exec artifact for target <{}> with command <{}>",
            self.target_info.name, cmd
        );
        info!("[{}] Building with command {}", self.target_info.name, cmd);
        run_command_with_env(&cmd, env.as_slice())
    }
}
