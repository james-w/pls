use std::collections::HashMap;

use log::debug;

use crate::config::Config;

#[derive(Debug, Default)]
pub struct Context {
    pub variables: HashMap<String, HashMap<String, String>>,
    pub outputs: HashMap<String, HashMap<String, String>>,
}

impl Context {
    pub fn from_config(config: &Config) -> Context {
        let mut context = Context::default();
        if let Some(ref globals) = config.globals {
            context
                .variables
                .insert("globals".to_string(), globals.clone());
        }
        for target in config.all_targets().iter() {
            if let Some(ref variables) = target.variables() {
                context
                    .variables
                    .insert(target.name().to_string(), (*variables).clone());
            }
        }
        context
    }

    pub fn resolve_substitutions(&self, command: &str, this_target_name: &str) -> String {
        let mut resolved = command.to_string();
        for (target_name, value) in self.variables.iter() {
            for (key, value) in value.iter() {
                let mut new_resolved =
                    resolved.replace(format!("{{{}.{}}}", target_name, key).as_str(), value);
                if target_name == this_target_name {
                    new_resolved = new_resolved.replace(format!("{{{}}}", key).as_str(), value);
                }
                if new_resolved != resolved {
                    debug!("Resolved variable <{}> to <{}>", key, value);
                }
                resolved = new_resolved;
            }
        }
        for (target_name, value) in self.outputs.iter() {
            for (key, value) in value.iter() {
                let new_resolved = if target_name == "" {
                    resolved.replace(format!("{{{}}}", key).as_str(), value)
                } else {
                    resolved.replace(
                        format!("{{{}.output.{}}}", target_name, key).as_str(),
                        value,
                    )
                };
                if new_resolved != resolved {
                    debug!("Resolved variable <{}> to <{}>", key, value);
                }
                resolved = new_resolved;
            }
        }
        resolved
    }

    // TODO: store the outputs for re-runs
    pub fn store_output(&mut self, target_name: &str, key: &str, value: &str) {
        debug!(
            "Setting <{}> output of target <{}> to <{}>",
            key, target_name, value
        );
        let target_outputs = self
            .outputs
            .entry(target_name.to_string())
            .or_insert(HashMap::new());
        target_outputs.insert(key.to_string(), value.to_string());
    }
}
