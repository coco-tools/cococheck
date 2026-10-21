use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct HeaderLength {
    #[serde(default = "HeaderLength::default_max_length")]
    pub max_length: usize,
}

impl HeaderLength {
    fn default_max_length() -> usize {
        72
    }
}

impl Default for HeaderLength {
    fn default() -> Self {
        HeaderLength {
            max_length: Self::default_max_length(),
        }
    }
}

impl Rule for HeaderLength {
    fn name(&self) -> &str {
        "HeaderLength"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let length = commit.header.chars().count();
        if length > self.max_length {
            vec![RuleError {
                message: format!(
                    "Header exceeds maximum length of {} characters (actual: {}).",
                    self.max_length, length
                ),
                span: Some(0..commit.header.len()),
            }]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};

    #[test]
    fn succeeds_for_valid_length() {
        let commit =
            parse_conventional_commit("feat: add new feature", &ParserConfig::default()).unwrap();
        let rule = HeaderLength { max_length: 50 };
        assert!(rule.check(&commit).is_empty());
    }

    #[test]
    fn counts_characters_instead_of_bytes() {
        // 20 characters, but 23 bytes in UTF-8.
        let commit =
            parse_conventional_commit("feat: Größenänderung", &ParserConfig::default()).unwrap();
        let rule = HeaderLength { max_length: 20 };
        assert!(rule.check(&commit).is_empty());
    }

    #[test]
    fn fails_when_header_exceeds_max_length() {
        let long_commit = parse_conventional_commit(
            "feat: this is a very long header that exceeds the maximum length",
            &ParserConfig::default(),
        )
        .unwrap();
        let rule = HeaderLength { max_length: 30 };
        assert_eq!(rule.check(&long_commit).len(), 1);
    }
}
