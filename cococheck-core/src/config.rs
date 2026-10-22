use crate::parser::ParserConfig;
use crate::rules::{
    AllowedTypes, BlankLineBeforeFooter, BodyLineLength, HeaderLength,
    NoDescriptionTerminalPunctuation, RequiredFooters, RuleConfig, TypeCase,
};
use regex::Regex;
use serde::{Deserialize, Deserializer};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// The patterns of the messages that Git and Git hosting platforms create, which do not follow the
/// Conventional Commits specification and are ignored by default.
const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    // Merges created by `git merge` and `git pull`, and by Git hosting platforms.
    r"^Merge (branch|branches|tag|tags|commit|remote-tracking branch|pull request) ",
    r"^Merged (in|PR) ",
    // Reverts created by `git revert`, which creates `Reapply` for the revert of a revert.
    r#"^(Revert|Reapply) ""#,
    // Commits created by `git commit --fixup` and `git commit --squash`.
    r"^(amend|fixup|squash)! ",
];

static DEFAULT_IGNORES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    DEFAULT_IGNORE_PATTERNS
        .iter()
        .map(|pattern| Regex::new(pattern).expect("default ignore patterns are valid"))
        .collect()
});

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Whether the messages that Git creates, like merge and revert commits, are ignored.
    #[serde(default = "default_default_ignores")]
    pub default_ignores: bool,

    /// Regular expressions for further messages to ignore. A message is ignored if any part of it
    /// matches one of them.
    #[serde(default, deserialize_with = "deserialize_patterns")]
    pub ignores: Vec<Regex>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub parser: ParserConfig,

    #[serde(default, deserialize_with = "null_as_default")]
    pub rules: RulesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            default_ignores: default_default_ignores(),
            ignores: Vec::new(),
            parser: ParserConfig::default(),
            rules: RulesConfig::default(),
        }
    }
}

impl Config {
    /// Return whether the commit `message` is ignored, because it matches one of the default ignore
    /// patterns, if they are enabled, or one of the configured `ignores`.
    pub fn is_ignored(&self, message: &str) -> bool {
        // Git terminates the last line with a newline, but without the multi-line flag, `$` only
        // matches at the very end of the message, so a pattern like `^Initial commit$` would never
        // match.
        let message = message.trim_end();

        let defaults: &[Regex] = if self.default_ignores {
            &DEFAULT_IGNORES
        } else {
            &[]
        };

        defaults
            .iter()
            .chain(&self.ignores)
            .any(|pattern| pattern.is_match(message))
    }
}

fn default_default_ignores() -> bool {
    true
}

/// Deserialize a list of regular expressions, failing for an invalid one. A YAML null value is
/// deserialized as an empty list.
fn deserialize_patterns<'de, D>(deserializer: D) -> Result<Vec<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vec<String>>::deserialize(deserializer)?
        .unwrap_or_default()
        .iter()
        .map(|pattern| {
            Regex::new(pattern).map_err(|e| {
                serde::de::Error::custom(format!("invalid ignore pattern '{pattern}': {e}"))
            })
        })
        .collect()
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesConfig {
    #[serde(default, deserialize_with = "null_as_default")]
    pub allowed_types: RuleConfig<AllowedTypes>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub blank_line_before_footer: RuleConfig<BlankLineBeforeFooter>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub body_line_length: RuleConfig<BodyLineLength>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub header_length: RuleConfig<HeaderLength>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub no_description_terminal_punctuation: RuleConfig<NoDescriptionTerminalPunctuation>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub required_footers: RuleConfig<RequiredFooters>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub type_case: RuleConfig<TypeCase>,
}

/// Deserialize a YAML null value, like an empty `rules:` key, as the default value, instead of
/// failing because a null value cannot be deserialized into a struct.
fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Option::<T>::deserialize(deserializer).map(Option::unwrap_or_default)
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("failed to read configuration file '{}'", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    // The parse error is not exposed as a source, because `serde_saphyr::Error` already includes
    // the message of its own source in its `Display` output, so it would be printed twice.
    #[error("failed to parse configuration: {}", render_parse_error(error, path.as_deref()))]
    Parse {
        /// The path of the configuration file, or `None` if the configuration was not read from a
        /// file.
        path: Option<PathBuf>,
        error: Box<serde_saphyr::Error>,
    },
}

/// Render a parse error, using `path` as the source name in the snippet instead of the default name.
fn render_parse_error(error: &serde_saphyr::Error, path: Option<&Path>) -> String {
    match path {
        Some(path) => {
            let source_name = path.display().to_string();
            error.render_with_options(serde_saphyr::render_options! {
                source_name: Some(&source_name),
            })
        }
        None => error.render(),
    }
}

pub fn parse_config(content: &str) -> Result<Config, ConfigError> {
    serde_saphyr::from_str(content).map_err(|error| ConfigError::Parse {
        path: None,
        error: Box::new(error),
    })
}

pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_saphyr::from_str(&content).map_err(|error| ConfigError::Parse {
        path: Some(path.to_path_buf()),
        error: Box::new(error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::FooterDetection;
    use crate::rules::Severity;
    use pretty_assertions::assert_eq;

    #[test]
    fn read_default_config_for_empty_config() {
        let config = parse_config("").unwrap();
        assert_eq!(config.rules.allowed_types, RuleConfig::default());
        assert_eq!(config.rules.blank_line_before_footer, RuleConfig::default());
        assert_eq!(config.rules.body_line_length, RuleConfig::default());
        assert_eq!(config.rules.header_length, RuleConfig::default());
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            RuleConfig::default()
        );
        assert_eq!(config.rules.required_footers, RuleConfig::default());
        assert_eq!(config.rules.type_case, RuleConfig::default());
    }

    #[test]
    fn ignores_messages_created_by_git_by_default() {
        let config = parse_config("").unwrap();
        let messages = [
            "Merge branch 'feature'",
            "Merge branch 'main' of github.com:example/repo",
            "Merge branches 'a' and 'b'",
            "Merge remote-tracking branch 'origin/main'",
            "Merge tag 'v1.0.0'",
            "Merge commit '0123456'",
            "Merge pull request #1 from example/feature\n\nfeat: Add a feature",
            "Merged in feature (pull request #1)",
            "Merged PR 1: Add a feature",
            "Revert \"feat: Add a feature\"\n\nThis reverts commit 0123456.",
            "Reapply \"feat: Add a feature\"",
            "fixup! feat: Add a feature",
            "squash! feat: Add a feature",
            "amend! feat: Add a feature",
        ];

        for message in messages {
            assert!(config.is_ignored(message), "{message}");
        }
    }

    #[test]
    fn does_not_ignore_other_messages_by_default() {
        let config = parse_config("").unwrap();
        let messages = [
            "feat: Add a feature",
            "revert: Remove a feature",
            "fix: Merge branch names correctly",
            "Merge the configurations",
            "Update stuff\n\nMerge branch 'feature'",
            "fixup: Not a fixup commit",
        ];

        for message in messages {
            assert!(!config.is_ignored(message), "{message}");
        }
    }

    #[test]
    fn default_ignores_can_be_disabled() {
        let config = parse_config("default_ignores: false").unwrap();
        assert!(!config.is_ignored("Merge branch 'feature'"));
        assert!(!config.is_ignored("fixup! feat: Add a feature"));
    }

    #[test]
    fn ignores_messages_matching_configured_patterns() {
        let yaml = r#"
        default_ignores: false
        ignores:
          - '^WIP\b'
          - '(?m)^\[skip ci\]$'
        "#;
        let config = parse_config(yaml).unwrap();
        assert!(config.is_ignored("WIP: Add a feature"));
        assert!(config.is_ignored("feat: Add a feature\n\n[skip ci]"));
        assert!(!config.is_ignored("feat: Add WIP feature"));
        assert!(!config.is_ignored("Merge branch 'feature'"));
    }

    #[test]
    fn matches_the_end_of_the_message_before_trailing_whitespace() {
        let config = parse_config("ignores: ['^Initial commit$']").unwrap();
        assert!(config.is_ignored("Initial commit\n"));
        assert!(config.is_ignored("Initial commit \n\n"));
        assert!(!config.is_ignored("Initial commit\n\nBody.\n"));
    }

    #[test]
    fn configured_patterns_extend_the_default_ignores() {
        let config = parse_config("ignores: ['^WIP']").unwrap();
        assert!(config.is_ignored("WIP: Add a feature"));
        assert!(config.is_ignored("Merge branch 'feature'"));
    }

    #[test]
    fn read_default_value_for_null_ignores() {
        let config = parse_config("ignores:").unwrap();
        assert!(config.ignores.is_empty());
        assert!(config.default_ignores);
    }

    #[test]
    fn fails_for_invalid_ignore_pattern() {
        let message = parse_config("ignores: ['(unclosed']")
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("invalid ignore pattern '(unclosed'"),
            "{message}"
        );
    }

    #[test]
    fn read_default_parser_config_for_empty_config() {
        assert_eq!(parse_config("").unwrap().parser, ParserConfig::default());
        assert_eq!(
            parse_config("parser:").unwrap().parser,
            ParserConfig::default()
        );
        assert_eq!(
            ParserConfig::default().footer_detection,
            FooterDetection::LastParagraph
        );
    }

    #[test]
    fn read_footer_detection() {
        let yaml = r#"
        parser:
          footer_detection: trailing_paragraphs
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.parser.footer_detection,
            FooterDetection::TrailingParagraphs
        );
    }

    #[test]
    fn fails_for_unknown_footer_detection() {
        let yaml = r#"
        parser:
          footer_detection: first_paragraph
        "#;
        assert!(matches!(parse_config(yaml), Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn read_default_value_for_empty_rules() {
        let yaml = r#"
        rules: {}
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(config.rules.allowed_types, RuleConfig::default());
        assert_eq!(config.rules.blank_line_before_footer, RuleConfig::default());
        assert_eq!(config.rules.body_line_length, RuleConfig::default());
        assert_eq!(config.rules.header_length, RuleConfig::default());
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            RuleConfig::default()
        );
        assert_eq!(config.rules.required_footers, RuleConfig::default());
        assert_eq!(config.rules.type_case, RuleConfig::default());
    }

    #[test]
    fn read_default_value_for_missing_rules() {
        let yaml = r#"
        rules:
          body_line_length:
            enabled: false
            max_length: 100
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.body_line_length,
            RuleConfig {
                enabled: false,
                severity: Severity::Error,
                rule: BodyLineLength {
                    max_length: 100,
                    ignore_urls: true,
                },
            }
        );
        assert_eq!(config.rules.header_length, RuleConfig::default());
    }

    #[test]
    fn read_ignore_urls() {
        let yaml = r#"
        rules:
          body_line_length:
            ignore_urls: false
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.body_line_length.rule,
            BodyLineLength {
                max_length: 72,
                ignore_urls: false,
            }
        );
    }

    #[test]
    fn read_default_value_for_missing_properties() {
        let yaml = r#"
        rules:
          body_line_length:
            enabled: true
          header_length:
            max_length: 100
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.body_line_length,
            RuleConfig {
                enabled: true,
                ..Default::default()
            }
        );
        assert_eq!(
            config.rules.header_length,
            RuleConfig {
                rule: HeaderLength { max_length: 100 },
                ..Default::default()
            }
        );
    }

    #[test]
    fn read_default_value_for_null_rules() {
        let config = parse_config("rules:").unwrap();
        assert_eq!(config.rules.allowed_types, RuleConfig::default());
        assert_eq!(config.rules.header_length, RuleConfig::default());
    }

    #[test]
    fn read_default_value_for_null_rule() {
        let yaml = r#"
        rules:
          header_length:
          no_description_terminal_punctuation:
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(config.rules.header_length, RuleConfig::default());
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            RuleConfig::default()
        );
    }

    #[test]
    fn read_rule_options_next_to_enabled() {
        let yaml = r#"
        rules:
          allowed_types:
            enabled: false
            allowed_types: [feat, fix]
          no_description_terminal_punctuation:
            enabled: false
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.allowed_types,
            RuleConfig {
                enabled: false,
                severity: Severity::Error,
                rule: AllowedTypes {
                    allowed_types: vec!["feat".to_string(), "fix".to_string()]
                },
            }
        );
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            RuleConfig {
                enabled: false,
                severity: Severity::Error,
                rule: NoDescriptionTerminalPunctuation {},
            }
        );
    }

    #[test]
    fn read_required_footers() {
        let yaml = r#"
        rules:
          required_footers:
            tokens: [Signed-off-by]
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.required_footers,
            RuleConfig {
                enabled: true,
                severity: Severity::Error,
                rule: RequiredFooters {
                    tokens: vec!["Signed-off-by".to_string()]
                },
            }
        );
    }

    #[test]
    fn read_severity() {
        let yaml = r#"
        rules:
          header_length:
            severity: warning
          body_line_length:
            severity: error
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(config.rules.header_length.severity, Severity::Warning);
        assert_eq!(config.rules.body_line_length.severity, Severity::Error);
        assert_eq!(config.rules.allowed_types.severity, Severity::Error);
    }

    #[test]
    fn fails_for_unknown_severity() {
        let yaml = r#"
        rules:
          header_length:
            severity: info
        "#;
        assert!(matches!(parse_config(yaml), Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn fails_for_unknown_rule() {
        let yaml = r#"
        rules:
          unknown_rule:
            enabled: true
        "#;
        assert!(matches!(parse_config(yaml), Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn read_default_value_for_empty_rule() {
        let yaml = r#"
        rules:
          no_description_terminal_punctuation: {}
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            RuleConfig::default()
        );
    }

    #[cfg(unix)]
    #[test]
    fn loads_config_from_path_that_is_not_valid_utf8() {
        use std::os::unix::ffi::OsStrExt;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(std::ffi::OsStr::from_bytes(b"caf\xe9.yml"));
        std::fs::write(&path, "rules: {}").unwrap();

        assert!(load_config(&path).is_ok());
    }

    #[test]
    fn names_the_config_file_in_parse_errors() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("cococheck.yml");
        std::fs::write(&path, "rules:\n  unknown_rule:\n    enabled: true\n").unwrap();

        let message = load_config(&path).unwrap_err().to_string();
        assert!(message.contains(&path.display().to_string()));
        assert!(!message.contains("<input>"));
    }
}
