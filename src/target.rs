use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::File;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use glob::glob;
use log::{debug, info, warn};

use crate::cleanup::CleanupManager;
use crate::context::Context;
use crate::name::FullyQualifiedName;
use crate::outputs::OutputsManager;
use crate::targets::{ContainerArtifact, ContainerCommand, ExecCommand};

#[derive(Debug, Clone)]
pub enum Target {
    Artifact(Artifact),
    Command(Command),
}

#[derive(Clone, Debug)]
pub struct TargetInfo {
    pub name: FullyQualifiedName,
    pub requires: Vec<FullyQualifiedName>,
    pub variables: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct CommandInfo {
    pub daemon: bool,
}

#[derive(Clone, Debug)]
pub struct ArtifactInfo {
    pub updates_paths: Option<Vec<String>>,
    pub if_files_changed: Option<Vec<String>>,
}

impl Target {
    pub fn target_info(&self) -> &TargetInfo {
        match self {
            Self::Artifact(artifact) => artifact.target_info(),
            Self::Command(command) => command.target_info(),
        }
    }

    pub fn command_info(&self) -> Option<&CommandInfo> {
        match self {
            Self::Command(command) => Some(command.command_info()),
            _ => None,
        }
    }
}

impl Targetable for Target {
    fn as_runnable(&self) -> Option<&dyn Runnable> {
        match self {
            Self::Command(c) => c.as_runnable(),
            Self::Artifact(a) => a.as_runnable(),
        }
    }

    fn as_buildable(&self) -> Option<&dyn Buildable> {
        match self {
            Self::Command(c) => c.as_buildable(),
            Self::Artifact(a) => a.as_buildable(),
        }
    }

    fn as_startable(&self) -> Option<&dyn Startable> {
        match self {
            Self::Command(c) => c.as_startable(),
            Self::Artifact(a) => a.as_startable(),
        }
    }
}

impl Target {
    pub fn artifact(&self) -> Result<&Artifact> {
        match self {
            Self::Artifact(a) => Ok(a),
            _ => Err(anyhow!("Expected an artifact")),
        }
    }

    pub fn command(&self) -> Result<&Command> {
        match self {
            Self::Command(c) => Ok(c),
            _ => Err(anyhow!("Expected a command")),
        }
    }
}

pub trait Targetable {
    fn as_runnable(&self) -> Option<&dyn Runnable> {
        None
    }

    fn as_startable(&self) -> Option<&dyn Startable> {
        None
    }

    fn as_buildable(&self) -> Option<&dyn Buildable> {
        None
    }
}

#[derive(Debug, Clone)]
pub enum Artifact {
    ContainerImage(ContainerArtifact),
}

impl Artifact {
    fn target_info(&self) -> &TargetInfo {
        match self {
            Self::ContainerImage(image) => &image.target_info,
        }
    }

    pub fn artifact_info(&self) -> &ArtifactInfo {
        match self {
            Self::ContainerImage(image) => &image.artifact_info,
        }
    }

    fn inner_as_buildable(&self) -> &dyn Buildable {
        match self {
            Self::ContainerImage(image) => image,
        }
    }
}

impl Targetable for Artifact {
    fn as_buildable(&self) -> Option<&dyn Buildable> {
        Some(self)
    }

    fn as_runnable(&self) -> Option<&dyn Runnable> {
        Some(self)
    }
}

impl Buildable for Artifact {
    fn build(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let mut to_stop: Vec<&Target> = vec![];
        let result = self.build_target_inner(
            context,
            outputs,
            &mut to_stop,
            cleanup_manager.clone(),
            true,
        );
        // TODO: use cleanup manager to handle the to_stop stuff?
        // Reverse the order that they were started
        to_stop.reverse();
        for target in to_stop.iter() {
            // TODO: add in errors to result
            if let Some(s) = target.as_startable() {
                if let Err(e) = s.stop(context, outputs, cleanup_manager.clone()) {
                    warn!(
                        "Error stopping target <{}>: {}",
                        target.target_info().name,
                        e
                    );
                }
            } else {
                panic!(
                    "Supposed to stop <{}> but as_startable is None",
                    target.target_info().name
                );
            }
        }
        result
    }
}

impl Artifact {
    fn build_target_inner(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        _to_stop: &mut Vec<&Target>,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        check_should_rerun: bool,
    ) -> Result<()> {
        debug!(
            "Building target <{}>, with definition <{:?}>",
            self.target_info().name,
            self
        );
        run_required(
            self.target_info(),
            context,
            outputs,
            cleanup_manager.clone(),
        )?;
        let resolved_requirements = find_required(self.target_info(), context)?;
        if check_should_rerun
            && !should_rerun(
                self.target_info(),
                self.artifact_info(),
                &resolved_requirements,
                context,
                outputs,
            )?
        {
            debug!(
                "Skipping target <{}> as it does not need to be run",
                self.target_info().name
            );
            info!("[{}] Up to date", self.target_info().name);
            return Ok(());
        }

        // TODO: to_start
        self.inner_as_buildable()
            .build(context, outputs, cleanup_manager.clone())?;
        // TODO: check that updates_paths were created?
        let _ = create_metadata_dir(self.target_info().name.to_string().as_str())?;
        File::create(last_run_path(self.target_info())?)?;
        Ok(())
    }
}

impl Runnable for Artifact {
    fn run(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        if !args.is_empty() {
            return Err(anyhow!("Artifacts do not accept arguments"));
        }
        let mut to_stop: Vec<&Target> = vec![];
        let result = self.build_target_inner(
            context,
            outputs,
            &mut to_stop,
            cleanup_manager.clone(),
            false,
        );
        // TODO: use cleanup manager to handle the to_stop stuff?
        // Reverse the order that they were started
        to_stop.reverse();
        for target in to_stop.iter() {
            // TODO: add in errors to result
            if let Some(s) = target.as_startable() {
                if let Err(e) = s.stop(context, outputs, cleanup_manager.clone()) {
                    warn!(
                        "Error stopping target <{}>: {}",
                        target.target_info().name,
                        e
                    );
                }
            } else {
                panic!(
                    "Supposed to stop <{}> but as_startable is None",
                    target.target_info().name
                );
            }
        }
        result
    }
}

impl Artifact {
    pub fn container_image(&self) -> Result<&ContainerArtifact> {
        match self {
            Self::ContainerImage(image) => Ok(image),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    Exec(ExecCommand),
    Container(ContainerCommand),
}

impl Command {
    fn target_info(&self) -> &TargetInfo {
        match self {
            Self::Exec(exec) => &exec.target_info,
            Self::Container(container) => &container.target_info,
        }
    }

    pub fn command_info(&self) -> &CommandInfo {
        match self {
            Self::Exec(exec) => &exec.command_info,
            Self::Container(container) => &container.command_info,
        }
    }

    fn inner_as_runnable(&self) -> &dyn Runnable {
        match self {
            Self::Exec(exec) => exec,
            Self::Container(container) => container,
        }
    }
}

impl Targetable for Command {
    fn as_runnable(&self) -> Option<&dyn Runnable> {
        Some(self)
    }

    fn as_startable(&self) -> Option<&dyn Startable> {
        Some(self)
    }
}

impl Runnable for Command {
    fn run(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        let mut to_stop: Vec<&Target> = vec![];
        let result = self.run_target_inner(
            context,
            outputs,
            &mut to_stop,
            cleanup_manager.clone(),
            args,
        );
        // TODO: use cleanup manager to handle the to_stop stuff?
        // Reverse the order that they were started
        to_stop.reverse();
        for target in to_stop.iter() {
            // TODO: add in errors to result
            if let Some(s) = target.as_startable() {
                if let Err(e) = s.stop(context, outputs, cleanup_manager.clone()) {
                    warn!(
                        "Error stopping target <{}>: {}",
                        target.target_info().name,
                        e
                    );
                }
            } else {
                panic!(
                    "Supposed to stop <{}> but as_startable is None",
                    target.target_info().name
                );
            }
        }
        result
    }
}

fn run_required(
    target_info: &TargetInfo,
    context: &Context,
    outputs: &mut OutputsManager,
    cleanup_manager: Arc<Mutex<CleanupManager>>,
) -> Result<()> {
    let resolved_requirements = find_required(target_info, context)?;
    for required_target in resolved_requirements.clone().into_iter() {
        match (
            required_target.as_buildable(),
            required_target
                .command_info()
                .map(|c| c.daemon)
                .unwrap_or(false),
            required_target.as_startable(),
            required_target.as_runnable(),
        ) {
            (Some(buildable), _, _, _) => {
                debug!(
                    "Building required target <{}> for target <{}>",
                    required_target.target_info().name,
                    required_target.target_info().name
                );
                buildable.build(context, outputs, cleanup_manager.clone())?;
            }
            (None, true, None, _) => panic!(
                "Don't know how to start as it is a daemon with as_startable None {:?}",
                required_target
            ),
            (None, true, Some(startable), _) => {
                debug!(
                    "Starting required target <{}> for target <{}>",
                    required_target.target_info().name,
                    required_target.target_info().name
                );
                startable.start(context, outputs, cleanup_manager.clone(), vec![])?;
            }
            (None, false, _, Some(runnable)) => {
                debug!(
                    "Running required target <{}> for target <{}>",
                    required_target.target_info().name,
                    required_target.target_info().name
                );
                runnable.run(context, outputs, cleanup_manager.clone(), vec![])?;
            }
            _ => panic!("Don't know how to build {:?}", required_target),
        }
    }
    Ok(())
}

impl Command {
    fn run_target_inner(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        _to_stop: &mut Vec<&Target>,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        debug!(
            "Running target <{}>, with definition <{:?}>",
            self.target_info().name,
            self
        );
        run_required(
            self.target_info(),
            context,
            outputs,
            cleanup_manager.clone(),
        )?;
        self.inner_as_runnable()
            .run(context, outputs, cleanup_manager.clone(), args)?;
        let _ = create_metadata_dir(self.target_info().name.to_string().as_str())?;
        File::create(last_run_path(self.target_info())?)?;
        Ok(())
    }

    fn start_target_inner(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        _to_stop: &mut Vec<&Target>,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        debug!(
            "Starting target <{}>, with definition <{:?}>",
            self.target_info().name,
            self
        );
        run_required(
            self.target_info(),
            context,
            outputs,
            cleanup_manager.clone(),
        )?;
        self.inner_as_startable()
            .start(context, outputs, cleanup_manager, args)?;
        // TODO: last timestamp file
        Ok(())
    }
}

impl Startable for Command {
    fn start(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        let mut to_stop: Vec<&Target> = vec![];
        let result = self.start_target_inner(
            context,
            outputs,
            &mut to_stop,
            cleanup_manager.clone(),
            args,
        );
        // TODO: use cleanup manager to handle the to_stop stuff?
        // Reverse the order that they were started
        to_stop.reverse();
        for target in to_stop.iter() {
            // TODO: add in errors to result
            if let Some(s) = target.as_startable() {
                if let Err(e) = s.stop(context, outputs, cleanup_manager.clone()) {
                    warn!(
                        "Error stopping target <{}>: {}",
                        target.target_info().name,
                        e
                    );
                }
            } else {
                panic!(
                    "Supposed to stop <{}> but as_startable is None",
                    target.target_info().name
                );
            }
        }
        result
    }

    fn stop(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        debug!(
            "Stopping target <{}>, with definition <{:?}>",
            self.target_info().name,
            self
        );
        // Should this stop things that were started when this was started?
        self.inner_as_startable()
            .stop(context, outputs, cleanup_manager.clone())?;
        // TODO: last run file?
        Ok(())
    }
}

impl Command {
    pub fn exec(&self) -> Result<&ExecCommand> {
        match self {
            Self::Exec(exec) => Ok(exec),
            _ => Err(anyhow!("Expected an exec command")),
        }
    }

    pub fn container(&self) -> Result<&ContainerCommand> {
        match self {
            Self::Container(container) => Ok(container),
            _ => Err(anyhow!("Expected a container command")),
        }
    }

    fn inner_as_startable(&self) -> &dyn Startable {
        match self {
            Self::Exec(exec) => exec,
            Self::Container(container) => container,
        }
    }
}

pub trait Runnable {
    fn run(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()>;
}

pub trait Startable {
    fn start(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()>;

    fn stop(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()>;
}

pub trait Buildable {
    fn build(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()>;
}

fn find_required(target: &TargetInfo, context: &Context) -> Result<Vec<Target>> {
    let mut resolved_requirements = vec![];
    for require in target.requires.iter() {
        if require == &target.name {
            return Err(anyhow!("Target <{}> requires itself", target.name));
        }
        match context.targets.get(require) {
            Some(target) => {
                resolved_requirements.push(target.clone());
            }
            None => {
                return Err(anyhow!(
                    "Target <{}> not found in config file <{}>",
                    require,
                    context.config_path,
                ))
            }
        };
    }
    Ok(resolved_requirements)
}

fn metadata_path(name: &str) -> Result<std::path::PathBuf> {
    Ok(std::env::current_dir()?.join(".taskrunner").join(name))
}

pub fn create_metadata_dir(name: &str) -> Result<std::path::PathBuf> {
    let taskrunner_dir = metadata_path(name)?;
    debug!(
        "Creating metadata dir for target <{}> at <{}>",
        name,
        taskrunner_dir.display()
    );
    std::fs::create_dir_all(&taskrunner_dir)?;
    Ok(taskrunner_dir)
}

fn last_run_path(target: &TargetInfo) -> Result<std::path::PathBuf> {
    Ok(metadata_path(target.name.to_string().as_str())?.join("last_run"))
}

#[derive(Debug, PartialEq, Eq)]
enum LastRun {
    Never,
    Time(std::time::SystemTime),
}

fn latest_update_time(times: Vec<LastRun>) -> LastRun {
    times
        .into_iter()
        .fold(None, |acc, modified_time| match (acc, modified_time) {
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
        })
        .unwrap_or(LastRun::Never)
}

fn update_times_of_glob(glob_str: &str) -> Result<Vec<LastRun>> {
    Ok(glob(glob_str)?
        .map(|entry| match entry {
            Ok(path) => std::fs::metadata(&path).map_or_else(
                |_| {
                    debug!("File <{}> does not exist", path.display());
                    LastRun::Never
                },
                |metadata| LastRun::Time(metadata.modified().unwrap()),
            ),
            Err(e) => {
                warn!("Error globbing file <{}>: {}", glob_str, e);
                LastRun::Never
            }
        })
        .collect())
}

fn update_times_of_glob_ignoring_missing(glob_str: &str) -> Result<Vec<LastRun>> {
    Ok(glob(glob_str)?
        .filter_map(|entry| match entry {
            Ok(path) => std::fs::metadata(&path).map_or_else(
                |_| {
                    debug!("File <{}> does not exist", path.display());
                    None
                },
                |metadata| Some(LastRun::Time(metadata.modified().unwrap())),
            ),
            Err(e) => {
                warn!("Error globbing file <{}>: {}", glob_str, e);
                None
            }
        })
        .collect())
}

fn latest_update_time_of_paths(
    paths: &Vec<String>,
    target: &TargetInfo,
    context: &Context,
    outputs: &OutputsManager,
) -> Result<LastRun> {
    // Where do the Err values go?
    let update_times: Result<Vec<Vec<LastRun>>> = paths
        .iter()
        .map(|path| context.resolve_substitutions(path, &target.name, outputs))
        .map(|path| path.and_then(|path| update_times_of_glob(path.as_str())))
        .collect();
    Ok(latest_update_time(
        update_times?.into_iter().flatten().collect(),
    ))
}

fn latest_update_time_of_paths_ignoring_missing(
    paths: &Vec<String>,
    target: &TargetInfo,
    context: &Context,
    outputs: &OutputsManager,
) -> Result<LastRun> {
    // Where do the Err values go?
    let update_times: Result<Vec<Vec<LastRun>>> = paths
        .iter()
        .map(|path| context.resolve_substitutions(path, &target.name, outputs))
        .map(|path| path.and_then(|path| update_times_of_glob_ignoring_missing(path.as_str())))
        .collect();
    Ok(latest_update_time(
        update_times?.into_iter().flatten().collect(),
    ))
}

fn last_run(
    target: &TargetInfo,
    artifact_info: &ArtifactInfo,
    context: &Context,
    outputs: &OutputsManager,
) -> Result<LastRun> {
    let last_run_path = last_run_path(target)?;
    match artifact_info.updates_paths {
        Some(ref updates_paths) => {
            debug!(
                "Checking if updates_paths have changed for target <{}>",
                target.name
            );
            latest_update_time_of_paths(updates_paths, target, context, outputs)
        }
        None => {
            debug!(
                "Checking last run time for target <{}> based on <{}>",
                target.name,
                last_run_path.display()
            );
            std::fs::metadata(&last_run_path).map_or_else(
                |_| {
                    debug!(
                        "Last run file does not exist at <{}> for target <{}>",
                        last_run_path.display(),
                        target.name
                    );
                    Ok(LastRun::Never)
                },
                |metadata| Ok(LastRun::Time(metadata.modified().unwrap())),
            )
        }
    }
}

fn should_rerun(
    target: &TargetInfo,
    artifact_info: &ArtifactInfo,
    resolved_requirements: &Vec<Target>,
    context: &Context,
    outputs: &OutputsManager,
) -> Result<bool> {
    if let Some(ref if_files_changed) = artifact_info.if_files_changed {
        debug!("Checking if files changed for target <{}>", target.name);
        let last_run = last_run(target, artifact_info, context, outputs)?;
        debug!("Last run time: {:?}", last_run);
        let mut run_again = false;
        if let LastRun::Time(last_run) = last_run {
            let latest_time_of_deps = latest_update_time_of_paths_ignoring_missing(
                if_files_changed,
                target,
                context,
                outputs,
            )?;
            debug!("Latest time on dependencies: {:?}", latest_time_of_deps);
            if let LastRun::Time(latest_time_of_deps) = latest_time_of_deps {
                if latest_time_of_deps > last_run {
                    debug!(
                        "Running task as dependencies have changed for target <{}>",
                        target.name
                    );
                    run_again = true;
                }
            }
            if !run_again {
                for required_target in resolved_requirements.iter() {
                    let required_last_run_path =
                        metadata_path(required_target.target_info().name.to_string().as_str())?
                            .join("last_run");
                    if !required_last_run_path.exists() {
                        debug!(
                            "Running task as required target <{}> has not been run for target <{}>",
                            required_target.target_info().name,
                            target.name
                        );
                        run_again = true;
                    } else {
                        let required_last_run =
                            std::fs::metadata(&required_last_run_path)?.modified()?;
                        if required_last_run > last_run {
                            debug!("Running task as required target <{}> was run after target for target <{}>", required_target.target_info().name, target.name);
                            run_again = true;
                        } else {
                            debug!("Required target <{}> was run at <{:?}>, before target, for target <{}>", required_target.target_info().name, required_last_run, target.name);
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
