use std::collections::HashMap;

use validator::ValidationError;

pub fn non_empty_strings(value: &Vec<String>) -> Result<(), ValidationError> {
    for s in value {
        if s.is_empty() {
            return Err(ValidationError::new("empty_string_in_vec")
                .with_message(std::borrow::Cow::from("string cannot be empty")));
        }
    }
    Ok(())
}

pub fn keys_non_empty_strings<T>(value: &HashMap<String, T>) -> Result<(), ValidationError> {
    for s in value.keys() {
        if s.is_empty() {
            return Err(ValidationError::new("invalid_hash_key")
                .with_message(std::borrow::Cow::from("key cannot be empty")));
        }
    }
    Ok(())
}

pub fn keys_and_values_non_empty_strings(
    value: &HashMap<String, String>,
) -> Result<(), ValidationError> {
    for (k, v) in value.iter() {
        if k.is_empty() {
            return Err(ValidationError::new("invalid_hash_key")
                .with_message(std::borrow::Cow::from("key cannot be empty")));
        }
        if v.is_empty() {
            return Err(ValidationError::new("invalid_hash_value")
                .with_message(std::borrow::Cow::from("value cannot be empty")));
        }
    }
    Ok(())
}

pub fn validate_variables_option(vars: &crate::config::Variables) -> Result<(), ValidationError> {
    // Validate base values - only check keys are non-empty, allow empty values
    keys_non_empty_strings(&vars.values)?;

    // Check for reserved word "platform" in base values
    if vars.values.contains_key("platform") {
        return Err(
            ValidationError::new("reserved_word").with_message(std::borrow::Cow::from(
                "Variable name 'platform' is reserved for platform-specific overrides",
            )),
        );
    }

    // Validate platform overrides - only check keys are non-empty, allow empty values
    if let Some(ref platform) = vars.platform {
        if let Some(ref windows) = platform.windows {
            keys_non_empty_strings(windows)?;
        }
        if let Some(ref linux) = platform.linux {
            keys_non_empty_strings(linux)?;
        }
        if let Some(ref macos) = platform.macos {
            keys_non_empty_strings(macos)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn non_empty_strings_no_strings() {
        non_empty_strings(&vec![]).unwrap()
    }

    #[test]
    fn non_empty_strings_empty_string() {
        let res = non_empty_strings(&vec!["".to_string()]);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "string cannot be empty");
    }

    #[test]
    fn non_empty_strings_non_empty_string() {
        non_empty_strings(&vec!["foo".to_string()]).unwrap()
    }

    #[test]
    fn keys_non_empty_strings_no_strings() {
        keys_non_empty_strings::<()>(&HashMap::from([])).unwrap()
    }

    #[test]
    fn keys_non_empty_strings_non_empty_string() {
        keys_non_empty_strings::<()>(&HashMap::from([("foo".to_string(), ())])).unwrap()
    }

    #[test]
    fn keys_non_empty_strings_empty_string() {
        let res = keys_non_empty_strings::<()>(&HashMap::from([("".to_string(), ())]));
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "key cannot be empty");
    }

    #[test]
    fn keys_and_values_non_empty_strings_no_strings() {
        keys_and_values_non_empty_strings(&HashMap::from([])).unwrap()
    }

    #[test]
    fn keys_and_values_non_empty_strings_non_empty_string() {
        keys_and_values_non_empty_strings(&HashMap::from([("foo".to_string(), "bar".to_string())]))
            .unwrap()
    }

    #[test]
    fn keys_and_values_non_empty_strings_empty_key() {
        let res = keys_and_values_non_empty_strings(&HashMap::from([(
            "".to_string(),
            "bar".to_string(),
        )]));
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "key cannot be empty");
    }

    #[test]
    fn keys_and_values_non_empty_strings_empty_value() {
        let res = keys_and_values_non_empty_strings(&HashMap::from([(
            "foo".to_string(),
            "".to_string(),
        )]));
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "value cannot be empty");
    }

    #[test]
    fn keys_and_values_non_empty_strings_empty_key_and_value() {
        let res =
            keys_and_values_non_empty_strings(&HashMap::from([("".to_string(), "".to_string())]));
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "key cannot be empty");
    }

    #[test]
    fn validate_variables_reserved_word() {
        // Test that "platform" is rejected as a variable name
        let mut values = HashMap::new();
        values.insert("platform".to_string(), "production".to_string());

        let vars = crate::config::Variables {
            values,
            platform: None,
        };

        let res = validate_variables_option(&vars);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().to_string(),
            "Variable name 'platform' is reserved for platform-specific overrides"
        );
    }

    #[test]
    fn validate_variables_platform_empty_key_windows() {
        // Test that empty keys in windows platform overrides are rejected
        let mut windows = HashMap::new();
        windows.insert("".to_string(), "value".to_string());

        let vars = crate::config::Variables {
            values: HashMap::new(),
            platform: Some(crate::config::PlatformOverrides {
                windows: Some(windows),
                linux: None,
                macos: None,
            }),
        };

        let res = validate_variables_option(&vars);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "key cannot be empty");
    }

    #[test]
    fn validate_variables_platform_empty_value_linux() {
        // Test that empty values in linux platform overrides are ALLOWED
        let mut linux = HashMap::new();
        linux.insert("key".to_string(), "".to_string());

        let vars = crate::config::Variables {
            values: HashMap::new(),
            platform: Some(crate::config::PlatformOverrides {
                windows: None,
                linux: Some(linux),
                macos: None,
            }),
        };

        let res = validate_variables_option(&vars);
        assert!(res.is_ok());
    }

    #[test]
    fn validate_variables_platform_empty_key_macos() {
        // Test that empty keys in macos platform overrides are rejected
        let mut macos = HashMap::new();
        macos.insert("".to_string(), "value".to_string());

        let vars = crate::config::Variables {
            values: HashMap::new(),
            platform: Some(crate::config::PlatformOverrides {
                windows: None,
                linux: None,
                macos: Some(macos),
            }),
        };

        let res = validate_variables_option(&vars);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().to_string(), "key cannot be empty");
    }
}
