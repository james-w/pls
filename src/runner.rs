use std::fs::File;
use std::path::PathBuf;

use glob::glob;
use log::{debug, info, warn};

use crate::commands::{build_command, is_process_alive, stop_process, spawn_command_with_pidfile};
use crate::config::{Config, Command, Target, ContainerBuild, Container};
use crate::context::Context;

trait Runnable {
    fn run(&self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>>;
}

impl<'a> Runnable for Target<'a> {
    fn run(&self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Target::Command(command) => command.run(context),
            Target::ContainerBuild(build) => build.run(context),
            Target::Container(container) => container.run(context),
        }
    }
}

impl Runnable for Command {
    fn run(&self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        let command = resolve_command(self, context);
        debug!("Running target <{}> with command <{}>", self.name, command);
        info!("[{}] Running {}", self.name, command);
        // TODO: cwd
        let mut cmd = build_command(command.as_str())?;
        let status = cmd.status()?;
        if !status.success() {
            return Err(Box::from(format!("Command failed with exit code: {}", status.code().unwrap())));
        }
        Ok(())
    }
}

impl Runnable for ContainerBuild {
    fn run(&self, _context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        let command = format!("podman build -t \"{}\" \"{}\"", self.tag, self.context);
        debug!("Building container for target <{}> with command <{}>", self.name, command);
        info!("[{}] Building tag {}", self.name, self.tag);
        let mut cmd = build_command(command.as_str())?;
        let status = cmd.status()?;
        if !status.success() {
            return Err(Box::from(format!("Command failed with exit code: {}", status.code().unwrap())));
        }
        // TODO: output sha
        Ok(())
    }
}

impl Runnable for Container {
    fn run(&self, _context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        let env_str = self.env.as_ref().map_or_else(|| "".to_string(), |env| env.iter().map(|e| format!("-e \"{}\"", e)).collect::<Vec<_>>().join(" "));
        let mount_str = self.mount.as_ref().map_or_else(|| "".to_string(), |mount| mount.iter().map(|(key, value)| format!("-v \"{}:{}\"", key, value)).collect::<Vec<_>>().join(" "));
        let workdir_str = self.workdir.as_ref().map_or_else(|| "".to_string(), |workdir| format!("-w \"{}\"", workdir));
        let cmd_str = self.command.as_ref().map_or_else(|| "".to_string(), |c| c.clone());
        let command = format!("podman run --init --rm {} {} {} \"{}\" {}", env_str, mount_str, workdir_str, self.image, cmd_str);
        debug!("Running container for target <{}> with command <{:?}>", self.name, self.command);
        info!("[{}] Running container using {}", self.name, self.image);
        let mut cmd = build_command(command.as_str())?;
        let status = cmd.status()?;
        if !status.success() {
            return Err(Box::from(format!("Command failed with exit code: {}", status.code().unwrap())));
        }
        Ok(())
    }
}

trait Startable {
    fn start(&self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>>;
}

impl<'a> Startable for Target<'a> {
    fn start(&self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        match self {
            Target::Command(command) => command.start(context),
            Target::ContainerBuild(build) => build.start(context),
            Target::Container(container) => container.start(context),
        }
    }
}

impl Startable for Command {
    fn start(&self, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        let taskrunner_dir = create_metadata_dir(self.name.as_str())?;

        let pid_path = taskrunner_dir.join("pid");
        let log_path = taskrunner_dir.join("log");
        let cmd = resolve_command(self, context);
        let log_start = || {
            info!("[{}] Starting {}", self.name, cmd);
        };
        spawn_command_with_pidfile(cmd.as_str(), &pid_path, &log_path, log_start)
    }
}

impl Startable for ContainerBuild {
    fn start(&self, _context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        unimplemented!()
    }
}

impl Startable for Container {
    fn start(&self, _context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
        let env_str = self.env.as_ref().map_or_else(|| "".to_string(), |env| env.iter().map(|e| format!("-e \"{}\"", e)).collect::<Vec<_>>().join(" "));
        let mount_str = self.mount.as_ref().map_or_else(|| "".to_string(), |mount| mount.iter().map(|(key, value)| format!("-v \"{}:{}\"", key, value)).collect::<Vec<_>>().join(" "));
        let workdir_str = self.workdir.as_ref().map_or_else(|| "".to_string(), |workdir| format!("-w \"{}\"", workdir));
        let cmd_str = self.command.as_ref().map_or_else(|| "".to_string(), |c| c.clone());
        let command = format!("podman run --init --rm {} {} {} \"{}\" {}", env_str, mount_str, workdir_str, self.image, cmd_str);
        debug!("Running container for target <{}> with command <{:?}>", self.name, self.command);

        let taskrunner_dir = create_metadata_dir(self.name.as_str())?;
        let pid_path = taskrunner_dir.join("pid");
        let log_path = taskrunner_dir.join("log");
        let log_start = || {
            info!("[{}] Starting container using {}", self.name, self.image);
        };
        spawn_command_with_pidfile(command.as_str(), &pid_path, &log_path, log_start)
    }
}

fn resolve_command(target: &Command, context: &Context) -> String {
    debug!("Resolving command <{}> for target <{}>", target.command, target.name);
    let resolved = context.resolve_substitutions(target.command.as_str(), target.name.as_str());
    debug!("Resolved command to <{}>", resolved);
    resolved
}

fn metadata_path(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    Ok(std::env::current_dir()?.join(".taskrunner").join(name))
}

fn create_metadata_dir(name: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let taskrunner_dir = metadata_path(name)?;
    debug!("Creating metadata dir for target <{}> at <{}>", name, taskrunner_dir.display());
    std::fs::create_dir_all(&taskrunner_dir)?;
    Ok(taskrunner_dir)
}

fn find_required<'a>(target: &Target, config: &'a Config) -> Result<Vec<Target<'a>>, Box<dyn std::error::Error>> {
    let mut resolved_requirements = vec![];
    if let Some(ref requires) = target.requires() {
        for require in requires.iter() {
            if require == &target.name() {
                return Err(Box::from(format!("Target <{}> requires itself", target.name())));
            }
            config.find_target(require).map_or_else(|| {
                Err(Box::<dyn std::error::Error>::from(format!("Required target <{}> not found in config file", require)))
            }, |required_target| {
                resolved_requirements.push(required_target);
                Ok(())
            })?;
        }
    }
    Ok(resolved_requirements)
}

fn last_run_path(target: &Target) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    Ok(metadata_path(target.name())?.join("last_run"))
}

#[derive(Debug, PartialEq, Eq)]
enum LastRun {
    Never,
    Time(std::time::SystemTime),
}

fn latest_update_time(times: Vec<LastRun>) -> LastRun {
    times.into_iter().fold(None, |acc, modified_time| {
        match (acc, modified_time) {
            (Some(LastRun::Never), _) => Some(LastRun::Never),
            (_, LastRun::Never) => Some(LastRun::Never),
            (Some(LastRun::Time(acc_time)), LastRun::Time(time)) => {
                if time > acc_time {
                    Some(LastRun::Time(time))
                } else {
                    Some(LastRun::Time(acc_time))
                }
            }
            (None, t) => Some(t),
        }
    }).unwrap_or(LastRun::Never)
}

fn update_times_of_glob(glob_str: &str) -> Result<Vec<LastRun>, Box<dyn std::error::Error>> {
    Ok(glob(glob_str)?.map(|entry| {
        match entry {
            Ok(path) => {
                std::fs::metadata(&path).map_or_else(|_| {
                    debug!("File <{}> does not exist", path.display());
                    LastRun::Never
                }, |metadata| {
                    LastRun::Time(metadata.modified().unwrap())
                })
            }
            Err(e) => {
                warn!("Error globbing file <{}>: {}", glob_str, e);
                LastRun::Never
            }
        }
    }).collect())
}

fn update_times_of_glob_ignoring_missing(glob_str: &str) -> Result<Vec<LastRun>, Box<dyn std::error::Error>> {
    Ok(glob(glob_str)?.map(|entry| {
        match entry {
            Ok(path) => {
                std::fs::metadata(&path).map_or_else(|_| {
                    debug!("File <{}> does not exist", path.display());
                    None
                }, |metadata| {
                    Some(LastRun::Time(metadata.modified().unwrap()))
                })
            }
            Err(e) => {
                warn!("Error globbing file <{}>: {}", glob_str, e);
                None
            }
        }
    }).filter_map(|x| x).collect())
}

fn latest_update_time_of_paths(paths: &Vec<String>, target: &Target, context: &Context) -> Result<LastRun, Box<dyn std::error::Error>> {
    // Where do the Err values go?
    let update_times: Result<Vec<Vec<LastRun>>, Box<dyn std::error::Error>> = paths.iter().map(|path| {
        context.resolve_substitutions(path, target.name())
    }).map(|path| {
        update_times_of_glob(path.as_str())
    }).collect();
    Ok(latest_update_time(update_times?.into_iter().flatten().collect()))
}

fn latest_update_time_of_paths_ignoring_missing(paths: &Vec<String>, target: &Target, context: &Context) -> Result<LastRun, Box<dyn std::error::Error>> {
    // Where do the Err values go?
    let update_times: Result<Vec<Vec<LastRun>>, Box<dyn std::error::Error>> = paths.iter().map(|path| {
        context.resolve_substitutions(path, target.name())
    }).map(|path| {
        update_times_of_glob_ignoring_missing(path.as_str())
    }).collect();
    Ok(latest_update_time(update_times?.into_iter().flatten().collect()))
}

fn last_run(target: &Target, context: &Context) -> Result<LastRun, Box<dyn std::error::Error>> {
    let last_run_path = last_run_path(target)?;
    match target.updates_paths() {
        Some(ref updates_paths) => {
            debug!("Checking if updates_paths have changed for target <{}>", target.name());
            latest_update_time_of_paths(updates_paths, target, context)
        },
        None => {
            debug!("Checking last run time for target <{}> based on <{}>", target.name(), last_run_path.display());
            std::fs::metadata(&last_run_path).map_or_else(|_| {
                debug!("Last run file does not exist at <{}> for target <{}>", last_run_path.display(), target.name());
                Ok(LastRun::Never)
            }, |metadata| {
                Ok(LastRun::Time(metadata.modified().unwrap()))
            })
        },
    }
}

fn should_rerun(target: &Target, resolved_requirements: Vec<Target>, context: &Context) -> Result<bool, Box<dyn std::error::Error>> {
    if let Some(ref if_files_changed) = target.if_files_changed() {
        debug!("Checking if files changed for target <{}>", target.name());
        let last_run = last_run(target, context)?;
        debug!("Last run time: {:?}", last_run);
        let mut run_again = false;
        if let LastRun::Time(last_run) = last_run {
            let latest_time_of_deps = latest_update_time_of_paths_ignoring_missing(if_files_changed, target, context)?;
            debug!("Latest time on dependencies: {:?}", latest_time_of_deps);
            if let LastRun::Time(latest_time_of_deps) = latest_time_of_deps {
                if latest_time_of_deps > last_run {
                    debug!("Running task as dependencies have changed for target <{}>", target.name());
                    run_again = true;
                }
            }
            if !run_again {
                for required_target in resolved_requirements.iter() {
                    let required_last_run_path = metadata_path(required_target.name())?.join("last_run");
                    if !required_last_run_path.exists() {
                        debug!("Running task as required target <{}> has not been run for target <{}>", required_target.name(), target.name());
                        run_again = true;
                    } else {
                        let required_last_run = std::fs::metadata(&required_last_run_path)?.modified()?;
                        if required_last_run > last_run {
                            debug!("Running task as required target <{}> was run after target for target <{}>", required_target.name(), target.name());
                            run_again = true;
                        } else {
                            debug!("Required target <{}> was run at <{:?}>, before target, for target <{}>", required_target.name(), required_last_run, target.name());
                        }
                    }
                }
            }
        } else {
            run_again = true;
        }
        Ok(run_again)
    } else {
        Ok(true)
    }
}

fn run_target_inner<'a>(target: &'a Target, config: &'a Config, config_path: &PathBuf, context: &mut Context, to_stop: &mut Vec<Target<'a>>) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Running target <{}>, with definition <{:?}>", target.name(), target);
    let resolved_requirements = find_required(target, config)?;
    for required_target in resolved_requirements.iter() {
        if required_target.daemon() {
            debug!("Starting required target <{}> for target <{}>", required_target.name(), target.name());
            start_target(&required_target, config, config_path, context)?;
            to_stop.push(required_target.clone());
        } else {
            debug!("Running required target <{}> for target <{}>", required_target.name(), target.name());
            run_target(&required_target, config, config_path, context)?;
        }
    }
    if !should_rerun(target, resolved_requirements, context)? {
        debug!("Skipping target <{}> as it does not need to be run", target.name());
        info!("[{}] Up to date", target.name());
        return Ok(());
    }
    target.run(context)?;
    // TODO: check that updates_paths were created?
    let _ = create_metadata_dir(target.name())?;
    File::create(last_run_path(target)?)?;
    Ok(())
}

pub fn run_target(target: &Target, config: &Config, config_path: &PathBuf, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
    let mut to_stop = vec![];
    let result = run_target_inner(target, config, config_path, context, &mut to_stop);
    // Reverse the order that they were started
    to_stop.reverse();
    for target in to_stop.iter() {
        // TODO: add in errors stopping to result
        if let Err(e) = stop_target(target, config, config_path) {
            warn!("Error stopping target <{}>: {}", target.name(), e);
        }
    }
    result
}

pub fn start_target_inner<'a>(target: &Target, config: &'a Config, config_path: &PathBuf, context: &mut Context, to_stop: &mut Vec<Target<'a>>) -> Result<(), Box<dyn std::error::Error>> {
    if !target.daemon() {
        warn!("Target <{}> is not a daemon, did you mean to use `run` instead?", target.name());
    }
    debug!("Starting target <{}>, with definition <{:?}>", target.name(), target);

    let resolved_requirements = find_required(target, config)?;
    for required_target in resolved_requirements.iter() {
        if required_target.daemon() {
            debug!("Starting required target <{}> for target <{}>", required_target.name(), target.name());
            start_target(required_target, config, config_path, context)?;
            to_stop.push(required_target.clone());
        } else {
            debug!("Running required target <{}> for target <{}>", required_target.name(), target.name());
            run_target(required_target, config, config_path, context)?;
        }
    }
    target.start(context)
}

pub fn start_target(target: &Target, config: &Config, config_path: &PathBuf, context: &mut Context) -> Result<(), Box<dyn std::error::Error>> {
    let mut to_stop = vec![];
    let result = start_target_inner(target, config, config_path, context, &mut to_stop);
    if let Err(_) = result {
        // Reverse the order that they were started
        to_stop.reverse();
        for target in to_stop.iter() {
            // TODO: add in errors stopping to result
            if let Err(e) = stop_target(target, config, config_path) {
                warn!("Error stopping target <{}>: {}", target.name(), e);
            }
        }
    }
    result
}

pub fn stop_target(target: &Target, _config: &Config, _config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    debug!("Stopping target <{}>, with definition <{:?}>", target.name(), target);
    let taskrunner_dir = std::env::current_dir()?.join(".taskrunner").join(target.name());

    let pid_path = taskrunner_dir.join("pid");
    debug!("Searching for pid file for target <{}> at <{}>", target.name(), pid_path.display());
    let mut pid_str = std::fs::read_to_string(&pid_path).map_err(
        |e|
        match e.kind() {
            std::io::ErrorKind::NotFound => Box::<dyn std::error::Error>::from("Task not running"),
            _ => Box::<dyn std::error::Error>::from(format!("Error reading pid file for target <{}> at <{}>: {}", target.name(), pid_path.display(), e)),
        }
        )?;
    pid_str = pid_str.trim().to_string();
    debug!("Found pid <{}> for target <{}> at <{}>", pid_str, target.name(), pid_path.display());

    let pid = nix::unistd::Pid::from_raw(pid_str.parse::<i32>()?);
    if is_process_alive(pid) {
        info!("[{}] Stopping", target.name());
        stop_process(pid)?;
    } else {
        debug!("Process with pid <{}> for target <{}> is no longer alive", pid, target.name());
    }
    debug!("Removing pid file for target <{}> at <{}>", target.name(), pid_path.display());
    std::fs::remove_file(&pid_path)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::{latest_update_time, LastRun};

    #[test]
    fn latest_update_time_no_times() {
        let times = vec![];
        assert_eq!(latest_update_time(times), LastRun::Never);
    }

    #[test]
    fn latest_update_time_one_never() {
        let times = vec![LastRun::Never];
        assert_eq!(latest_update_time(times), LastRun::Never);
    }

    #[test]
    fn latest_update_time_one_time() {
        let time = std::time::SystemTime::now();
        let times = vec![LastRun::Time(time)];
        assert_eq!(latest_update_time(times), LastRun::Time(time));
    }

    #[test]
    fn latest_update_time_one_never_one_time() {
        let time = std::time::SystemTime::now();
        assert_eq!(latest_update_time(vec![LastRun::Never, LastRun::Time(time)]), LastRun::Never);
        assert_eq!(latest_update_time(vec![LastRun::Time(time), LastRun::Never]), LastRun::Never);
    }

    #[test]
    fn latest_update_time_latest_time() {
        let earlier = std::time::SystemTime::now();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(1);
        assert_eq!(latest_update_time(vec![LastRun::Time(earlier), LastRun::Time(later)]), LastRun::Time(later));
        assert_eq!(latest_update_time(vec![LastRun::Time(later), LastRun::Time(earlier)]), LastRun::Time(later));
    }
}