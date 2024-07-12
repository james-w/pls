use std::collections::HashMap;

use anyhow::{anyhow, Result};
use log::debug;
use validator::Validate;

use crate::{
    config::{
        ArtifactInfo as ConfigArtifactInfo, CommandInfo as ConfigCommandInfo, Config,
        ContainerBuild as ConfigContainerBuild, ContainerCommand as ConfigContainerCommand,
        ExecArtifact as ConfigExecArtifact, ExecCommand as ConfigExecCommand,
        TargetInfo as ConfigTargetInfo,
    },
    default::default_to,
    name::FullyQualifiedName,
    outputs::OutputsManager,
    shell::escape_string,
    target::{Artifact, ArtifactInfo, Command, CommandInfo, Target, TargetInfo},
    targets::{ContainerArtifact, ContainerCommand, ExecArtifact, ExecCommand},
};

enum Variable {
    Simple(String),
    Global(String),
    Ref(String, String),
    Output(String, String),
}

impl Variable {
    pub fn from_string(input: &str) -> Result<Self> {
        if !input.contains('.') {
            Ok(Self::Simple(input.to_string()))
        } else if let Some(key) = input.strip_prefix("globals.") {
            Ok(Self::Global(key.to_string()))
        } else {
            let parts = input.split('.').collect::<Vec<_>>();
            if parts.len() > 2 && parts[parts.len() - 2] == "output" {
                Ok(Self::Output(
                    parts[0..parts.len() - 2].join(".").to_string(),
                    parts[parts.len() - 1].to_string(),
                ))
            } else {
                Ok(Self::Ref(
                    parts[0..parts.len() - 1].join(".").to_string(),
                    parts[parts.len() - 1].to_string(),
                ))
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct Context {
    pub variables: HashMap<FullyQualifiedName, HashMap<String, String>>,
    pub globals: HashMap<String, String>,

    pub targets: HashMap<FullyQualifiedName, Target>,

    pub config_path: String,
}

fn get_lookup_name(name: String, default_tag: String) -> FullyQualifiedName {
    if let Some((tag, name)) = name.split_once('.') {
        FullyQualifiedName {
            tag: tag.to_string(),
            name: name.to_string(),
        }
    } else {
        FullyQualifiedName {
            tag: default_tag,
            name,
        }
    }
}

fn resolve_requires<'a, I>(
    requires: I,
    name_map: &HashMap<String, Vec<FullyQualifiedName>>,
) -> Result<Vec<FullyQualifiedName>>
where
    I: Iterator<Item = &'a String>,
{
    requires
        .map(|r| {
            let candidates = name_map.get(r);
            match candidates {
                Some(candidates) => {
                    if candidates.len() > 1 {
                        Err(anyhow!(
                            "Ambiguous reference <{}>, could be <{}>",
                            r,
                            candidates
                                .iter()
                                .map(|c| c.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))
                    } else {
                        Ok(candidates.first().unwrap().clone())
                    }
                }
                None => Err(anyhow!("Non-existent reference <{}>", r)),
            }
        })
        .collect()
}

fn extract_variables(input: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut start = None;

    for (i, c) in input.chars().enumerate() {
        match c {
            '{' => start = Some(i + 1),
            '}' => {
                if let Some(s) = start {
                    results.push(input[s..i].to_string());
                    start = None;
                }
            }
            _ => (),
        }
    }

    results
}

fn resolve_variables<'a, I>(
    variables: I,
    name_map: &HashMap<String, Vec<FullyQualifiedName>>,
) -> Result<HashMap<String, String>>
where
    I: Iterator<Item = (&'a String, &'a String)>,
{
    variables
        .map(|r| {
            Ok((
                resolve_target_names_in(r.0, name_map)?,
                resolve_target_names_in(r.1, name_map)?,
            ))
        })
        .collect()
}

/// Resolve {foo.bar} to {command.exec.foo.bar} or similar
pub fn resolve_target_names_in(
    input: &str,
    name_map: &HashMap<String, Vec<FullyQualifiedName>>,
) -> Result<String> {
    let mut output = input.to_string();
    for variable in extract_variables(input) {
        let var = Variable::from_string(variable.as_str())?;
        let (target_name, key) = match var {
            Variable::Simple(_) => continue,
            Variable::Global(_) => continue,
            Variable::Ref(target_name, key) => (target_name, key),
            Variable::Output(target_name, key) => (target_name, format!("output.{}", key)),
        };
        let candidates = name_map.get(&target_name.to_string());
        let matched = match candidates {
            Some(candidates) => {
                if candidates.len() > 1 {
                    return Err(anyhow!(
                        "Ambiguous reference <{}>, could be <{}>",
                        target_name,
                        candidates
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                } else {
                    candidates.first().unwrap()
                }
            }
            None => return Err(anyhow!("Non-existent reference <{}>", target_name)),
        };
        let resolved = format!("{}.{}", matched, key);
        debug!("Resolved <{}> to <{}>", variable, resolved);
        output = output.replace(
            format!("{{{}}}", variable).as_str(),
            format!("{{{}}}", resolved.as_str()).as_str(),
        );
    }
    Ok(output)
}

pub fn resolve_target_names_in_map(
    input: &HashMap<String, String>,
    name_map: &HashMap<String, Vec<FullyQualifiedName>>,
) -> Result<HashMap<String, String>> {
    input
        .iter()
        .map(|(k, v)| {
            Ok((
                resolve_target_names_in(k, name_map)?,
                resolve_target_names_in(v, name_map)?,
            ))
        })
        .collect()
}

pub fn resolve_target_names_in_vec(
    input: &[String],
    name_map: &HashMap<String, Vec<FullyQualifiedName>>,
) -> Result<Vec<String>> {
    input
        .iter()
        .map(|v| resolve_target_names_in(v, name_map))
        .collect()
}

fn target_info_from_config(
    name: FullyQualifiedName,
    config: &ConfigTargetInfo,
    name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    base: Option<&TargetInfo>,
) -> Result<TargetInfo> {
    let mut requires = base
        .as_ref()
        .map(|b| b.requires.clone())
        .unwrap_or_default();
    let others = config
        .requires
        .as_ref()
        .map(|rs| {
            resolve_requires(rs.iter(), name_map)
                .map_err(|e| anyhow!("Invalid reference from <{}>: {}", name, e))
        })
        .transpose()?;
    if let Some(others) = others {
        requires.extend(others);
    }
    let mut variables = base
        .as_ref()
        .map(|b| b.variables.clone())
        .unwrap_or_default();
    let other_variables = config
        .variables
        .as_ref()
        .map(|vs| {
            resolve_variables(vs.iter(), name_map)
                .map_err(|e| anyhow!("Invalid reference from <{}>: {}", name, e))
        })
        .transpose()?;
    if let Some(other_variables) = other_variables {
        variables.extend(other_variables);
    }
    Ok(TargetInfo {
        name,
        requires,
        variables,
        description: config.description.clone(),
    })
}

fn command_info_from_config(
    _name: FullyQualifiedName,
    config: &ConfigCommandInfo,
    base: Option<&CommandInfo>,
) -> CommandInfo {
    CommandInfo {
        daemon: default_to!(config, base, daemon, false),
    }
}

fn artifact_info_from_config(
    name: FullyQualifiedName,
    config: &ConfigArtifactInfo,
    name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    base: Option<&ArtifactInfo>,
) -> Result<ArtifactInfo> {
    let updates_paths = base
        .as_ref()
        .map(|b| b.updates_paths.clone())
        .unwrap_or_default();
    let others = config
        .updates_paths
        .as_ref()
        .map(|rs| {
            resolve_target_names_in_vec(rs, name_map)
                .map_err(|e| anyhow!("Invalid reference from <{}>: {}", name, e))
        })
        .transpose()?;
    let updates_paths = match (updates_paths, others) {
        (res @ Some(_), None) => res,
        (None, res @ Some(_)) => res,
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
        _ => None,
    };
    let if_files_changed = base
        .as_ref()
        .map(|b| b.if_files_changed.clone())
        .unwrap_or_default();
    let other_changed = config
        .if_files_changed
        .as_ref()
        .map(|rs| {
            resolve_target_names_in_vec(rs, name_map)
                .map_err(|e| anyhow!("Invalid reference from <{}>: {}", name, e))
        })
        .transpose()?;
    let if_files_changed = match (if_files_changed, other_changed) {
        (res @ Some(_), None) => res,
        (None, res @ Some(_)) => res,
        (Some(mut a), Some(b)) => {
            a.extend(b);
            Some(a)
        }
        _ => None,
    };
    Ok(ArtifactInfo {
        updates_paths,
        if_files_changed,
    })
}

fn resolve_extends(
    name: FullyQualifiedName,
    command: &ConfigWrapper,
    commands: &HashMap<FullyQualifiedName, ConfigWrapper>,
    name_map: &HashMap<String, Vec<FullyQualifiedName>>,
) -> Result<Target> {
    let base = if let Some(extends) = command.extends() {
        let extends_fully_qualified =
            get_lookup_name(extends.clone(), command.type_tag().to_string());
        let base = commands.get(&extends_fully_qualified);
        if let Some(base) = base {
            resolve_extends(extends_fully_qualified, base, commands, name_map).map(Some)
        } else {
            Err(anyhow!(
                "<{}> extends non-existent <{}>",
                name,
                extends_fully_qualified
            ))
        }
    } else {
        Ok(None)
    }?;
    let target_info = target_info_from_config(
        name.clone(),
        &command.target_info().with_resolved_targets(name_map)?,
        name_map,
        base.as_ref().map(|b| b.target_info()),
    )?;
    if command.is_artifact() {
        let artifact_info = artifact_info_from_config(
            name,
            &command
                .artifact_info()
                .expect("{} doesn't have artifact_info")
                .with_resolved_targets(name_map)?,
            name_map,
            base.as_ref()
                .map(|b| b.artifact().map(|a| a.artifact_info()))
                .transpose()?,
        )?;
        match command {
            ConfigWrapper::ContainerBuild(command) => {
                let base = base
                    .as_ref()
                    .map::<Result<_>, _>(|b| b.artifact()?.container_image())
                    .transpose()?;
                let artifact = ContainerArtifact::from_config(
                    target_info,
                    artifact_info,
                    &command.with_resolved_targets(name_map)?,
                    base,
                );
                artifact.validate()?;
                Ok(Target::Artifact(Artifact::ContainerImage(artifact)))
            }
            ConfigWrapper::ExecArtifact(command) => {
                let base = base
                    .as_ref()
                    .map::<Result<_>, _>(|b| b.artifact()?.exec())
                    .transpose()?;
                let artifact = ExecArtifact::from_config(
                    target_info,
                    artifact_info,
                    &command.with_resolved_targets(name_map)?,
                    base,
                );
                artifact.validate()?;
                Ok(Target::Artifact(Artifact::Exec(artifact)))
            }
            _ => panic!("Unknown artifact type, got <{}>", command.type_tag()),
        }
    } else {
        let command_info = command_info_from_config(
            name.clone(),
            &command
                .command_info()
                .expect("{} doesn't have config_info")
                .with_resolved_targets(name_map)?,
            base.as_ref()
                .map(|b| b.command().map(|c| c.command_info()))
                .transpose()?,
        );
        match command {
            ConfigWrapper::Exec(command) => {
                let base = base
                    .as_ref()
                    .map::<Result<_>, _>(|b| b.command()?.exec())
                    .transpose()?;
                let exec = ExecCommand::from_config(
                    target_info,
                    command_info,
                    &command.with_resolved_targets(name_map)?,
                    base,
                );
                exec.validate()?;
                Ok(Target::Command(Command::Exec(exec)))
            }
            ConfigWrapper::Container(command) => {
                let base = base
                    .as_ref()
                    .map::<Result<_>, _>(|b| b.command()?.container())
                    .transpose()?;
                let container = ContainerCommand::from_config(
                    target_info,
                    command_info,
                    &command.with_resolved_targets(name_map)?,
                    base,
                );
                container
                    .validate()
                    .map_err(|e| anyhow!("Error validating <{}>: {}", name, e))?;
                Ok(Target::Command(Command::Container(container)))
            }
            _ => panic!("Unknown command type, got <{}>", command.type_tag()),
        }
    }
}

pub enum CommandLookupResult<'a> {
    NotFound,
    Found(&'a Target),
    Duplicates(Vec<String>),
}

enum ConfigWrapper {
    Exec(ConfigExecCommand),
    Container(ConfigContainerCommand),
    ContainerBuild(ConfigContainerBuild),
    ExecArtifact(ConfigExecArtifact),
}

impl ConfigWrapper {
    fn target_info(&self) -> &ConfigTargetInfo {
        match self {
            Self::Exec(command) => &command.target_info,
            Self::Container(command) => &command.target_info,
            Self::ContainerBuild(command) => &command.target_info,
            Self::ExecArtifact(command) => &command.target_info,
        }
    }

    fn command_info(&self) -> Option<&ConfigCommandInfo> {
        match self {
            Self::Exec(command) => Some(&command.command_info),
            Self::Container(command) => Some(&command.command_info),
            Self::ContainerBuild(_) => None,
            Self::ExecArtifact(_) => None,
        }
    }

    fn artifact_info(&self) -> Option<&ConfigArtifactInfo> {
        match self {
            Self::Exec(_) => None,
            Self::Container(_) => None,
            Self::ContainerBuild(command) => Some(&command.artifact_info),
            Self::ExecArtifact(command) => Some(&command.artifact_info),
        }
    }

    fn extends(&self) -> Option<String> {
        self.target_info().extends.clone()
    }

    fn type_tag(&self) -> &'static str {
        match self {
            Self::Exec(c) => c.type_tag(),
            Self::Container(c) => c.type_tag(),
            Self::ContainerBuild(c) => c.type_tag(),
            Self::ExecArtifact(c) => c.type_tag(),
        }
    }

    fn is_artifact(&self) -> bool {
        match self {
            Self::Exec(c) => c.is_artifact(),
            Self::Container(c) => c.is_artifact(),
            Self::ContainerBuild(c) => c.is_artifact(),
            Self::ExecArtifact(c) => c.is_artifact(),
        }
    }
}

impl Context {
    pub fn from_config(config: &Config, path: String) -> Result<Context> {
        let mut context = Context {
            config_path: path,
            ..Default::default()
        };
        if let Some(ref globals) = config.globals {
            context.globals = globals.clone();
        }
        let mut commands = HashMap::new();
        let mut name_map = HashMap::new();
        if let Some(ref c) = config.command {
            for (name, config_command) in c.exec.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::Exec(config_command.clone()),
                );
                name_map
                    .entry(name.clone())
                    .or_insert_with(Vec::new)
                    .push(fully_qualified_name.clone());
                name_map
                    .entry(fully_qualified_name.to_string())
                    .or_insert_with(Vec::new)
                    .push(fully_qualified_name);
            }
            for (name, config_command) in c.container.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::Container(config_command.clone()),
                );
                name_map
                    .entry(name.clone())
                    .or_insert_with(Vec::new)
                    .push(fully_qualified_name.clone());
                name_map
                    .entry(fully_qualified_name.to_string())
                    .or_insert_with(Vec::new)
                    .push(fully_qualified_name);
            }
        }
        if let Some(ref c) = config.artifact {
            for (name, config_command) in c.container_image.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::ContainerBuild(config_command.clone()),
                );
                name_map
                    .entry(name.clone())
                    .or_insert_with(Vec::new)
                    .push(fully_qualified_name.clone());
                name_map
                    .entry(fully_qualified_name.to_string())
                    .or_insert_with(Vec::new)
                    .push(fully_qualified_name);
            }
            for (name, config_command) in c.exec.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::ExecArtifact(config_command.clone()),
                );
                name_map
                    .entry(name.clone())
                    .or_insert_with(Vec::new)
                    .push(fully_qualified_name.clone());
                name_map
                    .entry(fully_qualified_name.to_string())
                    .or_insert_with(Vec::new)
                    .push(fully_qualified_name);
            }
        }
        for (name, command) in commands.iter() {
            if let Some(ref variables) = command.target_info().variables {
                context.variables.insert(name.clone(), (*variables).clone());
            }
        }
        context.resolve_extends(&commands, &name_map)?;
        Ok(context)
    }

    fn resolve_extends(
        &mut self,
        commands: &HashMap<FullyQualifiedName, ConfigWrapper>,
        name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<()> {
        for (name, command) in commands.iter() {
            self.targets.insert(
                name.clone(),
                resolve_extends(name.clone(), command, commands, name_map)?,
            );
        }
        Ok(())
    }

    pub fn resolve_substitutions(
        &self,
        command: &str,
        this_target_name: &FullyQualifiedName,
        outputs: &OutputsManager,
    ) -> Result<String> {
        self.resolve_substitutions_inner(command, this_target_name, outputs, None, &None)
    }

    fn resolve_substitutions_inner(
        &self,
        command: &str,
        this_target_name: &FullyQualifiedName,
        outputs: &OutputsManager,
        args: Option<Vec<String>>,
        default_args: &Option<String>,
    ) -> Result<String> {
        debug!(
            "Resolving variables in <{}> for <{}>",
            command, this_target_name
        );
        let escaped_args_str = if let Some(ref args) = args {
            if args.is_empty() {
                default_args.clone().unwrap_or_default()
            } else {
                let mut escaped_args = vec![];
                for arg in args {
                    escaped_args.push(
                        escape_string(arg)
                            .map_err(|e| anyhow!("While escaping argument <{}>: {}", arg, e))?,
                    );
                }
                escaped_args.join(" ")
            }
        } else {
            "".to_string()
        };
        let mut replaced_args = false;
        let mut resolved = command.to_string();
        let mut index = 0;
        while index < command.len() {
            let res = command[index..].find('{').and_then(|found| {
                command[index + found..].find('}').map(|end_index| {
                    let variable = &command[index + found + 1..index + found + end_index];
                    debug!("Found variable <{}>", variable);
                    index += found + end_index + 1;
                    let replacement = match Variable::from_string(variable)? {
                        Variable::Simple(key) => {
                            if key == "args" {
                                replaced_args = true;
                                Some(&escaped_args_str)
                            } else {
                                self.variables
                                    .get(this_target_name)
                                    .and_then(|variables| variables.get(&key))
                            }
                        }
                        Variable::Global(key) => self.globals.get(&key),
                        Variable::Output(target_name, key) => outputs
                            .get(&FullyQualifiedName::from_string(target_name.as_str()), &key),
                        Variable::Ref(target_name, key) => self
                            .variables
                            .get(&FullyQualifiedName::from_string(target_name.as_str()))
                            .and_then(|variables| variables.get(&key)),
                    };
                    if let Some(replacement) = replacement {
                        let new_resolved =
                            resolved.replace(format!("{{{}}}", variable).as_str(), replacement);
                        if new_resolved != resolved {
                            debug!("Resolved variable <{}> to <{}>", variable, replacement);
                        }
                        resolved = new_resolved;
                        Ok(())
                    } else {
                        Err(anyhow!("Variable <{}> not found", variable))
                    }
                })
            });
            if let Some(res) = res {
                res?;
            } else {
                break;
            }
        }
        if !replaced_args && args.is_some() {
            return Ok(format!("{} {}", resolved, escaped_args_str));
        }
        Ok(resolved)
    }

    pub fn resolve_substitutions_with_args(
        &self,
        command: &str,
        this_target_name: &FullyQualifiedName,
        outputs: &OutputsManager,
        args: Vec<String>,
        default_args: &Option<String>,
    ) -> Result<String> {
        self.resolve_substitutions_inner(
            command,
            this_target_name,
            outputs,
            Some(args),
            default_args,
        )
    }

    pub fn get_target(&self, name: &str) -> CommandLookupResult {
        if name.contains('.') {
            let (tag, name) = name.split_once('.').unwrap();
            let fully_qualified_name = FullyQualifiedName {
                tag: tag.to_string(),
                name: name.to_string(),
            };
            return self
                .targets
                .get(&fully_qualified_name)
                .map(CommandLookupResult::Found)
                .unwrap_or(CommandLookupResult::NotFound);
        } else {
            debug!(
                "Looking up command <{}> in <{:?}>",
                name,
                self.targets
                    .keys()
                    .map(|key| key.to_string())
                    .collect::<Vec<_>>()
            );
            let duplicates = self
                .targets
                .keys()
                .filter(|key| key.name == name)
                .collect::<Vec<_>>();
            if duplicates.len() > 1 {
                return CommandLookupResult::Duplicates(
                    duplicates.iter().map(|key| key.to_string()).collect(),
                );
            }
            if let Some(name) = duplicates.first() {
                self.targets
                    .get(name)
                    .map(CommandLookupResult::Found)
                    .unwrap_or(CommandLookupResult::NotFound)
            } else {
                CommandLookupResult::NotFound
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::command::exec::ExecCommand;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn from_empty_config() {
        let config = Config::default();
        let context = Context::from_config(&config, "test".to_string()).unwrap();
        assert_eq!(context.variables.len(), 0);
    }

    #[test]
    fn uses_globals() {
        let mut config = Config {
            globals: Some(HashMap::new()),
            ..Default::default()
        };
        config
            .globals
            .as_mut()
            .unwrap()
            .insert("key".to_string(), "value".to_string());
        let context = Context::from_config(&config, "test".to_string()).unwrap();
        assert_eq!(context.variables.len(), 0);
        assert_eq!(context.globals.len(), 1);
        assert_eq!(context.globals.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn resolve_substitutions_with_variable() {
        init();
        let mut context = Context::default();
        let outputs = OutputsManager::default();
        context
            .globals
            .insert("key".to_string(), "value".to_string());
        let qualified_name = FullyQualifiedName {
            tag: "command".to_string(),
            name: "foo".to_string(),
        };
        let resolved = context
            .resolve_substitutions("echo {globals.key}", &qualified_name, &outputs)
            .unwrap();
        assert_eq!(resolved, "echo value");
    }

    #[test]
    fn resolve_substitutions_with_output() {
        init();
        let mut context = Context::default();
        let mut outputs = OutputsManager::default();
        let qualified_name = FullyQualifiedName {
            tag: ConfigExecCommand::tag().to_string(),
            name: "foo".to_string(),
        };
        outputs.store_output(qualified_name.clone(), "key", "value");
        let cmd = ExecCommand {
            target_info: TargetInfo {
                name: qualified_name.clone(),
                requires: vec![],
                variables: HashMap::new(),
                description: None,
            },
            command_info: CommandInfo { daemon: false },
            command: "echo {foo.output.key}".to_string(),
            default_args: None,
            env: vec![],
        };
        context
            .targets
            .insert(qualified_name, Target::Command(Command::Exec(cmd)));
        let this_target = FullyQualifiedName {
            tag: ConfigContainerCommand::tag().to_string(),
            name: "bar".to_string(),
        };
        let resolved = context
            .resolve_substitutions("echo {command.exec.foo.output.key}", &this_target, &outputs)
            .unwrap();
        assert_eq!(resolved, "echo value");
    }

    #[test]
    fn resolve_substitutions_with_no_match() {
        let context = Context::default();
        let qualified_name = FullyQualifiedName {
            tag: "command".to_string(),
            name: "foo".to_string(),
        };
        let resolved = context.resolve_substitutions(
            "echo {globals.key}",
            &qualified_name,
            &OutputsManager::default(),
        );
        assert!(resolved.is_err());
        assert_eq!(
            resolved.unwrap_err().to_string(),
            "Variable <globals.key> not found"
        );
    }

    #[test]
    fn resolve_substitutions_for_current_target_name() {
        let mut context = Context::default();
        let qualified_name = FullyQualifiedName {
            tag: "command".to_string(),
            name: "foo".to_string(),
        };
        let test_variables = context
            .variables
            .entry(qualified_name.clone())
            .or_insert(HashMap::new());
        test_variables.insert("key".to_string(), "value".to_string());
        let resolved = context
            .resolve_substitutions("echo {key}", &qualified_name, &OutputsManager::default())
            .unwrap();
        assert_eq!(resolved, "echo value");
    }

    #[test]
    fn resolve_substitutions_with_args_replaces_args() {
        let context = Context::default();
        let qualified_name = FullyQualifiedName {
            tag: "command".to_string(),
            name: "foo".to_string(),
        };
        let resolved = context
            .resolve_substitutions_with_args(
                "echo {args}",
                &qualified_name,
                &OutputsManager::default(),
                vec!["arg".to_string()],
                &None,
            )
            .unwrap();
        assert_eq!(resolved, "echo arg");
    }

    #[test]
    fn resolve_substitutions_with_args_appends_args() {
        let context = Context::default();
        let qualified_name = FullyQualifiedName {
            tag: "command".to_string(),
            name: "foo".to_string(),
        };
        let resolved = context
            .resolve_substitutions_with_args(
                "echo",
                &qualified_name,
                &OutputsManager::default(),
                vec!["arg".to_string()],
                &None,
            )
            .unwrap();
        assert_eq!(resolved, "echo arg");
    }

    #[test]
    fn resolve_substitutions_with_args_escapes_args() {
        let context = Context::default();
        let qualified_name = FullyQualifiedName {
            tag: "command".to_string(),
            name: "foo".to_string(),
        };
        let resolved = context
            .resolve_substitutions_with_args(
                "echo",
                &qualified_name,
                &OutputsManager::default(),
                vec!["$arg".to_string()],
                &None,
            )
            .unwrap();
        assert_eq!(resolved, "echo '$arg'");
    }

    #[test]
    fn resolve_substitutions_with_args_uses_default_args() {
        let context = Context::default();
        let qualified_name = FullyQualifiedName {
            tag: "command".to_string(),
            name: "foo".to_string(),
        };
        let resolved = context
            .resolve_substitutions_with_args(
                "echo",
                &qualified_name,
                &OutputsManager::default(),
                vec![],
                &Some("arg".to_string()),
            )
            .unwrap();
        assert_eq!(resolved, "echo arg");
    }

    #[test]
    fn test_get_lookup_name() {
        let name = get_lookup_name("tag.name".to_string(), "default".to_string());
        assert_eq!(name.tag, "tag");
        assert_eq!(name.name, "name");
        let name = get_lookup_name("name".to_string(), "default".to_string());
        assert_eq!(name.tag, "default");
        assert_eq!(name.name, "name");
    }

    #[test]
    fn test_resolve_requires() {
        let mut name_map = HashMap::new();
        name_map.insert(
            "a".to_string(),
            vec![FullyQualifiedName {
                tag: "tag".to_string(),
                name: "a".to_string(),
            }],
        );
        name_map.insert(
            "b".to_string(),
            vec![FullyQualifiedName {
                tag: "tag".to_string(),
                name: "b".to_string(),
            }],
        );
        let requires = vec!["a".to_string(), "b".to_string()];
        let resolved = resolve_requires(requires.iter(), &name_map).unwrap();
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].name, "a");
        assert_eq!(resolved[1].name, "b");
    }

    #[test]
    fn test_resolve_requires_ambiguous() {
        let mut name_map = HashMap::new();
        name_map.insert(
            "a".to_string(),
            vec![FullyQualifiedName {
                tag: "tag".to_string(),
                name: "a".to_string(),
            }],
        );
        name_map.insert(
            "b".to_string(),
            vec![
                FullyQualifiedName {
                    tag: "tag".to_string(),
                    name: "b".to_string(),
                },
                FullyQualifiedName {
                    tag: "tag".to_string(),
                    name: "b".to_string(),
                },
            ],
        );
        let requires = vec!["a".to_string(), "b".to_string()];
        let resolved = resolve_requires(requires.iter(), &name_map);
        assert!(resolved.is_err());
        assert_eq!(
            resolved.unwrap_err().to_string(),
            "Ambiguous reference <b>, could be <tag.b, tag.b>"
        );
    }

    #[test]
    fn test_resolve_requires_non_existent() {
        let name_map = HashMap::new();
        let requires = vec!["a".to_string(), "b".to_string()];
        let resolved = resolve_requires(requires.iter(), &name_map);
        assert!(resolved.is_err());
        assert_eq!(
            resolved.unwrap_err().to_string(),
            "Non-existent reference <a>"
        );
    }

    #[test]
    fn test_target_info_from_config() {
        let config = ConfigTargetInfo {
            requires: Some(vec!["a".to_string(), "b".to_string()]),
            variables: Some(HashMap::new()),
            extends: None,
            description: Some("description".to_string()),
        };
        let mut name_map = HashMap::new();
        name_map.insert(
            "a".to_string(),
            vec![FullyQualifiedName {
                tag: "tag".to_string(),
                name: "a".to_string(),
            }],
        );
        name_map.insert(
            "b".to_string(),
            vec![FullyQualifiedName {
                tag: "tag".to_string(),
                name: "b".to_string(),
            }],
        );
        let target_info = target_info_from_config(
            FullyQualifiedName {
                tag: "tag".to_string(),
                name: "name".to_string(),
            },
            &config,
            &name_map,
            None,
        )
        .unwrap();
        assert_eq!(target_info.name.name, "name");
        assert_eq!(target_info.requires.len(), 2);
        assert_eq!(target_info.variables.len(), 0);
        assert_eq!(target_info.description, Some("description".to_string()));
    }

    #[test]
    fn test_variable_from_string_simple() {
        let variable = Variable::from_string("foo").unwrap();
        match variable {
            Variable::Simple(s) => assert_eq!(s, "foo"),
            _ => panic!("Expected simple variable"),
        }
    }

    #[test]
    fn test_variable_from_string_global() {
        let variable = Variable::from_string("globals.foo").unwrap();
        match variable {
            Variable::Global(s) => assert_eq!(s, "foo"),
            _ => panic!("Expected global variable"),
        }
    }

    #[test]
    fn test_variable_from_string_output() {
        let variable = Variable::from_string("commands.foo.output.bar").unwrap();
        match variable {
            Variable::Output(target, s) => {
                assert_eq!(target, "commands.foo");
                assert_eq!(s, "bar");
            }
            _ => panic!("Expected output variable"),
        }
    }

    #[test]
    fn test_variable_from_string_output_multi_part() {
        let variable = Variable::from_string("commands.exec.foo.output.bar").unwrap();
        match variable {
            Variable::Output(target, s) => {
                assert_eq!(target, "commands.exec.foo");
                assert_eq!(s, "bar");
            }
            _ => panic!("Expected output variable"),
        }
    }

    #[test]
    fn test_variable_from_string_ref() {
        let variable = Variable::from_string("commands.foo.bar").unwrap();
        match variable {
            Variable::Ref(target, s) => {
                assert_eq!(target, "commands.foo");
                assert_eq!(s, "bar");
            }
            _ => panic!("Expected ref variable"),
        }
    }

    #[test]
    fn test_variable_from_string_ref_multi_part() {
        let variable = Variable::from_string("commands.exec.foo.bar").unwrap();
        match variable {
            Variable::Ref(target, s) => {
                assert_eq!(target, "commands.exec.foo");
                assert_eq!(s, "bar");
            }
            _ => panic!("Expected ref variable"),
        }
    }

    #[test]
    fn test_resolve_target_names_in() {
        let mut name_map = HashMap::new();
        name_map.insert(
            "foo".to_string(),
            vec![FullyQualifiedName {
                tag: "command".to_string(),
                name: "foo".to_string(),
            }],
        );
        let resolved = resolve_target_names_in("{foo.bar}", &name_map).unwrap();
        assert_eq!(resolved, "{command.foo.bar}");
    }

    #[test]
    fn test_resolve_target_names_in_with_output() {
        let mut name_map = HashMap::new();
        name_map.insert(
            "foo".to_string(),
            vec![FullyQualifiedName {
                tag: "commands".to_string(),
                name: "foo".to_string(),
            }],
        );
        let resolved = resolve_target_names_in("{foo.output.bar}", &name_map).unwrap();
        assert_eq!(resolved, "{commands.foo.output.bar}");
    }

    #[test]
    fn test_resolve_target_names_in_with_global() {
        let name_map = HashMap::new();
        let resolved = resolve_target_names_in("{globals.bar}", &name_map).unwrap();
        assert_eq!(resolved, "{globals.bar}");
    }

    #[test]
    fn test_resolve_target_names_in_with_simple() {
        let name_map = HashMap::new();
        let resolved = resolve_target_names_in("{bar}", &name_map).unwrap();
        assert_eq!(resolved, "{bar}");
    }

    // TODO: from_config tests
}
