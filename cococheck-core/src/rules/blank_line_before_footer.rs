use super::{Rule, RuleError};
use crate::parser::{ConventionalCommit, parse_footer_line};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct BlankLineBeforeFooter {}

impl Rule for BlankLineBeforeFooter {
    fn name(&self) -> &str {
        "BlankLineBeforeFooter"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let lines = &commit.body_lines;

        // Footers that are not separated from the body by a blank line are parsed as part of the
        // last body paragraph, so only check that paragraph to not report prose like "Note: ...".
        let last_paragraph_start = lines
            .iter()
            .rposition(|line| line.text.is_empty())
            .map_or(0, |i| i + 1);
        let first_footer_line = lines[last_paragraph_start..]
            .iter()
            .skip(1)
            .find(|line| parse_footer_line(&line.text).is_some());

        match first_footer_line {
            Some(line) => vec![RuleError {
                message: format!(
                    "The footer '{}' must be separated from the body by a blank line.",
                    line.text
                ),
                span: Some(line.span.clone()),
            }],
            None => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        BlankLineBeforeFooter::default().check(&commit)
    }

    #[test]
    fn succeeds_for_footer_after_blank_line() {
        assert!(check("feat: add new feature\n\nThis is the body.\n\nRefs: #1").is_empty());
    }

    #[test]
    fn succeeds_without_body() {
        assert!(check("feat: add new feature").is_empty());
        assert!(check("feat: add new feature\n\nRefs: #1").is_empty());
    }

    #[test]
    fn succeeds_for_footer_like_prose_in_earlier_paragraph() {
        assert!(
            check("feat: add new feature\n\nIntro.\nNote: this is prose.\n\nMore text.").is_empty()
        );
    }

    #[test]
    fn succeeds_for_footer_like_first_line_of_last_paragraph() {
        assert!(
            check("feat: add new feature\n\nNote: this is prose.\nMore text.\n\nRefs: #1")
                .is_empty()
        );
    }

    #[test]
    fn fails_for_footer_without_blank_line() {
        assert_eq!(
            check("feat: add new feature\n\nThis is the body.\nRefs: #1\nSigned-off-by: X"),
            vec![RuleError {
                message: "The footer 'Refs: #1' must be separated from the body by a blank line."
                    .to_string(),
                span: Some(41..49),
            }]
        );
    }

    #[test]
    fn fails_for_footer_without_blank_line_in_last_body_paragraph() {
        let message = "feat: add new feature\n\nIntro.\n\nThis is the body.\nFixes #123";
        let result = check(message);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].span, Some(49..59));
        assert_eq!(&message[49..59], "Fixes #123");
    }
}
