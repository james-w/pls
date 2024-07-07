use crate::context::Context;

pub fn escape_string(s: &str) -> Result<String, shlex::QuoteError> {
    Ok(shlex::try_quote(s)?.to_string())
}

pub fn prepend_argument_if_set(arg: &str, value: &Option<&str>) -> Result<String, shlex::QuoteError> {
    prepend_arguments_if_set(arg, &value.map(|v| vec![v]))
}

pub fn prepend_arguments_if_set(
    arg: &str,
    value: &Option<Vec<&str>>,
) -> Result<String, shlex::QuoteError> {
    value.as_ref().map_or_else(
        || Ok("".to_string()),
        |v| {
            v.iter()
                .map(|e| escape_string(e).map(|e| format!("{} {}", arg, e)))
                .collect::<Result<Vec<_>, _>>()
                .map(|vs| vs.join(" "))
        },
    )
}

pub fn escape_and_prepend(
    target_name: &str,
    context: &Context,
    arg: &str,
    value: &Option<String>,
) -> Result<String, shlex::QuoteError> {
    if let Some(v) = value {
        prepend_argument_if_set(
            arg,
            &Some(
                context
                    .resolve_substitutions(v.as_ref(), target_name)
                    .as_str(),
            ),
        )
    } else {
        Ok("".to_string())
    }
}

pub fn escape_and_prepend_vec(
    target_name: &str,
    context: &Context,
    arg: &str,
    value: &Option<Vec<String>>,
) -> Result<String, shlex::QuoteError> {
    if let Some(v) = value {
        let resolved = v.iter()
                    .map(|ref e| context.resolve_substitutions(e, target_name))
                    .collect::<Vec<_>>();
        prepend_arguments_if_set(
            arg,
            &Some(resolved.iter().map(|e| e.as_str()).collect()),
        )
    } else {
        Ok("".to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("foo").unwrap(), "foo");
        assert_eq!(escape_string("foo bar").unwrap(), "'foo bar'");
        assert_eq!(escape_string("foo'bar").unwrap(), "\"foo'bar\"");
    }

    #[test]
    fn test_prepend_argument_if_set() {
        assert_eq!(prepend_argument_if_set("-e", &Some("foo")).unwrap(), "-e foo");
        assert_eq!(prepend_argument_if_set("-e", &None).unwrap(), "");
    }

    #[test]
    fn test_prepend_argument_if_set_escapes() {
        assert_eq!(prepend_argument_if_set("-e", &Some("$foo")).unwrap(), "-e '$foo'");
        assert_eq!(prepend_argument_if_set("-e", &None).unwrap(), "");
    }

    #[test]
    fn test_prepend_arguments_if_set() {
        assert_eq!(prepend_arguments_if_set("-e", &Some(vec!["foo", "bar"])).unwrap(), "-e foo -e bar");
        assert_eq!(prepend_arguments_if_set("-e", &None).unwrap(), "");
    }

    #[test]
    fn test_prepend_arguments_if_set_escapes() {
        assert_eq!(prepend_arguments_if_set("-e", &Some(vec!["$foo", "bar"])).unwrap(), "-e '$foo' -e bar");
        assert_eq!(prepend_arguments_if_set("-e", &None).unwrap(), "");
    }

    #[test]
    fn test_escape_and_prepend() {
        let mut context = Context::default();
        context.variables.insert("bar".to_string(), HashMap::from([("foo".to_string(), "baz".to_string())]));
        assert_eq!(escape_and_prepend("bar", &context, "-e", &Some("{foo}".to_string())).unwrap(), "-e baz");
        assert_eq!(escape_and_prepend("bar", &context, "-e", &None).unwrap(), "");
    }

    #[test]
    fn test_escape_and_prepend_escapes_after() {
        let mut context = Context::default();
        context.variables.insert("bar".to_string(), HashMap::from([("foo".to_string(), "$baz".to_string())]));
        assert_eq!(escape_and_prepend("bar", &context, "-e", &Some("{foo}".to_string())).unwrap(), "-e '$baz'");
    }

    #[test]
    fn test_escape_and_prepend_vec() {
        let mut context = Context::default();
        context.variables.insert("bar".to_string(), HashMap::from([("foo".to_string(), "baz".to_string())]));
        assert_eq!(escape_and_prepend_vec("bar", &context, "-e", &Some(vec!["{foo}".to_string(), "qux".to_string()])).unwrap(), "-e baz -e qux");
        assert_eq!(escape_and_prepend_vec("bar", &context, "-e", &None).unwrap(), "");
    }

    #[test]
    fn test_escape_and_prepend_vec_escapes_after() {
        let mut context = Context::default();
        context.variables.insert("bar".to_string(), HashMap::from([("foo".to_string(), "$baz".to_string())]));
        assert_eq!(escape_and_prepend_vec("bar", &context, "-e", &Some(vec!["{foo}".to_string(), "qux".to_string()])).unwrap(), "-e '$baz' -e qux");
    }
}
