use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use log::debug;
use serde::Deserialize;

use crate::context::{
    resolve_target_names_in, resolve_target_names_in_map, resolve_target_names_in_vec,
};
use crate::name::FullyQualifiedName;

#[derive(Deserialize, Clone, Default, Debug)]
pub struct Config {
    pub globals: Option<HashMap<String, String>>,

    pub command: Option<Command>,
    pub artifact: Option<Artifact>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Artifact {
    //pub command: Option<HashMap<String, CommandArtifact>>,
    pub container_image: Option<HashMap<String, ContainerBuild>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct TargetInfo {
    pub requires: Option<Vec<String>>,
    pub extends: Option<String>,
    pub variables: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CommandInfo {
    pub daemon: Option<bool>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ArtifactInfo {
    pub updates_paths: Option<Vec<String>>,
    pub if_files_changed: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Command {
    pub exec: Option<HashMap<String, ExecCommand>>,
    pub container: Option<HashMap<String, ContainerCommand>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ExecCommand {
    pub command: Option<String>,
    pub default_args: Option<String>,

    #[serde(flatten)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
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
}

#[derive(Deserialize, Clone, Debug)]
pub struct ContainerCommand {
    pub image: Option<String>,
    pub env: Option<Vec<String>>,
    pub command: Option<String>,
    pub mount: Option<HashMap<String, String>>,
    pub workdir: Option<String>,
    pub network: Option<String>,
    pub create_network: Option<bool>,
    pub default_args: Option<String>,

    #[serde(flatten)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
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

#[derive(Deserialize, Clone, Debug)]
pub struct ContainerBuild {
    pub context: Option<String>,
    pub tag: Option<String>,

    #[serde(flatten)]
    pub artifact_info: ArtifactInfo,

    #[serde(flatten)]
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
    pub fn validate(&self) -> Result<()> {
        Ok(())
    }

    pub fn load_and_validate(config_path: &PathBuf) -> Result<Self> {
        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(config_str.as_str())?;
        debug!("Loaded config: {:?}", config);
        config.validate()?;
        Ok(config)
    }
}
