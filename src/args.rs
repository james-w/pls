use clap::{Parser, Subcommand};

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

#[derive(Parser, Debug)]
pub struct RunCommand {
    /// The name of the command to run
    pub name: String,

    /// The arguments to pass to the command
    pub args: Vec<String>,
}

#[derive(Parser, Debug)]
pub struct StartCommand {
    /// The name of the target to start
    pub name: String,

    /// The arguments to pass to the command
    pub args: Vec<String>,
}

#[derive(Parser, Debug)]
pub struct BuildCommand {
    /// The name of the artifact to build
    pub artifact: String,
}

#[derive(Parser, Debug)]
pub struct StopCommand {
    /// The name of the target to stop
    pub target: String,
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
