use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{anyhow, Result};
use clap::Parser;
use log::{debug, error, warn, Log};

mod args;
mod cleanup;
mod commands;
mod config;
mod containers;
mod context;
mod runner;
mod shell;

use args::{Args, Commands, RunCommand, StartCommand, StopCommand, BuildCommand};
use cleanup::CleanupManager;
use config::{find_config_file, Config};
use context::{CommandLookupResult, Context, OutputsManager};
use runner::{run_target, start_target, stop_target, build_target};

fn do_run(
    _args: &Args,
    cmd: &RunCommand,
    context: Context,
    config_path: &PathBuf,
    cleanup_manager: Arc<Mutex<CleanupManager>>,
) -> Result<()> {
    let mut outputs = OutputsManager::default();
    match context.get_command(cmd.name.as_str()) {
        CommandLookupResult::Found(target) => {
            run_target(&target.clone(), &context, &mut outputs, cleanup_manager, cmd.args.clone())
        },
        CommandLookupResult::NotFound => {
            Err(anyhow!(
                "Target <{}> not found in config file <{}>",
                cmd.name,
                config_path.display()
            ))
        },
        CommandLookupResult::Duplicates(duplicates) => {
            Err(anyhow!(
                "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                cmd.name, duplicates.join(", ")
            ))
        },
    }
}

fn do_start(
    _args: &Args,
    cmd: &StartCommand,
    context: Context,
    config_path: &PathBuf,
    cleanup_manager: Arc<Mutex<CleanupManager>>,
) -> Result<()> {
    let mut outputs = OutputsManager::default();
    match context.get_command(cmd.name.as_str()) {
        CommandLookupResult::Found(target) => {
            start_target(
                &target,
                &context,
                &mut outputs,
                cleanup_manager,
                cmd.args.clone(),
            )
        },
        CommandLookupResult::NotFound => {
            Err(anyhow!(
                "Target <{}> not found in config file <{}>",
                cmd.name,
                config_path.display()
            ))
        },
        CommandLookupResult::Duplicates(duplicates) => {
            Err(anyhow!(
                "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                cmd.name, duplicates.join(", ")
            ))
        },
    }
}

fn do_stop(
    _args: &Args,
    cmd: &StopCommand,
    context: Context,
    config_path: &PathBuf,
    cleanup_manager: Arc<Mutex<CleanupManager>>,
) -> Result<()> {
    let mut outputs = OutputsManager::default();
    match context.get_command(cmd.target.as_str()) {
        CommandLookupResult::Found(target) => {
            stop_target(&target, &context, &mut outputs, cleanup_manager)
        },
        CommandLookupResult::NotFound => {
            Err(anyhow!(
                "Target <{}> not found in config file <{}>",
                cmd.target,
                config_path.display()
            ))
        },
        CommandLookupResult::Duplicates(duplicates) => {
            Err(anyhow!(
                "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                cmd.target, duplicates.join(", ")
            ))
        },
    }
}

fn do_build(
    _args: &Args,
    cmd: &BuildCommand,
    context: Context,
    config_path: &PathBuf,
    cleanup_manager: Arc<Mutex<CleanupManager>>,
) -> Result<()> {
    let mut outputs = OutputsManager::default();
    match context.get_command(cmd.artifact.as_str()) {
        CommandLookupResult::Found(target) => {
            build_target(&target.clone(), &context, &mut outputs, cleanup_manager)
        },
        CommandLookupResult::NotFound => {
            Err(anyhow!(
                "Target <{}> not found in config file <{}>",
                cmd.artifact,
                config_path.display()
            ))
        },
        CommandLookupResult::Duplicates(duplicates) => {
            Err(anyhow!(
                "Target <{}> is ambiguous, possible values are <{}>, please specify the command to run using one of those names",
                cmd.artifact, duplicates.join(", ")
            ))
        },
    }
}

fn run(
    args: Args,
    cleanup_manager: Arc<Mutex<CleanupManager>>,
) -> Result<()> {
    let config_path =
        find_config_file().expect("Could not find config file in this directory or any parent");
    let config = Config::load_and_validate(&config_path)?;
    let context = Context::from_config(&config, config_path.display().to_string())?;
    match args.command {
        Some(Commands::Run(ref cmd)) => do_run(&args, cmd, context, &config_path, cleanup_manager),
        Some(Commands::Start(ref cmd)) => {
            do_start(&args, cmd, context, &config_path, cleanup_manager)
        }
        Some(Commands::Stop(ref cmd)) => {
            do_stop(&args, cmd, context, &config_path, cleanup_manager)
        }
        Some(Commands::Build(ref cmd)) => do_build(&args, cmd, context, &config_path, cleanup_manager),
        None => panic!("No command provided"),
    }
}

pub struct CombineLogger<L1, L2>(pub L1, pub Option<L2>);

impl<L1: Log, L2: Log> Log for CombineLogger<L1, L2> {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.0.enabled(metadata)
            || self.1.is_none()
            || self.1.as_ref().is_some_and(|l| l.enabled(metadata))
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

fn start_cleanup_thread(cleanup_manager: Arc<Mutex<CleanupManager>>, running: Arc<AtomicBool>) {
    thread::spawn(move || {
        while running.load(std::sync::atomic::Ordering::SeqCst) {}
        warn!("Received stop signal, cleaning up...");
        let mut manager = cleanup_manager.lock().unwrap();
        let cleanups = std::mem::take(&mut (*manager).cleanups);
        for cleanup in cleanups.into_iter().rev() {
            debug!("Running cleanup");
            cleanup();
        }
        debug!("All cleanups run, exiting...");
        std::process::exit(130);
    });
}

fn main() {
    let info_logger = env_logger::builder()
        .format(|buf, record| writeln!(buf, "{}", record.args()))
        .filter_level(log::LevelFilter::Info)
        .build();
    let args = Args::parse();
    let debug_logger = if args.debug {
        Some(
            env_logger::builder()
                .filter_level(log::LevelFilter::Debug)
                .build(),
        )
    } else {
        None
    };
    let logger = CombineLogger(info_logger, debug_logger);
    log::set_boxed_logger(Box::new(logger)).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        if !r.load(std::sync::atomic::Ordering::SeqCst) {
            warn!("Received stop signal a second time, stopping abrubtly...");
            std::process::exit(130);
        }
        r.store(false, std::sync::atomic::Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");
    let cleanup_manager = Arc::new(Mutex::new(CleanupManager::new()));
    start_cleanup_thread(cleanup_manager.clone(), running);
    if let Err(e) = run(args, cleanup_manager.clone()) {
        error!("Error: {}", e);
        std::process::exit(1);
    }
}
