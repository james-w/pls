use std::collections::HashMap;

use validator::ValidationError;

pub fn non_empty_strings(value: &Vec<String>) -> Result<(), ValidationError> {
    for s in value {
        if s.is_empty() {
            return Err(ValidationError::new("empty_string_in_vec").with_message(std::borrow::Cow::from("string cannot be empty")));
        }
    }
    Ok(())
}

pub fn keys_non_empty_strings<T>(value: &HashMap<String, T>) -> Result<(), ValidationError> {
    for s in value.keys() {
        if s.is_empty() {
            return Err(ValidationError::new("invalid_hash_key").with_message(std::borrow::Cow::from("key cannot be empty")));
        }
    }
    Ok(())
}

pub fn keys_and_values_non_empty_strings(value: &HashMap<String, String>) -> Result<(), ValidationError> {
    for (k, v) in value.iter() {
        if k.is_empty() {
            return Err(ValidationError::new("invalid_hash_key").with_message(std::borrow::Cow::from("key cannot be empty")));
        }
        if v.is_empty() {
            return Err(ValidationError::new("invalid_hash_value").with_message(std::borrow::Cow::from("value cannot be empty")));
        }
    }
    Ok(())
}
