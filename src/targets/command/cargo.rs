use std::sync::{Arc, Mutex};

use anyhow::Result;
use validator::Validate;

use crate::cleanup::CleanupManager;
use crate::command_builder::CommandBuilder;
use crate::config::{CargoCommand as ConfigCargoCommand, ExecCommand as ConfigExecCommand};
use crate::context::Context;
use crate::default::default_optional;
use crate::outputs::OutputsManager;
use crate::target::{CommandInfo, Runnable, Startable, StatusResult, TargetInfo};
use crate::targets::command::exec::ExecCommand;

#[derive(Debug, Clone, Validate)]
pub struct CargoCommand {
    #[validate(nested)]
    inner: ExecCommand,

    pub target_info: TargetInfo,
    pub command_info: CommandInfo,

    // Store original config for extends support
    subcommand: Option<String>,
    package: Option<String>,
    release: bool,
    features: Option<Vec<String>>,
    all_features: bool,
    all_targets: bool,
    no_default_features: bool,
    target: Option<String>,
    args: Option<String>,
}

impl CargoCommand {
    pub fn from_config(
        target_info: TargetInfo,
        command_info: CommandInfo,
        defn: &ConfigCargoCommand,
        base: Option<&Self>,
    ) -> Self {
        // Merge fields with base (current defn takes precedence)
        let subcommand = defn
            .subcommand
            .clone()
            .or_else(|| base.and_then(|b| b.subcommand.clone()));
        let package = defn
            .package
            .clone()
            .or_else(|| base.and_then(|b| b.package.clone()));
        let release = defn
            .release
            .or_else(|| base.map(|b| b.release))
            .unwrap_or(false);
        let features = defn
            .features
            .clone()
            .or_else(|| base.and_then(|b| b.features.clone()));
        let all_features = defn
            .all_features
            .or_else(|| base.map(|b| b.all_features))
            .unwrap_or(false);
        let all_targets = defn
            .all_targets
            .or_else(|| base.map(|b| b.all_targets))
            .unwrap_or(false);
        let no_default_features = defn
            .no_default_features
            .or_else(|| base.map(|b| b.no_default_features))
            .unwrap_or(false);
        let target = defn
            .target
            .clone()
            .or_else(|| base.and_then(|b| b.target.clone()));
        let args = defn
            .args
            .clone()
            .or_else(|| base.and_then(|b| b.args.clone()));

        // Validate that subcommand is present
        if subcommand.is_none() {
            panic!(
                "Cargo command '{}' must specify a subcommand (e.g., build, test, check, run)",
                target_info.name
            );
        }

        // Build cargo command string from merged fields
        let command = build_cargo_command_string(
            &subcommand,
            &package,
            release,
            &features,
            all_features,
            all_targets,
            no_default_features,
            &target,
            &args,
        )
        .expect("Failed to escape cargo command arguments");

        // Merge env (base first, then current)
        let mut env = vec![];
        if let Some(base) = base {
            env.extend(base.inner.env.clone());
        }
        env.extend(defn.env.clone().unwrap_or_default());

        // Create ExecCommand config to wrap
        let exec_config = ConfigExecCommand {
            command: Some(command),
            default_args: None,
            env: if env.is_empty() { None } else { Some(env) },
            dir: default_optional!(defn, base.map(|b| &b.inner), dir),
            target_info: defn.target_info.clone(),
            command_info: defn.command_info.clone(),
        };

        // Construct wrapped ExecCommand
        let inner = ExecCommand::from_config(
            target_info.clone(),
            command_info.clone(),
            &exec_config,
            base.map(|b| &b.inner),
        );

        Self {
            inner,
            target_info,
            command_info,
            subcommand,
            package,
            release,
            features,
            all_features,
            all_targets,
            no_default_features,
            target,
            args,
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

// Pure delegation to inner ExecCommand
impl Runnable for CargoCommand {
    fn run(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        self.inner.run(context, outputs, cleanup_manager, args)
    }
}

impl Startable for CargoCommand {
    fn start(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        self.inner.start(context, outputs, cleanup_manager, args)
    }

    fn start_if_needed(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        self.inner
            .start_if_needed(context, outputs, cleanup_manager, args)
    }

    fn stop(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        self.inner.stop(context, outputs, cleanup_manager)
    }

    fn status(&self, context: &Context, outputs: &mut OutputsManager) -> Result<StatusResult> {
        self.inner.status(context, outputs)
    }
}
