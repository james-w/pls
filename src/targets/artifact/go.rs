use std::sync::{Arc, Mutex};

use anyhow::Result;
use validator::Validate;

use crate::cleanup::CleanupManager;
use crate::config::{ExecArtifact as ConfigExecArtifact, GoArtifact as ConfigGoArtifact};
use crate::context::Context;
use crate::default::default_optional;
use crate::outputs::OutputsManager;
use crate::target::{ArtifactInfo, Buildable, TargetInfo};
use crate::targets::artifact::exec::ExecArtifact;

#[derive(Debug, Clone, Validate)]
pub struct GoArtifact {
    #[validate(nested)]
    inner: ExecArtifact,

    pub target_info: TargetInfo,
    pub artifact_info: ArtifactInfo,
}

impl GoArtifact {
    pub fn from_config(
        target_info: TargetInfo,
        mut artifact_info: ArtifactInfo,
        defn: &ConfigGoArtifact,
        base: Option<&Self>,
    ) -> Self {
        // Build go command string
        let command = build_go_command_string(
            &defn.subcommand,
            &defn.output,
            defn.verbose,
            &defn.tags,
            &defn.ldflags,
            defn.race,
            &defn.args,
        );

        // SMART DEFAULT 1: Auto-detect binary paths
        if artifact_info.updates_paths.is_none() {
            if let Some(output) = &defn.output {
                artifact_info.updates_paths = Some(vec![output.clone()]);
            } else if let Some(bin_name) = &defn.bin {
                artifact_info.updates_paths = Some(vec![format!("./{}", bin_name)]);
            }
        }

        // SMART DEFAULT 2: Auto-add Go file watching
        if artifact_info.if_files_changed.is_none() {
            artifact_info.if_files_changed = Some(vec![
                "**/*.go".to_string(),
                "go.mod".to_string(),
                "go.sum".to_string(),
            ]);
        }

        // Merge env (base first, then current)
        let mut env = vec![];
        if let Some(base) = base {
            env.extend(base.inner.env.clone());
        }
        env.extend(defn.env.clone().unwrap_or_default());

        // Create ExecArtifact config to wrap
        let exec_config = ConfigExecArtifact {
            command: Some(command),
            env: if env.is_empty() { None } else { Some(env) },
            dir: default_optional!(defn, base.map(|b| &b.inner), dir),
            target_info: defn.target_info.clone(),
            artifact_info: defn.artifact_info.clone(),
        };

        // Construct wrapped ExecArtifact
        let inner = ExecArtifact::from_config(
            target_info.clone(),
            artifact_info.clone(),
            &exec_config,
            base.map(|b| &b.inner),
        );

        Self {
            inner,
            target_info,
            artifact_info,
        }
    }
}

fn build_go_command_string(
    subcommand: &Option<String>,
    output: &Option<String>,
    verbose: Option<bool>,
    tags: &Option<Vec<String>>,
    ldflags: &Option<String>,
    race: Option<bool>,
    args: &Option<String>,
) -> String {
    let mut parts = vec!["go".to_string()];

    if let Some(subcmd) = subcommand {
        parts.push(subcmd.clone());
    }

    // Common flags
    if verbose.unwrap_or(false) {
        parts.push("-v".to_string());
    }

    // Build flags
    if let Some(out) = output {
        parts.push(format!("-o {}", out));
    }

    if let Some(tags_vec) = tags {
        if !tags_vec.is_empty() {
            parts.push(format!("-tags={}", tags_vec.join(",")));
        }
    }

    if let Some(flags) = ldflags {
        parts.push(format!("-ldflags={}", flags));
    }

    if race.unwrap_or(false) {
        parts.push("-race".to_string());
    }

    // Extra args
    if let Some(extra) = args {
        parts.push(extra.clone());
    }

    parts.join(" ")
}

// Pure delegation to inner ExecArtifact
impl Buildable for GoArtifact {
    fn build(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        self.inner.build(context, outputs, cleanup_manager)
    }
}
