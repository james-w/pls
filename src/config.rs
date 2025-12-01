use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use log::debug;
use serde::Deserialize;
use validator::Validate;

use crate::config_deserialize;
use crate::context::{
    resolve_target_names_in, resolve_target_names_in_map, resolve_target_names_in_vec,
};
use crate::name::FullyQualifiedName;
use crate::validation_error;

pub use crate::validation_error::SpanMap;

#[derive(Deserialize, Clone, Default, Debug, Validate)]
pub struct Config {
    #[validate(custom(function = "crate::validate::keys_and_values_non_empty_strings"))]
    pub globals: Option<HashMap<String, String>>,

    #[validate(nested)]
    pub command: Option<Command>,
    #[validate(nested)]
    pub artifact: Option<Artifact>,
    #[validate(nested)]
    pub group: Option<HashMap<String, GroupDef>>,
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct Artifact {
    //pub command: Option<HashMap<String, CommandArtifact>>,
    #[validate(nested)]
    pub container_image: Option<HashMap<String, ContainerBuild>>,

    #[validate(nested)]
    pub exec: Option<HashMap<String, ExecArtifact>>,

    #[validate(nested)]
    pub cargo: Option<HashMap<String, CargoArtifact>>,

    #[validate(nested)]
    pub go: Option<HashMap<String, GoArtifact>>,
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct TargetInfo {
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub requires: Option<Vec<String>>,
    #[validate(length(min = 1, message = "Name must not be empty"))]
    pub extends: Option<String>,
    #[validate(custom(function = "crate::validate::keys_non_empty_strings"))]
    pub variables: Option<HashMap<String, String>>,
    pub description: Option<String>,
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
    #[validate(nested)]
    pub cargo: Option<HashMap<String, CargoCommand>>,
    #[validate(nested)]
    pub go: Option<HashMap<String, GoCommand>>,
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct ExecCommand {
    #[validate(length(min = 1, message = "Command must not be empty"))]
    pub command: Option<String>,
    pub default_args: Option<String>,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Option<Vec<String>>,
    #[validate(length(min = 1, message = "dir must not be empty"))]
    pub dir: Option<String>,

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
        new.env = self
            .env
            .as_ref()
            .map(|e| resolve_target_names_in_vec(e, name_map))
            .transpose()?;
        new.dir = self
            .dir
            .as_ref()
            .map(|d| resolve_target_names_in(d, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct CargoCommand {
    pub subcommand: Option<String>,
    pub args: Option<String>,
    pub package: Option<String>,
    pub release: Option<bool>,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub features: Option<Vec<String>>,
    pub all_features: Option<bool>,
    pub no_default_features: Option<bool>,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Option<Vec<String>>,
    #[validate(length(min = 1, message = "dir must not be empty"))]
    pub dir: Option<String>,

    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
    #[validate(nested)]
    pub command_info: CommandInfo,
}

impl CargoCommand {
    pub fn tag() -> &'static str {
        "command.cargo"
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
        new.subcommand = self
            .subcommand
            .as_ref()
            .map(|s| resolve_target_names_in(s, name_map))
            .transpose()?;
        new.args = self
            .args
            .as_ref()
            .map(|a| resolve_target_names_in(a, name_map))
            .transpose()?;
        new.package = self
            .package
            .as_ref()
            .map(|p| resolve_target_names_in(p, name_map))
            .transpose()?;
        new.features = self
            .features
            .as_ref()
            .map(|f| resolve_target_names_in_vec(f, name_map))
            .transpose()?;
        new.env = self
            .env
            .as_ref()
            .map(|e| resolve_target_names_in_vec(e, name_map))
            .transpose()?;
        new.dir = self
            .dir
            .as_ref()
            .map(|d| resolve_target_names_in(d, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct GoCommand {
    pub subcommand: Option<String>,
    pub args: Option<String>,

    // Common flags
    pub verbose: Option<bool>,
    pub output: Option<String>,

    // Build flags
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub tags: Option<Vec<String>>,
    pub ldflags: Option<String>,
    pub race: Option<bool>,
    pub cover: Option<bool>,

    // Test-specific flags
    pub run_pattern: Option<String>,
    pub bench: Option<String>,
    pub timeout: Option<String>,
    pub short: Option<bool>,
    pub count: Option<i32>,

    // Module operations
    pub mod_operation: Option<String>,

    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Option<Vec<String>>,
    #[validate(length(min = 1, message = "dir must not be empty"))]
    pub dir: Option<String>,

    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
    #[validate(nested)]
    pub command_info: CommandInfo,
}

impl GoCommand {
    pub fn tag() -> &'static str {
        "command.go"
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
        new.subcommand = self
            .subcommand
            .as_ref()
            .map(|s| resolve_target_names_in(s, name_map))
            .transpose()?;
        new.args = self
            .args
            .as_ref()
            .map(|a| resolve_target_names_in(a, name_map))
            .transpose()?;
        new.output = self
            .output
            .as_ref()
            .map(|o| resolve_target_names_in(o, name_map))
            .transpose()?;
        new.tags = self
            .tags
            .as_ref()
            .map(|t| resolve_target_names_in_vec(t, name_map))
            .transpose()?;
        new.ldflags = self
            .ldflags
            .as_ref()
            .map(|l| resolve_target_names_in(l, name_map))
            .transpose()?;
        new.run_pattern = self
            .run_pattern
            .as_ref()
            .map(|r| resolve_target_names_in(r, name_map))
            .transpose()?;
        new.bench = self
            .bench
            .as_ref()
            .map(|b| resolve_target_names_in(b, name_map))
            .transpose()?;
        new.timeout = self
            .timeout
            .as_ref()
            .map(|t| resolve_target_names_in(t, name_map))
            .transpose()?;
        new.mod_operation = self
            .mod_operation
            .as_ref()
            .map(|m| resolve_target_names_in(m, name_map))
            .transpose()?;
        new.env = self
            .env
            .as_ref()
            .map(|e| resolve_target_names_in_vec(e, name_map))
            .transpose()?;
        new.dir = self
            .dir
            .as_ref()
            .map(|d| resolve_target_names_in(d, name_map))
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
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Option<Vec<String>>,
    #[validate(length(min = 1, message = "dir must not be empty"))]
    pub dir: Option<String>,

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
        new.env = self
            .env
            .as_ref()
            .map(|e| resolve_target_names_in_vec(e, name_map))
            .transpose()?;
        new.dir = self
            .dir
            .as_ref()
            .map(|d| resolve_target_names_in(d, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct CargoArtifact {
    pub subcommand: Option<String>,
    pub args: Option<String>,
    pub package: Option<String>,
    pub release: Option<bool>,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub features: Option<Vec<String>>,
    pub all_features: Option<bool>,
    pub no_default_features: Option<bool>,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Option<Vec<String>>,
    #[validate(length(min = 1, message = "dir must not be empty"))]
    pub dir: Option<String>,
    pub bin: Option<String>,

    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
    #[validate(nested)]
    pub artifact_info: ArtifactInfo,
}

impl CargoArtifact {
    pub fn tag() -> &'static str {
        "artifact.cargo"
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
        new.subcommand = self
            .subcommand
            .as_ref()
            .map(|s| resolve_target_names_in(s, name_map))
            .transpose()?;
        new.args = self
            .args
            .as_ref()
            .map(|a| resolve_target_names_in(a, name_map))
            .transpose()?;
        new.package = self
            .package
            .as_ref()
            .map(|p| resolve_target_names_in(p, name_map))
            .transpose()?;
        new.features = self
            .features
            .as_ref()
            .map(|f| resolve_target_names_in_vec(f, name_map))
            .transpose()?;
        new.env = self
            .env
            .as_ref()
            .map(|e| resolve_target_names_in_vec(e, name_map))
            .transpose()?;
        new.dir = self
            .dir
            .as_ref()
            .map(|d| resolve_target_names_in(d, name_map))
            .transpose()?;
        new.bin = self
            .bin
            .as_ref()
            .map(|b| resolve_target_names_in(b, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct GoArtifact {
    pub subcommand: Option<String>,
    pub args: Option<String>,

    pub output: Option<String>,
    pub verbose: Option<bool>,
    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub tags: Option<Vec<String>>,
    pub ldflags: Option<String>,
    pub race: Option<bool>,

    pub bin: Option<String>,

    #[validate(custom(function = "crate::validate::non_empty_strings"))]
    pub env: Option<Vec<String>>,
    #[validate(length(min = 1, message = "dir must not be empty"))]
    pub dir: Option<String>,

    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,

    #[serde(flatten)]
    #[validate(nested)]
    pub artifact_info: ArtifactInfo,
}

impl GoArtifact {
    pub fn tag() -> &'static str {
        "artifact.go"
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
        new.subcommand = self
            .subcommand
            .as_ref()
            .map(|s| resolve_target_names_in(s, name_map))
            .transpose()?;
        new.args = self
            .args
            .as_ref()
            .map(|a| resolve_target_names_in(a, name_map))
            .transpose()?;
        new.output = self
            .output
            .as_ref()
            .map(|o| resolve_target_names_in(o, name_map))
            .transpose()?;
        new.tags = self
            .tags
            .as_ref()
            .map(|t| resolve_target_names_in_vec(t, name_map))
            .transpose()?;
        new.ldflags = self
            .ldflags
            .as_ref()
            .map(|l| resolve_target_names_in(l, name_map))
            .transpose()?;
        new.env = self
            .env
            .as_ref()
            .map(|e| resolve_target_names_in_vec(e, name_map))
            .transpose()?;
        new.dir = self
            .dir
            .as_ref()
            .map(|d| resolve_target_names_in(d, name_map))
            .transpose()?;
        new.bin = self
            .bin
            .as_ref()
            .map(|b| resolve_target_names_in(b, name_map))
            .transpose()?;
        Ok(new)
    }
}

#[derive(Deserialize, Clone, Debug, Validate)]
pub struct GroupDef {
    #[serde(flatten)]
    #[validate(nested)]
    pub target_info: TargetInfo,
}

impl GroupDef {
    pub fn tag() -> &'static str {
        "group"
    }

    pub fn type_tag(&self) -> &'static str {
        Self::tag()
    }

    pub fn is_artifact(&self) -> bool {
        false
    }

    pub fn with_resolved_targets(
        &self,
        _name_map: &HashMap<String, Vec<FullyQualifiedName>>,
    ) -> Result<Self> {
        let new = self.clone();
        Ok(new)
    }
}

pub const CONFIG_FILE_NAME: &str = "pls.toml";

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
    pub fn load_and_validate(config_path: &PathBuf) -> Result<(Self, SpanMap)> {
        let config_str = std::fs::read_to_string(config_path)?;
        let (config, span_map) = config_deserialize::parse_with_spans(&config_str)?;
        debug!("Loaded config: {:?}", config);

        // Enhanced validation with span context
        debug!("About to validate config");
        match config.validate() {
            Ok(_) => {
                debug!("Validation passed");
            }
            Err(e) => {
                debug!("Validation failed with errors: {:?}", e);
                return Err(validation_error::format_validation_error(e, &span_map));
            }
        }

        Ok((config, span_map))
    }
}
