use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FullyQualifiedName {
    pub tag: String,
    pub name: String,
}

impl fmt::Display for FullyQualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}", self.tag, self.name)
    }
}

impl FullyQualifiedName {
    pub fn from_string(input: &str) -> Self {
        let parts: Vec<&str> = input.split('.').collect();
        FullyQualifiedName {
            tag: parts[0..parts.len() - 1].join(".").to_string(),
            name: parts[parts.len() - 1].to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_string() {
        let fqn = FullyQualifiedName::from_string("tag.name");
        assert_eq!(fqn.tag, "tag");
        assert_eq!(fqn.name, "name");
    }

    #[test]
    fn test_from_string_with_multiple_tags() {
        let fqn = FullyQualifiedName::from_string("tag1.tag2.name");
        assert_eq!(fqn.tag, "tag1.tag2");
        assert_eq!(fqn.name, "name");
    }

    #[test]
    fn test_from_string_with_no_tags() {
        let fqn = FullyQualifiedName::from_string("name");
        assert_eq!(fqn.tag, "");
        assert_eq!(fqn.name, "name");
    }
}
