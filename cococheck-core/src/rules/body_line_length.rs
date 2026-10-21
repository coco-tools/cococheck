use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;
use regex::regex;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct BodyLineLength {
    pub max_length: usize,
    /// Whether lines that contain a URL are ignored, because long URLs cannot be wrapped.
    pub ignore_urls: bool,
}

impl Default for BodyLineLength {
    fn default() -> Self {
        BodyLineLength {
            max_length: 72,
            ignore_urls: true,
        }
    }
}

impl Rule for BodyLineLength {
    fn name(&self) -> &str {
        "BodyLineLength"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let mut errors = Vec::new();
        // Like commitlint, accept anything that starts like a URL, instead of trying to match only
        // valid URLs.
        let url_regex = regex!(r"\bhttps?://\S+");

        for (i, line) in commit.body_lines.iter().enumerate() {
            if self.ignore_urls && url_regex.is_match(&line.text) {
                continue;
            }

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
        let rule = BodyLineLength {
            max_length: 100,
            ..Default::default()
        };
        assert!(check(&rule, message).is_empty());
    }

    #[test]
    fn counts_characters_instead_of_bytes() {
        // The body line has 25 characters, but 30 bytes in UTF-8.
        let message = "feat: add new feature\n\nÄnderungen für Übergrößen";
        let rule = BodyLineLength {
            max_length: 25,
            ..Default::default()
        };
        assert!(check(&rule, message).is_empty());
    }

    #[test]
    fn reports_all_body_lines_exceeding_max_length() {
        let message = "feat: add new feature\n\nThis is ok.\nThis line is way too long.\nAlso ok.\n\
            This line is also too long.";
        let rule = BodyLineLength {
            max_length: 12,
            ..Default::default()
        };

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
    fn ignores_lines_with_urls_by_default() {
        let message = "feat: add new feature\n\nSee [1] for details.\n\n\
            [1] https://example.com/a/very/long/path\n\
            Long text with a URL: http://example.com\n\
            Long text without a URL: example.com/path";
        let rule = BodyLineLength {
            max_length: 20,
            ..Default::default()
        };

        let errors = check(&rule, message);

        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "Body line 5 exceeds maximum length of 20 characters (actual: 41)."
        );
    }

    #[test]
    fn reports_lines_with_urls_if_configured() {
        let message = "feat: add new feature\n\n[1] https://example.com/a/very/long/path";
        let rule = BodyLineLength {
            max_length: 20,
            ignore_urls: false,
        };

        assert_eq!(check(&rule, message).len(), 1);
    }

    #[test]
    fn reports_spans_relative_to_the_full_message() {
        // Multiple blank lines between paragraphs are collapsed in the parsed body, and the line
        // also appears in the header, which must not affect the span.
        let message = "feat: too long\n\nShort.\n\n\n\ntoo long\n\nRefs: #1";
        let rule = BodyLineLength {
            max_length: 6,
            ..Default::default()
        };

        let errors = check(&rule, message);

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, Some(26..34));
        assert_eq!(&message[26..34], "too long");
    }
}
