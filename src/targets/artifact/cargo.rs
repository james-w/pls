use std::sync::{Arc, Mutex};

use anyhow::Result;
use validator::Validate;

use crate::cleanup::CleanupManager;
use crate::command_builder::CommandBuilder;
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
        // Default to "build" subcommand for artifacts if not specified
        let subcommand = defn
            .subcommand
            .clone()
            .or_else(|| Some("build".to_string()));

        // Build cargo command string
        let command = build_cargo_command_string(
            &subcommand,
            &defn.package,
            defn.release.unwrap_or(false),
            &defn.features,
            defn.all_features.unwrap_or(false),
            defn.all_targets.unwrap_or(false),
            defn.no_default_features.unwrap_or(false),
            &defn.target,
            &defn.args,
        )
        .expect("Failed to escape cargo command arguments");

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

#[allow(clippy::too_many_arguments)]
fn build_cargo_command_string(
    subcommand: &Option<String>,
    package: &Option<String>,
    release: bool,
    features: &Option<Vec<String>>,
    all_features: bool,
    all_targets: bool,
    no_default_features: bool,
    target: &Option<String>,
    args: &Option<String>,
) -> Result<String, shlex::QuoteError> {
    let mut builder = CommandBuilder::new("cargo")
        .subcommand(subcommand)?
        .flag_with_value("--package", package)?
        .flag("--release", release)
        .flag("--all-targets", all_targets)
        .flag_with_value("--target", target)?;

    // all_features overrides other feature flags
    if all_features {
        builder = builder.flag("--all-features", true);
    } else {
        builder = builder
            .flag("--no-default-features", no_default_features)
            .array_flag("--features ", features, ",")?;
    }

    Ok(builder.raw_args(args).build())
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
