use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct NoDescriptionTerminalPunctuation {}

impl Rule for NoDescriptionTerminalPunctuation {
    fn name(&self) -> &str {
        "no_description_terminal_punctuation"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        if commit.description.ends_with(['.', '!', '?']) {
            // The description is at the end of the header, so its last character is the
            // punctuation, which is a single byte.
            let end = commit.header.len();
            vec![RuleError {
                message: "Description should not end with terminal punctuation (., !, ?)."
                    .to_string(),
                span: Some(end - 1..end),
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
        NoDescriptionTerminalPunctuation::default().check(&commit)
    }

    #[test]
    fn succeeds_without_terminal_punctuation() {
        assert!(check("feat: add new feature\n\nThis is the body.").is_empty());
    }

    #[test]
    fn fails_for_terminal_punctuation() {
        for message in [
            "feat: add new feature.\n\nBody.",
            "feat: add new feature!",
            "feat: add new feature?",
        ] {
            assert_eq!(
                check(message),
                vec![RuleError {
                    message: "Description should not end with terminal punctuation (., !, ?)."
                        .to_string(),
                    span: Some(21..22),
                }]
            );
        }
    }
}
