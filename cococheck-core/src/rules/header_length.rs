use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct HeaderLength {
    pub max_length: usize,
}

impl Default for HeaderLength {
    fn default() -> Self {
        HeaderLength { max_length: 72 }
    }
}

impl Rule for HeaderLength {
    fn name(&self) -> &str {
        "header_length"
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
    use pretty_assertions::assert_eq;

    fn check(rule: &HeaderLength, message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        rule.check(&commit)
    }

    #[test]
    fn succeeds_for_valid_length() {
        let rule = HeaderLength { max_length: 50 };
        assert!(check(&rule, "feat: add new feature").is_empty());
    }

    #[test]
    fn counts_characters_instead_of_bytes() {
        // 20 characters, but 23 bytes in UTF-8.
        let rule = HeaderLength { max_length: 20 };
        assert!(check(&rule, "feat: Größenänderung").is_empty());
    }

    #[test]
    fn fails_when_header_exceeds_max_length() {
        let message = "feat: this is a very long header that exceeds the maximum length\n\nBody.";
        let rule = HeaderLength { max_length: 30 };

        assert_eq!(
            check(&rule, message),
            vec![RuleError {
                message: "Header exceeds maximum length of 30 characters (actual: 64).".to_string(),
                span: Some(0..64),
            }]
        );
    }
}
