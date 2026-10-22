use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError, scope_span};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScopePresence {
    pub scope: Presence,
}

/// Whether a commit must have a scope.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    /// A commit can have a scope, but does not have to.
    #[default]
    Optional,

    /// A commit must have a scope.
    Required,

    /// A commit must not have a scope.
    Forbidden,
}

impl Rule for ScopePresence {
    fn name(&self) -> &str {
        "ScopePresence"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        match (self.scope, scope_span(commit)) {
            (Presence::Required, None) => vec![RuleError {
                message: "Commit must have a scope.".to_string(),
                span: Some(0..commit.commit_type.len()),
            }],
            (Presence::Forbidden, Some(span)) => vec![RuleError {
                message: "Commit must not have a scope.".to_string(),
                // Include the parentheses around the scope.
                span: Some(span.start - 1..span.end + 1),
            }],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(message: &str, scope: Presence) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        ScopePresence { scope }.check(&commit)
    }

    #[test]
    fn succeeds_with_and_without_scope_by_default() {
        assert_eq!(ScopePresence::default().scope, Presence::Optional);
        assert!(check("feat: Add a feature", Presence::Optional).is_empty());
        assert!(check("feat(core): Add a feature", Presence::Optional).is_empty());
    }

    #[test]
    fn succeeds_with_required_scope() {
        assert!(check("feat(core): Add a feature", Presence::Required).is_empty());
    }

    #[test]
    fn fails_without_required_scope() {
        assert_eq!(
            check("feat!: Add a feature", Presence::Required),
            vec![RuleError {
                message: "Commit must have a scope.".to_string(),
                span: Some(0..4),
            }]
        );
    }

    #[test]
    fn succeeds_without_forbidden_scope() {
        assert!(check("feat: Add a feature", Presence::Forbidden).is_empty());
    }

    #[test]
    fn fails_with_forbidden_scope() {
        assert_eq!(
            check("feat(core)!: Add a feature", Presence::Forbidden),
            vec![RuleError {
                message: "Commit must not have a scope.".to_string(),
                span: Some(4..10),
            }]
        );
    }
}
