use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use log::debug;
use serde::Deserialize;
use validator::Validate;

use crate::context::{
    resolve_target_names_in, resolve_target_names_in_map, resolve_target_names_in_vec,
};
use crate::name::FullyQualifiedName;

#[derive(Deserialize, Clone, Default, Debug, Validate)]
pub struct Config {
    #[validate(custom(function = "crate::validate::keys_and_values_non_empty_strings"))]
    pub globals: Option<HashMap<String, String>>,

    #[validate(nested)]
    pub command: Option<Command>,
    #[validate(nested)]
    pub artifact: Option<Artifact>,
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct Artifact {
    //pub command: Option<HashMap<String, CommandArtifact>>,
    #[validate(nested)]
    pub container_image: Option<HashMap<String, ContainerBuild>>,

    #[validate(nested)]
    pub exec: Option<HashMap<String, ExecArtifact>>,
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct TargetInfo {
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub requires: Option<Vec<String>>,
    #[validate(length(min = 1, message = "Name must not be empty"))]
    pub extends: Option<String>,
    #[validate(custom(function = "crate::validate::keys_non_empty_strings"))]
    pub variables: Option<HashMap<String, String>>,
}

impl TargetInfo {
    pub fn with_resolved_targets(
        &self,
        name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<Self> {
        let mut new = self.clone();
        new.variables = self
            .variables
            .as_ref()
            .map(|i| resolve_target_names_in_map(i, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct CommandInfo {
    pub daemon: Option<bool>,
}

impl CommandInfo {
    pub fn with_resolved_targets(
        &self,
        _name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<Self> {
        let new = self.clone();
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct ArtifactInfo {
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub updates_paths: Option<Vec<String>>,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub if_files_changed: Option<Vec<String>>,
}

impl ArtifactInfo {
    pub fn with_resolved_targets(
        &self,
        name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<Self> {
        let mut new = self.clone();
        new.updates_paths = self
            .updates_paths
            .as_ref()
            .map(|i| resolve_target_names_in_vec(i, name_map))
            .transpose()?;
        new.if_files_changed = self
            .if_files_changed
            .as_ref()
            .map(|i| resolve_target_names_in_vec(i, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct Command {
    #[validate(nested)]
    pub exec: Option<HashMap<String, ExecCommand>>,
    #[validate(nested)]
    pub container: Option<HashMap<String, ContainerCommand>>,
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct ExecCommand {
    #[validate(length(min = 1, message = "Command must not be empty"))]
    pub command: Option<String>,
    pub default_args: Option<String>,

    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
    #[validate(nested)]
    pub command_info: CommandInfo,
}

impl ExecCommand {
    pub fn tag() -> &'static str {
        "command.exec"
    }

    pub fn type_tag(&self) -> &'static str {
        Self::tag()
    }

    pub fn is_artifact(&self) -> bool {
        false
    }

    pub fn with_resolved_targets(
        &self,
        name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<Self> {
        let mut new = self.clone();
        new.command = self
            .command
            .as_ref()
            .map(|i| resolve_target_names_in(i, name_map))
            .transpose()?;
        new.default_args = self
            .default_args
            .as_ref()
            .map(|e| resolve_target_names_in(e, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct ContainerCommand {
    #[validate(length(min = 1, message = "image must not be empty"))]
    pub image: Option<String>,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Option<Vec<String>>,
    #[validate(length(min = 1, message = "command must not be empty"))]
    pub command: Option<String>,
    #[validate(custom(function = "crate::validate::keys_and_values_non_empty_strings"))]
    pub mount: Option<HashMap<String, String>>,
    #[validate(length(min = 1, message = "workdir must not be empty"))]
    pub workdir: Option<String>,
    #[validate(length(min = 1, message = "network must not be empty"))]
    pub network: Option<String>,
    pub create_network: Option<bool>,
    pub default_args: Option<String>,

    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
    #[validate(nested)]
    pub command_info: CommandInfo,
}

impl ContainerCommand {
    pub fn tag() -> &'static str {
        "command.container"
    }

    pub fn type_tag(&self) -> &'static str {
        Self::tag()
    }

    pub fn is_artifact(&self) -> bool {
        false
    }

    pub fn with_resolved_targets(
        &self,
        name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<Self> {
        let mut new = self.clone();
        new.image = self
            .image
            .as_ref()
            .map(|i| resolve_target_names_in(i, name_map))
            .transpose()?;
        new.env = self
            .env
            .as_ref()
            .map(|e| resolve_target_names_in_vec(e, name_map))
            .transpose()?;
        new.command = self
            .command
            .as_ref()
            .map(|c| resolve_target_names_in(c, name_map))
            .transpose()?;
        new.mount = self
            .mount
            .as_ref()
            .map(|m| resolve_target_names_in_map(m, name_map))
            .transpose()?;
        new.workdir = self
            .workdir
            .as_ref()
            .map(|w| resolve_target_names_in(w, name_map))
            .transpose()?;
        new.network = self
            .network
            .as_ref()
            .map(|n| resolve_target_names_in(n, name_map))
            .transpose()?;
        new.create_network = self.create_network;
        new.default_args = self
            .default_args
            .as_ref()
            .map(|d| resolve_target_names_in(d, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct ContainerBuild {
    #[validate(length(min = 1, message = "context must not be empty"))]
    pub context: Option<String>,
    #[validate(length(min = 1, message = "tag must not be empty"))]
    pub tag: Option<String>,

    #[serde(flatten)]
    #[validate(nested)]
    pub artifact_info: ArtifactInfo,

    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,
}

impl ContainerBuild {
    pub fn tag() -> &'static str {
        "artifact.container_image"
    }

    pub fn type_tag(&self) -> &'static str {
        Self::tag()
    }

    pub fn is_artifact(&self) -> bool {
        true
    }

    pub fn with_resolved_targets(
        &self,
        name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<Self> {
        let mut new = self.clone();
        new.context = self
            .context
            .as_ref()
            .map(|i| resolve_target_names_in(i, name_map))
            .transpose()?;
        new.tag = self
            .tag
            .as_ref()
            .map(|i| resolve_target_names_in(i, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct ExecArtifact {
    #[validate(length(min = 1, message = "Command must not be empty"))]
    pub command: Option<String>,

    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
    #[validate(nested)]
    pub artifact_info: ArtifactInfo,
}

impl ExecArtifact {
    pub fn tag() -> &'static str {
        "artifact.exec"
    }

    pub fn type_tag(&self) -> &'static str {
        Self::tag()
    }

    pub fn is_artifact(&self) -> bool {
        true
    }

    pub fn with_resolved_targets(
        &self,
        name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<Self> {
        let mut new = self.clone();
        new.command = self
            .command
            .as_ref()
            .map(|i| resolve_target_names_in(i, name_map))
            .transpose()?;
        Ok(new)
    }
}

pub const CONFIG_FILE_NAME: &str = "taskrunner.toml";

pub fn find_config_file() -> Option<std::path::PathBuf> {
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

impl Config {
    pub fn load_and_validate(config_path: &PathBuf) -> Result<Self> {
        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(config_str.as_str())?;
        debug!("Loaded config: {:?}", config);
        config.validate()?;
        Ok(config)
    }
}
