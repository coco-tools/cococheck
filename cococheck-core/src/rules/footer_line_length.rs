use super::{Rule, RuleError, contains_url};
use crate::parser::ConventionalCommit;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct FooterLineLength {
    /// The maximum number of characters per footer line, or `None` for no limit.
    pub max_length: Option<usize>,
    /// Whether lines that contain a URL are ignored, because long URLs cannot be wrapped.
    pub ignore_urls: bool,
}

impl Default for FooterLineLength {
    fn default() -> Self {
        FooterLineLength {
            max_length: None,
            ignore_urls: true,
        }
    }
}

impl Rule for FooterLineLength {
    fn name(&self) -> &str {
        "footer_line_length"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let Some(max_length) = self.max_length else {
            return vec![];
        };

        let mut errors = Vec::new();

        for (i, line) in commit.footer_lines.iter().enumerate() {
            if self.ignore_urls && contains_url(&line.text) {
                continue;
            }

            let length = line.text.chars().count();
            if length > max_length {
                errors.push(RuleError {
                    message: format!(
                        "Footer line {} exceeds maximum length of {} characters (actual: {}).",
                        i + 1,
                        max_length,
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

    fn check(rule: &FooterLineLength, message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        rule.check(&commit)
    }

    fn with_max_length(max_length: usize) -> FooterLineLength {
        FooterLineLength {
            max_length: Some(max_length),
            ..Default::default()
        }
    }

    #[test]
    fn passes_every_commit_by_default() {
        let message = format!("feat: Add a feature\n\nRefs: {}", "x".repeat(1_000));
        assert!(check(&FooterLineLength::default(), &message).is_empty());
    }

    #[test]
    fn reports_all_footer_lines_exceeding_max_length() {
        let message = "feat: Add a feature\n\nA body line that is long.\n\n\
            Refs: #1\n\
            BREAKING CHANGE: Too long\n\
            and continued too long";

        assert_eq!(
            check(&with_max_length(16), message),
            vec![
                RuleError {
                    message: "Footer line 2 exceeds maximum length of 16 characters (actual: 25)."
                        .to_string(),
                    span: Some(57..82),
                },
                RuleError {
                    message: "Footer line 3 exceeds maximum length of 16 characters (actual: 22)."
                        .to_string(),
                    span: Some(83..105),
                },
            ]
        );
        assert_eq!(&message[57..82], "BREAKING CHANGE: Too long");
        assert_eq!(&message[83..105], "and continued too long");
    }

    #[test]
    fn counts_characters_instead_of_bytes() {
        // The footer line has 16 characters, but 18 bytes in UTF-8.
        let message = "feat: Add a feature\n\nReviewed-by: Bär";
        assert!(check(&with_max_length(16), message).is_empty());
        assert_eq!(check(&with_max_length(15), message).len(), 1);
    }

    #[test]
    fn ignores_lines_with_urls_by_default() {
        let message = "feat: Add a feature\n\nRefs: https://example.org/a/very/long/path";
        assert!(check(&with_max_length(20), message).is_empty());

        let rule = FooterLineLength {
            ignore_urls: false,
            ..with_max_length(20)
        };
        assert_eq!(check(&rule, message).len(), 1);
    }
}
