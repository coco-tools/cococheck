use regex::Regex;
use serde::{Serialize, Serializer};

/// A list of regular expressions from the configuration. Two lists are equal if their patterns are
/// equal, because [`Regex`] does not implement [`PartialEq`].
#[derive(Clone, Debug, Default)]
pub struct Patterns(pub Vec<Regex>);

impl Patterns {
    /// Compile the `patterns`. Fail for an invalid one with an error message that describes it as a
    /// pattern of the given `kind`, like `ignore` or `ignore author`.
    pub(crate) fn compile(patterns: &[String], kind: &str) -> Result<Self, String> {
        patterns
            .iter()
            .map(|pattern| {
                Regex::new(pattern).map_err(|e| format!("invalid {kind} pattern '{pattern}': {e}"))
            })
            .collect::<Result<_, _>>()
            .map(Patterns)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return whether any part of the commit `message`, without trailing whitespace, matches one of
    /// the patterns.
    pub fn matches_message(&self, message: &str) -> bool {
        matches_message(&self.0, message)
    }

    /// Return whether any part of the `author`, given as `Name <email>`, matches one of the
    /// patterns.
    pub fn matches_author(&self, author: &str) -> bool {
        self.0.iter().any(|pattern| pattern.is_match(author))
    }
}

impl PartialEq for Patterns {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(&other.0)
                .all(|(a, b)| a.as_str() == b.as_str())
    }
}

impl Serialize for Patterns {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter().map(Regex::as_str))
    }
}

/// Return whether any part of the commit `message` matches one of the `patterns`. Trailing
/// whitespace is removed from the message before matching: Git terminates the last line with a
/// newline, but without the multi-line flag, `$` only matches at the very end of the message, so a
/// pattern like `^Initial commit$` would never match.
pub(crate) fn matches_message<'a>(
    patterns: impl IntoIterator<Item = &'a Regex>,
    message: &str,
) -> bool {
    let message = message.trim_end();
    patterns
        .into_iter()
        .any(|pattern| pattern.is_match(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patterns(patterns: &[&str]) -> Patterns {
        let patterns: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
        Patterns::compile(&patterns, "test").unwrap()
    }

    #[test]
    fn lists_with_the_same_patterns_are_equal() {
        assert_eq!(patterns(&["^a", "b$"]), patterns(&["^a", "b$"]));
        assert_ne!(patterns(&["^a", "b$"]), patterns(&["^a"]));
        assert_ne!(patterns(&["^a", "b$"]), patterns(&["b$", "^a"]));
    }

    #[test]
    fn names_the_kind_of_an_invalid_pattern() {
        let error = Patterns::compile(&["(unclosed".to_string()], "test").unwrap_err();
        assert!(
            error.starts_with("invalid test pattern '(unclosed': "),
            "{error}"
        );
    }

    #[test]
    fn matches_the_end_of_the_message_before_trailing_whitespace() {
        let patterns = patterns(&["^Initial commit$"]);
        assert!(patterns.matches_message("Initial commit\n"));
        assert!(!patterns.matches_message("Initial commit\n\nBody.\n"));
    }
}
