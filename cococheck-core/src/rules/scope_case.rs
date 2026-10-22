use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError, scope_span};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct ScopeCase {
    pub case: ScopeCaseMode,
}

/// The required case of the scope.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeCaseMode {
    /// The scope must not contain uppercase letters, like `core`.
    #[default]
    Lower,

    /// The scope can contain letters of any case.
    Any,
}

impl Rule for ScopeCase {
    fn name(&self) -> &str {
        "ScopeCase"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let (Some(scope), Some(span)) = (&commit.scope, scope_span(commit)) else {
            return vec![];
        };

        if self.case == ScopeCaseMode::Lower && *scope != scope.to_lowercase() {
            vec![RuleError {
                message: format!("Scope '{scope}' must be lowercase."),
                span: Some(span),
            }]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(message: &str, case: ScopeCaseMode) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        ScopeCase { case }.check(&commit)
    }

    #[test]
    fn requires_lowercase_by_default() {
        assert_eq!(ScopeCase::default().case, ScopeCaseMode::Lower);
    }

    #[test]
    fn succeeds_without_scope() {
        assert!(check("feat: Add a feature", ScopeCaseMode::Lower).is_empty());
    }

    #[test]
    fn succeeds_for_lowercase_scope() {
        assert!(check("feat(core-api, cli): Add a feature", ScopeCaseMode::Lower).is_empty());
    }

    #[test]
    fn fails_for_scope_that_is_not_lowercase() {
        assert_eq!(
            check("feat(Core): Add a feature", ScopeCaseMode::Lower),
            vec![RuleError {
                message: "Scope 'Core' must be lowercase.".to_string(),
                span: Some(5..9),
            }]
        );
    }

    #[test]
    fn succeeds_for_any_case_in_any_mode() {
        assert!(check("feat(Core): Add a feature", ScopeCaseMode::Any).is_empty());
    }
}
