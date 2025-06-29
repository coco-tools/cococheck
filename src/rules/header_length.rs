use super::{Rule, RuleError};
use crate::parser::ConventionalCommit;

pub struct HeaderLength {
    pub max_length: usize,
}

impl Default for HeaderLength {
    fn default() -> Self {
        HeaderLength { max_length: 72 }
    }
}

impl Rule for HeaderLength {
    fn name(&self) -> &str {
        "HeaderLength"
    }

    fn check(&self, commit: &ConventionalCommit) -> Result<(), Vec<RuleError>> {
        if commit.header.len() > self.max_length {
            Err(vec![RuleError {
                message: format!(
                    "Header exceeds maximum length of {} characters (actual: {}).",
                    self.max_length,
                    commit.header.len()
                ),
                span: Some(0..commit.header.len()),
            }])
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_conventional_commit;

    #[test]
    fn succeeds_for_valid_length() {
        let commit = parse_conventional_commit("feat: add new feature").unwrap();
        let rule = HeaderLength { max_length: 50 };
        assert!(rule.check(&commit).is_ok());
    }

    #[test]
    fn fails_when_header_exceeds_max_length() {
        let long_commit = parse_conventional_commit(
            "feat: this is a very long header that exceeds the maximum length",
        )
        .unwrap();
        let rule = HeaderLength { max_length: 30 };
        assert!(rule.check(&long_commit).is_err());
    }
}
