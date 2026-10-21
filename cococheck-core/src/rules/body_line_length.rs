use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct BodyLineLength {
    pub max_length: usize,
}

impl Default for BodyLineLength {
    fn default() -> Self {
        BodyLineLength { max_length: 72 }
    }
}

impl Rule for BodyLineLength {
    fn name(&self) -> &str {
        "BodyLineLength"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let mut errors = Vec::new();
        for (i, line) in commit.body_lines.iter().enumerate() {
            let length = line.text.chars().count();
            if length > self.max_length {
                errors.push(RuleError {
                    message: format!(
                        "Body line {} exceeds maximum length of {} characters (actual: {}).",
                        i + 1,
                        self.max_length,
                        length
                    ),
                    span: Some(line.span.clone()),
                });
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(rule: &BodyLineLength, message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        rule.check(&commit)
    }

    #[test]
    fn succeeds_for_valid_body_length() {
        let message = "feat: add new feature\n\nThis is a valid body line.\nThis is another one.";
        let rule = BodyLineLength { max_length: 100 };
        assert!(check(&rule, message).is_empty());
    }

    #[test]
    fn counts_characters_instead_of_bytes() {
        // The body line has 25 characters, but 30 bytes in UTF-8.
        let message = "feat: add new feature\n\nÄnderungen für Übergrößen";
        let rule = BodyLineLength { max_length: 25 };
        assert!(check(&rule, message).is_empty());
    }

    #[test]
    fn reports_all_body_lines_exceeding_max_length() {
        let message = "feat: add new feature\n\nThis is ok.\nThis line is way too long.\nAlso ok.\n\
            This line is also too long.";
        let rule = BodyLineLength { max_length: 12 };

        assert_eq!(
            check(&rule, message),
            vec![
                RuleError {
                    message: "Body line 2 exceeds maximum length of 12 characters (actual: 26)."
                        .to_string(),
                    span: Some(35..61),
                },
                RuleError {
                    message: "Body line 4 exceeds maximum length of 12 characters (actual: 27)."
                        .to_string(),
                    span: Some(71..98),
                },
            ]
        );
        assert_eq!(&message[35..61], "This line is way too long.");
        assert_eq!(&message[71..98], "This line is also too long.");
    }

    #[test]
    fn reports_spans_relative_to_the_full_message() {
        // Multiple blank lines between paragraphs are collapsed in the parsed body, and the line
        // also appears in the header, which must not affect the span.
        let message = "feat: too long\n\nShort.\n\n\n\ntoo long\n\nRefs: #1";
        let rule = BodyLineLength { max_length: 6 };

        let errors = check(&rule, message);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, Some(26..34));
        assert_eq!(&message[26..34], "too long");
    }
}
