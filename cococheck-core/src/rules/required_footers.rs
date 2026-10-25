use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct RequiredFooters {
    /// The tokens of the footers that every commit must have.
    pub tokens: Vec<String>,
}

impl Rule for RequiredFooters {
    fn name(&self) -> &str {
        "required_footers"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        self.tokens
            .iter()
            .filter(|token| {
                !commit
                    .footer
                    .iter()
                    .any(|footer| tokens_match(&footer.token, token))
            })
            .map(|token| RuleError {
                message: format!("The footer '{token}' is required, but missing."),
                span: None,
            })
            .collect()
    }
}

/// Return whether the footer tokens `a` and `b` are equal. The Conventional Commits specification
/// requires tokens to be case-insensitive, except for `BREAKING CHANGE`, which must be uppercase.
fn tokens_match(a: &str, b: &str) -> bool {
    let is_breaking_change = |token: &str| token == "BREAKING CHANGE" || token == "BREAKING-CHANGE";

    if is_breaking_change(a) || is_breaking_change(b) {
        a == b
    } else {
        a.to_lowercase() == b.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(tokens: &[&str], message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        let rule = RequiredFooters {
            tokens: tokens.iter().map(|token| token.to_string()).collect(),
        };
        rule.check(&commit)
    }

    fn missing(token: &str) -> RuleError {
        RuleError {
            message: format!("The footer '{token}' is required, but missing."),
            span: None,
        }
    }

    #[test]
    fn succeeds_without_required_tokens() {
        assert!(check(&[], "feat: add new feature").is_empty());
    }

    #[test]
    fn succeeds_for_present_footer() {
        let message = "feat: add new feature\n\nBody.\n\nRefs: #1\nSigned-off-by: Jane Doe";
        assert!(check(&["Signed-off-by"], message).is_empty());
    }

    #[test]
    fn reports_missing_footer() {
        let message = "feat: add new feature\n\nBody.\n\nRefs: #1";
        assert_eq!(
            check(&["Signed-off-by"], message),
            vec![missing("Signed-off-by")]
        );
    }

    #[test]
    fn reports_every_missing_footer() {
        let message = "feat: add new feature\n\nRefs: #1";
        assert_eq!(
            check(&["Signed-off-by", "Refs", "Reviewed-by"], message),
            vec![missing("Signed-off-by"), missing("Reviewed-by")]
        );
    }

    #[test]
    fn reports_footer_that_is_only_in_the_body() {
        // The footer is not separated from the body by a blank line, so it is part of the body.
        let message = "feat: add new feature\n\nBody.\nSigned-off-by: Jane Doe";
        assert_eq!(
            check(&["Signed-off-by"], message),
            vec![missing("Signed-off-by")]
        );
    }

    #[test]
    fn matches_tokens_case_insensitively() {
        let message = "feat: add new feature\n\nSigned-Off-By: Jane Doe";
        assert!(check(&["Signed-off-by"], message).is_empty());
    }

    #[test]
    fn matches_breaking_change_case_sensitively() {
        let message = "feat: add new feature\n\nbreaking-change: not a breaking change";
        assert_eq!(
            check(&["BREAKING-CHANGE"], message),
            vec![missing("BREAKING-CHANGE")]
        );
    }
}
