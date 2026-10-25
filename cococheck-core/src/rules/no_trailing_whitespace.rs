use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct NoTrailingWhitespace {}

impl Rule for NoTrailingWhitespace {
    fn name(&self) -> &str {
        "no_trailing_whitespace"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let mut errors = Vec::new();
        let mut start = 0;

        for (i, line) in commit.message.split('\n').enumerate() {
            // Like the parser, treat a carriage return before the line feed as part of the line
            // ending, not as trailing whitespace.
            let text = line.strip_suffix('\r').unwrap_or(line);
            let trimmed = text.trim_end();

            if trimmed.len() < text.len() {
                errors.push(RuleError {
                    message: format!("Line {} has trailing whitespace.", i + 1),
                    span: Some(start + trimmed.len()..start + text.len()),
                });
            }

            start += line.len() + 1;
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        NoTrailingWhitespace {}.check(&commit)
    }

    #[test]
    fn succeeds_without_trailing_whitespace() {
        let message = "feat: Add a feature\n\n  Indented body.\n\nRefs: #1\n";
        assert!(check(message).is_empty());
    }

    #[test]
    fn reports_trailing_whitespace_in_all_parts_of_the_message() {
        let message = "feat: Add a feature \n\nBody.\t\n \nMore body.\n\nRefs: #1  \n";

        let errors = check(message);

        assert_eq!(
            errors,
            vec![
                RuleError {
                    message: "Line 1 has trailing whitespace.".to_string(),
                    span: Some(19..20),
                },
                RuleError {
                    message: "Line 3 has trailing whitespace.".to_string(),
                    span: Some(27..28),
                },
                RuleError {
                    message: "Line 4 has trailing whitespace.".to_string(),
                    span: Some(29..30),
                },
                RuleError {
                    message: "Line 7 has trailing whitespace.".to_string(),
                    span: Some(51..53),
                },
            ]
        );
        for error in &errors {
            let span = error.span.clone().unwrap();
            assert!(message[span].chars().all(char::is_whitespace));
        }
    }

    #[test]
    fn ignores_carriage_returns_of_line_endings() {
        let message = "feat: Add a feature\r\n\r\nBody. \r\n";

        assert_eq!(
            check(message),
            vec![RuleError {
                message: "Line 3 has trailing whitespace.".to_string(),
                span: Some(28..29),
            }]
        );
    }
}
