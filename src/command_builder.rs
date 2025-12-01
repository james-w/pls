use crate::shell::escape_string;

/// A fluent builder for constructing shell command strings with proper escaping.
///
/// # Example
/// ```
/// let cmd = CommandBuilder::new("cargo")
///     .subcommand(&Some("build".to_string()))?
///     .flag_with_value("--package", &Some("my-pkg".to_string()))?
///     .flag("--release", true)
///     .build();
/// assert_eq!(cmd, "cargo build --package my-pkg --release");
/// ```
pub struct CommandBuilder {
    parts: Vec<String>,
}

impl CommandBuilder {
    /// Create a new command builder with the given program name.
    pub fn new(program: &str) -> Self {
        Self {
            parts: vec![program.to_string()],
        }
    }

    /// Add a subcommand (e.g., "build", "test").
    /// The subcommand is escaped if it contains special characters.
    pub fn subcommand(mut self, subcmd: &Option<String>) -> Result<Self, shlex::QuoteError> {
        if let Some(s) = subcmd {
            self.parts.push(escape_string(s)?);
        }
        Ok(self)
    }

    /// Add a flag with a value (e.g., "--package foo" becomes "--package 'foo'").
    /// The value is properly escaped.
    pub fn flag_with_value(
        mut self,
        flag: &str,
        value: &Option<String>,
    ) -> Result<Self, shlex::QuoteError> {
        if let Some(v) = value {
            self.parts.push(format!("{} {}", flag, escape_string(v)?));
        }
        Ok(self)
    }

    /// Add a flag with a value using = syntax (e.g., "-ldflags='-s -w'").
    /// The value is properly escaped.
    pub fn flag_equals_value(
        mut self,
        flag: &str,
        value: &Option<String>,
    ) -> Result<Self, shlex::QuoteError> {
        if let Some(v) = value {
            self.parts.push(format!("{}={}", flag, escape_string(v)?));
        }
        Ok(self)
    }

    /// Add a boolean flag (e.g., "--release").
    /// Only adds the flag if enabled is true.
    pub fn flag(mut self, flag: &str, enabled: bool) -> Self {
        if enabled {
            self.parts.push(flag.to_string());
        }
        self
    }

    /// Add a flag with multiple values joined by a separator (e.g., "--features foo,bar").
    /// Each value is individually escaped, then joined with the separator.
    pub fn array_flag(
        mut self,
        flag: &str,
        values: &Option<Vec<String>>,
        separator: &str,
    ) -> Result<Self, shlex::QuoteError> {
        if let Some(vals) = values {
            if !vals.is_empty() {
                let escaped: Result<Vec<_>, _> = vals.iter().map(|v| escape_string(v)).collect();
                self.parts
                    .push(format!("{}{}", flag, escaped?.join(separator)));
            }
        }
        Ok(self)
    }

    /// Add a flag with multiple values joined by a separator using = syntax.
    /// (e.g., "-tags=foo,bar").
    pub fn array_flag_equals(
        mut self,
        flag: &str,
        values: &Option<Vec<String>>,
        separator: &str,
    ) -> Result<Self, shlex::QuoteError> {
        if let Some(vals) = values {
            if !vals.is_empty() {
                let escaped: Result<Vec<_>, _> = vals.iter().map(|v| escape_string(v)).collect();
                self.parts
                    .push(format!("{}={}", flag, escaped?.join(separator)));
            }
        }
        Ok(self)
    }

    /// Add a single positional argument with escaping.
    /// Use this for individual arguments that need to be escaped.
    pub fn arg(mut self, arg: &Option<String>) -> Result<Self, shlex::QuoteError> {
        if let Some(a) = arg {
            self.parts.push(escape_string(a)?);
        }
        Ok(self)
    }

    /// Add raw arguments without escaping.
    /// Use this for user-provided args strings that may contain multiple arguments.
    pub fn raw_args(mut self, args: &Option<String>) -> Self {
        if let Some(a) = args {
            self.parts.push(a.clone());
        }
        self
    }

    /// Build the final command string by joining all parts with spaces.
    pub fn build(self) -> String {
        self.parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let cmd = CommandBuilder::new("cargo").build();
        assert_eq!(cmd, "cargo");
    }

    #[test]
    fn test_with_subcommand() {
        let cmd = CommandBuilder::new("cargo")
            .subcommand(&Some("build".to_string()))
            .unwrap()
            .build();
        assert_eq!(cmd, "cargo build");
    }

    #[test]
    fn test_with_arg() {
        let cmd = CommandBuilder::new("go")
            .subcommand(&Some("mod".to_string()))
            .unwrap()
            .arg(&Some("tidy".to_string()))
            .unwrap()
            .build();
        assert_eq!(cmd, "go mod tidy");
    }

    #[test]
    fn test_with_arg_containing_spaces() {
        let cmd = CommandBuilder::new("echo")
            .arg(&Some("hello world".to_string()))
            .unwrap()
            .build();
        assert_eq!(cmd, "echo 'hello world'");
    }

    #[test]
    fn test_with_flag_value() {
        let cmd = CommandBuilder::new("cargo")
            .subcommand(&Some("build".to_string()))
            .unwrap()
            .flag_with_value("--package", &Some("my-pkg".to_string()))
            .unwrap()
            .build();
        assert_eq!(cmd, "cargo build --package my-pkg");
    }

    #[test]
    fn test_with_flag_value_containing_spaces() {
        let cmd = CommandBuilder::new("cargo")
            .flag_with_value("--package", &Some("my package".to_string()))
            .unwrap()
            .build();
        assert_eq!(cmd, "cargo --package 'my package'");
    }

    #[test]
    fn test_boolean_flag() {
        let cmd = CommandBuilder::new("cargo")
            .flag("--release", true)
            .flag("--debug", false)
            .build();
        assert_eq!(cmd, "cargo --release");
    }

    #[test]
    fn test_array_flag() {
        let cmd = CommandBuilder::new("cargo")
            .array_flag(
                "--features ",
                &Some(vec!["feat1".to_string(), "feat2".to_string()]),
                ",",
            )
            .unwrap()
            .build();
        assert_eq!(cmd, "cargo --features feat1,feat2");
    }

    #[test]
    fn test_array_flag_with_special_chars() {
        let cmd = CommandBuilder::new("cargo")
            .array_flag(
                "--features ",
                &Some(vec!["default-feature".to_string(), "other".to_string()]),
                ",",
            )
            .unwrap()
            .build();
        assert_eq!(cmd, "cargo --features default-feature,other");
    }

    #[test]
    fn test_flag_equals_value() {
        let cmd = CommandBuilder::new("go")
            .flag_equals_value("-ldflags", &Some("-s -w".to_string()))
            .unwrap()
            .build();
        assert_eq!(cmd, "go -ldflags='-s -w'");
    }

    #[test]
    fn test_array_flag_equals() {
        let cmd = CommandBuilder::new("go")
            .array_flag_equals(
                "-tags",
                &Some(vec!["tag1".to_string(), "tag2".to_string()]),
                ",",
            )
            .unwrap()
            .build();
        assert_eq!(cmd, "go -tags=tag1,tag2");
    }

    #[test]
    fn test_raw_args() {
        let cmd = CommandBuilder::new("cargo")
            .subcommand(&Some("build".to_string()))
            .unwrap()
            .raw_args(&Some("--verbose --color always".to_string()))
            .build();
        assert_eq!(cmd, "cargo build --verbose --color always");
    }

    #[test]
    fn test_complex_command() {
        let cmd = CommandBuilder::new("cargo")
            .subcommand(&Some("test".to_string()))
            .unwrap()
            .flag_with_value("--package", &Some("my-pkg".to_string()))
            .unwrap()
            .flag("--release", true)
            .array_flag(
                "--features ",
                &Some(vec!["feat1".to_string(), "feat2".to_string()]),
                ",",
            )
            .unwrap()
            .raw_args(&Some("-- --nocapture".to_string()))
            .build();
        assert_eq!(
            cmd,
            "cargo test --package my-pkg --release --features feat1,feat2 -- --nocapture"
        );
    }
}
