use super::{Rule, RuleError};
use crate::parser::{ConventionalCommit, parse_footer_line};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct BlankLineBeforeFooter {
    #[serde(default = "BlankLineBeforeFooter::default_enabled")]
    pub enabled: bool,
}

impl BlankLineBeforeFooter {
    fn default_enabled() -> bool {
        true
    }
}

impl Default for BlankLineBeforeFooter {
    fn default() -> Self {
        BlankLineBeforeFooter {
            enabled: Self::default_enabled(),
        }
    }
}

impl Rule for BlankLineBeforeFooter {
    fn name(&self) -> &str {
        "BlankLineBeforeFooter"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn check(&self, commit: &ConventionalCommit) -> Result<(), Vec<RuleError>> {
        let Some(body) = &commit.body else {
            return Ok(());
        };

        // Footers that are not separated from the body by a blank line are parsed as part of the
        // last body paragraph, so only check that paragraph to not report prose like "Note: ...".
        let last_paragraph = body.rsplit("\n\n").next().unwrap_or(body);
        let first_footer_line = last_paragraph
            .lines()
            .skip(1)
            .find(|line| parse_footer_line(line).is_some());

        match first_footer_line {
            Some(line) => Err(vec![RuleError {
                message: format!(
                    "The footer '{}' must be separated from the body by a blank line.",
                    line
                ),
                span: Some(0..line.len()),
            }]),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(message: &str) -> Result<(), Vec<RuleError>> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        BlankLineBeforeFooter::default().check(&commit)
    }

    #[test]
    fn succeeds_for_footer_after_blank_line() {
        assert!(check("feat: add new feature\n\nThis is the body.\n\nRefs: #1").is_ok());
    }

    #[test]
    fn succeeds_without_body() {
        assert!(check("feat: add new feature").is_ok());
        assert!(check("feat: add new feature\n\nRefs: #1").is_ok());
    }

    #[test]
    fn succeeds_for_footer_like_prose_in_earlier_paragraph() {
        assert!(
            check("feat: add new feature\n\nIntro.\nNote: this is prose.\n\nMore text.").is_ok()
        );
    }

    #[test]
    fn succeeds_for_footer_like_first_line_of_last_paragraph() {
        assert!(
            check("feat: add new feature\n\nNote: this is prose.\nMore text.\n\nRefs: #1").is_ok()
        );
    }

    #[test]
    fn fails_for_footer_without_blank_line() {
        assert_eq!(
            check("feat: add new feature\n\nThis is the body.\nRefs: #1\nSigned-off-by: X"),
            Err(vec![RuleError {
                message: "The footer 'Refs: #1' must be separated from the body by a blank line."
                    .to_string(),
                span: Some(0..8),
            }])
        );
    }

    #[test]
    fn fails_for_footer_without_blank_line_in_last_body_paragraph() {
        let result = check("feat: add new feature\n\nIntro.\n\nThis is the body.\nFixes #123");
        assert_eq!(result.unwrap_err().len(), 1);
    }
}
