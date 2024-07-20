use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Formatter};
use std::path::Path;

use anyhow::{anyhow, Result};
use glob::Pattern;
use log::debug;

use crate::context::Context;
use crate::target::Target;

pub struct WatchTrigger<'a> {
    pub paths: Vec<Pattern>,
    pub target: &'a Target,
    pub and_then: Vec<&'a Target>,
}

impl Debug for WatchTrigger<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchTrigger")
            .field(
                "paths",
                &self.paths.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
            )
            .field("target", &self.target.target_info().name.to_string())
            .field(
                "and_then",
                &self
                    .and_then
                    .iter()
                    .map(|t| t.target_info().name.to_string())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl WatchTrigger<'_> {
    fn get_one(target: &Target) -> Result<WatchTrigger> {
        let paths = if let Ok(artifact) = target.artifact() {
            let artifact_info = artifact.artifact_info();
            // TODO: variables
            artifact_info
                .if_files_changed
                .as_ref()
                .map(|fs| fs.iter().map(|f| Pattern::new(f)).collect())
                .transpose()?
                .unwrap_or_default()
        } else {
            vec![]
        };
        Ok(WatchTrigger {
            paths,
            target,
            and_then: vec![],
        })
    }

    pub fn get_all<'a>(target: &'a Target, context: &'a Context) -> Result<Vec<WatchTrigger<'a>>> {
        let mut triggers = HashMap::new();
        triggers.insert(
            target.target_info().name.clone(),
            WatchTrigger::get_one(target)?,
        );
        let mut to_find = HashSet::new();
        let mut and_then_map = HashMap::new();
        let requires = target.target_info().requires.iter().collect::<Vec<_>>();
        let mut newly_found_targets = vec![];
        for req in requires {
            newly_found_targets.push(req);
            let and_then: &mut Vec<&Target> = and_then_map.entry(req).or_default();
            and_then.push(target);
            debug!("and_then: {:?}", and_then);
        }
        while !newly_found_targets.is_empty() {
            debug!("newly_found_targets: {:?}", newly_found_targets);
            to_find.extend(newly_found_targets.iter());
            newly_found_targets.clear();
            for next in to_find.drain() {
                debug!("candidate: {:?}", next);
                if triggers.contains_key(next) {
                    continue;
                }
                let next_target = context
                    .targets
                    .get(next)
                    .ok_or(anyhow!("Target <{}> not known", next))?;
                triggers.insert((*next).clone(), WatchTrigger::get_one(next_target)?);
                let requires = next_target
                    .target_info()
                    .requires
                    .iter()
                    .collect::<Vec<_>>();
                for req in requires {
                    if !triggers.contains_key(req) {
                        debug!("Adding {} to newly_found_targets: {:?}", req, triggers);
                        newly_found_targets.push(req);
                    }
                    let and_then: &mut Vec<&Target> = and_then_map.entry(req).or_default();
                    and_then.push(next_target);
                    debug!("and_then: {:?}", and_then);
                }
            }
        }
        for (name, and_then) in and_then_map {
            debug!("Setting and_then for {}: {:?}", name, and_then);
            triggers.get_mut(name).unwrap().and_then = and_then;
        }
        Ok(triggers.into_values().collect())
    }

    pub fn matches(&self, test_paths: &[&Path]) -> bool {
        self.paths
            .iter()
            .any(|p| test_paths.iter().any(|t| p.matches_path(t)))
    }

    pub fn find_minimal_watches(watches: &[WatchTrigger]) -> HashSet<String> {
        find_matching_paths(&watches.iter().flat_map(|w| &w.paths).collect::<Vec<_>>())
    }
}

fn find_matching_paths(patterns: &[&Pattern]) -> HashSet<String> {
    let mut result: HashSet<String> = HashSet::new();

    for pattern in patterns {
        let path = pattern.as_str();
        let mut components = path.split('/').collect::<Vec<&str>>();

        // Remove the file pattern part
        while let Some(last) = components.pop() {
            if !last.contains('*') && !last.contains('?') && !last.contains('[') {
                // If the last component doesn't contain any glob characters,
                // it means we're at the directory part we want to keep.
                components.push(last);
                break;
            }
        }

        let parent_dir = if !components.is_empty() {
            components.join("/") + "/"
        } else {
            "./".to_string()
        };

        // Check if the parent directory is already in the result or is more specific
        let mut should_add = true;
        for existing in &result {
            if parent_dir.starts_with(existing) {
                should_add = false;
                break;
            }
        }

        if should_add {
            // Remove any entries that are less specific
            result.retain(|existing| !existing.starts_with(&parent_dir));
            result.insert(parent_dir);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        name::FullyQualifiedName,
        target::{any_artifact_target, any_target, Command, CommandInfo, NullCommand, TargetInfo},
    };

    #[test]
    fn test_debug() {
        let trigger = WatchTrigger {
            paths: vec![Pattern::new("*.rs").unwrap()],
            target: &any_target(),
            and_then: vec![],
        };
        assert_eq!(
            format!("{:?}", trigger),
            "WatchTrigger { paths: [\"*.rs\"], target: \"test.any\", and_then: [] }"
        );
    }

    #[test]
    fn test_watch_trigger_matches() {
        let trigger = WatchTrigger {
            paths: vec![Pattern::new("*.rs").unwrap()],
            target: &any_target(),
            and_then: vec![],
        };
        assert!(trigger.matches(&[Path::new("test.rs")]));
        assert!(!trigger.matches(&[Path::new("test.c")]));
    }

    #[test]
    fn test_get_all() {
        let mut context = Context {
            variables: HashMap::new(),
            targets: HashMap::new(),
            config_path: "<test>".to_string(),
            globals: HashMap::new(),
        };
        let target = any_target();
        context
            .targets
            .insert(target.target_info().name.clone(), target.clone());
        let triggers = WatchTrigger::get_all(&target, &context).unwrap();
        assert_eq!(triggers.len(), 1);
        assert_eq!(
            triggers[0].target.target_info().name,
            target.target_info().name
        );
        assert_eq!(triggers[0].paths.len(), 0,);
    }

    #[test]
    fn test_get_all_with_artifact() {
        let mut context = Context {
            variables: HashMap::new(),
            targets: HashMap::new(),
            config_path: "<test>".to_string(),
            globals: HashMap::new(),
        };
        let target = any_artifact_target();
        context
            .targets
            .insert(target.target_info().name.clone(), target.clone());
        let triggers = WatchTrigger::get_all(&target, &context).unwrap();
        assert_eq!(triggers.len(), 1);
        assert_eq!(
            triggers[0].target.target_info().name,
            target.target_info().name
        );
    }

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn test_get_all_with_dependency() {
        init();
        let mut context = Context {
            variables: HashMap::new(),
            targets: HashMap::new(),
            config_path: "<test>".to_string(),
            globals: HashMap::new(),
        };
        let dependency = any_target();
        context
            .targets
            .insert(dependency.target_info().name.clone(), dependency.clone());
        let target_info = TargetInfo {
            name: FullyQualifiedName {
                tag: "test".to_string(),
                name: "target".to_string(),
            },
            requires: vec![dependency.target_info().name.clone()],
            variables: HashMap::new(),
            description: None,
        };
        let target = Target::Command(Command::Null(NullCommand {
            target_info,
            command_info: CommandInfo { daemon: false },
        }));
        context
            .targets
            .insert(target.target_info().name.clone(), target.clone());
        let mut triggers = WatchTrigger::get_all(&target, &context).unwrap();
        triggers.sort_by(|a, b| {
            a.target
                .target_info()
                .name
                .cmp(&b.target.target_info().name)
        });
        assert_eq!(triggers.len(), 2);
        assert_eq!(
            triggers[0].target.target_info().name,
            dependency.target_info().name
        );
        assert_eq!(triggers[0].and_then.len(), 1);
        assert_eq!(
            triggers[0].and_then[0].target_info().name,
            target.target_info().name
        );
        assert_eq!(
            triggers[1].target.target_info().name,
            target.target_info().name
        );
    }

    fn create_set(vec: Vec<&str>) -> HashSet<String> {
        vec.into_iter().map(String::from).collect()
    }

    #[test]
    fn test_single_pattern() {
        let patterns = vec![Pattern::new("test/*.rs").unwrap()];
        let result = find_matching_paths(&patterns.iter().collect::<Vec<_>>());

        // Expected to return 'test/' as it covers all possible matches for 'test/*.rs'
        let expected = create_set(vec!["test/"]);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_overlapping_patterns() {
        let patterns = vec![
            Pattern::new("test/*.rs").unwrap(),
            Pattern::new("test/sub/*").unwrap(),
        ];
        let result = find_matching_paths(&patterns.iter().collect::<Vec<_>>());

        // Expected to return 'test/' as it covers all possible matches for both patterns
        let expected = create_set(vec!["test/"]);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_duplicate_patterns() {
        let patterns = vec![
            Pattern::new("test/*.rs").unwrap(),
            Pattern::new("test/*.rs").unwrap(),
        ];
        let result = find_matching_paths(&patterns.iter().collect::<Vec<_>>());

        // Expected to return 'test/' as it covers all possible matches for both patterns
        let expected = create_set(vec!["test/"]);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_disjoint_patterns() {
        let patterns = vec![
            Pattern::new("test/*").unwrap(),
            Pattern::new("other/*").unwrap(),
        ];
        let result = find_matching_paths(&patterns.iter().collect::<Vec<_>>());

        // Expected to return 'test/' and 'other/' as they are disjoint and can't overlap
        let expected = create_set(vec!["test/", "other/"]);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_nested_patterns() {
        let patterns = vec![
            Pattern::new("src/**/*.rs").unwrap(),
            Pattern::new("src/lib/**/*.rs").unwrap(),
        ];
        let result = find_matching_paths(&patterns.iter().collect::<Vec<_>>());

        // Expected to return 'src/' as it covers all possible matches for both patterns
        let expected = create_set(vec!["src/"]);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_complex_patterns() {
        let patterns = vec![
            Pattern::new("src/*/*.rs").unwrap(),
            Pattern::new("tests/*/*.rs").unwrap(),
            Pattern::new("docs/*/*.md").unwrap(),
        ];
        let result = find_matching_paths(&patterns.iter().collect::<Vec<_>>());

        // Expected to return 'src/', 'tests/', and 'docs/' as they are the most specific non-overlapping paths
        let expected = create_set(vec!["src/", "tests/", "docs/"]);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_star_covers_non_star() {
        let patterns = vec![
            Pattern::new("src/*/*.rs").unwrap(),
            Pattern::new("src/foo/*.rs").unwrap(),
        ];
        let result = find_matching_paths(&patterns.iter().collect::<Vec<_>>());

        let expected = create_set(vec!["src/"]);

        assert_eq!(result, expected);
    }

    #[test]
    fn test_root_patterns() {
        let patterns = vec![Pattern::new("*/*/*.rs").unwrap()];
        let result = find_matching_paths(&patterns.iter().collect::<Vec<_>>());

        // Expected to return './' as it covers all possible matches
        let expected = create_set(vec!["./"]);

        assert_eq!(result, expected);
    }
}
