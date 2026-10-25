use crate::parser::ConventionalCommit;
use regex::regex;
use serde::{Deserialize, Serialize};

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
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
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
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

/// Return whether the `line` contains a URL, for the rules that skip such lines, because long URLs
/// cannot be wrapped. Accept anything that starts like a URL, instead of trying to match only
/// valid URLs.
fn contains_url(line: &str) -> bool {
    regex!(r"\bhttps?://\S+").is_match(line)
}

/// Return the byte range of the scope in the full commit message, without the parentheses, or
/// `None` if the commit has no scope. The header has the form `type(scope)`, and it is the first
/// line of the message.
fn scope_span(commit: &ConventionalCommit) -> Option<std::ops::Range<usize>> {
    let start = commit.commit_type.len() + 1;
    commit
        .scope
        .as_ref()
        .map(|scope| start..start + scope.len())
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

mod allowed_scopes;
pub use allowed_scopes::AllowedScopes;

mod allowed_types;
pub use allowed_types::AllowedTypes;

mod blank_line_before_footer;
pub use blank_line_before_footer::BlankLineBeforeFooter;

mod body_length;
pub use body_length::BodyLength;

mod body_line_length;
pub use body_line_length::BodyLineLength;

mod description_case;
pub use description_case::{DescriptionCase, DescriptionCaseMode};

mod footer_length;
pub use footer_length::FooterLength;

mod footer_line_length;
pub use footer_line_length::FooterLineLength;

mod header_length;
pub use header_length::HeaderLength;

mod no_description_terminal_punctuation;
pub use no_description_terminal_punctuation::NoDescriptionTerminalPunctuation;

mod no_trailing_whitespace;
pub use no_trailing_whitespace::NoTrailingWhitespace;

mod required_footers;
pub use required_footers::RequiredFooters;

mod scope_presence;
pub use scope_presence::{Presence, ScopePresence};

mod scope_case;
pub use scope_case::{ScopeCase, ScopeCaseMode};

mod type_case;
pub use type_case::TypeCase;
