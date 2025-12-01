use std::sync::{Arc, Mutex};

use anyhow::Result;
use validator::Validate;

use crate::cleanup::CleanupManager;
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
        );

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
) -> String {
    let mut parts = vec!["go".to_string()];

    if let Some(subcmd) = subcommand {
        parts.push(subcmd.clone());

        // Special handling for mod
        if subcmd == "mod" {
            if let Some(op) = mod_operation {
                parts.push(op.clone());
            }
        }
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

    if cover.unwrap_or(false) {
        parts.push("-cover".to_string());
    }

    // Test flags
    if let Some(pattern) = run_pattern {
        parts.push(format!("-run={}", pattern));
    }

    if let Some(b) = bench {
        parts.push(format!("-bench={}", b));
    }

    if let Some(t) = timeout {
        parts.push(format!("-timeout={}", t));
    }

    if short.unwrap_or(false) {
        parts.push("-short".to_string());
    }

    if let Some(c) = count {
        parts.push(format!("-count={}", c));
    }

    // Extra args
    if let Some(extra) = args {
        parts.push(extra.clone());
    }

    parts.join(" ")
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
