use std::sync::{Arc, Mutex};

use anyhow::Result;
use validator::Validate;

use crate::cleanup::CleanupManager;
use crate::command_builder::CommandBuilder;
use crate::config::{ExecCommand as ConfigExecCommand, GoCommand as ConfigGoCommand};
use crate::context::Context;
use crate::default::default_optional;
use crate::outputs::OutputsManager;
use crate::target::{CommandInfo, Runnable, Startable, StatusResult, TargetInfo};
use crate::targets::command::exec::ExecCommand;

#[derive(Debug, Clone, Validate)]
pub struct GoCommand {
    #[validate(nested)]
    inner: ExecCommand,

    pub target_info: TargetInfo,
    pub command_info: CommandInfo,

    // Store original config for extends support
    subcommand: Option<String>,
    mod_operation: Option<String>,
    output: Option<String>,
    verbose: Option<bool>,
    tags: Option<Vec<String>>,
    ldflags: Option<String>,
    race: Option<bool>,
    cover: Option<bool>,
    run_pattern: Option<String>,
    bench: Option<String>,
    timeout: Option<String>,
    short: Option<bool>,
    count: Option<i32>,
    args: Option<String>,
}

impl GoCommand {
    pub fn from_config(
        target_info: TargetInfo,
        command_info: CommandInfo,
        defn: &ConfigGoCommand,
        base: Option<&Self>,
    ) -> Self {
        // Merge fields with base (current defn takes precedence)
        let subcommand = defn
            .subcommand
            .clone()
            .or_else(|| base.and_then(|b| b.subcommand.clone()));
        let mod_operation = defn
            .mod_operation
            .clone()
            .or_else(|| base.and_then(|b| b.mod_operation.clone()));
        let output = defn
            .output
            .clone()
            .or_else(|| base.and_then(|b| b.output.clone()));
        let verbose = defn.verbose.or_else(|| base.and_then(|b| b.verbose));
        let tags = defn
            .tags
            .clone()
            .or_else(|| base.and_then(|b| b.tags.clone()));
        let ldflags = defn
            .ldflags
            .clone()
            .or_else(|| base.and_then(|b| b.ldflags.clone()));
        let race = defn.race.or_else(|| base.and_then(|b| b.race));
        let cover = defn.cover.or_else(|| base.and_then(|b| b.cover));
        let run_pattern = defn
            .run_pattern
            .clone()
            .or_else(|| base.and_then(|b| b.run_pattern.clone()));
        let bench = defn
            .bench
            .clone()
            .or_else(|| base.and_then(|b| b.bench.clone()));
        let timeout = defn
            .timeout
            .clone()
            .or_else(|| base.and_then(|b| b.timeout.clone()));
        let short = defn.short.or_else(|| base.and_then(|b| b.short));
        let count = defn.count.or_else(|| base.and_then(|b| b.count));
        let args = defn
            .args
            .clone()
            .or_else(|| base.and_then(|b| b.args.clone()));

        // Validate that subcommand is present
        if subcommand.is_none() {
            panic!(
                "Go command '{}' must specify a subcommand (e.g., build, test, run, mod)",
                target_info.name
            );
        }

        // Build go command string from merged fields
        let command = build_go_command_string(
            &subcommand,
            &mod_operation,
            &output,
            verbose,
            &tags,
            &ldflags,
            race,
            cover,
            &run_pattern,
            &bench,
            &timeout,
            short,
            count,
            &args,
        )
        .expect("Failed to escape go command arguments");

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
            mod_operation,
            output,
            verbose,
            tags,
            ldflags,
            race,
            cover,
            run_pattern,
            bench,
            timeout,
            short,
            count,
            args,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_go_command_string(
    subcommand: &Option<String>,
    mod_operation: &Option<String>,
    output: &Option<String>,
    verbose: Option<bool>,
    tags: &Option<Vec<String>>,
    ldflags: &Option<String>,
    race: Option<bool>,
    cover: Option<bool>,
    run_pattern: &Option<String>,
    bench: &Option<String>,
    timeout: &Option<String>,
    short: Option<bool>,
    count: Option<i32>,
    args: &Option<String>,
) -> Result<String, shlex::QuoteError> {
    let mut builder = CommandBuilder::new("go")
        .subcommand(subcommand)?
        .arg(mod_operation)? // For "go mod <operation>"
        .flag("-v", verbose.unwrap_or(false))
        .flag_with_value("-o", output)?
        .array_flag_equals("-tags", tags, ",")?
        .flag_equals_value("-ldflags", ldflags)?
        .flag("-race", race.unwrap_or(false))
        .flag("-cover", cover.unwrap_or(false));

    // Test-specific flags (only for test/bench subcommands)
    let is_test_command = subcommand
        .as_ref()
        .map(|s| s == "test" || s == "bench")
        .unwrap_or(false);

    if is_test_command {
        builder = builder
            .flag_equals_value("-run", run_pattern)?
            .flag_equals_value("-bench", bench)?
            .flag_equals_value("-timeout", timeout)?
            .flag("-short", short.unwrap_or(false));

        if let Some(c) = count {
            builder = builder.flag_equals_value("-count", &Some(c.to_string()))?;
        }
    }

    Ok(builder.raw_args(args).build())
}

// Pure delegation to inner ExecCommand
impl Runnable for GoCommand {
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

impl Startable for GoCommand {
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
