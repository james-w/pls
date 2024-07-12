use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use log::{debug, info};
use validator::Validate;

use crate::cleanup::CleanupManager;
use crate::commands::{run_command, spawn_command_with_pidfile, stop_using_pidfile};
use crate::config::ContainerCommand as ConfigContainerCommand;
use crate::context::Context;
use crate::default::{default_optional, default_to};
use crate::outputs::OutputsManager;
use crate::rand::rand_string;
use crate::shell::{escape_and_prepend, escape_and_prepend_vec, escape_string};
use crate::target::{create_metadata_dir, CommandInfo, Runnable, Startable, TargetInfo};

#[derive(Debug, Clone, Validate)]
pub struct ContainerCommand {
    #[validate(length(min = 1, message = "image must not be empty"))]
    pub image: String,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Vec<String>,
    #[validate(length(min = 1))]
    pub command: Option<String>,
    #[validate(custom(function = "crate::validate::keys_and_values_non_empty_strings"))]
    pub mount: HashMap<String, String>,
    #[validate(length(min = 1))]
    pub workdir: Option<String>,
    #[validate(length(min = 1))]
    pub network: Option<String>,
    pub create_network: bool,
    pub default_args: Option<String>,

    #[validate(nested)]
    pub target_info: TargetInfo,
    #[validate(nested)]
    pub command_info: CommandInfo,
}

impl ContainerCommand {
    pub fn from_config(
        target_info: TargetInfo,
        command_info: CommandInfo,
        defn: &ConfigContainerCommand,
        base: Option<&Self>,
    ) -> Self {
        let mut env = vec![];
        if let Some(base) = base {
            env.extend(base.env.clone());
        }
        env.extend(defn.env.clone().unwrap_or_default());
        ContainerCommand {
            target_info,
            command_info,
            image: default_to!(defn, base, image),
            env,
            command: default_optional!(defn, base, command),
            mount: default_to!(defn, base, mount),
            workdir: default_optional!(defn, base, workdir),
            network: default_optional!(defn, base, network),
            create_network: default_to!(defn, base, create_network, false),
            default_args: default_optional!(defn, base, default_args),
        }
    }
}

impl Runnable for ContainerCommand {
    fn run(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        let container_name = format!("{}-{}", self.target_info.name, rand_string(8));
        // TODO: default_args
        let command = container_run_command(self, context, outputs, container_name.as_str(), args)
            .map_err(|e| {
                anyhow!(
                    "Error escaping podman command for <{}>: {}",
                    self.target_info.name,
                    e
                )
            })?;
        for pre_command in command.pre_commands.iter() {
            run_command(pre_command.as_str())?;
        }
        debug!(
            "Running container for target <{}> with command <{:?}>",
            self.target_info.name, self.command
        );
        info!(
            "[{}] Running container using {}",
            self.target_info.name.name,
            context.resolve_substitutions(self.image.as_str(), &self.target_info.name, outputs)?
        );
        let container_name = command.name;
        for post_command in command.post_stop_commands.into_iter() {
            cleanup_manager.lock().unwrap().push_cleanup(
                "clean_up_network".to_string(),
                move || {
                    debug!("Running post stop command <{}>", post_command);
                    run_command(post_command.as_str()).unwrap();
                },
            );
        }
        cleanup_manager
            .lock()
            .unwrap()
            .push_cleanup("stop_container".to_string(), move || {
                let stop_command = format!("podman stop -i {}", container_name);
                debug!("Stopping container with command <{}>", stop_command);
                run_command(stop_command.as_str()).unwrap();
            });
        let result = run_command(command.command.as_str());
        result
    }
}

impl Startable for ContainerCommand {
    fn start(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
        args: Vec<String>,
    ) -> Result<()> {
        let container_name = format!("{}-{}", self.target_info.name, rand_string(8));
        // TODO: default_args
        let command = container_run_command(self, context, outputs, container_name.as_str(), args)
            .map_err(|e| {
                anyhow!(
                    "Error escaping podman command for <{}>: {}",
                    self.target_info.name,
                    e
                )
            })?;
        debug!(
            "Running container for target <{}> with command <{:?}>",
            self.target_info.name, self.command
        );

        let config_dir = create_metadata_dir(self.target_info.name.to_string().as_str())?;
        let pid_path = config_dir.join("pid");
        let log_path = config_dir.join("log");
        let image_name =
            context.resolve_substitutions(self.image.as_str(), &self.target_info.name, outputs)?;
        let log_start = || {
            info!(
                "[{}] Starting container using {}",
                self.target_info.name, image_name
            );
        };
        for pre_command in command.pre_commands.iter() {
            run_command(pre_command.as_str())?;
        }
        spawn_command_with_pidfile(
            command.command.as_str(),
            &[],
            &pid_path,
            &log_path,
            log_start,
        )?;
        // TODO: post_stop_commands
        outputs.store_output(self.target_info.name.clone(), "name", command.name.as_str());
        if let Some(network) = command.network {
            outputs.store_output(self.target_info.name.clone(), "network", network.as_str());
        }
        Ok(())
    }

    fn stop(
        &self,
        _context: &Context,
        _outputs: &mut OutputsManager,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let config_dir = create_metadata_dir(self.target_info.name.to_string().as_str())?;

        let pid_path = config_dir.join("pid");
        debug!(
            "Searching for pid file for target <{}> at <{}>",
            self.target_info.name,
            pid_path.display()
        );
        let log_stop = || {
            info!("[{}] Stopping", self.target_info.name);
        };
        stop_using_pidfile(&pid_path, log_stop)
    }
}

pub struct ContainerRunInfo {
    pub pre_commands: Vec<String>,
    pub post_stop_commands: Vec<String>,
    pub command: String,
    pub name: String,
    pub network: Option<String>,
}

// TODO: generate the name and return it in the result
fn container_run_command(
    container: &ContainerCommand,
    context: &Context,
    outputs: &OutputsManager,
    container_name: &str,
    args: Vec<String>,
) -> Result<ContainerRunInfo> {
    let env_str = escape_and_prepend_vec(
        &container.target_info.name,
        context,
        outputs,
        "-e",
        &Some(container.env.clone()),
    )
    .map_err(|e| anyhow!("Failed to escape env: {}", e))?;
    let mount_str = escape_and_prepend_vec(
        &container.target_info.name,
        context,
        outputs,
        "-v",
        &Some(
            container
                .mount
                .iter()
                .map(|(k, v)| format!("{}:{}", shellexpand::tilde(k), v))
                .collect(),
        ),
    )
    .map_err(|e| anyhow!("Failed to escape mount: {}", e))?;
    let workdir_str = escape_and_prepend(
        &container.target_info.name,
        context,
        outputs,
        "-w",
        &container.workdir,
    )
    .map_err(|e| anyhow!("Failed to escape workdir: {}", e))?;
    let cmd_str = container
        .command
        .as_ref()
        .map_or_else(
            || Ok("".to_string()),
            |c| {
                context.resolve_substitutions_with_args(
                    c,
                    &container.target_info.name,
                    outputs,
                    args,
                    &container.default_args,
                )
            },
        )
        .map_err(|e| anyhow!("Failed to escape command: {}", e))?;
    let image = escape_string(
        context
            .resolve_substitutions(
                container.image.as_str(),
                &container.target_info.name,
                outputs,
            )?
            .as_str(),
    )?;
    let mut network = container.network.clone();
    let mut pre_commands = vec![];
    let mut post_stop_commands = vec![];
    if network.is_none() && container.create_network {
        network = Some(container_name.to_string());
        pre_commands.push(format!("podman network create {}", container_name));
        post_stop_commands.push(format!("podman network rm {}", container_name));
    }
    let network_str = escape_and_prepend(
        &container.target_info.name,
        context,
        outputs,
        "--network",
        &network,
    )
    .map_err(|e| anyhow!("Failed to escape network: {}", e))?;
    let cmd = format!(
        "podman run --name {} --rm {} {} {} {} {} {}",
        escape_string(container_name)?,
        env_str,
        mount_str,
        workdir_str,
        network_str,
        image,
        cmd_str
    );
    Ok(ContainerRunInfo {
        pre_commands,
        post_stop_commands,
        command: cmd,
        name: container_name.to_string(),
        network,
    })
}
