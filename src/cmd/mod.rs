use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::{Parser, Subcommand};

mod build;
mod execute;
mod list;
mod run;
mod start;
mod status;
mod stop;
mod watch;

use crate::cleanup::CleanupManager;
use crate::context::Context;
use build::BuildCommand;
pub use execute::Execute;
use list::ListCommand;
use run::RunCommand;
use start::StartCommand;
use status::StatusCommand;
use stop::StopCommand;
use watch::WatchCommand;

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

    #[arg(short = 'C', long)]
    pub directory: Option<String>,
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

    /// List available targets
    List(ListCommand),

    /// Get the status of a daemon
    Status(StatusCommand),

    /// Watch for changes and trigger targets in response
    Watch(WatchCommand),
    // TODO: logs
}

impl Execute for Commands {
    fn execute(&self, context: Context, cleanup_manager: Arc<Mutex<CleanupManager>>) -> Result<()> {
        match self {
            Commands::Run(cmd) => cmd.execute(context, cleanup_manager),
            Commands::Start(cmd) => cmd.execute(context, cleanup_manager),
            Commands::Stop(cmd) => cmd.execute(context, cleanup_manager),
            Commands::Build(cmd) => cmd.execute(context, cleanup_manager),
            Commands::List(cmd) => cmd.execute(context, cleanup_manager),
            Commands::Status(cmd) => cmd.execute(context, cleanup_manager),
            Commands::Watch(cmd) => cmd.execute(context, cleanup_manager),
        }
    }
}
