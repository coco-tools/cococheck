use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BodyLineLength {
    #[serde(default = "BodyLineLength::default_max_length")]
    pub max_length: usize,
}

impl BodyLineLength {
    fn default_max_length() -> usize {
        72
    }
}

impl Default for BodyLineLength {
    fn default() -> Self {
        BodyLineLength {
            max_length: Self::default_max_length(),
        }
    }
}

impl Rule for BodyLineLength {
    fn name(&self) -> &str {
        "BodyLineLength"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let mut errors = Vec::new();
        if let Some(body) = &commit.body {
            for (i, line) in body.lines().enumerate() {
                let length = line.chars().count();
                if length > self.max_length {
                    errors.push(RuleError {
                        message: format!(
                            "Body line {} exceeds maximum length of {} characters (actual: {}).",
                            i + 1,
                            self.max_length,
                            length
                        ),
                        span: Some(0..line.len()),
                    });
                }
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

    #[test]
    fn succeeds_for_valid_body_length() {
        let commit = parse_conventional_commit(
            r#"
            feat: add new feature

            This is a valid body line.
            This is another one.
            "#
            .trim(),
            &ParserConfig::default(),
        )
        .unwrap();
        let rule = BodyLineLength { max_length: 100 };
        assert!(rule.check(&commit).is_empty());
    }

    #[test]
    fn counts_characters_instead_of_bytes() {
        // The body line has 25 characters, but 30 bytes in UTF-8.
        let commit = parse_conventional_commit(
            "feat: add new feature\n\nÄnderungen für Übergrößen",
            &ParserConfig::default(),
        )
        .unwrap();
        let rule = BodyLineLength { max_length: 25 };
        assert!(rule.check(&commit).is_empty());
    }

    #[test]
    fn reports_all_body_lines_exceeding_max_length() {
        let commit = parse_conventional_commit(
            r#"
            feat: add new feature

            This is ok.
            This line is way too long.
            Also ok.
            This line is also too long.
            "#
            .trim(),
            &ParserConfig::default(),
        )
        .unwrap();
        let rule = BodyLineLength { max_length: 12 };

        assert_eq!(
            rule.check(&commit),
            vec![
                RuleError {
                    message: "Body line 2 exceeds maximum length of 12 characters (actual: 26)."
                        .to_string(),
                    span: Some(0..26),
                },
                RuleError {
                    message: "Body line 4 exceeds maximum length of 12 characters (actual: 27)."
                        .to_string(),
                    span: Some(0..27),
                },
            ]
        );
    }
}
