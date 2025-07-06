use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AllowedTypes {
    #[serde(default = "AllowedTypes::default_enabled")]
    pub enabled: bool,
    #[serde(default = "AllowedTypes::default_allowed_types")]
    pub allowed_types: Vec<String>,
}

impl AllowedTypes {
    fn default_enabled() -> bool {
        true
    }

    fn default_allowed_types() -> Vec<String> {
        vec![
            "build".to_string(),
            "chore".to_string(),
            "ci".to_string(),
            "deps".to_string(),
            "docs".to_string(),
            "feat".to_string(),
            "fix".to_string(),
            "perf".to_string(),
            "refactor".to_string(),
            "revert".to_string(),
            "style".to_string(),
            "test".to_string(),
        ]
    }
}

impl Default for AllowedTypes {
    fn default() -> Self {
        AllowedTypes {
            enabled: Self::default_enabled(),
            allowed_types: Self::default_allowed_types(),
        }
    }
}

impl Rule for AllowedTypes {
    fn name(&self) -> &str {
        "AllowedTypes"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn check(&self, commit: &ConventionalCommit) -> Result<(), Vec<RuleError>> {
        if !self.allowed_types.contains(&commit.commit_type) {
            Err(vec![RuleError {
                message: format!(
                    "Commit type '{}' is not allowed. Allowed types are: {:?}.",
                    commit.commit_type, self.allowed_types
                ),
                span: Some(0..commit.commit_type.len()),
            }])
        } else {
            Ok(())
        }
    }
}
