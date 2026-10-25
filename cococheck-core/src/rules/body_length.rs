use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct BodyLength {
    /// The maximum number of characters in the body, or `None` for no limit.
    pub max_length: Option<usize>,
    /// The minimum number of characters in the body, or `None` for no limit. `1` requires a body.
    pub min_length: Option<usize>,
}

impl Rule for BodyLength {
    fn name(&self) -> &str {
        "body_length"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        // The body does not contain the footers. Each line break counts as one character.
        let length = commit.body.as_ref().map_or(0, |body| body.chars().count());
        let span = match (commit.body_lines.first(), commit.body_lines.last()) {
            (Some(first), Some(last)) => Some(first.span.start..last.span.end),
            _ => None,
        };

        let mut errors = Vec::new();

        if let Some(min_length) = self.min_length.filter(|&min_length| length < min_length) {
            let message = if commit.body.is_none() {
                "Body is required, but missing.".to_string()
            } else {
                format!(
                    "Body is shorter than the minimum length of {min_length} characters (actual: \
                    {length})."
                )
            };
            errors.push(RuleError {
                message,
                span: span.clone(),
            });
        }

        if let Some(max_length) = self.max_length.filter(|&max_length| length > max_length) {
            errors.push(RuleError {
                message: format!(
                    "Body exceeds maximum length of {max_length} characters (actual: {length})."
                ),
                span,
            });
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(rule: &BodyLength, message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        rule.check(&commit)
    }

    #[test]
    fn passes_every_commit_by_default() {
        let rule = BodyLength::default();
        assert!(check(&rule, "feat: Add a feature").is_empty());
        assert!(
            check(
                &rule,
                &format!("feat: Add a feature\n\n{}", "x".repeat(10_000))
            )
            .is_empty()
        );
    }

    #[test]
    fn reports_a_body_that_exceeds_the_maximum_length() {
        let message = "feat: Add a feature\n\nFirst line.\nSecond.\n\nRefs: #1";
        let rule = BodyLength {
            max_length: Some(15),
            ..Default::default()
        };

        assert_eq!(
            check(&rule, message),
            vec![RuleError {
                message: "Body exceeds maximum length of 15 characters (actual: 19).".to_string(),
                span: Some(21..40),
            }]
        );
        assert_eq!(&message[21..40], "First line.\nSecond.");
    }

    #[test]
    fn counts_characters_and_line_breaks() {
        // The body has 11 characters, with one line break, but 13 bytes in UTF-8.
        let message = "feat: Add a feature\n\nÄpfel\nBäume";
        let exact = BodyLength {
            max_length: Some(11),
            min_length: Some(11),
        };
        assert!(check(&exact, message).is_empty());

        let shorter = BodyLength {
            max_length: Some(10),
            ..Default::default()
        };
        assert_eq!(check(&shorter, message).len(), 1);
    }

    #[test]
    fn counts_a_single_blank_line_between_paragraphs() {
        // Multiple blank lines between paragraphs are collapsed in the parsed body.
        let message = "feat: Add a feature\n\nOne.\n\n\n\nTwo.";
        let rule = BodyLength {
            max_length: Some(10),
            ..Default::default()
        };
        assert!(check(&rule, message).is_empty());
    }

    #[test]
    fn reports_a_body_that_is_shorter_than_the_minimum_length() {
        let message = "feat: Add a feature\n\nShort.";
        let rule = BodyLength {
            min_length: Some(20),
            ..Default::default()
        };

        assert_eq!(
            check(&rule, message),
            vec![RuleError {
                message: "Body is shorter than the minimum length of 20 characters (actual: 6)."
                    .to_string(),
                span: Some(21..27),
            }]
        );
    }

    #[test]
    fn reports_a_missing_body_if_a_minimum_length_is_set() {
        let rule = BodyLength {
            min_length: Some(1),
            ..Default::default()
        };
        let expected = vec![RuleError {
            message: "Body is required, but missing.".to_string(),
            span: None,
        }];

        assert_eq!(check(&rule, "feat: Add a feature"), expected);
        // Footers are not part of the body.
        assert_eq!(
            check(
                &rule,
                "feat: Add a feature\n\nSigned-off-by: Jane Doe <jane@example.org>"
            ),
            expected
        );
        assert!(check(&rule, "feat: Add a feature\n\nBody.").is_empty());
    }
}
