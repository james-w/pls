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
            context.variables.insert("globals".to_string(), globals.clone());
        }
        if let Some(ref targets) = config.target {
            for target in targets.iter() {
                if let Some(ref variables) = target.variables {
                    context.variables.insert(target.name.clone(), variables.clone());
                }
            }
        }
        context
    }

    pub fn resolve_substitutions(&self, command: &str, this_target_name: &str) -> String {
        let mut resolved = command.to_string();
        for (target_name, value) in self.variables.iter() {
            for (key, value) in value.iter() {
                let mut new_resolved = resolved.replace(format!("{{{}.{}}}", target_name, key).as_str(), value);
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
                    resolved.replace(format!("{{{}.output.{}}}", target_name, key).as_str(), value)
                };
                if new_resolved != resolved {
                    debug!("Resolved variable <{}> to <{}>", key, value);
                }
                resolved = new_resolved;
            }
        }
        resolved
    }
}
