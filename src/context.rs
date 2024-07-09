use std::collections::HashMap;
use std::fmt;

use anyhow::{anyhow, Result};
use log::debug;

use crate::{
    config::{
        Config, ContainerCommand as ConfigContainerCommand, ExecCommand as ConfigExecCommand, ContainerBuild as ConfigContainerBuild,
    },
    shell::escape_string,
};

#[derive(Debug, Default)]
pub struct Context {
    pub variables: HashMap<FullyQualifiedName, HashMap<String, String>>,
    pub globals: HashMap<String, String>,

    pub targets: HashMap<FullyQualifiedName, Target>,

    pub config_path: String,
}

#[derive(Debug, Default)]
pub struct OutputsManager {
    outputs: HashMap<FullyQualifiedName, HashMap<String, String>>,
}

impl OutputsManager {
    // TODO: store the outputs for re-runs
    pub fn store_output(&mut self, target_name: FullyQualifiedName, key: &str, value: &str) {
        debug!(
            "Setting <{}> output of target <{}> to <{}>",
            key, target_name, value
        );
        let target_outputs = self.outputs.entry(target_name).or_insert(HashMap::new());
        target_outputs.insert(key.to_string(), value.to_string());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FullyQualifiedName {
    pub tag: String,
    pub name: String,
}

impl fmt::Display for FullyQualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}", self.tag, self.name)
    }
}

#[derive(Debug, Clone)]
pub struct ExecCommand {
    pub name: FullyQualifiedName,
    pub command: String,
    pub daemon: bool,
    pub requires: Vec<String>,
    pub variables: HashMap<String, String>,
    pub default_args: Option<String>,
}

fn default_to<'a, T: Default + Clone, U>(prefer: &Option<T>, base: Option<U>, f: fn (U) -> &'a T) -> T {
    prefer.clone().or(base.map(f).map(|t| t.clone())).unwrap_or_default()
}

fn default_to_with_default<T: Default + Clone, U>(prefer: Option<T>, base: Option<U>, f: fn (U) -> T, default: T) -> T {
    prefer.clone().or(base.map(f).map(|t| t.clone())).unwrap_or(default)
}

fn default_optional<'a, T: Clone, U>(prefer: &Option<T>, base: Option<U>, f: fn (U) -> &'a Option<T>) -> Option<T> {
    prefer.clone().or(base.map(f).map(|b| b.clone()).flatten())
}

macro_rules! default_to {
    ($prefer:expr, $base:expr, $f:ident) => {
        default_to(&$prefer.$f, $base, |b| &b.$f)
    };
    ($prefer:expr, $base:expr, $f:ident, $def:expr) => {
        default_to_with_default($prefer.$f, $base, |b| b.$f, $def)
    };
}

macro_rules! default_optional {
    ($prefer:expr, $base:expr, $f:ident) => {
        default_optional(&$prefer.$f, $base, |b| &b.$f)
    };
}

impl ExecCommand {
    pub fn from_config(name: FullyQualifiedName, defn: &ConfigExecCommand, base: Option<&ExecCommand>) -> Self {
        ExecCommand {
            name,
            command: default_to!(defn, base, command),
            daemon: default_to!(defn, base, daemon, false),
            requires: default_to!(defn, base, requires),
            variables: default_to!(defn, base, variables),
            default_args: default_optional!(defn, base, default_args),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContainerCommand {
    pub name: FullyQualifiedName,
    pub image: String,
    pub daemon: bool,
    pub env: Vec<String>,
    pub requires: Vec<String>,
    pub variables: HashMap<String, String>,
    pub command: Option<String>,
    pub mount: HashMap<String, String>,
    pub workdir: Option<String>,
    pub network: Option<String>,
    pub create_network: bool,
    pub default_args: Option<String>,
}

impl ContainerCommand {
    pub fn from_config(
        name: FullyQualifiedName,
        defn: &ConfigContainerCommand,
        base: Option<&ContainerCommand>,
    ) -> Self {
        let mut env = vec![];
        if let Some(base) = base {
            env.extend(base.env.clone());
        }
        env.extend(defn.env.clone().unwrap_or_default());
        ContainerCommand {
            name,
            image: default_to!(defn, base, image),
            daemon: default_to!(defn, base, daemon, false),
            env,
            requires: default_to!(defn, base, requires),
            variables: default_to!(defn, base, variables),
            command: default_optional!(defn, base, command),
            mount: default_to!(defn, base, mount),
            workdir: default_optional!(defn, base, workdir),
            network: default_optional!(defn, base, network),
            create_network: default_to!(defn, base, create_network, false),
            default_args: default_optional!(defn, base, default_args),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContainerBuild {
    pub name: FullyQualifiedName,
    pub context: String,
    pub tag: String,
    pub requires: Vec<String>,
    pub variables: HashMap<String, String>,
    pub if_files_changed: Option<Vec<String>>,
}

impl ContainerBuild {
    pub fn from_config(
        name: FullyQualifiedName,
        defn: &ConfigContainerBuild,
        base: Option<&ContainerBuild>,
    ) -> Self {
        ContainerBuild {
            name,
            context: default_to!(defn, base, context),
            tag: default_to!(defn, base, tag),
            requires: default_to!(defn, base, requires),
            variables: default_to!(defn, base, variables),
            if_files_changed: default_optional!(defn, base, if_files_changed),
        }
    }
}

fn resolve_extends(
    name: FullyQualifiedName,
    command: &ConfigWrapper,
    commands: &HashMap<FullyQualifiedName, ConfigWrapper>,
) -> Result<Target> {
    let base = if let Some(extends) = command.extends() {
        let extends_fully_qualified = if extends.contains(".") {
            let (tag, name) = extends.split_once(".").unwrap();
            FullyQualifiedName {
                tag: tag.to_string(),
                name: name.to_string(),
            }
        } else {
            FullyQualifiedName {
                tag: command.type_tag().to_string(),
                name: extends.clone(),
            }
        };
        let base = commands.get(&extends_fully_qualified);
        if let Some(base) = base {
            resolve_extends(extends_fully_qualified, base, commands).map(Some)
        } else {
            Err(anyhow!(
                "<{}> extends non-existent <{}>",
                name, extends
            ))
        }
    } else {
        Ok(None)
    };
    base.map(|base| command.to_context(name, base.as_ref()))
}

#[derive(Debug, Clone)]
pub enum Target {
    Exec(ExecCommand),
    Container(ContainerCommand),
    ContainerBuild(ContainerBuild),
}

impl Target {
    pub fn fully_qualified_name(&self) -> &FullyQualifiedName {
        match self {
            Self::Exec(command) => &command.name,
            Self::Container(command) => &command.name,
            Self::ContainerBuild(command) => &command.name,
        }
    }

    pub fn exec(&self) -> &ExecCommand {
        match self {
            Self::Exec(command) => command,
            Self::Container(_) => panic!("Expected exec command, got container command"),
            Self::ContainerBuild(_) => panic!("Expected exec command, got container build"),
        }
    }

    pub fn container(&self) -> &ContainerCommand {
        match self {
            Self::Exec(_) => panic!("Expected container command, got exec command"),
            Self::Container(command) => command,
            Self::ContainerBuild(_) => panic!("Expected exec command, got container build"),
        }
    }

    pub fn container_build(&self) -> &ContainerBuild {
        match self {
            Self::Exec(_) => panic!("Expected container build, got exec command"),
            Self::Container(_) => panic!("Expected container build, got container command"),
            Self::ContainerBuild(command) => command,
        }
    }

    pub fn name(&self) -> &str {
        self.fully_qualified_name().name.as_str()
    }

    pub fn daemon(&self) -> bool {
        match self {
            Self::Exec(command) => command.daemon,
            Self::Container(command) => command.daemon,
            Self::ContainerBuild(_) => false,
        }
    }

    pub fn requires(&self) -> Vec<String> {
        match self {
            Self::Exec(command) => command.requires.clone(),
            Self::Container(command) => command.requires.clone(),
            Self::ContainerBuild(command) => command.requires.clone(),
        }
    }

    pub fn updates_paths(&self) -> Option<Vec<String>> {
        match self {
            Self::Exec(_) => None,
            Self::Container(_) => None,
            Self::ContainerBuild(_) => None,
        }
    }

    pub fn if_files_changed(&self) -> Option<Vec<String>> {
        match self {
            Self::Exec(_) => None,
            Self::Container(_) => None,
            Self::ContainerBuild(command) => command.if_files_changed.clone(),
        }
    }
}

pub enum CommandLookupResult {
    NotFound,
    Found(Target),
    Duplicates(Vec<String>),
}

enum ConfigWrapper {
    Exec(ConfigExecCommand),
    Container(ConfigContainerCommand),
    ContainerBuild(ConfigContainerBuild),
}

impl ConfigWrapper {
    fn extends(&self) -> Option<String> {
        match self {
            ConfigWrapper::Exec(command) => command.extends.clone(),
            ConfigWrapper::Container(command) => command.extends.clone(),
            ConfigWrapper::ContainerBuild(command) => command.extends.clone(),
        }
    }

    fn type_tag(&self) -> &'static str {
        match self {
            ConfigWrapper::Exec(c) => c.type_tag(),
            ConfigWrapper::Container(c) => c.type_tag(),
            ConfigWrapper::ContainerBuild(c) => c.type_tag(),
        }
    }

    fn to_context(&self, name: FullyQualifiedName, base: Option<&Target>) -> Target {
        match self {
            ConfigWrapper::Exec(command) => Target::Exec(ExecCommand::from_config(name, command, base.map(|b| b.exec()))),
            ConfigWrapper::Container(command) => Target::Container(ContainerCommand::from_config(name, command, base.map(|b| b.container()))),
            ConfigWrapper::ContainerBuild(command) => Target::ContainerBuild(ContainerBuild::from_config(name, command, base.map(|b| b.container_build()))),
        }
    }
}

impl Context {
    pub fn from_config(config: &Config, path: String) -> Result<Context> {
        let mut context = Context::default();
        context.config_path = path;
        if let Some(ref globals) = config.globals {
            context.globals = globals.clone();
        }
        let mut commands = HashMap::new();
        if let Some(ref c) = config.command {
            for (name, config_command) in c.exec.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                if let Some(ref variables) = config_command.variables {
                    context.variables.insert(
                        fully_qualified_name.clone(),
                        (*variables).clone(),
                    );
                }
                commands.insert(fully_qualified_name, ConfigWrapper::Exec(config_command.clone()));
            }
            for (name, config_command) in c.container.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                if let Some(ref variables) = config_command.variables {
                    context.variables.insert(
                        fully_qualified_name.clone(),
                        (*variables).clone(),
                    );
                }
                commands.insert(fully_qualified_name, ConfigWrapper::Container(config_command.clone()));
            }
        }
        if let Some(ref c) = config.artifact {
            for (name, config_command) in c.container_image.iter().flatten() {
                let fully_qualified_name = FullyQualifiedName {
                    tag: config_command.type_tag().to_string(),
                    name: name.clone(),
                };
                if let Some(ref variables) = config_command.variables {
                    context.variables.insert(
                        fully_qualified_name.clone(),
                        (*variables).clone(),
                    );
                }
                commands.insert(fully_qualified_name, ConfigWrapper::ContainerBuild(config_command.clone()));
            }
        }
        context.resolve_extends(&commands)?;
        Ok(context)
    }

    fn resolve_extends(&mut self, commands: &HashMap<FullyQualifiedName, ConfigWrapper>) -> Result<()> {
        for (name, command) in commands.iter() {
            self.targets.insert(name.clone(), resolve_extends(name.clone(), command, commands)?);
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
        debug!("Resolving variables in <{}> for <{}>", command, this_target_name);
        debug!("{:?}", self.variables);
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
            let res = command[index..].find("{").and_then(|found| {
                command[index + found..].find("}").map(|end_index| {
                    let variable = &command[index + found + 1..index + found + end_index];
                    debug!("Found variable <{}>", variable);
                    index += found + end_index + 1;
                    let replacement = if variable.contains(".") {
                        let (target_name, variable) = variable.split_once(".").unwrap();
                        debug!("Resolving variable <{}> for <{}>", variable, target_name);
                        if target_name == "globals" {
                            self.globals.get(variable)
                        } else {
                            match self.get_command(target_name) {
                                CommandLookupResult::Found(cmd) => {
                                    if variable.starts_with("output.") {
                                        let variable = &variable["output.".len()..];
                                        outputs.outputs.get(&cmd.fully_qualified_name()).and_then(|outputs| {
                                            outputs.get(variable)
                                        })
                                    } else {
                                        self.variables.get(&cmd.fully_qualified_name()).and_then(|variables| {
                                            variables.get(variable)
                                        })
                                    }
                                },
                                CommandLookupResult::NotFound => None,
                                CommandLookupResult::Duplicates(duplicates) => {
                                    return Err(anyhow!("While resolving substution <{{{}.{}}}>: command <{}> is ambiguous, could be <{}>", target_name, variable, target_name, duplicates.join(", ")));
                                }
                            }
                        }
                    } else {
                        if variable == "args" {
                            replaced_args = true;
                            Some(&escaped_args_str)
                        } else {
                            self.variables.get(this_target_name).and_then(|variables| {
                                variables.get(variable)
                            })
                        }
                    };
                    if let Some(replacement) = replacement {
                        let new_resolved = resolved.replace(format!("{{{}}}", variable).as_str(), replacement);
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
        self.resolve_substitutions_inner(command, this_target_name, outputs, Some(args), default_args)
    }

    pub fn get_command(&self, name: &str) -> CommandLookupResult {
        if name.contains(".") {
            let (tag, name) = name.split_once(".").unwrap();
            let fully_qualified_name = FullyQualifiedName {
                tag: tag.to_string(),
                name: name.to_string(),
            };
            return self.targets.get(&fully_qualified_name).map(|target| {
                CommandLookupResult::Found(target.clone())
            }).unwrap_or(CommandLookupResult::NotFound);
        } else {
            debug!("Looking up command <{}> in <{:?}>", name, self.targets.keys().map(|key| key.to_string()).collect::<Vec<_>>());
            let duplicates = self.targets.keys().filter(|key| key.name == name).collect::<Vec<_>>();
            if duplicates.len() > 1 {
                return CommandLookupResult::Duplicates(duplicates.iter().map(|key| key.to_string()).collect());
            }
            if let Some(name) = duplicates.first() {
                self.targets.get(name).map(|target| {
                    CommandLookupResult::Found(target.clone())
                }).unwrap_or(CommandLookupResult::NotFound)
            } else {
                CommandLookupResult::NotFound
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let mut config = Config::default();
        config.globals = Some(HashMap::new());
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
    fn store_output() {
        let mut outputs = OutputsManager::default();
        let target_name = FullyQualifiedName {
            tag: "tag".to_string(),
            name: "test".to_string(),
        };
        outputs.store_output(target_name.clone(), "key", "value");
        assert_eq!(outputs.outputs.len(), 1);
        let test_outputs = outputs.outputs.get(&target_name).unwrap();
        assert_eq!(test_outputs.len(), 1);
        assert_eq!(test_outputs.get("key"), Some(&"value".to_string()));
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
        init();
        let mut context = Context::default();
        let mut outputs = OutputsManager::default();
        let qualified_name = FullyQualifiedName {
            tag: ConfigExecCommand::tag().to_string(),
            name: "foo".to_string(),
        };
        outputs.store_output(qualified_name.clone(), "key", "value");
        let cmd = ExecCommand {
            name: qualified_name.clone(),
            command: "echo {foo.output.key}".to_string(),
            daemon: false,
            requires: vec![],
            variables: HashMap::new(),
            default_args: None,
        };
        context.targets.insert(qualified_name, Target::Exec(cmd));
        let this_target = FullyQualifiedName {
            tag: ConfigContainerCommand::tag().to_string(),
            name: "bar".to_string(),
        };
        let resolved = context
            .resolve_substitutions("echo {foo.output.key}", &this_target, &outputs)
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
        assert_eq!(resolved.unwrap_err().to_string(), "Variable <globals.key> not found");
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

    // TODO: from_config tests
}
