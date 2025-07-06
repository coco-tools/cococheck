use crate::parser::ConventionalCommit;

#[derive(Debug, PartialEq)]
pub struct RuleError {
    pub message: String,
    pub span: Option<std::ops::Range<usize>>,
}

pub trait Rule {
    fn name(&self) -> &str;
    fn is_enabled(&self) -> bool;
    fn check(&self, message: &ConventionalCommit) -> Result<(), Vec<RuleError>>;
}

mod rule_set;
pub use rule_set::RuleSet;

mod allowed_types;
pub use allowed_types::AllowedTypes;

mod body_line_length;
pub use body_line_length::BodyLineLength;

mod header_length;
pub use header_length::HeaderLength;

mod no_description_terminal_punctuation;
pub use no_description_terminal_punctuation::NoDescriptionTerminalPunctuation;
