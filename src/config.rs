use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use log::debug;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    // Rename to command?
    pub target: Option<Vec<Command>>,
    pub container_build: Option<Vec<ContainerBuild>>,
    pub container: Option<Vec<Container>>,
    pub globals: Option<HashMap<String, String>>,
}

fn default_false() -> bool {
    false
}

#[derive(Deserialize, Clone, Debug)]
pub struct Command {
    pub name: String,
    pub command: String,
    pub variables: Option<HashMap<String, String>>,
    pub requires: Option<Vec<String>>,
    #[serde(default = "default_false")]
    pub daemon: bool,
    pub updates_paths: Option<Vec<String>>,
    pub if_files_changed: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ContainerBuild {
    pub name: String,
    pub context: String,
    pub tag: String,
    pub variables: Option<HashMap<String, String>>,
    pub requires: Option<Vec<String>>,
    pub if_files_changed: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Container {
    pub name: String,
    pub image: String,
    #[serde(default = "default_false")]
    pub daemon: bool,
    pub env: Option<Vec<String>>,
    pub requires: Option<Vec<String>>,
    pub if_files_changed: Option<Vec<String>>,
    pub updates_paths: Option<Vec<String>>,
    pub variables: Option<HashMap<String, String>>,
    pub command: Option<String>,
    pub mount: Option<HashMap<String, String>>,
    pub workdir: Option<String>,
    pub network: Option<String>,
    #[serde(default = "default_false")]
    pub create_network: bool,
}

#[derive(Debug, Clone)]
pub enum Target<'a> {
    Command(&'a Command),
    ContainerBuild(&'a ContainerBuild),
    Container(&'a Container),
}

impl<'a> Into<Target<'a>> for &'a Command {
    fn into(self) -> Target<'a> {
        Target::Command(self)
    }
}

impl<'a> Target<'a> {
    pub fn name(&self) -> &str {
        match self {
            Target::Command(c) => c.name.as_str(),
            Target::ContainerBuild(c) => c.name.as_str(),
            Target::Container(c) => c.name.as_str(),
        }
    }

    pub fn requires(&self) -> Option<&Vec<String>> {
        match self {
            Target::Command(c) => c.requires.as_ref(),
            Target::ContainerBuild(c) => c.requires.as_ref(),
            Target::Container(c) => c.requires.as_ref(),
        }
    }

    pub fn if_files_changed(&self) -> Option<&Vec<String>> {
        match self {
            Target::Command(c) => c.if_files_changed.as_ref(),
            Target::ContainerBuild(c) => c.if_files_changed.as_ref(),
            Target::Container(c) => c.if_files_changed.as_ref(),
        }
    }

    pub fn updates_paths(&self) -> Option<&Vec<String>> {
        match self {
            Target::Command(c) => c.updates_paths.as_ref(),
            Target::ContainerBuild(_) => None,
            Target::Container(c) => c.updates_paths.as_ref(),
        }
    }

    pub fn daemon(&self) -> bool {
        match self {
            Target::Command(c) => c.daemon,
            Target::ContainerBuild(_) => false,
            Target::Container(c) => c.daemon,
        }
    }

    pub fn variables(&self) -> Option<&HashMap<String, String>> {
        match self {
            Target::Command(c) => c.variables.as_ref(),
            Target::ContainerBuild(c) => c.variables.as_ref(),
            Target::Container(c) => c.variables.as_ref(),
        }
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
    pub fn all_targets(&self) -> Vec<Target> {
        let mut targets = vec![];
        if let Some(ref target) = self.target {
            targets.extend(target.iter().map(|t| t.into()));
        }
        if let Some(ref container_build) = self.container_build {
            targets.extend(container_build.iter().map(|t| Target::ContainerBuild(t)));
        }
        if let Some(ref container) = self.container {
            targets.extend(container.iter().map(|t| Target::Container(t)));
        }
        targets
    }

    pub fn find_target(&self, name: &str) -> Option<Target> {
        self.all_targets().into_iter().find(|t| match t {
            Target::Command(c) => c.name == name,
            Target::ContainerBuild(c) => c.name == name,
            Target::Container(c) => c.name == name,
        })
    }

    pub fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut uniq = HashSet::new();
        let targets = self.all_targets();
        let dupes = targets.iter().filter(|x| !uniq.insert(x.name()));
        let dupe_names = dupes.map(|x| x.name()).collect::<HashSet<_>>();
        if !dupe_names.is_empty() {
            return Err(Box::from(format!(
                "Duplicate target names found in config file: {}",
                dupe_names.into_iter().collect::<Vec<_>>().join(", ")
            )));
        }
        // TODO: validations for targets, e.g. command isn't empty string
        Ok(())
    }

    pub fn load_and_validate(config_path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(config_str.as_str())?;
        config.validate()?;
        Ok(config)
    }
}
