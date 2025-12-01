use std::sync::{Arc, Mutex};

use anyhow::Result;
use validator::Validate;

use crate::cleanup::CleanupManager;
use crate::config::{CargoArtifact as ConfigCargoArtifact, ExecArtifact as ConfigExecArtifact};
use crate::context::Context;
use crate::default::default_optional;
use crate::outputs::OutputsManager;
use crate::target::{ArtifactInfo, Buildable, TargetInfo};
use crate::targets::artifact::exec::ExecArtifact;

#[derive(Debug, Clone, Validate)]
pub struct CargoArtifact {
    #[validate(nested)]
    inner: ExecArtifact,

    pub target_info: TargetInfo,
    pub artifact_info: ArtifactInfo,
}

impl CargoArtifact {
    pub fn from_config(
        target_info: TargetInfo,
        mut artifact_info: ArtifactInfo,
        defn: &ConfigCargoArtifact,
        base: Option<&Self>,
    ) -> Self {
        // Build cargo command string
        let command = build_cargo_command_string(
            &defn.subcommand,
            &defn.package,
            defn.release,
            &defn.features,
            defn.all_features,
            defn.no_default_features,
            &defn.args,
        );

        // SMART DEFAULT 1: Auto-detect binary paths
        if artifact_info.updates_paths.is_none() && defn.bin.is_some() {
            let profile = if defn.release.unwrap_or(false) {
                "release"
            } else {
                "debug"
            };
            let bin_name = defn.bin.as_ref().unwrap();
            artifact_info.updates_paths = Some(vec![format!("target/{}/{}", profile, bin_name)]);
        }

        // SMART DEFAULT 2: Auto-add Rust file watching
        if artifact_info.if_files_changed.is_none() {
            artifact_info.if_files_changed = Some(vec![
                "src/**/*.rs".to_string(),
                "Cargo.toml".to_string(),
                "Cargo.lock".to_string(),
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

fn build_cargo_command_string(
    subcommand: &Option<String>,
    package: &Option<String>,
    release: Option<bool>,
    features: &Option<Vec<String>>,
    all_features: Option<bool>,
    no_default_features: Option<bool>,
    args: &Option<String>,
) -> String {
    let mut parts = vec!["cargo".to_string()];

    if let Some(subcmd) = subcommand {
        parts.push(subcmd.clone());
    }

    if let Some(pkg) = package {
        parts.push(format!("--package {}", pkg));
    }

    if release.unwrap_or(false) {
        parts.push("--release".to_string());
    }

    if all_features.unwrap_or(false) {
        parts.push("--all-features".to_string());
    } else if no_default_features.unwrap_or(false) {
        parts.push("--no-default-features".to_string());
    }

    if let Some(feats) = features {
        if !feats.is_empty() {
            parts.push(format!("--features {}", feats.join(",")));
        }
    }

    if let Some(extra) = args {
        parts.push(extra.clone());
    }

    parts.join(" ")
}

// Pure delegation to inner ExecArtifact
impl Buildable for CargoArtifact {
    fn build(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        self.inner.build(context, outputs, cleanup_manager)
    }
}
