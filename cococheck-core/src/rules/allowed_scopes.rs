use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError, scope_span};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct AllowedScopes {
    /// The allowed scopes. If it is empty, any scope is allowed.
    pub scopes: Vec<String>,
}

impl Rule for AllowedScopes {
    fn name(&self) -> &str {
        "allowed_scopes"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        let (Some(scope), Some(span)) = (&commit.scope, scope_span(commit)) else {
            return vec![];
        };

        if self.scopes.is_empty() {
            return vec![];
        }

        // A scope can name multiple scopes separated by commas, like `core, cli`.
        let mut errors = Vec::new();
        let mut part_start = span.start;
        for part in scope.split(',') {
            let trimmed = part.trim();
            if !self.scopes.iter().any(|allowed| allowed == trimmed) {
                let start = part_start + (part.len() - part.trim_start().len());
                errors.push(RuleError {
                    message: format!(
                        "Scope '{}' is not allowed. Allowed scopes are: {:?}.",
                        trimmed, self.scopes
                    ),
                    span: Some(start..start + trimmed.len()),
                });
            }
            part_start += part.len() + 1;
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    fn check(message: &str, scopes: &[&str]) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        let rule = AllowedScopes {
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
        };
        rule.check(&commit)
    }

    #[test]
    fn succeeds_for_any_scope_by_default() {
        assert!(check("feat(anything): Add a feature", &[]).is_empty());
    }

    #[test]
    fn succeeds_without_scope() {
        assert!(check("feat: Add a feature", &["core"]).is_empty());
    }

    #[test]
    fn succeeds_for_allowed_scope() {
        assert!(check("feat(core): Add a feature", &["cli", "core"]).is_empty());
    }

    #[test]
    fn succeeds_for_multiple_allowed_scopes() {
        assert!(check("feat(cli, core): Add a feature", &["cli", "core"]).is_empty());
    }

    #[test]
    fn fails_for_scope_that_is_not_allowed() {
        assert_eq!(
            check("feat(api): Add a feature", &["cli", "core"]),
            vec![RuleError {
                message: "Scope 'api' is not allowed. Allowed scopes are: [\"cli\", \"core\"]."
                    .to_string(),
                span: Some(5..8),
            }]
        );
    }

    #[test]
    fn fails_for_each_scope_that_is_not_allowed() {
        let message = "feat(api,core, web): Add a feature";
        let errors = check(message, &["core"]);

        let marked: Vec<&str> = errors
            .iter()
            .map(|e| &message[e.span.clone().unwrap()])
            .collect();
        assert_eq!(marked, vec!["api", "web"]);
    }
}
