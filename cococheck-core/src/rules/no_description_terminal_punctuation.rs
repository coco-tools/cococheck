use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct NoDescriptionTerminalPunctuation {
    /// The characters that the description must not end with, each character of the string being
    /// one of them.
    pub characters: String,
}

impl Default for NoDescriptionTerminalPunctuation {
    fn default() -> Self {
        NoDescriptionTerminalPunctuation {
            characters: ".!?".to_string(),
        }
    }
}

impl Rule for NoDescriptionTerminalPunctuation {
    fn name(&self) -> &str {
        "no_description_terminal_punctuation"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let Some(last) = commit.description.chars().last() else {
            return vec![];
        };

        if self.characters.contains(last) {
            // The description is at the end of the header, so its last character is the end of the
            // header.
            let end = commit.header.len();
            vec![RuleError {
                message: format!("Description should not end with terminal punctuation '{last}'."),
                span: Some(end - last.len_utf8()..end),
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

    fn check(rule: &NoDescriptionTerminalPunctuation, message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        rule.check(&commit)
    }

    fn with_characters(characters: &str) -> NoDescriptionTerminalPunctuation {
        NoDescriptionTerminalPunctuation {
            characters: characters.to_string(),
        }
    }

    #[test]
    fn succeeds_without_terminal_punctuation() {
        let rule = NoDescriptionTerminalPunctuation::default();
        assert!(check(&rule, "feat: add new feature\n\nThis is the body.").is_empty());
        assert!(check(&rule, "feat: add new feature,").is_empty());
    }

    #[test]
    fn fails_for_terminal_punctuation() {
        let rule = NoDescriptionTerminalPunctuation::default();
        for message in [
            "feat: add new feature.\n\nBody.",
            "feat: add new feature!",
            "feat: add new feature?",
        ] {
            assert_eq!(
                check(&rule, message),
                vec![RuleError {
                    message: format!(
                        "Description should not end with terminal punctuation '{}'.",
                        &message[21..22]
                    ),
                    span: Some(21..22),
                }]
            );
        }
    }

    #[test]
    fn checks_only_the_configured_characters() {
        let rule = with_characters(".,;:");
        assert!(check(&rule, "feat: add new feature!").is_empty());
        assert_eq!(
            check(&rule, "feat: add new feature;"),
            vec![RuleError {
                message: "Description should not end with terminal punctuation ';'.".to_string(),
                span: Some(21..22),
            }]
        );
    }

    #[test]
    fn reports_the_span_of_a_multi_byte_character() {
        let message = "feat: add new feature。";
        let errors = check(&with_characters("。"), message);
        assert_eq!(errors.len(), 1);
        assert_eq!(&message[errors[0].span.clone().unwrap()], "。");
    }

    #[test]
    fn passes_every_commit_without_characters() {
        assert!(check(&with_characters(""), "feat: add new feature.").is_empty());
    }
}
