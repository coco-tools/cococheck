use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct NoDescriptionTerminalPunctuation {
    #[serde(default = "NoDescriptionTerminalPunctuation::default_enabled")]
    pub enabled: bool,
}

impl NoDescriptionTerminalPunctuation {
    fn default_enabled() -> bool {
        true
    }
}

impl Default for NoDescriptionTerminalPunctuation {
    fn default() -> Self {
        NoDescriptionTerminalPunctuation {
            enabled: Self::default_enabled(),
        }
    }
}

impl Rule for NoDescriptionTerminalPunctuation {
    fn name(&self) -> &str {
        "NoDescriptionTerminalPunctuation"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn check(&self, commit: &ConventionalCommit) -> Result<(), Vec<RuleError>> {
        if commit.description.ends_with('.')
            || commit.description.ends_with('!')
            || commit.description.ends_with('?')
        {
            Err(vec![RuleError {
                message: "Description should not end with terminal punctuation (., !, ?)."
                    .to_string(),
                span: Some(commit.header.len() - 1..commit.header.len() - 1),
            }])
        } else {
            Ok(())
        }
    }
}
