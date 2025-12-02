//! TOML config parsing with source span tracking.
//!
//! This module parses the pls configuration TOML file while preserving source location
//! information for error reporting. We parse the file twice:
//!
//! 1. With the `toml` crate to deserialize into our `Config` struct (with serde + validation)
//! 2. With the `toml-span` crate to extract source location information
//!
//! The `toml-span` crate provides span-tracked values but doesn't integrate directly
//! with serde, so we need both parses. For typical config files (~100 lines), this overhead is negligible.
//!
//! ## Span coordinate system
//!
//! Spans use 0-indexed coordinates (line 0 = first line, column 0 = first character).
//! The `offset_to_line_col` function converts byte offsets from `toml-span` into line/column pairs.

use crate::config::Config;
use crate::validation_error::{Span, SpanMap};
use anyhow::Result;

/// Parse TOML with span information
///
/// We parse the TOML twice because:
/// 1. `toml` crate deserializes into our Config struct with proper validation
/// 2. `toml-span` crate preserves source location information but doesn't support
///    deserializing into custom structs with serde
///
/// For typical config files (~100 lines), the double parse is negligible.
pub fn parse_with_spans(toml_str: &str) -> Result<(Config, SpanMap)> {
    // Parse with regular toml for the Config struct
    let config: Config = toml::from_str(toml_str)?;

    // Parse with toml-span to extract span information
    let value = toml_span::parse(toml_str)?;

    // Build span map by walking the toml-span Value tree
    let mut span_map = SpanMap::new(toml_str.to_string());
    extract_spans(&value, String::new(), &mut span_map, toml_str);

    Ok((config, span_map))
}

/// Convert byte offset to line/column
fn offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 0;
    let mut col = 0;
    let mut current_offset = 0;

    for ch in source.chars() {
        if current_offset >= offset {
            break;
        }
        current_offset += ch.len_utf8();

        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    (line, col)
}

/// Convert toml_span::Span to our Span
fn convert_span(span: toml_span::Span, source: &str) -> Span {
    let (line, column) = offset_to_line_col(source, span.start);
    let (end_line, end_column) = offset_to_line_col(source, span.end);

    Span {
        line,
        column,
        end_line,
        end_column,
    }
}

/// Recursively extract spans from the toml-span Value tree
fn extract_spans(value: &toml_span::Value, path: String, span_map: &mut SpanMap, source: &str) {
    use toml_span::value::ValueInner;

    // First, record the span for this value
    if !path.is_empty() {
        span_map.insert_field(path.clone(), convert_span(value.span, source));
    }

    // Then recurse into nested structures
    match value.as_ref() {
        ValueInner::Table(entries) => {
            // Check if we're at a target table (e.g., "command.exec", "artifact.cargo")
            // by counting dots - target tables are at depth 2 (e.g., "command.exec")
            let is_target_table = (path.starts_with("command.") || path.starts_with("artifact."))
                && path.matches('.').count() == 1;

            for (key, val) in entries.iter() {
                let key_str = key.name.as_ref();
                let new_path = if path.is_empty() {
                    key_str.to_string()
                } else {
                    format!("{}.{}", path, key_str)
                };

                // Record the target name and its span if we're at a target table
                if is_target_table {
                    span_map.insert_target(
                        path.to_string(),
                        key_str.to_string(),
                        convert_span(key.span, source),
                    );
                }

                extract_spans(val, new_path, span_map, source);
            }
        }
        ValueInner::Array(items) => {
            for (idx, item) in items.iter().enumerate() {
                let new_path = format!("{}[{}]", path, idx);
                extract_spans(item, new_path, span_map, source);
            }
        }
        _ => {
            // For primitive types (String, Integer, Float, Boolean), we already recorded the span above
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_to_line_col() {
        let source = "abc\ndef\nghi";
        assert_eq!(offset_to_line_col(source, 0), (0, 0)); // 'a'
        assert_eq!(offset_to_line_col(source, 3), (0, 3)); // '\n'
        assert_eq!(offset_to_line_col(source, 4), (1, 0)); // 'd'
        assert_eq!(offset_to_line_col(source, 8), (2, 0)); // 'g'
    }

    #[test]
    fn test_parse_simple_config() {
        let toml = r#"
[command.exec.test]
command = "echo hello"
"#;
        let result = parse_with_spans(toml);
        assert!(result.is_ok());

        let (_config, span_map) = result.unwrap();
        // Verify we have some span information
        assert!(span_map.get_target("command.exec.test").is_some());
        assert_eq!(span_map.resolve_path("command.exec[0].command"), "command.exec.test.command");
        assert_eq!(span_map.source(), toml);
    }
}
