use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::colors;
use crate::context::Context;

#[derive(Parser, Debug)]
pub struct ListCommand {
    /// Search term to filter targets by name or description
    pub search: Option<String>,
}

impl Execute for ListCommand {
    fn execute(
        &self,
        context: Context,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        let mut targets = context.targets.iter().collect::<Vec<_>>();
        targets.sort_by(|a, b| a.0.cmp(b.0));

        // Filter by search term if provided
        if let Some(search_term) = &self.search {
            let search_lower = search_term.to_lowercase();
            targets.retain(|(name, target)| {
                let name_matches = name.to_string().to_lowercase().contains(&search_lower);
                let desc_matches = target
                    .target_info()
                    .description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&search_lower))
                    .unwrap_or(false);
                name_matches || desc_matches
            });
        }

        for (name, target) in targets {
            println!(
                "{} - {}",
                colors::success_msg(&name.to_string()),
                colors::grey_msg(&target.target_info().description.clone().unwrap_or_default())
            );
        }
        Ok(())
    }
}
