use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use log::{Log, error};

mod args;
mod commands;
mod config;
mod context;
mod runner;

use args::{Args, Commands, RunCommand, StartCommand, StopCommand};
use config::{Config, find_config_file};
use context::Context;
use runner::{run_target, start_target, stop_target};

fn do_run(_args: &Args, cmd: &RunCommand, config: &Config, config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    config.find_target(cmd.target.as_str()).map_or_else(|| {
        Err(Box::from(format!("Required target <{}> not found in config file <{}>", cmd.target, config_path.display())))
    }, |target| {
        let mut context = Context::from_config(config);
        run_target(&target, config, config_path, &mut context)
    })
}

fn do_start(_args: &Args, cmd: &StartCommand, config: &Config, config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    config.find_target(cmd.target.as_str()).map_or_else(|| {
        Err(Box::from(format!("Target <{}> not found in config file <{}>", cmd.target, config_path.display())))
    }, |target| {
        start_target(&target, config, config_path, &mut Context::from_config(config))
    })
}

fn do_stop(_args: &Args, cmd: &StopCommand, config: &Config, config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    config.find_target(cmd.target.as_str()).map_or_else(|| {
        Err(Box::from(format!("Target <{}> not found in config file <{}>", cmd.target, config_path.display())))
    }, |target| {
        stop_target(&target, config, config_path)
    })
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = find_config_file().expect("Could not find config file in this directory or any parent");
    let config = Config::load_and_validate(&config_path)?;
    match args.command {
        Some(Commands::Run(ref cmd)) => do_run(&args, cmd, &config, &config_path),
        Some(Commands::Start(ref cmd)) => do_start(&args, cmd, &config, &config_path),
        Some(Commands::Stop(ref cmd)) => do_stop(&args, cmd, &config, &config_path),
        None => Err(Box::from("No subcommand specified")),
    }
}

pub struct CombineLogger<L1, L2>(pub L1, pub Option<L2>);

impl<L1: Log, L2: Log> Log for CombineLogger<L1, L2> {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.0.enabled(metadata) || self.1.is_none() || self.1.as_ref().is_some_and(|l| l.enabled(metadata))
    }

    fn log(&self, record: &log::Record<'_>) {
        self.0.log(record);
        if let Some(ref l) = self.1 {
            l.log(record);
        }
    }

    fn flush(&self) {
        self.0.flush();
        if let Some(ref l) = self.1 {
            l.flush();
        }
    }
}

fn main() {
    let info_logger = env_logger::builder()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .filter_level(log::LevelFilter::Info)
        .build();
    let args = Args::parse();
    let debug_logger = if args.debug {
        Some(env_logger::builder().filter_level(log::LevelFilter::Debug).build())
    } else {
        None
    };
    let logger = CombineLogger(info_logger, debug_logger);
    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
    if let Err(e) = run(args) {
        error!("Error: {}", e);
        std::process::exit(1);
    }
}
