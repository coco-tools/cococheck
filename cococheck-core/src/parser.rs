use regex::regex;
use serde::Deserialize;

#[derive(Debug, PartialEq)]
pub struct ConventionalCommit {
    pub header: String,
    pub commit_type: String,
    pub scope: Option<String>,
    pub is_breaking_change: bool,
    pub description: String,
    pub body: Option<String>,
    pub footer: Vec<Footer>,
}

#[derive(Debug, PartialEq)]
pub struct Footer {
    pub token: String,
    pub value: String,
}

#[derive(Debug, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    #[error("cannot parse header line '{header}'")]
    InvalidHeader { header: String },

    #[error("the line after the header must be empty")]
    MissingBlankLineAfterHeader,
}

#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParserConfig {
    /// How the footer paragraphs at the end of the commit message are detected.
    #[serde(default)]
    pub footer_detection: FooterDetection,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum FooterDetection {
    /// Only the last paragraph can contain footers, if its first line is a footer.
    #[default]
    LastParagraph,

    /// All trailing paragraphs whose first line is a footer contain footers.
    TrailingParagraphs,
}

pub fn parse_conventional_commit(
    message: &str,
    config: &ParserConfig,
) -> Result<ConventionalCommit, ParseError> {
    let mut lines = message.lines();

    let header_line = lines.next().unwrap_or("").trim();
    let header_regex =
        regex!(r"^(?P<type>\w+)(\((?P<scope>[^)]+)\))?(?P<breaking>!)?: (?P<desc>.+)$");

    let Some(caps) = header_regex.captures(header_line) else {
        return Err(ParseError::InvalidHeader {
            header: header_line.to_string(),
        });
    };

    if lines.next().is_some_and(|line| !line.trim().is_empty()) {
        return Err(ParseError::MissingBlankLineAfterHeader);
    }

    let paragraphs = split_paragraphs(lines);

    let starts_with_footer = |paragraph: &&Vec<&str>| parse_footer_line(paragraph[0]).is_some();
    let footer_paragraph_count = match config.footer_detection {
        FooterDetection::LastParagraph => paragraphs
            .last()
            .filter(starts_with_footer)
            .map_or(0, |_| 1),
        FooterDetection::TrailingParagraphs => paragraphs
            .iter()
            .rev()
            .take_while(starts_with_footer)
            .count(),
    };

    let (body_paragraphs, footer_paragraphs) =
        paragraphs.split_at(paragraphs.len() - footer_paragraph_count);

    let body = if body_paragraphs.is_empty() {
        None
    } else {
        Some(
            body_paragraphs
                .iter()
                .map(|paragraph| paragraph.join("\n"))
                .collect::<Vec<_>>()
                .join("\n\n"),
        )
    };

    let mut footers = Vec::new();
    let mut breaking_change_footer_found = false;

    let mut current_token: Option<String> = None;
    let mut current_value: Option<String> = None;

    // Blank lines only separate footer paragraphs, so iterating the lines of all footer paragraphs
    // ensures that they never end up in footer values.
    for &line in footer_paragraphs.iter().flatten() {
        if let Some((token, value)) = parse_footer_line(line) {
            if let (Some(token), Some(value)) = (current_token.take(), current_value.take()) {
                footers.push(Footer { token, value });
            }

            if token == "BREAKING CHANGE" || token == "BREAKING-CHANGE" {
                breaking_change_footer_found = true;
            }

            current_token = Some(token);
            current_value = Some(value);
        } else {
            if let Some(ref mut s) = current_value {
                s.push('\n');
                s.push_str(line);
            }
        }
    }

    if let (Some(token), Some(value)) = (current_token.take(), current_value.take()) {
        footers.push(Footer { token, value });
    }

    Ok(ConventionalCommit {
        header: header_line.to_string(),
        commit_type: caps["type"].to_string(),
        scope: caps.name("scope").map(|m| m.as_str().to_string()),
        is_breaking_change: breaking_change_footer_found || caps.name("breaking").is_some(),
        description: caps["desc"].to_string(),
        body,
        footer: footers,
    })
}

/// Split the trimmed lines into paragraphs separated by one or more empty lines.
fn split_paragraphs<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<Vec<&'a str>> {
    let mut paragraphs: Vec<Vec<&str>> = Vec::new();
    let mut last_line_empty = true;

    for line in lines.map(str::trim) {
        if line.is_empty() {
            last_line_empty = true;
            continue;
        }

        match paragraphs.last_mut() {
            Some(paragraph) if !last_line_empty => paragraph.push(line),
            _ => paragraphs.push(vec![line]),
        }

        last_line_empty = false;
    }

    paragraphs
}

pub fn parse_footer_line(line: &str) -> Option<(String, String)> {
    let footer_regex =
        regex!(r"^(?P<token>BREAKING CHANGE|\[[^]]+]|[\p{L}\p{N}-]+)(?P<sep>: | #)(?P<value>.+)$");

    footer_regex.captures(line).map(|caps| {
        let token = caps["token"].to_string();
        let sep = &caps["sep"];
        let value = &caps["value"];
        let value = if sep == " #" {
            format!("#{}", value)
        } else {
            value.to_string()
        };
        (token, value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_header_without_scope() {
        let message = "feat: add new feature";
        let expected = ConventionalCommit {
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: false,
            description: "add new feature".to_string(),
            body: None,
            footer: Vec::new(),
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_header_with_scope() {
        let message = "fix(core): correct minor issue";
        let expected = ConventionalCommit {
            header: "fix(core): correct minor issue".to_string(),
            commit_type: "fix".to_string(),
            scope: Some("core".to_string()),
            is_breaking_change: false,
            description: "correct minor issue".to_string(),
            body: None,
            footer: Vec::new(),
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_header_with_breaking_change() {
        let message = "feat!: introduce breaking change";
        let expected = ConventionalCommit {
            header: "feat!: introduce breaking change".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: true,
            description: "introduce breaking change".to_string(),
            body: None,
            footer: Vec::new(),
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_body_and_footer() {
        let message = r#"
        fix: resolve issue

        This is the body of the commit.
        This is the second line of the body.

        This is the third line of the body.

        BREAKING CHANGE: major update
        Signed-off-by: John Doe <john.doe@example.org>
        Fixes #123
        "#
        .trim();
        let expected = ConventionalCommit {
            header: "fix: resolve issue".to_string(),
            commit_type: "fix".to_string(),
            scope: None,
            is_breaking_change: true,
            description: "resolve issue".to_string(),
            body: Some("This is the body of the commit.\nThis is the second line of the body.\n\nThis is the third line of the body.".to_string()),
            footer: vec![Footer {
                token: "BREAKING CHANGE".to_string(),
                value: "major update".to_string(),
            }, Footer {
                token: "Signed-off-by".to_string(),
                value: "John Doe <john.doe@example.org>".to_string()
            }, Footer {
                token: "Fixes".to_string(),
                value: "#123".to_string()
            }],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_footers_with_multiple_lines() {
        let message = r#"
        feat: add new feature

        This is the body.

        BREAKING CHANGE: This is a long breaking change description
        that spans multiple lines and provides detailed information
        about the change.
        Signed-off-by: John Doe <john.doe@example.org>
        "#
        .trim();

        let expected = ConventionalCommit {
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: true,
            description: "add new feature".to_string(),
            body: Some("This is the body.".to_string()),
            footer: vec![Footer {
                token: "BREAKING CHANGE".to_string(),
                value: "This is a long breaking change description\nthat spans multiple lines and provides detailed information\nabout the change.".to_string(),
            }, Footer {
                token: "Signed-off-by".to_string(),
                value: "John Doe <john.doe@example.org>".to_string()
            }],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_footers_with_links() {
        let message = r#"
        feat: add new feature

        This [1] is the body [2].

        [1]: https://example.org/issue/123
        [2]: https://example.org/issue/456
        "#
        .trim();

        let expected = ConventionalCommit {
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: false,
            description: "add new feature".to_string(),
            body: Some("This [1] is the body [2].".to_string()),
            footer: vec![
                Footer {
                    token: "[1]".to_string(),
                    value: "https://example.org/issue/123".to_string(),
                },
                Footer {
                    token: "[2]".to_string(),
                    value: "https://example.org/issue/456".to_string(),
                },
            ],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_footers_with_footnotes() {
        let message = r#"
        feat: add new feature

        This[^1] is the body[^2].

        [^1]: https://example.org/issue/123
        [^2]: https://example.org/issue/456
        "#
        .trim();

        let expected = ConventionalCommit {
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: false,
            description: "add new feature".to_string(),
            body: Some("This[^1] is the body[^2].".to_string()),
            footer: vec![
                Footer {
                    token: "[^1]".to_string(),
                    value: "https://example.org/issue/123".to_string(),
                },
                Footer {
                    token: "[^2]".to_string(),
                    value: "https://example.org/issue/456".to_string(),
                },
            ],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn detects_breaking_change_in_footer() {
        let message = r#"
        feat: add new feature

        This is the body.

        BREAKING CHANGE: major update
        "#
        .trim();

        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default())
                .unwrap()
                .is_breaking_change,
            true
        );

        let message = r#"
        feat: add new feature

        This is the body.

        BREAKING-CHANGE: major update
        "#
        .trim();

        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default())
                .unwrap()
                .is_breaking_change,
            true
        );
    }

    #[test]
    fn ignores_lowercase_breaking_change_footer() {
        let message = r#"
        feat: add new feature

        breaking-change: not a breaking change
        "#
        .trim();

        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();

        assert!(!commit.is_breaking_change);
        assert_eq!(
            commit.footer,
            vec![Footer {
                token: "breaking-change".to_string(),
                value: "not a breaking change".to_string(),
            }]
        );
    }

    #[test]
    fn fails_on_invalid_header() {
        let message = "invalid commit message format";
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()),
            Err(ParseError::InvalidHeader {
                header: "invalid commit message format".to_string()
            })
        );
    }

    #[test]
    fn fails_on_non_empty_line_after_header() {
        let message = "feat: add new feature\nThis line should not be here";
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()),
            Err(ParseError::MissingBlankLineAfterHeader)
        );
    }

    #[test]
    fn recognizes_footer_only_after_empty_line() {
        let message = r#"
        feat: add new feature

        This is the body.
        BREAKING CHANGE: major update
        "#
        .trim();

        let expected = ConventionalCommit {
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: false,
            description: "add new feature".to_string(),
            body: Some("This is the body.\nBREAKING CHANGE: major update".to_string()),
            footer: Vec::new(),
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    const TRAILING_PARAGRAPHS: ParserConfig = ParserConfig {
        footer_detection: FooterDetection::TrailingParagraphs,
    };

    fn footer(token: &str, value: &str) -> Footer {
        Footer {
            token: token.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn does_not_parse_body_paragraphs_as_footers() {
        let message = r#"
        feat: add new feature

        Intro.

        Note: this is prose.

        More body text.

        Refs: #1
        "#
        .trim();

        for config in [ParserConfig::default(), TRAILING_PARAGRAPHS] {
            let commit = parse_conventional_commit(message, &config).unwrap();
            assert_eq!(
                commit.body,
                Some("Intro.\n\nNote: this is prose.\n\nMore body text.".to_string())
            );
            assert_eq!(commit.footer, vec![footer("Refs", "#1")]);
        }
    }

    #[test]
    fn detects_only_last_paragraph_as_footer_by_default() {
        let message = "feat: add new feature\n\nBody.\n\nRefs: #1\n\nSigned-off-by: John Doe";

        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();

        assert_eq!(commit.body, Some("Body.\n\nRefs: #1".to_string()));
        assert_eq!(commit.footer, vec![footer("Signed-off-by", "John Doe")]);
    }

    #[test]
    fn detects_trailing_paragraphs_as_footers() {
        let message =
            "feat: add new feature\n\nBody.\n\n\nRefs: #1\n\n\nSigned-off-by: John Doe\n\n";

        let commit = parse_conventional_commit(message, &TRAILING_PARAGRAPHS).unwrap();

        assert_eq!(commit.body, Some("Body.".to_string()));
        assert_eq!(
            commit.footer,
            vec![footer("Refs", "#1"), footer("Signed-off-by", "John Doe")]
        );
    }

    #[test]
    fn ignores_trailing_empty_lines() {
        let message = "feat: add new feature\n\nBody.\n\nBREAKING CHANGE: major\nupdate\n\n\n";

        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();

        assert_eq!(commit.body, Some("Body.".to_string()));
        assert_eq!(
            commit.footer,
            vec![footer("BREAKING CHANGE", "major\nupdate")]
        );
        assert!(commit.is_breaking_change);
    }

    #[test]
    fn parses_footers_without_body() {
        let message = "feat: add new feature\n\nRefs: #1\n\nSigned-off-by: John Doe";

        let commit = parse_conventional_commit(message, &TRAILING_PARAGRAPHS).unwrap();

        assert_eq!(commit.body, None);
        assert_eq!(
            commit.footer,
            vec![footer("Refs", "#1"), footer("Signed-off-by", "John Doe")]
        );
    }
}
