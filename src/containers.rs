use crate::config::Container;
use crate::context::Context;
use crate::shell::{escape_string, escape_and_prepend, escape_and_prepend_vec};

pub struct ContainerCommand {
    pub pre_commands: Vec<String>,
    pub post_stop_commands: Vec<String>,
    pub command: String,
    pub name: String,
    pub network: Option<String>,
}

pub fn run_command(container: &Container, context: &Context, container_name: &str) -> Result<ContainerCommand, shlex::QuoteError> {
    let target_name = container.name.as_str();
    let env_str = escape_and_prepend_vec(target_name, context, "-e", &container.env)?;
    let mount_str = escape_and_prepend_vec(
        target_name,
        context,
        "-v",
        &container
            .mount
            .as_ref()
            .map(|m| m.iter().map(|(k, v)| format!("{}:{}", k, v)).collect()),
    )?;
    let workdir_str = escape_and_prepend(target_name, context, "-w", &container.workdir)?;
    let cmd_str = container
            .command
            .as_ref()
            .map_or_else(
                || "".to_string(),
                |c| context.resolve_substitutions(c, target_name),
            );
    let image = escape_string(
        context.resolve_substitutions(container.image.as_str(), target_name).as_str(),
    )?;
    let mut network = container.network.clone();
    let mut pre_commands = vec![];
    let mut post_stop_commands = vec![];
    if network.is_none() && container.create_network {
        network = Some(container_name.to_string());
        pre_commands.push(format!("podman network create {}", container_name));
        post_stop_commands.push(format!("podman network rm {}", container_name));
    }
    let network_str = escape_and_prepend(target_name, context, "--network", &network)?;
    let cmd = format!(
            "podman run --name {} --rm {} {} {} {} {} {}",
            escape_string(container_name)?, env_str, mount_str, workdir_str, network_str, image, cmd_str
        );
    Ok(ContainerCommand {
        pre_commands,
        post_stop_commands,
        command: cmd,
        name: container_name.to_string(),
        network,
    })
}
