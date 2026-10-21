use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct NoDescriptionTerminalPunctuation {}

impl Rule for NoDescriptionTerminalPunctuation {
    fn name(&self) -> &str {
        "NoDescriptionTerminalPunctuation"
    }

    fn check(&self, commit: &ConventionalCommit) -> Vec<RuleError> {
        if commit.description.ends_with('.')
            || commit.description.ends_with('!')
            || commit.description.ends_with('?')
        {
            vec![RuleError {
                message: "Description should not end with terminal punctuation (., !, ?)."
                    .to_string(),
                span: Some(commit.header.len() - 1..commit.header.len() - 1),
            }]
        } else {
            vec![]
        }
    }
}
