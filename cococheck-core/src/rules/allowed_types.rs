use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct AllowedTypes {
    pub types: Vec<String>,
}

impl Default for AllowedTypes {
    fn default() -> Self {
        let types = [
            "build", "chore", "ci", "deps", "docs", "feat", "fix", "perf", "refactor", "revert",
            "style", "test",
        ];

        AllowedTypes {
            types: types.iter().map(|t| t.to_string()).collect(),
        }
    }
}

impl Rule for AllowedTypes {
    fn name(&self) -> &str {
        "allowed_types"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        if !self.types.contains(&commit.commit_type) {
            vec![RuleError {
                message: format!(
                    "Commit type '{}' is not allowed. Allowed types are: {:?}.",
                    commit.commit_type, self.types
                ),
                span: Some(0..commit.commit_type.len()),
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

    fn check(message: &str) -> Vec<RuleError> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        AllowedTypes::default().check(&commit)
    }

    #[test]
    fn succeeds_for_allowed_type() {
        assert!(check("feat: add new feature").is_empty());
    }

    #[test]
    fn fails_for_type_that_is_not_allowed() {
        let errors = check("wip(core): add new feature\n\nBody.");

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].span, Some(0..3));
    }
}
