use std::collections::HashMap;

use log::debug;

use crate::name::FullyQualifiedName;

#[derive(Debug, Default)]
pub struct OutputsManager {
    outputs: HashMap<FullyQualifiedName, HashMap<String, String>>,
}

impl OutputsManager {
    pub fn store_output(&mut self, target_name: FullyQualifiedName, key: &str, value: &str) {
        debug!(
            "Setting <{}> output of target <{}> to <{}>",
            key, target_name, value
        );
        let target_outputs = self.outputs.entry(target_name).or_insert(HashMap::new());
        target_outputs.insert(key.to_string(), value.to_string());
    }

    pub fn get_all(&self, target_name: &FullyQualifiedName) -> Option<&HashMap<String, String>> {
        self.outputs.get(target_name)
    }

    pub fn get(&self, target_name: &FullyQualifiedName, key: &str) -> Option<&String> {
        self.outputs
            .get(target_name)
            .and_then(|outputs| outputs.get(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
