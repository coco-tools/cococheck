use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct DescriptionCase {
    pub case: DescriptionCaseMode,
}

/// The required case of the first letter of the description.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptionCaseMode {
    /// The description must start with a lowercase letter, like `add a feature`.
    Lower,

    /// The description must start with an uppercase letter, like `Add a feature`.
    Sentence,

    /// The description can start with any letter.
    #[default]
    Any,
}

impl Rule for DescriptionCase {
    fn name(&self) -> &str {
        "description_case"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        // Only a letter has a case, so a description that starts with something else, like a
        // number or a code span, passes.
        let Some(first) = commit
            .description
            .chars()
            .next()
            .filter(|c| c.is_alphabetic())
        else {
            return vec![];
        };

        let message = match self.case {
            DescriptionCaseMode::Lower if !first.is_lowercase() => {
                "Description must start with a lowercase letter."
            }
            DescriptionCaseMode::Sentence if !first.is_uppercase() => {
                "Description must start with an uppercase letter."
            }
            _ => return vec![],
        };

        // The description is at the end of the header.
        let start = commit.header.len() - commit.description.len();
        vec![RuleError {
            message: message.to_string(),
            span: Some(start..start + first.len_utf8()),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(message: &str, case: DescriptionCaseMode) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        DescriptionCase { case }.check(&commit)
    }

    #[test]
    fn succeeds_for_any_case_by_default() {
        let rule = DescriptionCase::default();
        assert_eq!(rule.case, DescriptionCaseMode::Any);

        for message in ["feat: Add a feature", "feat: add a feature"] {
            assert!(check(message, DescriptionCaseMode::Any).is_empty());
        }
    }

    #[test]
    fn succeeds_for_lowercase_description_in_lower_mode() {
        assert!(check("feat: add a feature", DescriptionCaseMode::Lower).is_empty());
    }

    #[test]
    fn fails_for_uppercase_description_in_lower_mode() {
        assert_eq!(
            check("feat(core): Add a feature", DescriptionCaseMode::Lower),
            vec![RuleError {
                message: "Description must start with a lowercase letter.".to_string(),
                span: Some(12..13),
            }]
        );
    }

    #[test]
    fn succeeds_for_uppercase_description_in_sentence_mode() {
        assert!(check("feat: Add a feature", DescriptionCaseMode::Sentence).is_empty());
    }

    #[test]
    fn fails_for_lowercase_description_in_sentence_mode() {
        assert_eq!(
            check("feat!: add a feature", DescriptionCaseMode::Sentence),
            vec![RuleError {
                message: "Description must start with an uppercase letter.".to_string(),
                span: Some(7..8),
            }]
        );
    }

    #[test]
    fn succeeds_for_description_that_does_not_start_with_a_letter() {
        for case in [DescriptionCaseMode::Lower, DescriptionCaseMode::Sentence] {
            assert!(check("feat: `--from` is exclusive", case).is_empty());
            assert!(check("feat: 2FA is supported", case).is_empty());
        }
    }

    #[test]
    fn marks_a_multibyte_first_letter() {
        let message = "fix: über alles";
        let errors = check(message, DescriptionCaseMode::Sentence);

        assert_eq!(errors.len(), 1);
        assert_eq!(&message[errors[0].span.clone().unwrap()], "ü");
    }
}
