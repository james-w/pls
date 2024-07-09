use crate::context::{ContainerCommand, Context, OutputsManager};
use crate::shell::{escape_and_prepend, escape_and_prepend_vec, escape_string};

pub struct ContainerRunInfo {
    pub pre_commands: Vec<String>,
    pub post_stop_commands: Vec<String>,
    pub command: String,
    pub name: String,
    pub network: Option<String>,
}

// TODO: generate the name and return it in the result
pub fn run_command(
    container: &ContainerCommand,
    context: &Context,
    outputs: &OutputsManager,
    container_name: &str,
    args: Vec<String>,
) -> Result<ContainerRunInfo, Box<dyn std::error::Error>> {
    let env_str = escape_and_prepend_vec(
        &container.name,
        context,
        outputs,
        "-e",
        &Some(container.env.clone()),
    )?;
    let mount_str = escape_and_prepend_vec(
        &container.name,
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
    )?;
    let workdir_str = escape_and_prepend(&container.name, context, outputs, "-w", &container.workdir)?;
    let cmd_str = container.command.as_ref().map_or_else(
        || Ok("".to_string()),
        |c| context.resolve_substitutions_with_args(c, &container.name, outputs, args, &container.default_args),
    )?;
    let image = escape_string(
        context
            .resolve_substitutions(container.image.as_str(), &container.name, outputs)?
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
    let network_str = escape_and_prepend(&container.name, context, outputs, "--network", &network)?;
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
