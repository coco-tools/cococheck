use regex::Regex;
use std::fmt;

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

#[derive(Debug, PartialEq)]
pub enum ParseError {
    InvalidFormat(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
        }
    }
}

pub fn parse_conventional_commit(message: &str) -> Result<ConventionalCommit, ParseError> {
    let mut lines = message.lines();

    let header_line = lines.next().unwrap_or("").trim();
    let header_regex =
        Regex::new(r"^(?P<type>\w+)(\((?P<scope>[^)]+)\))?(?P<breaking>!)?: (?P<desc>.+)$")
            .unwrap();

    let caps = match header_regex.captures(header_line) {
        Some(caps) => caps,
        None => {
            return Err(ParseError::InvalidFormat(
                "Cannot parse header line".to_string(),
            ));
        }
    };

    let next_line = lines.next();
    if let Some(line) = next_line {
        if !line.trim().is_empty() {
            return Err(ParseError::InvalidFormat(
                "The line after the header must be empty".to_string(),
            ));
        }
    } else {
        return Ok(ConventionalCommit {
            header: header_line.to_string(),
            commit_type: caps["type"].to_string(),
            scope: caps.name("scope").map(|m| m.as_str().to_string()),
            is_breaking_change: caps.name("breaking").is_some(),
            description: caps["desc"].to_string(),
            body: None,
            footer: Vec::new(),
        });
    }

    let mut body_lines = Vec::new();
    let mut footer_lines = Vec::new();
    let mut footer_started = false;
    let mut last_line_empty = true;
    let mut breaking_change_footer_found = false;

    for line in lines {
        let line = line.trim();
        if !footer_started && last_line_empty && parse_footer_line(line).is_some() {
            footer_started = true;
        }

        if footer_started {
            footer_lines.push(line.to_string());
        } else {
            body_lines.push(line.to_string());
        }

        last_line_empty = line.is_empty();
    }

    if body_lines.last().is_some_and(|last| last.is_empty()) {
        body_lines.pop();
    }

    let body = if body_lines.is_empty() {
        None
    } else {
        Some(body_lines.join("\n"))
    };

    let mut footers = Vec::new();

    let mut current_token: Option<String> = None;
    let mut current_value: Option<String> = None;

    for line in footer_lines {
        if let Some((token, value)) = parse_footer_line(&line) {
            if let (Some(token), Some(value)) = (current_token.take(), current_value.take()) {
                footers.push(Footer { token, value });
            }

            if token.eq_ignore_ascii_case("BREAKING CHANGE")
                || token.eq_ignore_ascii_case("BREAKING-CHANGE")
            {
                breaking_change_footer_found = true;
            }

            current_token = Some(token);
            current_value = Some(value);
        } else {
            if let Some(ref mut s) = current_value {
                s.push('\n');
                s.push_str(&line);
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

pub fn parse_footer_line(line: &str) -> Option<(String, String)> {
    let re = Regex::new(r"^(?P<token>BREAKING CHANGE|[\p{L}\p{N}-]+)(?P<sep>: | #)(?P<value>.+)$")
        .unwrap();

    re.captures(line).map(|caps| {
        let token = caps.name("token").unwrap().as_str().to_string();
        let sep = caps.name("sep").unwrap().as_str();
        let value = caps.name("value").unwrap().as_str();
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
        assert_eq!(parse_conventional_commit(message).unwrap(), expected);
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
        assert_eq!(parse_conventional_commit(message).unwrap(), expected);
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
        assert_eq!(parse_conventional_commit(message).unwrap(), expected);
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
        assert_eq!(parse_conventional_commit(message).unwrap(), expected);
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
        assert_eq!(parse_conventional_commit(message).unwrap(), expected);
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
            parse_conventional_commit(message)
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
            parse_conventional_commit(message)
                .unwrap()
                .is_breaking_change,
            true
        );
    }

    #[test]
    fn fails_on_invalid_header() {
        let message = "invalid commit message format";
        let result = parse_conventional_commit(message);
        match result {
            Err(ParseError::InvalidFormat(msg)) => {
                assert_eq!(msg, "Cannot parse header line");
            }
            _ => panic!("Expected an InvalidFormat error"),
        }
    }

    #[test]
    fn fails_on_non_empty_line_after_header() {
        let message = "feat: add new feature\nThis line should not be here";
        let result = parse_conventional_commit(message);
        match result {
            Err(ParseError::InvalidFormat(msg)) => {
                assert_eq!(msg, "The line after the header must be empty");
            }
            _ => panic!("Expected an InvalidFormat error"),
        }
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
        assert_eq!(parse_conventional_commit(message).unwrap(), expected);
    }
}
