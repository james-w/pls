use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Result};
use clap::Parser;

use crate::cleanup::CleanupManager;
use crate::cmd::execute::Execute;
use crate::context::{CommandLookupResult, Context};
use crate::target::create_metadata_dir;

#[derive(Parser, Debug)]
pub struct LogsCommand {
    /// The name of the target to view logs for
    pub name: String,

    /// Follow log output (like tail -f)
    #[arg(short, long)]
    pub follow: bool,

    /// Number of lines to show from the end of the log
    #[arg(short = 'n', long, default_value = "10")]
    pub lines: usize,
}

impl Execute for LogsCommand {
    fn execute(
        &self,
        context: Context,
        _cleanup_manager: Arc<Mutex<CleanupManager>>,
    ) -> Result<()> {
        match context.get_target(self.name.as_str()) {
            CommandLookupResult::Found(target) => {
                // Check if target is a daemon
                if let Some(cmd_info) = target.command_info() {
                    if !cmd_info.daemon {
                        return Err(anyhow!(
                            "Target <{}> is not a daemon. Only daemons produce logs.",
                            self.name
                        ));
                    }
                } else {
                    return Err(anyhow!(
                        "Target <{}> is not a command. Only daemon commands produce logs.",
                        self.name
                    ));
                }

                // Get log file path
                let config_dir =
                    create_metadata_dir(target.target_info().name.to_string().as_str())?;
                let log_path = config_dir.join("log");

                if !log_path.exists() {
                    return Err(anyhow!(
                        "No log file found for target <{}>. Has it been started?",
                        self.name
                    ));
                }

                if self.follow {
                    // Follow mode - continuously tail the file
                    follow_log_file(&log_path, self.lines)?;
                } else {
                    // Static mode - show last N lines
                    let lines = read_last_n_lines(&log_path, self.lines)?;
                    for line in lines {
                        println!("{}", line);
                    }
                }

                Ok(())
            }
            CommandLookupResult::NotFound => {
                let suggestions = context.get_suggestions(&self.name);
                let mut error_msg = format!(
                    "Target <{}> not found in config file <{}>",
                    self.name, context.config_path
                );
                if !suggestions.is_empty() {
                    error_msg.push_str(&format!(
                        "\n\nDid you mean one of these?\n  {}",
                        suggestions.join("\n  ")
                    ));
                }
                Err(anyhow!(error_msg))
            }
            CommandLookupResult::Duplicates(ref mut duplicates) => {
                duplicates.sort();
                Err(anyhow!(
                    "Target <{}> is ambiguous, possible values are <{}>",
                    self.name,
                    duplicates.join(", ")
                ))
            }
        }
    }
}

/// Read the last N lines from a file
fn read_last_n_lines(path: &std::path::Path, n: usize) -> Result<Vec<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Read all lines and keep only the last N
    let all_lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let start = if all_lines.len() > n {
        all_lines.len() - n
    } else {
        0
    };

    Ok(all_lines[start..].to_vec())
}

/// Follow a log file (like tail -f)
fn follow_log_file(path: &std::path::Path, initial_lines: usize) -> Result<()> {
    // First, print the last N lines
    let lines = read_last_n_lines(path, initial_lines)?;
    for line in lines {
        println!("{}", line);
    }

    // Then continuously monitor for new content
    let mut file = File::open(path)?;
    file.seek(SeekFrom::End(0))?;
    let mut reader = BufReader::new(file);

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new data, sleep and try again
                thread::sleep(Duration::from_millis(100));
            }
            Ok(_) => {
                // New data available, print it
                print!("{}", line);
            }
            Err(e) => {
                return Err(anyhow!("Error reading log file: {}", e));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_read_last_n_lines_with_fewer_lines() {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("pls_test_{}", std::process::id()));

        {
            let mut file = File::create(&temp_path).unwrap();
            writeln!(file, "line1").unwrap();
            writeln!(file, "line2").unwrap();
            writeln!(file, "line3").unwrap();
            file.flush().unwrap();
        }

        let lines = read_last_n_lines(&temp_path, 5).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "line2");
        assert_eq!(lines[2], "line3");

        std::fs::remove_file(&temp_path).unwrap();
    }

    #[test]
    fn test_read_last_n_lines_with_more_lines() {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("pls_test_more_{}", std::process::id()));

        {
            let mut file = File::create(&temp_path).unwrap();
            writeln!(file, "line1").unwrap();
            writeln!(file, "line2").unwrap();
            writeln!(file, "line3").unwrap();
            writeln!(file, "line4").unwrap();
            writeln!(file, "line5").unwrap();
            file.flush().unwrap();
        }

        let lines = read_last_n_lines(&temp_path, 2).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "line4");
        assert_eq!(lines[1], "line5");

        std::fs::remove_file(&temp_path).unwrap();
    }

    #[test]
    fn test_read_last_n_lines_empty_file() {
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("pls_test_empty_{}", std::process::id()));

        File::create(&temp_path).unwrap();

        let lines = read_last_n_lines(&temp_path, 10).unwrap();
        assert_eq!(lines.len(), 0);

        std::fs::remove_file(&temp_path).unwrap();
    }
}
