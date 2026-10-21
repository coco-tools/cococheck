use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct AllowedTypes {
    pub allowed_types: Vec<String>,
}

impl Default for AllowedTypes {
    fn default() -> Self {
        let allowed_types = [
            "build", "chore", "ci", "deps", "docs", "feat", "fix", "perf", "refactor", "revert",
            "style", "test",
        ];

        AllowedTypes {
            allowed_types: allowed_types.iter().map(|t| t.to_string()).collect(),
        }
    }
}

impl Rule for AllowedTypes {
    fn name(&self) -> &str {
        "AllowedTypes"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        if !self.allowed_types.contains(&commit.commit_type) {
            vec![RuleError {
                message: format!(
                    "Commit type '{}' is not allowed. Allowed types are: {:?}.",
                    commit.commit_type, self.allowed_types
                ),
                span: Some(0..commit.commit_type.len()),
            }]
        } else {
            vec![]
        }
    }
}
