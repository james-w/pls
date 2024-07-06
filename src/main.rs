use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::{path::PathBuf, collections::HashSet};

use clap::{Parser, Subcommand};
use glob::glob;
use log::{Log, debug, info, warn, error};
use serde::Deserialize;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Print more verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Turn on debug output
    #[arg(long)]
    debug: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser, Debug)]
struct RunCommand {
    /// The name of the target to run
    target: String,
}

#[derive(Parser, Debug)]
struct StartCommand {
    /// The name of the target to start
    target: String,
}

#[derive(Parser, Debug)]
struct StopCommand {
    /// The name of the target to stop
    target: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the specified target
    Run(RunCommand),

    /// Start a daemon
    Start(StartCommand),

    /// Stop a daemon
    Stop(StopCommand),

    // TODO: status, logs
}

#[derive(Deserialize, Clone)]
struct Config {
    target: Vec<Target>,
    container_build: Option<Vec<ContainerBuild>>,
}

fn default_false() -> bool {
    false
}

#[derive(Deserialize, Clone, Debug)]
struct Target {
    name: String,
    command: String,
    variables: Option<HashMap<String, String>>,
    requires: Option<Vec<String>>,
    #[serde(default = "default_false")]
    daemon: bool,
    updates_paths: Option<Vec<String>>,
    if_files_changed: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
struct ContainerBuild {
    name: String,
    context: String,
    tag: String,
    variables: Option<HashMap<String, String>>,
    requires: Option<Vec<String>>,
    if_files_changed: Option<Vec<String>>,
}

const CONFIG_FILE_NAME: &str = ".taskrunner.toml";

fn find_config_file() -> Option<std::path::PathBuf> {
    let mut config_dir = std::env::current_dir().unwrap();
    loop {
        let config_path = config_dir.join(CONFIG_FILE_NAME);
        if config_path.exists() {
            debug!("Found config file at <{}>", config_path.display());
            return Some(config_path);
        }
        if !config_dir.pop() {
            return None;
        }
    }
}

fn substitute_variables(command: &str, variables: &HashMap<String, &HashMap<String, String>>) -> String {
    let mut resolved = command.to_string();
    for (target_name, value) in variables.iter() {
        for (key, value) in value.iter() {
            let new_resolved = if target_name == "" {
                resolved.replace(format!("{{{}}}", key).as_str(), value)
            } else {
                resolved.replace(format!("{{{}.{}}}", target_name, key).as_str(), value)
            };
            if new_resolved != resolved {
                debug!("Resolved variable <{}> to <{}>", key, value);
            }
            resolved = new_resolved;
        }
    }
    resolved
}

fn variables_map<'a>(target: &'a Target, required_targets: &Vec<&'a Target>) -> HashMap<String, &'a HashMap<String, String>> {
    let mut variables = HashMap::new();
    if let Some(ref target_variables) = target.variables {
        variables.insert("".to_string(), target_variables);
    }
    for required_target in required_targets.iter() {
        if let Some(ref required_variables) = required_target.variables {
            variables.insert(required_target.name.clone(), required_variables);
        }
    }
    variables
}

fn resolve_command(target: &Target, required_targets: Vec<&Target>) -> String {
    debug!("Resolving command <{}> for target <{}>", target.command, target.name);
    let variables = variables_map(target, &required_targets);
    let resolved = substitute_variables(target.command.as_str(), &variables);
    debug!("Resolved command to <{}>", resolved);
    resolved
}

fn find_target<'a>(name: &str, targets: &'a Vec<Target>) -> Option<&'a Target> {
    targets.iter().find(|target| target.name == name)
}

fn run_target_inner<'a>(target: &Target, config: &'a Config, config_path: &PathBuf, to_stop: &mut Vec<&'a Target>) -> Result<(), Box<dyn std::error::Error>> {
    let mut resolved_requirements = vec![];
    if let Some(ref requires) = target.requires {
        for require in requires.iter() {
            if require == &target.name {
                return Err(Box::from(format!("Target <{}> requires itself", target.name)));
            }
            find_target(require, &config.target).map_or_else(|| {
                Err(Box::<dyn std::error::Error>::from(format!("Required target <{}> not found in config file <{}>", require, config_path.display())))
            }, |required_target| {
                resolved_requirements.push(required_target);
                Ok(())
            })?;
        }
    }
    let last_run_path = metadata_path(target)?.join("last_run");
    if let Some(ref if_files_changed) = target.if_files_changed {
        debug!("Checking if files changed for target <{}>", target.name);
        let last_run = match &target.updates_paths {
            Some(ref updates_paths) => {
                debug!("Checking if updates_paths have changed for target <{}>", target.name);
                updates_paths.iter().map(|path| {
                    substitute_variables(path, &variables_map(target, &resolved_requirements))
                }).map(|path| {
                    std::fs::metadata(&path).map_or_else(|_| {
                        debug!("File <{}> does not exist for target <{}>", path, target.name);
                        None
                    }, |metadata| {
                        Some(metadata.modified().unwrap())
                    })
                }).fold(Some(None), |acc, modified_time| {
                    match (acc, modified_time) {
                        (Some(None), Some(time)) => Some(Some(time)),
                        (Some(Some(acc_time)), Some(time)) => {
                            if time > acc_time {
                                Some(Some(acc_time))
                            } else {
                                Some(Some(time))
                            }
                        }
                        _ => None,
                    }
                })
            },
            None => {
                debug!("Checking last run time for target <{}> based on <{}>", target.name, last_run_path.display());
                std::fs::metadata(&last_run_path).map_or_else(|_| {
                    debug!("Last run file does not exist at <{}> for target <{}>", last_run_path.display(), target.name);
                    Some(None)
                }, |metadata| {
                    Some(Some(metadata.modified().unwrap()))
                })
            },
        };
        debug!("Last run time: {:?}", last_run);
        let mut run_again = false;
        if let Some(last_run) = last_run {
            if let Some(last_run) = last_run {
                for file in if_files_changed.iter().map(
                    |path| substitute_variables(path, &variables_map(target, &resolved_requirements))
                    ) {
                    for entry in glob(file.as_str())? {
                        match entry {
                            Ok(path) => {
                                if !path.exists() {
                                    debug!("Running task as file <{}> does not exist for target <{}>", path.display(), target.name);
                                    run_again = true;
                                } else {
                                    let file_modified_time = std::fs::metadata(&path)?.modified()?;
                                    if file_modified_time > last_run {
                                        debug!("Running task as file <{}> was modified at <{:?}> for target <{}>", path.display(), file_modified_time, target.name);
                                        run_again = true;
                                    } else {
                                        debug!("File <{}> was modified at <{:?}>, before target, for target <{}>", path.display(), file_modified_time, target.name);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Error globbing file <{}> for target <{}>: {}", file, target.name, e);
                                run_again = true;
                            }
                        }
                    }
                }
            } else {
                run_again = true;
            }
        }
        if !run_again {
            info!("[{}] Up to date", target.name);
            return Ok(());
        }
    }
    for required_target in resolved_requirements.iter() {
        if required_target.daemon {
            debug!("Starting required target <{}> for target <{}>", required_target.name, target.name);
            start_target(required_target, config, config_path)?;
            to_stop.push(required_target);
        } else {
            debug!("Running required target <{}> for target <{}>", required_target.name, target.name);
            run_target(required_target, config, config_path)?;
        }
    }
    let command = resolve_command(target, resolved_requirements);
    debug!("Running target <{}> with command <{}>", target.name, command);
    info!("[{}] Running {}", target.name, command);
    // TODO: cwd
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(command.as_str())
        .status()?;
    if !status.success() {
        return Err(Box::from(format!("Command failed with exit code: {}", status.code().unwrap())));
    }
    // TODO: check that updates_paths were created
    let _ = create_metadata_dir(target)?;
    File::create(last_run_path)?;
    Ok(())
}

fn run_target(target: &Target, config: &Config, config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut to_stop = vec![];
    let result = run_target_inner(target, config, config_path, &mut to_stop);
    // Reverse the order that they were started
    to_stop.reverse();
    for target in to_stop.iter() {
        // TODO: add in errors stopping
        if let Err(e) = stop_target(target, config, config_path) {
            warn!("Error stopping target <{}>: {}", target.name, e);
        }
    }
    result
}

fn do_run(_args: &Args, cmd: &RunCommand, config: &Config, config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    find_target(cmd.target.as_str(), &config.target).map_or_else(|| {
        Err(Box::from(format!("Required target <{}> not found in config file <{}>", cmd.target, config_path.display())))
    }, |target| {
        run_target(target, config, config_path)
    })
}

/*
use daemonize::{Daemonize, Outcome};
        let daemonize = Daemonize::new()
            .pid_file(pid_path)
            .chown_pid_file(true)
            .working_directory(std::env::current_dir()?)
            .stdout(log.try_clone()?)
            .stderr(log);

        match daemonize.execute() {
            Outcome::Parent(Ok(_)) => {
                println!("Started daemon for target <{}>", target.name);
                Ok(())
            }
            Outcome::Parent(Err(e)) => {
                Err(Box::from(format!("Error starting daemon for target <{}>: {}", target.name, e)))
            }
            Outcome::Child(Ok(_)) => {
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(target.command.as_str())
                    .status()?;
                if !status.success() {
                    return Err(Box::from(format!("Command failed with exit code: {}", status.code().unwrap())));
                }
                Ok(())
            }
            Outcome::Child(Err(e)) => {
                Err(Box::from(format!("Error starting daemon for target <{}>: {}", target.name, e)))
            }
        }
*/

fn metadata_path(target: &Target) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    Ok(std::env::current_dir()?.join(".taskrunner").join(target.name.as_str()))
}

fn create_metadata_dir(target: &Target) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let taskrunner_dir = metadata_path(target)?;
    debug!("Creating metadata dir for target <{}> at <{}>", target.name, taskrunner_dir.display());
    std::fs::create_dir_all(&taskrunner_dir)?;
    Ok(taskrunner_dir)
}

fn start_target(target: &Target, _config: &Config, _config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if !target.daemon {
        warn!("Target <{}> is not a daemon, did you mean to use `run` instead?", target.name);
    }

    let taskrunner_dir = create_metadata_dir(target)?;

    let pid_path = taskrunner_dir.join("pid");
    if pid_path.exists() {
        let pid_str = std::fs::read_to_string(&pid_path)?;
        debug!("Found pid file for target <{}> at <{}>, with contents <{}>, checking if it is alive", target.name, pid_str.trim(), pid_path.display());
        let pid = pid_str.trim().parse::<i32>()?;
        if is_process_alive(nix::unistd::Pid::from_raw(pid)) {
            return Err(Box::from(format!("Daemon for target <{}> is already running with pid <{}>", target.name, pid)));
        }
        debug!("Process with pid <{}> is not running, continuing with taret <{}>", pid, target.name);
    }

    let log_path = taskrunner_dir.join("log");
    debug!("Creating log file for target <{}> at <{}>", target.name, log_path.display());
    let log = File::create(log_path)?;

    // TODO: requires
    let command = resolve_command(target, vec![]);
    debug!("Starting daemon for target <{}> with command <{}>", target.name, command);
    info!("[{}] Starting {}", target.name, command);
    // TODO: cwd
    let child = std::process::Command::new("sh")
        .arg("-c")
        .arg(command.as_str())
        .stdout(log.try_clone()?)
        .stderr(log.try_clone()?)
        .spawn()?;
    debug!("Started daemon for target <{}> with pid <{}>, storing at <{}>", target.name, child.id(), pid_path.display());
    std::fs::write(&pid_path, child.id().to_string())?;
    Ok(())
}

fn do_start(_args: &Args, cmd: &StartCommand, config: &Config, config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    config.target.iter().find(|target| target.name == cmd.target).map_or_else(|| {
        Err(Box::from(format!("Target <{}> not found in config file <{}>", cmd.target, config_path.display())))
    }, |target| {
        start_target(target, config, config_path)
    })
}

fn is_process_alive(pid: nix::unistd::Pid) -> bool {
    match nix::sys::signal::kill(pid, None) {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn stop_target(target: &Target, _config: &Config, _config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let taskrunner_dir = std::env::current_dir()?.join(".taskrunner").join(target.name.as_str());

    let pid_path = taskrunner_dir.join("pid");
    debug!("Searching for pid file for target <{}> at <{}>", target.name, pid_path.display());
    let mut pid_str = std::fs::read_to_string(&pid_path).map_err(
        |e|
        match e.kind() {
            std::io::ErrorKind::NotFound => Box::<dyn std::error::Error>::from("Task not running"),
            _ => Box::<dyn std::error::Error>::from(format!("Error reading pid file for target <{}> at <{}>: {}", target.name, pid_path.display(), e)),
        }
        )?;
    pid_str = pid_str.trim().to_string();
    debug!("Found pid <{}> for target <{}> at <{}>", pid_str, target.name, pid_path.display());

    let pid = pid_str.parse::<i32>()?;

    // TODO: don't send signal on every loop
    // TODO: switch to SIGKILL after a timeout
    if is_process_alive(nix::unistd::Pid::from_raw(pid)) {
        info!("[{}] Stopping", target.name);
        while is_process_alive(nix::unistd::Pid::from_raw(pid)) {
            debug!("Stopping daemon with pid <{}> for target <{}>", pid, target.name);
            match nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid), nix::sys::signal::Signal::SIGTERM) {
                Ok(_) => {
                    // TODO: timeout
                    nix::sys::wait::waitpid(nix::unistd::Pid::from_raw(pid), None)?;
                }
                Err(e) => {
                    return Err(Box::from(format!("Error stopping daemon for target <{}>: {}", target.name, e)));
                }
            }
        }
    } else {
        debug!("Process with pid <{}> for target <{}> is no longer alive", pid, target.name);
    }
    debug!("Removing pid file for target <{}> at <{}>", target.name, pid_path.display());
    std::fs::remove_file(&pid_path)?;
    Ok(())
}

fn do_stop(_args: &Args, cmd: &StopCommand, config: &Config, config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    config.target.iter().find(|target| target.name == cmd.target).map_or_else(|| {
        Err(Box::from(format!("Target <{}> not found in config file <{}>", cmd.target, config_path.display())))
    }, |target| {
        stop_target(target, config, config_path)
    })
}

fn convert_config(config: &Config) -> Result<Config, Box<dyn std::error::Error>> {
    let mut new_config = config.clone();
    if let Some(ref container_build) = config.container_build {
        for build in container_build.iter() {
            debug!("Converting container build <{}> to target", build.name);
            let target = Target {
                name: build.name.clone(),
                command: format!("podman build -t \"{}\" \"{}\"", build.tag, build.context),
                variables: build.variables.clone(),
                requires: build.requires.clone(),
                daemon: false,
                if_files_changed: build.if_files_changed.clone(),
                updates_paths: None,
            };
            new_config.target.push(target);
        }
    }
    Ok(new_config)
}

fn load_and_validate_config(config_path: &PathBuf) -> Result<Config, Box<dyn std::error::Error>> {
    let config_str = std::fs::read_to_string(config_path)?;
    let mut config: Config = toml::from_str(config_str.as_str())?;
    config = convert_config(&config)?;
    let mut uniq = HashSet::new();
    let dupes = config.target.iter().filter(|x| !uniq.insert(x.name.as_str()));
    let dupe_names = dupes.map(|x| x.name.clone()).collect::<HashSet<_>>();
    if !dupe_names.is_empty() {
        return Err(Box::from(format!("Duplicate target names found in config file <{}>: {}", config_path.display(), dupe_names.iter().map(String::as_str).collect::<Vec<_>>().join(", "))));
    }
    Ok(config)
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = find_config_file().expect("Could not find config file in this directory or any parent");
    let config = load_and_validate_config(&config_path)?;
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
