use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use log::{debug, info, warn};

use crate::cleanup::CleanupManager;
use crate::commands::build_command;
use crate::config::ContainerBuild as ConfigContainerBuild;
use crate::context::Context;
use crate::default::default_to;
use crate::outputs::OutputsManager;
use crate::target::{ArtifactInfo, Buildable, TargetInfo};

#[derive(Debug, Clone)]
pub struct ContainerArtifact {
    pub context: String,
    pub tag: String,

    pub artifact_info: ArtifactInfo,
    pub target_info: TargetInfo,
}

impl ContainerArtifact {
    pub fn from_config(
        target_info: TargetInfo,
        artifact_info: ArtifactInfo,
        defn: &ConfigContainerBuild,
        base: Option<&Self>,
    ) -> Self {
        Self {
            target_info,
            artifact_info,
            context: default_to!(defn, base, context),
            tag: default_to!(defn, base, tag),
        }
    }
}

impl Buildable for ContainerArtifact {
    fn build(
        &self,
        context: &Context,
        outputs: &mut OutputsManager,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let tag =
            context.resolve_substitutions(self.tag.as_str(), &self.target_info.name, outputs)?;
        let container_context = context.resolve_substitutions(
            self.context.as_str(),
            &self.target_info.name,
            outputs,
        )?;
        let command = format!("podman build -t \"{}\" \"{}\"", tag, container_context);
        debug!(
            "Building container for target <{}> with command <{}>",
            self.target_info.name, command
        );
        info!("[{}] Building tag {}", self.target_info.name, tag);
        let mut cmd = build_command(command.as_str())?;
        cmd.stdout(std::process::Stdio::piped());
        let mut child = cmd.spawn()?;
        let child_stdout = child.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            let stdout_reader = BufReader::new(child_stdout);
            let mut last_line = String::new();
            for line in stdout_reader.lines() {
                match line {
                    Ok(line) => {
                        last_line = line.clone();
                        println!("{}", line);
                    }
                    Err(e) => {
                        warn!("Error reading stdout from build: {}", e);
                    }
                }
            }
            tx.send(last_line).unwrap();
        });
        let status = child.wait()?;
        handle.join().unwrap();
        if !status.success() {
            return Err(anyhow!(
                "Command failed with exit code: {}",
                status.code().unwrap()
            ));
        }
        for line in rx.try_iter() {
            outputs.store_output(self.target_info.name.clone(), "sha", line.as_str());
        }
        Ok(())
    }
}
