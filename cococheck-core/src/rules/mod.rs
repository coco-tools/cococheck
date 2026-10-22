use crate::parser::ConventionalCommit;
use serde::Deserialize;

#[derive(Debug, PartialEq)]
pub struct RuleError {
    pub message: String,
    /// The byte range in the full commit message that the error refers to.
    pub span: Option<std::ops::Range<usize>>,
}

pub trait Rule {
    fn name(&self) -> &str;
    fn check(&self, message: &ConventionalCommit) -> Vec<RuleError>;
}

/// How a violation of a rule is treated.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The violation is reported, but the commit passes the checks.
    Warning,

    /// The violation is reported, and the commit fails the checks.
    #[default]
    Error,
}

/// The configuration of a rule: Whether it is enabled, its severity, and the rule with its own
/// options, which are flattened into the same YAML mapping as the `enabled` flag.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RuleConfig<R> {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub severity: Severity,
    #[serde(flatten)]
    pub rule: R,
}

fn default_enabled() -> bool {
    true
}

impl<R: Default> Default for RuleConfig<R> {
    fn default() -> Self {
        RuleConfig {
            enabled: default_enabled(),
            severity: Severity::default(),
            rule: R::default(),
        }
    }
}

mod rule_set;
pub use rule_set::{RuleResult, RuleSet};

mod allowed_types;
pub use allowed_types::AllowedTypes;

mod blank_line_before_footer;
pub use blank_line_before_footer::BlankLineBeforeFooter;

mod body_line_length;
pub use body_line_length::BodyLineLength;

mod header_length;
pub use header_length::HeaderLength;

mod no_description_terminal_punctuation;
pub use no_description_terminal_punctuation::NoDescriptionTerminalPunctuation;

mod required_footers;
pub use required_footers::RequiredFooters;

mod type_case;
pub use type_case::TypeCase;
