use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct TypeCase {}

impl Rule for TypeCase {
    fn name(&self) -> &str {
        "type_case"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        if commit.commit_type != commit.commit_type.to_lowercase() {
            vec![RuleError {
                message: format!("Commit type '{}' must be lowercase.", commit.commit_type),
                span: Some(0..commit.commit_type.len()),
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

    fn check(message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        TypeCase::default().check(&commit)
    }

    #[test]
    fn succeeds_for_lowercase_type() {
        assert!(check("feat: Add a feature").is_empty());
        assert!(check("feat2(Core): Add a feature").is_empty());
    }

    #[test]
    fn fails_for_type_that_is_not_lowercase() {
        for message in ["Feat: Add a feature", "FEAT: Add a feature"] {
            assert_eq!(
                check(message),
                vec![RuleError {
                    message: format!("Commit type '{}' must be lowercase.", &message[..4]),
                    span: Some(0..4),
                }]
            );
        }
    }
}
