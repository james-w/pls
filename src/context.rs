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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::{Command, Config};

    #[test]
    fn from_empty_config() {
        let config = Config::default();
        let context = Context::from_config(&config);
        assert_eq!(context.variables.len(), 0);
        assert_eq!(context.outputs.len(), 0);
    }

    #[test]
    fn uses_globals() {
        let mut config = Config::default();
        config.globals = Some(HashMap::new());
        config.globals
            .as_mut()
            .unwrap()
            .insert("key".to_string(), "value".to_string());
        let context = Context::from_config(&config);
        assert_eq!(context.variables.len(), 1);
        let global_variables = context.variables.get("globals").unwrap();
        assert_eq!(global_variables.len(), 1);
        assert_eq!(global_variables.get("key"), Some(&"value".to_string()));
        assert_eq!(context.outputs.len(), 0);
    }

    #[test]
    fn uses_target_variables() {
        let mut config = Config::default();
        let name = "test".to_string();
        let command = Command {
            name: name.clone(),
            command: "echo {{key}}".to_string(),
            variables: Some(HashMap::from([(String::from("key"), String::from("value"))])),
            requires: None,
            daemon: false,
            if_files_changed: None,
            updates_paths: None,
        };
        config.target = Some(vec![command]);
        let context = Context::from_config(&config);
        assert_eq!(context.variables.len(), 1);
        let test_variables = context.variables.get(name.as_str()).unwrap();
        assert_eq!(test_variables.len(), 1);
        assert_eq!(test_variables.get("key"), Some(&"value".to_string()));
        assert_eq!(context.outputs.len(), 0);
    }

    #[test]
    fn store_output() {
        let mut context = Context::default();
        context.store_output("test", "key", "value");
        assert_eq!(context.outputs.len(), 1);
        let test_outputs = context.outputs.get("test").unwrap();
        assert_eq!(test_outputs.len(), 1);
        assert_eq!(test_outputs.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn resolve_substitutions_with_variable() {
        let mut context = Context::default();
        let globals = context.variables.entry("globals".to_string()).or_insert(HashMap::new());
        globals.insert("key".to_string(), "value".to_string());
        let resolved = context.resolve_substitutions("echo {globals.key}", "test");
        assert_eq!(resolved, "echo value");
    }

    #[test]
    fn resolve_substitutions_with_output() {
        let mut context = Context::default();
        context.store_output("test", "key", "value");
        let resolved = context.resolve_substitutions("echo {test.output.key}", "other");
        assert_eq!(resolved, "echo value");
    }

    #[test]
    fn resolve_substitutions_with_no_match() {
        let context = Context::default();
        let resolved = context.resolve_substitutions("echo {globals.key}", "other");
        assert_eq!(resolved, "echo {globals.key}");
    }

    #[test]
    fn resolve_substitutions_for_current_target_name() {
        let mut context = Context::default();
        let test_variables = context.variables.entry("test".to_string()).or_insert(HashMap::new());
        test_variables.insert("key".to_string(), "value".to_string());
        let resolved = context.resolve_substitutions("echo {key}", "test");
        assert_eq!(resolved, "echo value");
    }
}
