use super::{Rule, RuleError};
use crate::parser::{ConventionalCommit, Line, parse_footer_line};
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
        let last_paragraph = &lines[last_paragraph_start..];

        // A paragraph that starts with a footer is not a body paragraph with a footer that misses
        // a blank line, but a footer paragraph that is followed by another one. It is parsed as
        // body because only the last paragraph can contain footers with the default footer
        // detection.
        if last_paragraph
            .first()
            .is_some_and(|line| parse_footer_line(&line.text).is_some())
        {
            return vec![];
        }

        // Only report a footer line if the previous line ends a sentence or a code block, or is a
        // footer itself. Otherwise, the line is more likely part of a wrapped sentence that happens
        // to contain a colon, like "multiple planned\nimplementations: A functional one".
        let in_code_block = &code_block_lines(lines)[last_paragraph_start..];
        let is_footer =
            |i: usize| !in_code_block[i] && parse_footer_line(&last_paragraph[i].text).is_some();
        let first_footer_line = (1..last_paragraph.len())
            .find(|&i| {
                let previous = i - 1;
                is_footer(i)
                    && (is_footer(previous)
                        || in_code_block[previous]
                        || ends_sentence(&last_paragraph[previous].text))
            })
            .map(|i| &last_paragraph[i]);

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

/// Return whether the `line` ends with sentence punctuation, which may be followed by closing
/// brackets or quotes. A colon does not end a sentence, because it introduces what follows.
fn ends_sentence(line: &str) -> bool {
    line.trim_end_matches([')', ']', '"', '\'', '`'])
        .ends_with(['.', '!', '?'])
}

/// Return for each of the `lines` whether it is part of a fenced code block, including the fences.
/// A code block can contain empty lines, so it can span multiple paragraphs.
fn code_block_lines(lines: &[Line]) -> Vec<bool> {
    let mut fence: Option<&str> = None;

    lines
        .iter()
        .map(|line| {
            let text = line.text.trim_start();
            match fence {
                Some(open) => {
                    if text.starts_with(open) {
                        fence = None;
                    }
                    true
                }
                None => {
                    fence = ["```", "~~~"]
                        .into_iter()
                        .find(|marker| text.starts_with(marker));
                    fence.is_some()
                }
            }
        })
        .collect()
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
    fn succeeds_for_footer_paragraph_followed_by_another_footer_paragraph() {
        let message = "feat(orchestrator): Set the transaction isolation level to serializable\n\n\
            When handling jobs, it is necessary to avoid phantom reads [1].\n\n\
            [1]: https://www.postgresql.org/docs/current/transaction-iso.html\n\
            [2]: https://wiki.postgresql.org/wiki/Serializable\n\n\
            Signed-off-by: Jane Doe <jane@example.com>";
        assert!(check(message).is_empty());

        let message = "fix: Fix the bugs\n\nThis is the body.\n\n\
            Fixes #803.\nFixes #882.\n\n\
            Signed-off-by: Jane Doe <jane@example.com>";
        assert!(check(message).is_empty());
    }

    #[test]
    fn succeeds_for_footer_like_lines_in_code_blocks() {
        let message = "fix: Fix the response\n\nIt now returns:\n```\ndata: {\n\t\"process_id\": \"522415\"\n}\n```";
        assert!(check(message).is_empty());

        let message =
            "fix: Fix the response\n\nIt now returns:\n~~~\nfirst\n\nsecond\ndata: {}\n~~~";
        assert!(check(message).is_empty());
    }

    #[test]
    fn fails_for_footer_after_code_block() {
        let message = "fix: Fix the response\n\nIt now returns:\n```\ndata: {}\n```\nRefs: #1";
        let result = check(message);
        assert_eq!(result.len(), 1);
        assert_eq!(&message[result[0].span.clone().unwrap()], "Refs: #1");
    }

    #[test]
    fn succeeds_for_footer_like_lines_in_wrapped_sentences() {
        for body in [
            "An interface is used here, as there are multiple planned\n\
            implementations: A functional one based on a file-cache and one that\n\
            basically disables caching.",
            "This removes an annoying UI build warning \"Unexpected token in attribute\n\
            selector: Colon\".",
            "It now returns:\ndata: {}",
        ] {
            let message = format!("fix: Fix a bug\n\n{body}");
            assert!(check(&message).is_empty(), "{message}");
        }
    }

    #[test]
    fn succeeds_for_numbered_list_of_issue_references() {
        let message = "fix: Fix a bug\n\nThe following commands were used:\n\n\
            [1] `go list -deps ./...`\n\
            [2] #5541.\n\
            [5] #5560.";
        assert!(check(message).is_empty());
    }

    #[test]
    fn fails_for_footer_after_sentence() {
        for (body, footer) in [
            ("This is the body.\nRefs: #123", "Refs: #123"),
            (
                "This is the body.\nSigned-off-by: Jane Doe <jane@example.com>",
                "Signed-off-by: Jane Doe <jane@example.com>",
            ),
            ("Is this the body?\nRefs: #123", "Refs: #123"),
            ("This is the body (really.)\nRefs: #123", "Refs: #123"),
            ("This is the body\nNote: prose\nRefs: #123", "Refs: #123"),
        ] {
            let message = format!("fix: Fix a bug\n\n{body}");
            let result = check(&message);
            assert_eq!(result.len(), 1, "{message}");
            assert_eq!(&message[result[0].span.clone().unwrap()], footer);
        }
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
    fn fails_for_footer_after_single_body_line() {
        assert_eq!(
            check("feat: add new feature\n\nThis is the body.\nRefs: #123"),
            vec![RuleError {
                message: "The footer 'Refs: #123' must be separated from the body by a blank line."
                    .to_string(),
                span: Some(41..51),
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
