use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use log::debug;
use serde::Deserialize;

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
pub struct Command {
    pub exec: Option<HashMap<String, ExecCommand>>,
    pub container: Option<HashMap<String, ContainerCommand>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ExecCommand {
    pub command: Option<String>,
    pub requires: Option<Vec<String>>,
    pub daemon: Option<bool>,
    pub extends: Option<String>,
    pub variables: Option<HashMap<String, String>>,
    pub default_args: Option<String>,
}

impl ExecCommand {
    pub fn tag() -> &'static str {
        "command.exec"
    }

    pub fn type_tag(&self) -> &'static str {
        Self::tag()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct ContainerCommand {
    pub image: Option<String>,
    pub daemon: Option<bool>,
    pub env: Option<Vec<String>>,
    pub requires: Option<Vec<String>>,
    pub variables: Option<HashMap<String, String>>,
    pub command: Option<String>,
    pub mount: Option<HashMap<String, String>>,
    pub workdir: Option<String>,
    pub network: Option<String>,
    pub create_network: Option<bool>,
    pub extends: Option<String>,
    pub default_args: Option<String>,
}

impl ContainerCommand {
    pub fn tag() -> &'static str {
        "command.container"
    }

    pub fn type_tag(&self) -> &'static str {
        Self::tag()
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct ContainerBuild {
    pub context: Option<String>,
    pub tag: Option<String>,
    pub variables: Option<HashMap<String, String>>,
    pub requires: Option<Vec<String>>,
    pub if_files_changed: Option<Vec<String>>,
    pub extends: Option<String>,
}

impl ContainerBuild {
    pub fn tag() -> &'static str {
        "artifact.container_image"
    }

    pub fn type_tag(&self) -> &'static str {
        Self::tag()
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
        config.validate()?;
        Ok(config)
    }
}
