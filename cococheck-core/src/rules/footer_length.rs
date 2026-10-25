use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct FooterLength {
    /// The maximum number of characters of all footers together, or `None` for no limit.
    pub max_length: Option<usize>,
}

impl Rule for FooterLength {
    fn name(&self) -> &str {
        "footer_length"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let Some(max_length) = self.max_length else {
            return vec![];
        };

        // Like for the body, each line break counts as one character, and the footer paragraphs
        // are separated by a single empty line.
        let lines = &commit.footer_lines;
        let length = lines
            .iter()
            .map(|line| line.text.chars().count())
            .sum::<usize>()
            + lines.len().saturating_sub(1);

        match (lines.first(), lines.last()) {
            (Some(first), Some(last)) if length > max_length => vec![RuleError {
                message: format!(
                    "Footers exceed maximum length of {max_length} characters (actual: {length})."
                ),
                span: Some(first.span.start..last.span.end),
            }],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{FooterDetection, ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(max_length: Option<usize>, message: &str, config: &ParserConfig) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, config).unwrap();
        FooterLength { max_length }.check(&commit)
    }

    #[test]
    fn passes_every_commit_by_default() {
        let message = format!("feat: Add a feature\n\nRefs: {}", "x".repeat(10_000));
        assert!(check(None, &message, &ParserConfig::default()).is_empty());
    }

    #[test]
    fn reports_footers_that_exceed_the_maximum_length() {
        let message = "feat: Add a feature\n\nBody.\n\nRefs: #1\nFixes: #2";

        assert_eq!(
            check(Some(17), message, &ParserConfig::default()),
            vec![RuleError {
                message: "Footers exceed maximum length of 17 characters (actual: 18).".to_string(),
                span: Some(28..46),
            }]
        );
        assert_eq!(&message[28..46], "Refs: #1\nFixes: #2");
        assert!(check(Some(18), message, &ParserConfig::default()).is_empty());
    }

    #[test]
    fn counts_characters_and_a_single_blank_line_between_paragraphs() {
        // The footers have 16 characters, counting the empty line and two line breaks, but more
        // bytes.
        let message = "feat: Add a feature\n\nRefs: Ä\n\n\n\nBy: Bäm";
        let config = ParserConfig {
            footer_detection: FooterDetection::TrailingParagraphs,
        };

        assert!(check(Some(16), message, &config).is_empty());
        assert_eq!(check(Some(15), message, &config).len(), 1);
    }

    #[test]
    fn passes_a_commit_without_footers() {
        assert!(
            check(
                Some(0),
                "feat: Add a feature\n\nBody.",
                &ParserConfig::default()
            )
            .is_empty()
        );
    }
}
