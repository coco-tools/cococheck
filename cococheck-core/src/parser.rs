use regex::regex;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// A parsed commit message. The header is always the first line of the message, so positions in
/// the header are also positions in the full message.
#[derive(Debug, PartialEq)]
pub struct ConventionalCommit {
    /// The full commit message, unchanged, so that rules can check what the parser normalizes,
    /// like trailing whitespace.
    pub message: String,
    pub header: String,
    pub commit_type: String,
    pub scope: Option<String>,
    /// Whether the commit is a breaking change, as marked by a `!` in the header or by a
    /// `BREAKING CHANGE` footer.
    pub is_breaking_change: bool,
    /// Whether the header contains the breaking change marker `!`.
    pub breaking_change_marker: bool,
    pub description: String,
    pub body: Option<String>,
    /// The lines of `body` with their positions in the full message. Like the lines of `body`, the
    /// paragraphs are separated by a single empty line, whose span is empty and at the end of the
    /// previous line.
    pub body_lines: Vec<Line>,
    pub footer: Vec<Footer>,
    /// The lines of the footers with their positions in the full message. Like in `body_lines`, the
    /// footer paragraphs are separated by a single empty line.
    pub footer_lines: Vec<Line>,
}

#[derive(Debug, PartialEq)]
pub struct Line {
    pub text: String,
    /// The byte range of the line in the full commit message.
    pub span: Range<usize>,
}

#[derive(Debug, PartialEq)]
pub struct Footer {
    pub token: String,
    pub value: String,
}

#[derive(Debug, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    #[error("invalid header: {problem}")]
    InvalidHeader {
        header: String,
        problem: HeaderProblem,
        /// The byte range in the full commit message that the problem refers to.
        span: Range<usize>,
    },

    #[error("the line after the header must be empty")]
    MissingBlankLineAfterHeader {
        /// The byte range of the line after the header in the full commit message.
        span: Range<usize>,
    },
}

/// The reason why a header does not have the form `type(scope)!: description`.
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum HeaderProblem {
    Empty,
    LeadingWhitespace,
    MissingType,
    InvalidTypeCharacter(char),
    UnclosedScope,
    EmptyScope,
    /// The header part after which `: ` was expected, but not found.
    MissingColon(HeaderPart),
    WhitespaceBeforeColon,
    MissingSpaceAfterColon,
    MissingDescription,
}

impl std::fmt::Display for HeaderProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeaderProblem::Empty => write!(f, "the header is empty"),
            HeaderProblem::LeadingWhitespace => {
                write!(f, "the header must not start with whitespace")
            }
            HeaderProblem::MissingType => write!(f, "the header must start with a type"),
            HeaderProblem::InvalidTypeCharacter(c) => write!(
                f,
                "the type must only contain letters, digits and underscores, but contains '{c}'"
            ),
            HeaderProblem::UnclosedScope => {
                write!(f, "the scope is missing a closing parenthesis")
            }
            HeaderProblem::EmptyScope => write!(f, "the scope must not be empty"),
            HeaderProblem::MissingColon(part) => write!(f, "expected ': ' after {part}"),
            HeaderProblem::WhitespaceBeforeColon => write!(f, "unexpected whitespace before ':'"),
            HeaderProblem::MissingSpaceAfterColon => write!(f, "expected a space after ':'"),
            HeaderProblem::MissingDescription => write!(f, "the description must not be empty"),
        }
    }
}

/// A part of the header before the `: ` separator.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HeaderPart {
    Type,
    Scope,
    BreakingChangeMarker,
}

impl std::fmt::Display for HeaderPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeaderPart::Type => write!(f, "the type"),
            HeaderPart::Scope => write!(f, "the scope"),
            HeaderPart::BreakingChangeMarker => write!(f, "'!'"),
        }
    }
}

#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ParserConfig {
    /// How the footer paragraphs at the end of the commit message are detected.
    #[serde(default)]
    pub footer_detection: FooterDetection,
}

#[derive(Debug, Default, Deserialize, PartialEq, Clone, Copy, Serialize)]
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

    // Only strip trailing whitespace, so that a header with leading whitespace is rejected.
    let header_line = lines.next().unwrap_or("").trim_end();
    let header_regex =
        regex!(r"^(?P<type>\w+)(\((?P<scope>[^)]+)\))?(?P<breaking>!)?: (?P<desc>.+)$");

    let Some(caps) = header_regex.captures(header_line) else {
        let (problem, span) = diagnose_header(header_line);
        return Err(ParseError::InvalidHeader {
            header: header_line.to_string(),
            problem,
            span,
        });
    };

    // The lines are slices of `message`, so their offsets in the message can be computed.
    let line_span = |line: &str| {
        let start = line.as_ptr() as usize - message.as_ptr() as usize;
        start..start + line.len()
    };

    if let Some(line) = lines.next().filter(|line| !line.trim().is_empty()) {
        return Err(ParseError::MissingBlankLineAfterHeader {
            span: line_span(line.trim_end()),
        });
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

    // Join the lines of the paragraphs, separated by a single empty line, whose span is empty and at
    // the end of the previous line.
    let paragraph_lines = |paragraphs: &[Vec<&str>]| {
        let mut lines: Vec<Line> = Vec::new();
        for paragraph in paragraphs {
            if let Some(previous) = lines.last() {
                let end = previous.span.end;
                lines.push(Line {
                    text: String::new(),
                    span: end..end,
                });
            }

            lines.extend(paragraph.iter().map(|&line| Line {
                text: line.to_string(),
                span: line_span(line),
            }));
        }
        lines
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
        message: message.to_string(),
        header: header_line.to_string(),
        commit_type: caps["type"].to_string(),
        scope: caps.name("scope").map(|m| m.as_str().to_string()),
        is_breaking_change: breaking_change_footer_found || caps.name("breaking").is_some(),
        breaking_change_marker: caps.name("breaking").is_some(),
        description: caps["desc"].to_string(),
        body,
        body_lines: paragraph_lines(body_paragraphs),
        footer: footers,
        footer_lines: paragraph_lines(footer_paragraphs),
    })
}

/// Find out why `header` does not match the header pattern, and return the problem with the byte
/// range in the header that it refers to. As the header is the first line of the message, the range
/// is also a range in the full message.
fn diagnose_header(header: &str) -> (HeaderProblem, Range<usize>) {
    let char_span = |pos: usize| pos..pos + header[pos..].chars().next().map_or(0, char::len_utf8);

    if header.is_empty() {
        return (HeaderProblem::Empty, 0..0);
    }

    let indent = header.len() - header.trim_start().len();
    if indent > 0 {
        return (HeaderProblem::LeadingWhitespace, 0..indent);
    }

    let mut pos = regex!(r"^\w*").find(header).map_or(0, |m| m.end());
    if pos == 0 {
        return (HeaderProblem::MissingType, char_span(0));
    }

    let mut previous = HeaderPart::Type;

    if header[pos..].starts_with('(') {
        let Some(close) = header[pos..].find(')') else {
            return (HeaderProblem::UnclosedScope, pos..header.len());
        };

        if close == 1 {
            return (HeaderProblem::EmptyScope, pos..pos + 2);
        }

        pos += close + 1;
        previous = HeaderPart::Scope;
    }

    if header[pos..].starts_with('!') {
        pos += 1;
        previous = HeaderPart::BreakingChangeMarker;
    }

    let rest = &header[pos..];

    if let Some(after_colon) = rest.strip_prefix(':') {
        // A colon followed by a space and a description matches the header pattern, so if there is
        // a description, the space is missing.
        let problem = if after_colon.trim().is_empty() {
            HeaderProblem::MissingDescription
        } else {
            HeaderProblem::MissingSpaceAfterColon
        };
        return (problem, pos..pos + 1);
    }

    let whitespace = rest.len() - rest.trim_start().len();
    if whitespace > 0 && rest[whitespace..].starts_with(':') {
        return (HeaderProblem::WhitespaceBeforeColon, pos..pos + whitespace);
    }

    match rest.chars().next() {
        Some(c) if previous == HeaderPart::Type && !c.is_whitespace() => {
            (HeaderProblem::InvalidTypeCharacter(c), char_span(pos))
        }
        _ => (HeaderProblem::MissingColon(previous), char_span(pos)),
    }
}

/// Split the lines into paragraphs separated by one or more empty lines. Trailing whitespace is
/// removed from the lines, but leading whitespace is kept to preserve indentation.
fn split_paragraphs<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<Vec<&'a str>> {
    let mut paragraphs: Vec<Vec<&str>> = Vec::new();
    let mut last_line_empty = true;

    for line in lines.map(str::trim_end) {
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

    // A bracketed token like `[1]` is a link reference, which is followed by `: `. With ` #`, it is
    // more likely an item of a numbered list, like `[1] #123`.
    let caps = footer_regex
        .captures(line)
        .filter(|caps| !(caps["token"].starts_with('[') && &caps["sep"] == " #"));

    caps.map(|caps| {
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

    /// Remove the leading line break and the common indentation of all non-empty lines, so that
    /// commit messages can be written as indented raw strings.
    fn dedent(text: &str) -> String {
        let text = text.strip_prefix('\n').unwrap_or(text);
        let indent = text
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| line.len() - line.trim_start().len())
            .min()
            .unwrap_or(0);

        text.lines()
            .map(|line| line.get(indent..).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn parses_header_without_scope() {
        let message = "feat: add new feature";
        let expected = ConventionalCommit {
            message: message.to_string(),
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: false,
            breaking_change_marker: false,
            description: "add new feature".to_string(),
            body: None,
            body_lines: vec![],
            footer: Vec::new(),
            footer_lines: vec![],
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
            message: message.to_string(),
            header: "fix(core): correct minor issue".to_string(),
            commit_type: "fix".to_string(),
            scope: Some("core".to_string()),
            is_breaking_change: false,
            breaking_change_marker: false,
            description: "correct minor issue".to_string(),
            body: None,
            body_lines: vec![],
            footer: Vec::new(),
            footer_lines: vec![],
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
            message: message.to_string(),
            header: "feat!: introduce breaking change".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: true,
            breaking_change_marker: true,
            description: "introduce breaking change".to_string(),
            body: None,
            body_lines: vec![],
            footer: Vec::new(),
            footer_lines: vec![],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_body_and_footer() {
        let message = &dedent(
            r#"
        fix: resolve issue

        This is the body of the commit.
        This is the second line of the body.

        This is the third line of the body.

        BREAKING CHANGE: major update
        Signed-off-by: John Doe <john.doe@example.org>
        Fixes #123
        "#,
        );
        let expected = ConventionalCommit {
            message: message.to_string(),
            header: "fix: resolve issue".to_string(),
            commit_type: "fix".to_string(),
            scope: None,
            is_breaking_change: true,
            breaking_change_marker: false,
            description: "resolve issue".to_string(),
            body: Some("This is the body of the commit.\nThis is the second line of the body.\n\nThis is the third line of the body.".to_string()),
            body_lines: vec![line("This is the body of the commit.", 20..51), line("This is the second line of the body.", 52..88), line("", 88..88), line("This is the third line of the body.", 90..125)],
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
            footer_lines: vec![
                line("BREAKING CHANGE: major update", 127..156),
                line("Signed-off-by: John Doe <john.doe@example.org>", 157..203),
                line("Fixes #123", 204..214),
            ],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_footers_with_multiple_lines() {
        let message = &dedent(
            r#"
        feat: add new feature

        This is the body.

        BREAKING CHANGE: This is a long breaking change description
        that spans multiple lines and provides detailed information
        about the change.
        Signed-off-by: John Doe <john.doe@example.org>
        "#,
        );

        let expected = ConventionalCommit {
            message: message.to_string(),
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: true,
            breaking_change_marker: false,
            description: "add new feature".to_string(),
            body: Some("This is the body.".to_string()),
            body_lines: vec![line("This is the body.", 23..40)],
            footer: vec![Footer {
                token: "BREAKING CHANGE".to_string(),
                value: "This is a long breaking change description\nthat spans multiple lines and provides detailed information\nabout the change.".to_string(),
            }, Footer {
                token: "Signed-off-by".to_string(),
                value: "John Doe <john.doe@example.org>".to_string()
            }],
            footer_lines: vec![
                line("BREAKING CHANGE: This is a long breaking change description", 42..101),
                line("that spans multiple lines and provides detailed information", 102..161),
                line("about the change.", 162..179),
                line("Signed-off-by: John Doe <john.doe@example.org>", 180..226),
            ],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_footers_with_links() {
        let message = &dedent(
            r#"
        feat: add new feature

        This [1] is the body [2].

        [1]: https://example.org/issue/123
        [2]: https://example.org/issue/456
        "#,
        );

        let expected = ConventionalCommit {
            message: message.to_string(),
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: false,
            breaking_change_marker: false,
            description: "add new feature".to_string(),
            body: Some("This [1] is the body [2].".to_string()),
            body_lines: vec![line("This [1] is the body [2].", 23..48)],
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
            footer_lines: vec![
                line("[1]: https://example.org/issue/123", 50..84),
                line("[2]: https://example.org/issue/456", 85..119),
            ],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn parses_footers_with_footnotes() {
        let message = &dedent(
            r#"
        feat: add new feature

        This[^1] is the body[^2].

        [^1]: https://example.org/issue/123
        [^2]: https://example.org/issue/456
        "#,
        );

        let expected = ConventionalCommit {
            message: message.to_string(),
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: false,
            breaking_change_marker: false,
            description: "add new feature".to_string(),
            body: Some("This[^1] is the body[^2].".to_string()),
            body_lines: vec![line("This[^1] is the body[^2].", 23..48)],
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
            footer_lines: vec![
                line("[^1]: https://example.org/issue/123", 50..85),
                line("[^2]: https://example.org/issue/456", 86..121),
            ],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn detects_breaking_change_in_footer() {
        let message = &dedent(
            r#"
        feat: add new feature

        This is the body.

        BREAKING CHANGE: major update
        "#,
        );

        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default())
                .unwrap()
                .is_breaking_change,
            true
        );

        let message = &dedent(
            r#"
        feat: add new feature

        This is the body.

        BREAKING-CHANGE: major update
        "#,
        );

        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default())
                .unwrap()
                .is_breaking_change,
            true
        );
    }

    #[test]
    fn ignores_lowercase_breaking_change_footer() {
        let message = &dedent(
            r#"
        feat: add new feature

        breaking-change: not a breaking change
        "#,
        );

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
                header: "invalid commit message format".to_string(),
                problem: HeaderProblem::MissingColon(HeaderPart::Type),
                span: 7..8,
            })
        );
    }

    /// Return the problem of the invalid `header` and the text that its span refers to.
    fn header_problem(header: &str) -> (HeaderProblem, &str) {
        match parse_conventional_commit(header, &ParserConfig::default()) {
            Err(ParseError::InvalidHeader { problem, span, .. }) => (problem, &header[span]),
            result => panic!("expected an invalid header, got {result:?}"),
        }
    }

    #[test]
    fn explains_why_a_header_is_invalid() {
        use HeaderProblem::*;

        let cases = [
            ("", Empty, ""),
            ("Upgrade Kotlin to 1.8", MissingColon(HeaderPart::Type), " "),
            ("Upgrade", MissingColon(HeaderPart::Type), ""),
            (": add new feature", MissingType, ":"),
            ("(core): add new feature", MissingType, "("),
            (
                "docker-compose: add new feature",
                InvalidTypeCharacter('-'),
                "-",
            ),
            ("README.md: add new feature", InvalidTypeCharacter('.'), "."),
            ("feat(): add new feature", EmptyScope, "()"),
            (
                "feat(core: add new feature",
                UnclosedScope,
                "(core: add new feature",
            ),
            (
                "fix(core)): add new feature",
                MissingColon(HeaderPart::Scope),
                ")",
            ),
            (
                "feat(core) add new feature",
                MissingColon(HeaderPart::Scope),
                " ",
            ),
            (
                "feat! add new feature",
                MissingColon(HeaderPart::BreakingChangeMarker),
                " ",
            ),
            (
                "feat!(core): add new feature",
                MissingColon(HeaderPart::BreakingChangeMarker),
                "(",
            ),
            ("dao : add new feature", WhitespaceBeforeColon, " "),
            (
                "feat(core) \t: add new feature",
                WhitespaceBeforeColon,
                " \t",
            ),
            ("feat:add new feature", MissingSpaceAfterColon, ":"),
            ("feat(core)!:add new feature", MissingSpaceAfterColon, ":"),
            ("feat:", MissingDescription, ":"),
            ("feat(core):   ", MissingDescription, ":"),
        ];

        for (header, problem, text) in cases {
            assert_eq!(
                header_problem(header),
                (problem, text),
                "header: {header:?}"
            );
        }
    }

    #[test]
    fn reports_spans_of_multi_byte_characters_in_headers() {
        assert_eq!(
            header_problem("feat→: add new feature"),
            (HeaderProblem::InvalidTypeCharacter('→'), "→")
        );
    }

    #[test]
    fn displays_the_header_problem() {
        let error = parse_conventional_commit("feat(): add", &ParserConfig::default()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid header: the scope must not be empty"
        );
    }

    #[test]
    fn fails_on_header_with_leading_whitespace() {
        let message = "  feat: add new feature";
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()),
            Err(ParseError::InvalidHeader {
                header: "  feat: add new feature".to_string(),
                problem: HeaderProblem::LeadingWhitespace,
                span: 0..2,
            })
        );
    }

    #[test]
    fn ignores_trailing_whitespace_in_header() {
        let message = "feat: add new feature  \r\n";
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();

        assert_eq!(commit.header, "feat: add new feature");
        assert_eq!(commit.description, "add new feature");
    }

    #[test]
    fn records_spans_of_body_lines_in_message() {
        let message = "feat: x\r\n\r\nFirst line  \r\n    indented\r\n\r\n\r\nLast";

        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();

        assert_eq!(
            commit.body_lines,
            vec![
                line("First line", 11..21),
                line("    indented", 25..37),
                line("", 37..37),
                line("Last", 43..47),
            ]
        );
        for line in commit.body_lines {
            assert_eq!(&message[line.span], line.text);
        }
    }

    #[test]
    fn records_spans_of_footer_lines_in_message() {
        let message =
            "feat: x\r\n\r\nBody.\r\n\r\nRefs: #1  \r\n  continued\r\n\r\n\r\nFixes #2\r\n";

        let commit = parse_conventional_commit(message, &TRAILING_PARAGRAPHS).unwrap();

        assert_eq!(
            commit.footer_lines,
            vec![
                line("Refs: #1", 20..28),
                line("  continued", 32..43),
                line("", 43..43),
                line("Fixes #2", 49..57),
            ]
        );
        for line in commit.footer_lines {
            assert_eq!(&message[line.span], line.text);
        }
    }

    #[test]
    fn preserves_indentation_in_body() {
        let message = dedent(
            r#"
            feat: add new feature

            Run the following command:

                cargo run -- --help
                  --verbose

            BREAKING CHANGE: the output changed
              and is indented
            "#,
        );

        let commit = parse_conventional_commit(&message, &ParserConfig::default()).unwrap();

        assert_eq!(
            commit.body,
            Some(
                "Run the following command:\n\n    cargo run -- --help\n      --verbose"
                    .to_string()
            )
        );
        assert_eq!(
            commit.footer,
            vec![footer(
                "BREAKING CHANGE",
                "the output changed\n  and is indented"
            )]
        );
    }

    #[test]
    fn removes_line_endings_and_trailing_whitespace_from_body() {
        let message = "feat: add new feature\r\n\r\n  Body. \r\n\tIndented.\t\r\n";

        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();

        assert_eq!(commit.body, Some("  Body.\n\tIndented.".to_string()));
    }

    #[test]
    fn fails_on_non_empty_line_after_header() {
        let message = "feat: add new feature\nThis line should not be here";
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()),
            Err(ParseError::MissingBlankLineAfterHeader { span: 22..50 })
        );
        assert_eq!(&message[22..50], "This line should not be here");
    }

    #[test]
    fn recognizes_footer_only_after_empty_line() {
        let message = &dedent(
            r#"
        feat: add new feature

        This is the body.
        BREAKING CHANGE: major update
        "#,
        );

        let expected = ConventionalCommit {
            message: message.to_string(),
            header: "feat: add new feature".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            is_breaking_change: false,
            breaking_change_marker: false,
            description: "add new feature".to_string(),
            body: Some("This is the body.\nBREAKING CHANGE: major update".to_string()),
            body_lines: vec![
                line("This is the body.", 23..40),
                line("BREAKING CHANGE: major update", 41..70),
            ],
            footer: Vec::new(),
            footer_lines: vec![],
        };
        assert_eq!(
            parse_conventional_commit(message, &ParserConfig::default()).unwrap(),
            expected
        );
    }

    const TRAILING_PARAGRAPHS: ParserConfig = ParserConfig {
        footer_detection: FooterDetection::TrailingParagraphs,
    };

    fn line(text: &str, span: Range<usize>) -> Line {
        Line {
            text: text.to_string(),
            span,
        }
    }

    fn footer(token: &str, value: &str) -> Footer {
        Footer {
            token: token.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn does_not_parse_body_paragraphs_as_footers() {
        let message = &dedent(
            r#"
        feat: add new feature

        Intro.

        Note: this is prose.

        More body text.

        Refs: #1
        "#,
        );

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

    #[test]
    fn parses_bracketed_footer_tokens_only_with_colon() {
        assert_eq!(
            parse_footer_line("[1]: https://example.org"),
            Some(("[1]".to_string(), "https://example.org".to_string()))
        );
        assert_eq!(parse_footer_line("[5] #5560."), None);
        assert_eq!(
            parse_footer_line("Fixes #5560."),
            Some(("Fixes".to_string(), "#5560.".to_string()))
        );
    }

    #[test]
    fn parses_numbered_list_with_issue_references_as_body() {
        let message = "feat: add new feature\n\nBody.\n\n[1] #123\n[2] #456";

        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();

        assert_eq!(commit.body.as_deref(), Some("Body.\n\n[1] #123\n[2] #456"));
        assert_eq!(commit.footer, vec![]);
    }
}
