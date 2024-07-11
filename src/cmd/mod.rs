use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::{Parser, Subcommand};

mod build;
mod execute;
mod run;
mod start;
mod stop;

use crate::cleanup::CleanupManager;
use crate::context::Context;
use build::BuildCommand;
pub use execute::Execute;
use run::RunCommand;
use start::StartCommand;
use stop::StopCommand;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(arg_required_else_help(true))]
pub struct Args {
    /// Print more verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Turn on debug output
    #[arg(long)]
    pub debug: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run the specified target
    Run(RunCommand),

    /// Start a daemon
    Start(StartCommand),

    /// Stop a daemon
    Stop(StopCommand),

    /// Build an artifact
    Build(BuildCommand),
    // TODO: status, logs
}

impl Execute for Commands {
    fn execute(&self, context: Context, cleanup_manager: Arc<Mutex<CleanupManager>>) -> Result<()> {
        match self {
            Commands::Run(cmd) => cmd.execute(context, cleanup_manager),
            Commands::Start(cmd) => cmd.execute(context, cleanup_manager),
            Commands::Stop(cmd) => cmd.execute(context, cleanup_manager),
            Commands::Build(cmd) => cmd.execute(context, cleanup_manager),
        }
    }
}
