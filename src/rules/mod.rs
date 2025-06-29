use crate::parser::ConventionalCommit;

#[derive(Debug, PartialEq)]
pub struct RuleError {
    pub message: String,
    pub span: Option<std::ops::Range<usize>>,
}

pub trait Rule {
    fn name(&self) -> &str;
    fn check(&self, message: &ConventionalCommit) -> Result<(), Vec<RuleError>>;
}

mod body_line_length;
mod header_length;

pub use body_line_length::BodyLineLength;
pub use header_length::HeaderLength;
