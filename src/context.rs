use std::collections::HashMap;

use anyhow::{anyhow, Result};
use log::debug;
use validator::Validate;

use crate::{
    config::{
        ArtifactInfo as ConfigArtifactInfo, CargoArtifact as ConfigCargoArtifact,
        CargoCommand as ConfigCargoCommand, CommandInfo as ConfigCommandInfo, Config,
        ContainerBuild as ConfigContainerBuild, ContainerCommand as ConfigContainerCommand,
        ExecArtifact as ConfigExecArtifact, ExecCommand as ConfigExecCommand,
        GoArtifact as ConfigGoArtifact, GoCommand as ConfigGoCommand, GroupDef as ConfigGroupDef,
        TargetInfo as ConfigTargetInfo,
    },
    default::default_to,
    name::FullyQualifiedName,
    outputs::OutputsManager,
    shell::escape_string,
    similarity::levenshtein_distance,
    target::{Artifact, ArtifactInfo, Command, CommandInfo, Target, TargetInfo},
    targets::{
        CargoArtifact, CargoCommand, ContainerArtifact, ContainerCommand, ExecArtifact,
        ExecCommand, GoArtifact, GoCommand, Group,
    },
    validation_error,
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

#[derive(Debug)]
pub struct Context {
    pub variables: HashMap<FullyQualifiedName, HashMap<String, String>>,
    pub globals: HashMap<String, String>,

    pub targets: HashMap<FullyQualifiedName, Target>,

    pub config_path: String,
    pub project_root: std::path::PathBuf,

    pub span_map: Option<crate::config::SpanMap>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            variables: HashMap::new(),
            globals: HashMap::new(),
            targets: HashMap::new(),
            config_path: String::new(),
            project_root: std::path::PathBuf::new(),
            span_map: None,
        }
    }
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

    for (i, c) in input.char_indices() {
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

    if let Some(ref config_vars) = config.variables {
        // 1. Resolve platform-specific overrides into HashMap<String, String>
        let resolved = config_vars.resolve()?;
        // 2. Then resolve target name references like {foo.bar} -> {command.exec.foo.bar}
        let other_variables = resolve_variables(resolved.iter(), name_map)
            .map_err(|e| anyhow!("Invalid reference from <{}>: {}", name, e))?;
        // 3. Merge into variables (child overrides parent)
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
    span_map: Option<&crate::config::SpanMap>,
) -> Result<Target> {
    let base = if let Some(extends) = command.extends() {
        let extends_fully_qualified =
            get_lookup_name(extends.clone(), command.type_tag().to_string());
        let base = commands.get(&extends_fully_qualified);
        if let Some(base) = base {
            resolve_extends(extends_fully_qualified, base, commands, name_map, span_map).map(Some)
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

    // Handle groups separately as they are neither artifacts nor commands
    if matches!(command, ConfigWrapper::Group(_)) {
        let group = Group { target_info };
        return Ok(Target::Group(group));
    }

    if command.is_artifact() {
        let artifact_info = artifact_info_from_config(
            name.clone(),
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
                if let Some(span_map) = span_map {
                    artifact.validate().map_err(|e| {
                        validation_error::format_runtime_validation_error(e, span_map, &name)
                    })?;
                } else {
                    artifact.validate()?;
                }
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
                if let Some(span_map) = span_map {
                    artifact.validate().map_err(|e| {
                        validation_error::format_runtime_validation_error(e, span_map, &name)
                    })?;
                } else {
                    artifact.validate()?;
                }
                Ok(Target::Artifact(Artifact::Exec(artifact)))
            }
            ConfigWrapper::CargoArtifact(command) => {
                let base = base
                    .as_ref()
                    .map::<Result<_>, _>(|b| b.artifact()?.cargo())
                    .transpose()?;
                let artifact = CargoArtifact::from_config(
                    target_info,
                    artifact_info,
                    &command.with_resolved_targets(name_map)?,
                    base,
                );
                if let Some(span_map) = span_map {
                    artifact.validate().map_err(|e| {
                        validation_error::format_runtime_validation_error(e, span_map, &name)
                    })?;
                } else {
                    artifact.validate()?;
                }
                Ok(Target::Artifact(Artifact::Cargo(artifact)))
            }
            ConfigWrapper::GoArtifact(command) => {
                let base = base
                    .as_ref()
                    .map::<Result<_>, _>(|b| b.artifact()?.go())
                    .transpose()?;
                let artifact = GoArtifact::from_config(
                    target_info,
                    artifact_info,
                    &command.with_resolved_targets(name_map)?,
                    base,
                );
                if let Some(span_map) = span_map {
                    artifact.validate().map_err(|e| {
                        validation_error::format_runtime_validation_error(e, span_map, &name)
                    })?;
                } else {
                    artifact.validate()?;
                }
                Ok(Target::Artifact(Artifact::Go(artifact)))
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
                if let Some(span_map) = span_map {
                    exec.validate().map_err(|e| {
                        validation_error::format_runtime_validation_error(e, span_map, &name)
                    })?;
                } else {
                    exec.validate()?;
                }
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
                if let Some(span_map) = span_map {
                    container.validate().map_err(|e| {
                        validation_error::format_runtime_validation_error(e, span_map, &name)
                    })?;
                } else {
                    container
                        .validate()
                        .map_err(|e| anyhow!("Error validating <{}>: {}", name, e))?;
                }
                Ok(Target::Command(Command::Container(container)))
            }
            ConfigWrapper::Cargo(command) => {
                let base = base
                    .as_ref()
                    .map::<Result<_>, _>(|b| b.command()?.cargo())
                    .transpose()?;
                let cargo = CargoCommand::from_config(
                    target_info,
                    command_info,
                    &command.with_resolved_targets(name_map)?,
                    base,
                );
                if let Some(span_map) = span_map {
                    cargo.validate().map_err(|e| {
                        validation_error::format_runtime_validation_error(e, span_map, &name)
                    })?;
                } else {
                    cargo.validate()?;
                }
                Ok(Target::Command(Command::Cargo(cargo)))
            }
            ConfigWrapper::Go(command) => {
                let base = base
                    .as_ref()
                    .map::<Result<_>, _>(|b| b.command()?.go())
                    .transpose()?;
                let go = GoCommand::from_config(
                    target_info,
                    command_info,
                    &command.with_resolved_targets(name_map)?,
                    base,
                );
                if let Some(span_map) = span_map {
                    go.validate().map_err(|e| {
                        validation_error::format_runtime_validation_error(e, span_map, &name)
                    })?;
                } else {
                    go.validate()?;
                }
                Ok(Target::Command(Command::Go(go)))
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
    Cargo(ConfigCargoCommand),
    CargoArtifact(ConfigCargoArtifact),
    Go(ConfigGoCommand),
    GoArtifact(ConfigGoArtifact),
    Exec(ConfigExecCommand),
    Container(ConfigContainerCommand),
    ContainerBuild(ConfigContainerBuild),
    ExecArtifact(ConfigExecArtifact),
    Group(ConfigGroupDef),
}

impl ConfigWrapper {
    fn target_info(&self) -> &ConfigTargetInfo {
        match self {
            Self::Cargo(command) => &command.target_info,
            Self::CargoArtifact(command) => &command.target_info,
            Self::Go(command) => &command.target_info,
            Self::GoArtifact(command) => &command.target_info,
            Self::Exec(command) => &command.target_info,
            Self::Container(command) => &command.target_info,
            Self::ContainerBuild(command) => &command.target_info,
            Self::ExecArtifact(command) => &command.target_info,
            Self::Group(group) => &group.target_info,
        }
    }

    fn command_info(&self) -> Option<&ConfigCommandInfo> {
        match self {
            Self::Cargo(command) => Some(&command.command_info),
            Self::CargoArtifact(_) => None,
            Self::Go(command) => Some(&command.command_info),
            Self::GoArtifact(_) => None,
            Self::Exec(command) => Some(&command.command_info),
            Self::Container(command) => Some(&command.command_info),
            Self::ContainerBuild(_) => None,
            Self::ExecArtifact(_) => None,
            Self::Group(_) => None,
        }
    }

    fn artifact_info(&self) -> Option<&ConfigArtifactInfo> {
        match self {
            Self::Cargo(_) => None,
            Self::CargoArtifact(command) => Some(&command.artifact_info),
            Self::Go(_) => None,
            Self::GoArtifact(command) => Some(&command.artifact_info),
            Self::Exec(_) => None,
            Self::Container(_) => None,
            Self::ContainerBuild(command) => Some(&command.artifact_info),
            Self::ExecArtifact(command) => Some(&command.artifact_info),
            Self::Group(_) => None,
        }
    }

    fn extends(&self) -> Option<String> {
        self.target_info().extends.clone()
    }

    fn type_tag(&self) -> &'static str {
        match self {
            Self::Cargo(c) => c.type_tag(),
            Self::CargoArtifact(c) => c.type_tag(),
            Self::Go(c) => c.type_tag(),
            Self::GoArtifact(c) => c.type_tag(),
            Self::Exec(c) => c.type_tag(),
            Self::Container(c) => c.type_tag(),
            Self::ContainerBuild(c) => c.type_tag(),
            Self::ExecArtifact(c) => c.type_tag(),
            Self::Group(g) => g.type_tag(),
        }
    }

    fn is_artifact(&self) -> bool {
        match self {
            Self::Cargo(c) => c.is_artifact(),
            Self::CargoArtifact(c) => c.is_artifact(),
            Self::Go(c) => c.is_artifact(),
            Self::GoArtifact(c) => c.is_artifact(),
            Self::Exec(c) => c.is_artifact(),
            Self::Container(c) => c.is_artifact(),
            Self::ContainerBuild(c) => c.is_artifact(),
            Self::ExecArtifact(c) => c.is_artifact(),
            Self::Group(g) => g.is_artifact(),
        }
    }
}

impl Context {
    pub fn from_config(
        config: &Config,
        path: String,
        span_map: Option<crate::config::SpanMap>,
    ) -> Result<Context> {
        let project_root = std::path::PathBuf::from(&path)
            .parent()
            .ok_or_else(|| anyhow!("Config path has no parent directory"))?
            .to_path_buf();
        let mut context = Context {
            config_path: path,
            project_root,
            span_map,
            ..Default::default()
        };
        if let Some(ref globals) = config.globals {
            context.globals = globals.resolve()?;
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
            for (name, config_command) in c.cargo.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::Cargo(config_command.clone()),
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
            for (name, config_command) in c.go.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::Go(config_command.clone()),
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
            for (name, config_command) in c.cargo.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::CargoArtifact(config_command.clone()),
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
            for (name, config_command) in c.go.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::GoArtifact(config_command.clone()),
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
        if let Some(ref groups) = config.group {
            for (name, config_group) in groups.iter() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_group.type_tag().to_string(),
                    name: name.clone(),
                };
                commands.insert(
                    fully_qualified_name.clone(),
                    ConfigWrapper::Group(config_group.clone()),
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
                context.variables.insert(name.clone(), variables.resolve()?);
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
                resolve_extends(
                    name.clone(),
                    command,
                    commands,
                    name_map,
                    self.span_map.as_ref(),
                )?,
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

    /// Resolve a directory path, making it absolute relative to project root if it's relative
    pub fn resolve_dir(
        &self,
        dir: &str,
        this_target_name: &FullyQualifiedName,
        outputs: &OutputsManager,
    ) -> Result<std::path::PathBuf> {
        let resolved = self.resolve_substitutions(dir, this_target_name, outputs)?;
        let path = std::path::Path::new(&resolved);
        Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
        })
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

    pub fn get_target(&self, name: &str) -> CommandLookupResult<'_> {
        if name.contains('.') {
            let fully_qualified_name = FullyQualifiedName::from_string(name);
            self.targets
                .get(&fully_qualified_name)
                .map(CommandLookupResult::Found)
                .unwrap_or(CommandLookupResult::NotFound)
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

    /// Get suggestions for similar target names based on multiple criteria
    /// Returns up to 5 suggestions, prioritized by match quality
    pub fn get_suggestions(&self, name: &str) -> Vec<String> {
        let input_len = name.len();
        let input_lower = name.to_lowercase();

        #[derive(Debug, PartialEq, Eq)]
        enum MatchType {
            Substring,    // Priority 0: substring/superstring match
            EditDistance, // Priority 1: close edit distance
            Description,  // Priority 2: matches word in description
        }

        let mut candidates: Vec<(String, MatchType, usize)> = self
            .targets
            .iter()
            .filter_map(|(key, target)| {
                let full_name = key.to_string();
                let short_name = &key.name;
                let full_name_lower = full_name.to_lowercase();
                let short_name_lower = short_name.to_lowercase();

                // Check for substring matches (high priority)
                if full_name_lower.contains(&input_lower) || input_lower.contains(&full_name_lower)
                {
                    return Some((full_name.clone(), MatchType::Substring, 0));
                }
                if short_name_lower.contains(&input_lower)
                    || input_lower.contains(&short_name_lower)
                {
                    return Some((full_name.clone(), MatchType::Substring, 0));
                }

                // Check edit distance (medium priority)
                let full_distance = levenshtein_distance(name, &full_name);
                let short_distance = levenshtein_distance(name, short_name);
                let distance = std::cmp::min(full_distance, short_distance);

                let max_distance = if input_len <= 3 {
                    1 // Very short strings: only 1 char difference
                } else if input_len <= 6 {
                    2 // Medium strings: up to 2 chars difference
                } else {
                    3 // Longer strings: up to 3 chars difference
                };

                if distance <= max_distance {
                    return Some((full_name.clone(), MatchType::EditDistance, distance));
                }

                // Check description matches (lower priority)
                if let Some(desc) = &target.target_info().description {
                    let desc_lower = desc.to_lowercase();
                    // Check if input matches any word in the description
                    if desc_lower.split_whitespace().any(|word| {
                        word.trim_matches(|c: char| !c.is_alphanumeric())
                            .contains(&input_lower)
                    }) {
                        return Some((full_name.clone(), MatchType::Description, 0));
                    }
                }

                None
            })
            .collect();

        // Sort by: match type (priority), then distance
        candidates.sort_by(|a, b| {
            let type_order = |t: &MatchType| match t {
                MatchType::Substring => 0,
                MatchType::EditDistance => 1,
                MatchType::Description => 2,
            };

            type_order(&a.1).cmp(&type_order(&b.1)).then(a.2.cmp(&b.2))
        });

        // Remove duplicates and return up to 5 suggestions
        let mut seen = std::collections::HashSet::new();
        candidates
            .iter()
            .filter(|(name, _, _)| seen.insert(name.clone()))
            .take(5)
            .map(|(name, _, _)| name.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::command::exec::ExecCommand;

    #[test]
    fn from_empty_config() {
        let config = Config::default();
        let context = Context::from_config(&config, "test".to_string(), None).unwrap();
        assert_eq!(context.variables.len(), 0);
    }

    #[test]
    fn uses_globals() {
        let mut config = Config {
            globals: Some(crate::config::Variables {
                values: HashMap::new(),
                platform: None,
            }),
            ..Default::default()
        };
        config
            .globals
            .as_mut()
            .unwrap()
            .values
            .insert("key".to_string(), "value".to_string());
        let context = Context::from_config(&config, "test".to_string(), None).unwrap();
        assert_eq!(context.variables.len(), 0);
        assert_eq!(context.globals.len(), 1);
        assert_eq!(context.globals.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn resolve_substitutions_with_variable() {
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
            dir: None,
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
        let requires = ["a".to_string(), "b".to_string()];
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
        let requires = ["a".to_string(), "b".to_string()];
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
        let requires = ["a".to_string(), "b".to_string()];
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
            variables: Some(crate::config::Variables {
                values: HashMap::new(),
                platform: None,
            }),
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
