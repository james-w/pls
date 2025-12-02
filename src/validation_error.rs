//! Validation error formatting with source code context.
//!
//! This module provides rich error formatting for validation errors by tracking the source
//! locations of fields in the TOML config file. When validation fails, we can show users
//! exactly where the problem is with line numbers, source snippets, and helpful examples.
//!
//! ## How it works
//!
//! 1. During config parsing (`config_deserialize`), we use `toml-span` to track byte offsets
//!    for every field and target in the TOML file
//! 2. We convert byte offsets to line/column coordinates and store them in `SpanMap`
//! 3. When validation errors occur (from the `validator` crate), we look up the source location
//!    and format a helpful error message with context
//!
//! ## Error formatting
//!
//! Errors show:
//! - The fully qualified target name (e.g., "command.exec.test")
//! - The specific field path (e.g., "command.exec.test.command")
//! - Source snippet with line numbers and caret highlighting
//! - Helpful examples for common mistakes
//!
//! ## Validation phases
//!
//! There are two validation phases:
//! - **Config-time**: Validates the raw TOML structure (uses `format_validation_error`)
//! - **Runtime**: Validates after extends resolution when fields are resolved (uses `format_runtime_validation_error`)

use log::debug;
use std::collections::HashMap;
use validator::ValidationErrors;

use crate::name::FullyQualifiedName;

/// Represents a span in the source TOML file.
///
/// Coordinates are 0-indexed internally (line 0 = first line, column 0 = first character).
/// When displaying to users, we convert to 1-indexed (line 1 = first line).
#[derive(Debug, Clone)]
pub struct Span {
    /// 0-indexed line number where the span starts
    pub line: usize,
    /// 0-indexed column number where the span starts
    pub column: usize,
    /// 0-indexed line number where the span ends
    pub end_line: usize,
    /// 0-indexed column number where the span ends
    pub end_column: usize,
}

/// Maps field paths to their source locations
#[derive(Debug, Default)]
pub struct SpanMap {
    fields: HashMap<String, Span>,
    targets: HashMap<FullyQualifiedName, Span>,
    // Maps "command.exec" → Vec<"test", "another", ...> to resolve indices
    target_order: HashMap<String, Vec<String>>,
    source: String,
}

impl SpanMap {
    pub fn new(source: String) -> Self {
        Self {
            fields: HashMap::new(),
            targets: HashMap::new(),
            target_order: HashMap::new(),
            source,
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn insert_field(&mut self, path: String, span: Span) {
        self.fields.insert(path, span);
    }

    pub fn insert_target(&mut self, table_path: String, name: String, span: Span) {
        let fqn = FullyQualifiedName {
            tag: table_path.clone(),
            name: name.clone(),
        };
        self.targets.insert(fqn, span);
        self.target_order.entry(table_path).or_default().push(name);
    }

    pub fn get(&self, field_path: &str) -> Option<&Span> {
        self.fields.get(field_path)
    }

    pub fn get_target(&self, fqn_str: &str) -> Option<&Span> {
        let fqn = FullyQualifiedName::from_string(fqn_str);
        self.targets.get(&fqn)
    }

    /// Resolve a validator path like "command.exec[0].command" to use target names
    pub fn resolve_path(&self, validator_path: &str) -> String {
        // Look for patterns like command.exec[0] and replace with actual target name
        let mut result = validator_path.to_string();

        // Find all [index] patterns
        if let Some(bracket_start) = result.find('[') {
            if let Some(bracket_end) = result.find(']') {
                // Extract the table path and index
                let table_path = &result[..bracket_start];
                let index_str = &result[bracket_start + 1..bracket_end];

                if let Ok(index) = index_str.parse::<usize>() {
                    // Look up the target name
                    if let Some(names) = self.target_order.get(table_path) {
                        if let Some(name) = names.get(index) {
                            // Replace [index] with .name
                            result =
                                format!("{}.{}{}", table_path, name, &result[bracket_end + 1..]);
                        }
                    }
                }
            }
        }

        result
    }
}

/// Extract fully qualified target name from a field path
/// "command.exec.test.command" → "command.exec.test"
/// "artifact.cargo.build.dir" → "artifact.cargo.build"
pub fn extract_target_fqn(field_path: &str) -> String {
    // After path resolution, paths look like: "command.exec.test.command"
    // We want to extract "command.exec.test" (everything except the last component)

    // Try bracket notation first (for unresolved paths like "command.exec[0].command")
    if let Some(bracket_start) = field_path.find('[') {
        if let Some(bracket_end) = field_path.find(']') {
            let target_name = &field_path[bracket_start + 1..bracket_end];
            let prefix = &field_path[..bracket_start];
            return format!("{}.{}", prefix, target_name);
        }
    }

    // For resolved paths like "command.exec.test.command", remove the last component
    if let Some(last_dot) = field_path.rfind('.') {
        return field_path[..last_dot].to_string();
    }

    // Fallback: return the whole path if we can't parse
    field_path.to_string()
}

/// Format source snippet with line numbers and caret highlighting
fn format_source_snippet(source: &str, span: &Span) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut output = String::new();

    // Show 1 line before and 1 line after for context (if available)
    let start_line = span.line.saturating_sub(1);
    let end_line = (span.end_line + 2).min(lines.len());

    // Calculate the width needed for line numbers (for proper caret alignment)
    let max_line_num = end_line;
    let line_num_width = max_line_num.to_string().len();

    for line_num in start_line..end_line {
        if line_num >= lines.len() {
            break;
        }

        let line = lines[line_num];
        // Add 1 to line_num because editor line numbers are 1-indexed
        output.push_str(&format!(
            "    {:width$} | {}\n",
            line_num + 1,
            line,
            width = line_num_width
        ));

        // Add caret highlighting for the error line
        if line_num == span.line {
            let caret_offset = span.column;
            let caret_len = if span.line == span.end_line {
                span.end_column.saturating_sub(span.column).max(1)
            } else {
                line.len().saturating_sub(caret_offset).max(1)
            };

            // Indent: 4 spaces + line_num_width + " | " (3 chars)
            // Note: This assumes single-byte characters. Multi-byte UTF-8 characters
            // (emoji, wide chars) may cause slight misalignment, but this is acceptable
            // since TOML configs rarely use such characters in meaningful locations.
            let indent = 4 + line_num_width + 3;
            output.push_str(&" ".repeat(indent));
            output.push_str(&" ".repeat(caret_offset));
            output.push_str(&"^".repeat(caret_len));
            output.push('\n');
        }
    }

    output
}

/// Get a helpful message based on the error
fn get_help_message(field_path: &str, error_msg: &str) -> String {
    // Check for empty/validation errors
    if error_msg.contains("empty") || error_msg.contains("validation failed") {
        if field_path.ends_with(".command") {
            return "  The 'command' field cannot be an empty string.\n  Example: command = \"npm run dev\"".to_string();
        } else if field_path.ends_with(".image") {
            return "  The 'image' field cannot be an empty string.\n  Example: image = \"ubuntu:22.04\"".to_string();
        } else if field_path.ends_with(".dir") {
            return "  The 'dir' field cannot be an empty string.\n  Example: dir = \"./src\""
                .to_string();
        } else if field_path.ends_with(".env") {
            return "  Environment variable entries cannot be empty strings.\n  Example: env = [\"FOO=bar\", \"DEBUG=true\"]".to_string();
        }
    }
    String::new()
}

/// Recursively collect all field errors from nested ValidationErrors
fn collect_field_errors(
    errors: &ValidationErrors,
    prefix: String,
    collected: &mut Vec<(String, Vec<String>)>,
) {
    use validator::ValidationErrorsKind;

    for (field, error_kind) in errors.errors() {
        let field_path = if prefix.is_empty() {
            field.to_string()
        } else {
            format!("{}.{}", prefix, field)
        };

        match error_kind {
            ValidationErrorsKind::Struct(nested_errors) => {
                collect_field_errors(nested_errors, field_path, collected);
            }
            ValidationErrorsKind::List(list_errors) => {
                for (index, nested_errors) in list_errors {
                    let list_path = format!("{}[{}]", field_path, index);
                    collect_field_errors(nested_errors, list_path, collected);
                }
            }
            ValidationErrorsKind::Field(field_errors) => {
                let messages: Vec<String> = field_errors
                    .iter()
                    .filter_map(|e| e.message.as_ref().map(|m| m.to_string()))
                    .collect();
                collected.push((field_path, messages));
            }
        }
    }
}

/// Format validation errors with span information
pub fn format_validation_error(errors: ValidationErrors, span_map: &SpanMap) -> anyhow::Error {
    use log::debug;

    let mut output = Vec::new();

    // Collect all field errors recursively
    let mut collected_errors = Vec::new();
    collect_field_errors(&errors, String::new(), &mut collected_errors);

    debug!("Collected {} field errors", collected_errors.len());
    for (path, msgs) in &collected_errors {
        debug!("  Field: {}, Messages: {:?}", path, msgs);
    }

    let source = span_map.source();

    for (field_path, error_messages) in collected_errors {
        // Resolve validator's indexed path to use target names
        let resolved_path = span_map.resolve_path(&field_path);
        let target_fqn = extract_target_fqn(&resolved_path);

        let error_msg = if error_messages.is_empty() {
            "validation failed".to_string()
        } else {
            error_messages.join(", ")
        };

        if let Some(span) = span_map.get(&resolved_path) {
            debug!("  Found span for {}: {:?}", resolved_path, span);
            // Format with line number and source snippet
            let mut msg = format!(
                "Error in target '{}':\n  {}: {}\n\n",
                target_fqn, resolved_path, error_msg
            );
            msg.push_str(&format_source_snippet(source, span));

            let help = get_help_message(&resolved_path, &error_msg);
            if !help.is_empty() {
                msg.push('\n');
                msg.push_str(&help);
            }

            output.push(msg);
        } else {
            debug!(
                "  No span found for {}, looking for target span",
                resolved_path
            );
            // Fallback: try to get the span for the target table itself using FQN
            if let Some(target_span) = span_map.get_target(&target_fqn) {
                debug!("  Found target span for {}: {:?}", target_fqn, target_span);
                let mut msg = format!(
                    "Error in target '{}':\n  {}: {}\n\n",
                    target_fqn, resolved_path, error_msg
                );
                msg.push_str(&format_source_snippet(source, target_span));

                let help = get_help_message(&resolved_path, &error_msg);
                if !help.is_empty() {
                    msg.push('\n');
                    msg.push_str(&help);
                }

                output.push(msg);
            } else {
                debug!("  No target span found for {}", target_fqn);
                // Fallback without span info
                output.push(format!(
                    "Error in target '{}': {}: {}",
                    target_fqn, resolved_path, error_msg
                ));
            }
        }
    }

    let final_error = output.join("\n\n");
    debug!("Final formatted error:\n{}", final_error);
    anyhow::anyhow!(final_error)
}

/// Format runtime validation errors from context.rs with span information
/// This handles errors that occur after extends resolution, when fields may be missing
pub fn format_runtime_validation_error(
    errors: ValidationErrors,
    span_map: &SpanMap,
    fqn: &FullyQualifiedName,
) -> anyhow::Error {
    use log::debug;

    let mut output = Vec::new();
    let source = span_map.source();

    // Collect all field errors recursively
    let mut collected_errors = Vec::new();
    collect_field_errors(&errors, String::new(), &mut collected_errors);

    debug!(
        "Runtime validation: collected {} field errors for {}",
        collected_errors.len(),
        fqn
    );

    for (field_path, error_messages) in collected_errors {
        // Build full path: fqn.to_string() + "." + field
        // e.g., "command.exec.test" + "." + "command" = "command.exec.test.command"
        let full_path = if field_path.is_empty() {
            fqn.to_string()
        } else {
            format!("{}.{}", fqn, field_path)
        };

        let error_msg = if error_messages.is_empty() {
            "validation failed".to_string()
        } else {
            error_messages.join(", ")
        };

        debug!("  Runtime validation error for field: {}", full_path);

        if let Some(span) = span_map.get(&full_path) {
            debug!("  Found span for {}: {:?}", full_path, span);
            let mut msg = format!(
                "Error in target '{}':\n  {}: {}\n\n",
                fqn, full_path, error_msg
            );
            msg.push_str(&format_source_snippet(source, span));

            let help = get_help_message(&full_path, &error_msg);
            if !help.is_empty() {
                msg.push('\n');
                msg.push_str(&help);
            }

            output.push(msg);
        } else {
            debug!("  No span found for {}, looking for target span", full_path);
            // Fallback: use the target table header span
            if let Some(target_span) = span_map.get_target(&fqn.to_string()) {
                debug!("  Found target span for {}: {:?}", fqn, target_span);
                let mut msg = format!(
                    "Error in target '{}':\n  {}: {}\n\n",
                    fqn, full_path, error_msg
                );
                msg.push_str(&format_source_snippet(source, target_span));

                let help = get_help_message(&full_path, &error_msg);
                if !help.is_empty() {
                    msg.push('\n');
                    msg.push_str(&help);
                }

                output.push(msg);
            } else {
                debug!("  No target span found for {}", fqn);
                // Final fallback without span info
                output.push(format!(
                    "Error in target '{}': {}: {}",
                    fqn, full_path, error_msg
                ));
            }
        }
    }

    let final_error = output.join("\n\n");
    debug!("Final runtime validation error:\n{}", final_error);
    anyhow::anyhow!(final_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_target_fqn() {
        // Test resolved paths (normal case - after resolve_path has been called)
        assert_eq!(
            extract_target_fqn("command.exec.test.command"),
            "command.exec.test"
        );
        assert_eq!(
            extract_target_fqn("command.exec.my-app.env"),
            "command.exec.my-app"
        );
        assert_eq!(
            extract_target_fqn("artifact.cargo.build.dir"),
            "artifact.cargo.build"
        );
        assert_eq!(
            extract_target_fqn("command.container.app.image"),
            "command.container.app"
        );

        // Bracket notation shouldn't appear in practice (resolve_path handles it first)
        // but the function handles it as a fallback
        assert_eq!(
            extract_target_fqn("command.exec[test].command"),
            "command.exec.test"
        );
    }

    #[test]
    fn test_format_source_snippet() {
        let source = "line 1\nline 2\nline 3\nline 4\nline 5";
        let span = Span {
            line: 1, // 0-indexed, so this is "line 2"
            column: 5,
            end_line: 1,
            end_column: 6,
        };

        let snippet = format_source_snippet(source, &span);
        assert!(snippet.contains("1 | line 1"));
        assert!(snippet.contains("2 | line 2"));
        assert!(snippet.contains("3 | line 3"));
        assert!(snippet.contains("^")); // Caret highlighting
    }

    #[test]
    fn test_get_help_message() {
        let help = get_help_message("command.exec[test].command", "Command must not be empty");
        assert!(help.contains("npm run dev"));

        let help = get_help_message("command.container[app].image", "image must not be empty");
        assert!(help.contains("ubuntu"));

        let help = get_help_message("some.other.field", "some error");
        assert_eq!(help, "");
    }
}
