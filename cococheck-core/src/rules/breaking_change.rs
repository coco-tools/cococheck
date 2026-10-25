use crate::parser::{ConventionalCommit, parse_footer_line};
use crate::rules::{Rule, RuleError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct BreakingChange {
    /// Whether the breaking change marker `!` in the header requires a `BREAKING CHANGE` footer that
    /// describes the change.
    pub footer_required: bool,
    /// Whether a `BREAKING CHANGE` footer requires the breaking change marker `!` in the header, so
    /// that breaking changes are visible in one-line logs.
    pub marker_required: bool,
}

impl Default for BreakingChange {
    fn default() -> Self {
        BreakingChange {
            footer_required: false,
            marker_required: true,
        }
    }
}

impl Rule for BreakingChange {
    fn name(&self) -> &str {
        "breaking_change"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        // A line of the footers that is a footer line starts a footer, as all other lines are
        // continuations of the values.
        let footer = commit.footer_lines.iter().find_map(|line| {
            parse_footer_line(&line.text)
                .filter(|(token, _)| token == "BREAKING CHANGE" || token == "BREAKING-CHANGE")
                .map(|(token, _)| line.span.start..line.span.start + token.len())
        });

        match footer {
            None if self.footer_required && commit.breaking_change_marker => {
                // The marker is right before the `: ` that separates the description.
                let marker = commit.header.len() - commit.description.len() - 3;
                vec![RuleError {
                    message: "The breaking change marker '!' requires a 'BREAKING CHANGE' footer \
                        that describes the change."
                        .to_string(),
                    span: Some(marker..marker + 1),
                }]
            }
            Some(span) if self.marker_required && !commit.breaking_change_marker => {
                vec![RuleError {
                    message:
                        "The 'BREAKING CHANGE' footer requires the breaking change marker '!' \
                        in the header."
                            .to_string(),
                    span: Some(span),
                }]
            }
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    const BOTH: BreakingChange = BreakingChange {
        footer_required: true,
        marker_required: true,
    };

    fn check(rule: &BreakingChange, message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        rule.check(&commit)
    }

    #[test]
    fn requires_only_the_marker_by_default() {
        let rule = BreakingChange::default();
        assert!(check(&rule, "feat!: Remove a feature").is_empty());
        assert_eq!(
            check(
                &rule,
                "feat: Remove a feature\n\nBREAKING CHANGE: It is gone."
            )
            .len(),
            1
        );
    }

    #[test]
    fn passes_every_commit_if_both_settings_are_disabled() {
        let rule = BreakingChange {
            footer_required: false,
            marker_required: false,
        };
        assert!(check(&rule, "feat!: Remove a feature").is_empty());
        assert!(
            check(
                &rule,
                "feat: Remove a feature\n\nBREAKING CHANGE: It is gone."
            )
            .is_empty()
        );
    }

    #[test]
    fn passes_consistent_commits() {
        for message in [
            "feat: Add a feature",
            "feat(api)!: Remove a feature\n\nBody.\n\nBREAKING CHANGE: It is gone.",
            "feat!: Remove a feature\n\nRefs: #1\nBREAKING-CHANGE: It is gone.",
        ] {
            assert!(check(&BOTH, message).is_empty(), "{message}");
        }
    }

    #[test]
    fn reports_a_marker_without_a_footer() {
        let message = "feat(api)!: Remove a feature\n\nBody.\n\nRefs: #1";
        let rule = BreakingChange {
            footer_required: true,
            marker_required: false,
        };

        let errors = check(&rule, message);

        assert_eq!(
            errors,
            vec![RuleError {
                message: "The breaking change marker '!' requires a 'BREAKING CHANGE' footer that \
                    describes the change."
                    .to_string(),
                span: Some(9..10),
            }]
        );
        assert_eq!(&message[9..10], "!");
    }

    #[test]
    fn does_not_accept_a_breaking_change_in_the_body() {
        // The footer is not separated from the body by a blank line, so it is part of the body.
        let message = "feat!: Remove a feature\n\nBody.\nBREAKING CHANGE: It is gone.";
        assert_eq!(check(&BOTH, message).len(), 1);
    }

    #[test]
    fn reports_a_footer_without_a_marker() {
        let message = "feat: Remove a feature\n\nRefs: #1\nBREAKING-CHANGE: It is\ngone.";
        let rule = BreakingChange::default();

        let errors = check(&rule, message);

        assert_eq!(
            errors,
            vec![RuleError {
                message: "The 'BREAKING CHANGE' footer requires the breaking change marker '!' in \
                    the header."
                    .to_string(),
                span: Some(33..48),
            }]
        );
        assert_eq!(&message[33..48], "BREAKING-CHANGE");
    }
}
