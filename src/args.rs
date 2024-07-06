use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
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
    /// The name of the target to run
    pub target: String,
}

#[derive(Parser, Debug)]
pub struct StartCommand {
    /// The name of the target to start
    pub target: String,
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

    // TODO: status, logs
}
