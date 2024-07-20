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
}
