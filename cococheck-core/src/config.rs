use crate::parser::ParserConfig;
use crate::rules::{
    AllowedScopes, AllowedTypes, BlankLineBeforeFooter, BodyLength, BodyLineLength, BreakingChange,
    DescriptionCase, FooterLength, FooterLineLength, HeaderLength,
    NoDescriptionTerminalPunctuation, NoTrailingWhitespace, RequiredFooters, RuleConfig, ScopeCase,
    ScopePresence, TypeCase,
};
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// The patterns of the messages that Git and Git hosting platforms create, which do not follow the
/// Conventional Commits specification and are ignored by default.
const DEFAULT_IGNORE_PATTERNS: &[&str] = &[
    // Merges created by `git merge` and `git pull`, and by Git hosting platforms. As `.` does not
    // match line breaks, the patterns with `into` only match the first line.
    r"^Merge (branch|branches|tag|tags|commit|remote-tracking branch|pull request) ",
    r"^Merge .+ into ",
    r"^Merged (in|PR) ",
    r"^Merged .+ into ",
    r"^Automatic merge",
    r"^Auto-merged .+ into ",
    // Reverts created by `git revert`, which creates `Reapply` for the revert of a revert.
    r#"^(Revert|Reapply) ""#,
    // Commits created by `git commit --fixup` and `git commit --squash`.
    r"^(amend|fixup|squash)! ",
    // Messages that only consist of a version, like the commits created by `npm version`.
    r"^v?(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$",
];

static DEFAULT_IGNORES: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    DEFAULT_IGNORE_PATTERNS
        .iter()
        .map(|pattern| Regex::new(pattern).expect("default ignore patterns are valid"))
        .collect()
});

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Regular expressions for messages that are not allowed, like the messages of fixup commits. A
    /// message fails the checks if any part of it matches one of them, even if it is also ignored.
    #[serde(
        default,
        deserialize_with = "deserialize_forbidden_patterns",
        serialize_with = "serialize_patterns"
    )]
    pub forbidden: Vec<Regex>,

    /// Whether the messages that Git creates, like merge and revert commits, are ignored.
    #[serde(default = "default_default_ignores")]
    pub default_ignores: bool,

    /// Whether merge commits, which have more than one parent, are ignored, regardless of their
    /// message.
    #[serde(default = "default_ignore_merge_commits")]
    pub ignore_merge_commits: bool,

    /// Regular expressions for further messages to ignore. A message is ignored if any part of it
    /// matches one of them.
    #[serde(
        default,
        deserialize_with = "deserialize_ignore_patterns",
        serialize_with = "serialize_patterns"
    )]
    pub ignores: Vec<Regex>,

    /// Regular expressions for the authors of commits to ignore, matched against `Name <email>`. A
    /// commit is ignored if any part of its author matches one of them.
    #[serde(
        default,
        deserialize_with = "deserialize_ignore_author_patterns",
        serialize_with = "serialize_patterns"
    )]
    pub ignore_authors: Vec<Regex>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub parser: ParserConfig,

    #[serde(default, deserialize_with = "null_as_default")]
    pub rules: RulesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            forbidden: Vec::new(),
            default_ignores: default_default_ignores(),
            ignore_merge_commits: default_ignore_merge_commits(),
            ignores: Vec::new(),
            ignore_authors: Vec::new(),
            parser: ParserConfig::default(),
            rules: RulesConfig::default(),
        }
    }
}

/// The part of a commit message that matches a forbidden pattern.
#[derive(Debug)]
pub struct ForbiddenMatch<'a> {
    /// The forbidden pattern.
    pub pattern: &'a Regex,
    /// The byte range of the match in the commit message.
    pub span: Range<usize>,
}

impl Config {
    /// Return the match of the first of the `forbidden` patterns that the commit `message` matches,
    /// or `None` if it matches none of them. Like for [`Config::is_ignored`], trailing whitespace is
    /// removed from the message before matching.
    pub fn find_forbidden(&self, message: &str) -> Option<ForbiddenMatch<'_>> {
        let message = message.trim_end();

        self.forbidden.iter().find_map(|pattern| {
            pattern.find(message).map(|m| ForbiddenMatch {
                pattern,
                span: m.range(),
            })
        })
    }

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

    /// Return whether commits by the `author`, given as `Name <email>`, are ignored, because it
    /// matches one of the configured `ignore_authors`.
    pub fn is_ignored_author(&self, author: &str) -> bool {
        self.ignore_authors
            .iter()
            .any(|pattern| pattern.is_match(author))
    }

    /// Serialize the configuration as YAML, with all settings, including the defaults of omitted
    /// ones, so that the result is a configuration file that is equivalent to this configuration.
    pub fn to_yaml(&self) -> Result<String, serde_saphyr::ser::Error> {
        serde_saphyr::to_string(self)
    }
}

fn default_default_ignores() -> bool {
    true
}

fn default_ignore_merge_commits() -> bool {
    true
}

fn deserialize_forbidden_patterns<'de, D>(deserializer: D) -> Result<Vec<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_patterns(deserializer, "forbidden")
}

fn deserialize_ignore_patterns<'de, D>(deserializer: D) -> Result<Vec<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_patterns(deserializer, "ignore")
}

fn deserialize_ignore_author_patterns<'de, D>(deserializer: D) -> Result<Vec<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_patterns(deserializer, "ignore author")
}

/// Deserialize a list of regular expressions, failing for an invalid one, whose error message
/// describes it as a pattern of the given `kind`. A YAML null value is deserialized as an empty
/// list.
fn deserialize_patterns<'de, D>(deserializer: D, kind: &str) -> Result<Vec<Regex>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vec<String>>::deserialize(deserializer)?
        .unwrap_or_default()
        .iter()
        .map(|pattern| {
            Regex::new(pattern).map_err(|e| {
                serde::de::Error::custom(format!("invalid {kind} pattern '{pattern}': {e}"))
            })
        })
        .collect()
}

/// Serialize a list of regular expressions as a list of their patterns.
fn serialize_patterns<S>(patterns: &[Regex], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_seq(patterns.iter().map(Regex::as_str))
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RulesConfig {
    #[serde(default, deserialize_with = "null_as_default")]
    pub allowed_scopes: RuleConfig<AllowedScopes>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub allowed_types: RuleConfig<AllowedTypes>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub blank_line_before_footer: RuleConfig<BlankLineBeforeFooter>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub body_length: RuleConfig<BodyLength>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub body_line_length: RuleConfig<BodyLineLength>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub breaking_change: RuleConfig<BreakingChange>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub description_case: RuleConfig<DescriptionCase>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub footer_length: RuleConfig<FooterLength>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub footer_line_length: RuleConfig<FooterLineLength>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub header_length: RuleConfig<HeaderLength>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub no_description_terminal_punctuation: RuleConfig<NoDescriptionTerminalPunctuation>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub no_trailing_whitespace: RuleConfig<NoTrailingWhitespace>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub required_footers: RuleConfig<RequiredFooters>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub scope_case: RuleConfig<ScopeCase>,

    #[serde(default, deserialize_with = "null_as_default")]
    pub scope_presence: RuleConfig<ScopePresence>,

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

    #[error(
        "found multiple configuration files, keep only one of them: {}",
        paths.iter().map(|path| format!("'{}'", path.display())).collect::<Vec<_>>().join(", ")
    )]
    MultipleFiles {
        /// The paths of the configuration files, in the order of [`CONFIG_FILE_NAMES`].
        paths: Vec<PathBuf>,
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

/// The names of the files in the root of a repository that [`find_config`] looks for.
pub const CONFIG_FILE_NAMES: &[&str] = &[
    "cococheck.yml",
    "cococheck.yaml",
    ".cococheck.yml",
    ".cococheck.yaml",
];

/// Find the configuration file in `dir`, which is one of [`CONFIG_FILE_NAMES`]. Return `None` if
/// there is none, and an error if there is more than one, because it would be ambiguous which one
/// applies.
pub fn find_config(dir: &Path) -> Result<Option<PathBuf>, ConfigError> {
    let mut paths = Vec::new();
    for name in CONFIG_FILE_NAMES {
        let path = dir.join(name);
        let exists = path.try_exists().map_err(|source| ConfigError::Io {
            path: path.clone(),
            source,
        })?;
        if exists {
            paths.push(path);
        }
    }

    match paths.len() {
        0 => Ok(None),
        1 => Ok(paths.pop()),
        _ => Err(ConfigError::MultipleFiles { paths }),
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
    use crate::rules::{DescriptionCaseMode, Presence, ScopeCaseMode, Severity};
    use pretty_assertions::assert_eq;

    #[test]
    fn read_default_config_for_empty_config() {
        let config = parse_config("").unwrap();
        assert_eq!(config.rules.allowed_scopes, RuleConfig::default());
        assert_eq!(config.rules.allowed_types, RuleConfig::default());
        assert_eq!(config.rules.blank_line_before_footer, RuleConfig::default());
        assert_eq!(config.rules.body_line_length, RuleConfig::default());
        assert_eq!(config.rules.description_case, RuleConfig::default());
        assert_eq!(config.rules.header_length, RuleConfig::default());
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            RuleConfig::default()
        );
        assert_eq!(config.rules.required_footers, RuleConfig::default());
        assert_eq!(config.rules.scope_case, RuleConfig::default());
        assert_eq!(config.rules.scope_presence, RuleConfig::default());
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
            "Merge 'feature' into main",
            "Merge feature/a into develop\n\n* feature/a:\n  feat: Add a feature",
            "Merged in feature (pull request #1)",
            "Merged PR 1: Add a feature",
            "Merged feature into main",
            "Automatic merge from release/1.0 -> main",
            "Auto-merged release/1.0 into main",
            "1.2.3",
            "v1.2.3\n",
            "v10.0.0-beta.1+build.5",
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
            "Merge the configurations\n\nThey are moved into one file.",
            "fixup: Not a fixup commit",
            "Revert the last change",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.2.3\n\nfeat: Add a feature",
            "Release 1.2.3",
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
    fn ignores_merge_commits_by_default() {
        assert!(parse_config("").unwrap().ignore_merge_commits);
        assert!(
            !parse_config("ignore_merge_commits: false")
                .unwrap()
                .ignore_merge_commits
        );
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
    fn ignores_authors_matching_configured_patterns() {
        let yaml = r#"
        ignore_authors:
          - '^dependabot\[bot\] '
          - '<.*@bots\.example\.org>$'
        "#;
        let config = parse_config(yaml).unwrap();
        assert!(config.is_ignored_author(
            "dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>"
        ));
        assert!(config.is_ignored_author("Release Bot <release@bots.example.org>"));
        assert!(!config.is_ignored_author("Jane Doe <jane@example.org>"));
        assert!(!Config::default().is_ignored_author("dependabot[bot] <bot@example.org>"));
    }

    #[test]
    fn fails_for_invalid_ignore_author_pattern() {
        let message = parse_config("ignore_authors: ['(unclosed']")
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("invalid ignore author pattern '(unclosed'"),
            "{message}"
        );
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
    fn finds_the_first_matching_forbidden_pattern() {
        let yaml = r#"
        forbidden:
          - '^(amend|fixup|squash)! '
          - '\bWIP\b'
        "#;
        let config = parse_config(yaml).unwrap();

        let found = config
            .find_forbidden("fixup! feat: Add a WIP feature")
            .unwrap();
        assert_eq!(found.pattern.as_str(), "^(amend|fixup|squash)! ");
        assert_eq!(found.span, 0..7);

        let found = config.find_forbidden("feat: Add a WIP feature").unwrap();
        assert_eq!(found.pattern.as_str(), "\\bWIP\\b");
        assert_eq!(found.span, 12..15);

        assert!(config.find_forbidden("feat: Add a feature").is_none());
    }

    #[test]
    fn matches_forbidden_patterns_before_trailing_whitespace() {
        let config = parse_config("forbidden: ['^WIP$']").unwrap();
        assert!(config.find_forbidden("WIP\n").is_some());
        assert!(config.find_forbidden("WIP\n\nBody.\n").is_none());
    }

    #[test]
    fn forbids_nothing_by_default() {
        let config = Config::default();
        assert!(config.forbidden.is_empty());
        assert!(
            config
                .find_forbidden("fixup! feat: Add a feature")
                .is_none()
        );
    }

    #[test]
    fn fails_for_invalid_forbidden_pattern() {
        let message = parse_config("forbidden: ['(unclosed']")
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("invalid forbidden pattern '(unclosed'"),
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
        assert_eq!(config.rules.allowed_scopes, RuleConfig::default());
        assert_eq!(config.rules.allowed_types, RuleConfig::default());
        assert_eq!(config.rules.blank_line_before_footer, RuleConfig::default());
        assert_eq!(config.rules.body_line_length, RuleConfig::default());
        assert_eq!(config.rules.description_case, RuleConfig::default());
        assert_eq!(config.rules.header_length, RuleConfig::default());
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            RuleConfig::default()
        );
        assert_eq!(config.rules.required_footers, RuleConfig::default());
        assert_eq!(config.rules.scope_case, RuleConfig::default());
        assert_eq!(config.rules.scope_presence, RuleConfig::default());
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
            types: [feat, fix]
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
                    types: vec!["feat".to_string(), "fix".to_string()]
                },
            }
        );
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            RuleConfig {
                enabled: false,
                severity: Severity::Error,
                rule: NoDescriptionTerminalPunctuation::default(),
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
    fn read_allowed_scopes() {
        let yaml = r#"
        rules:
          allowed_scopes:
            scopes: [cli, core]
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.allowed_scopes.rule,
            AllowedScopes {
                scopes: vec!["cli".to_string(), "core".to_string()]
            }
        );
    }

    #[test]
    fn read_terminal_punctuation_characters() {
        let yaml = r#"
        rules:
          no_description_terminal_punctuation:
            characters: ".,;:"
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.no_description_terminal_punctuation.rule,
            NoDescriptionTerminalPunctuation {
                characters: ".,;:".to_string()
            }
        );
    }

    #[test]
    fn read_footer_lengths() {
        let yaml = r#"
        rules:
          footer_length:
            max_length: 500
          footer_line_length:
            max_length: 100
            ignore_urls: false
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.footer_length.rule,
            FooterLength {
                max_length: Some(500)
            }
        );
        assert_eq!(
            config.rules.footer_line_length.rule,
            FooterLineLength {
                max_length: Some(100),
                ignore_urls: false,
            }
        );
    }

    #[test]
    fn read_breaking_change() {
        let yaml = r#"
        rules:
          breaking_change:
            footer_required: true
            marker_required: true
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.breaking_change.rule,
            BreakingChange {
                footer_required: true,
                marker_required: true,
            }
        );
    }

    #[test]
    fn read_description_case() {
        let yaml = r#"
        rules:
          description_case:
            case: sentence
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.description_case.rule,
            DescriptionCase {
                case: DescriptionCaseMode::Sentence
            }
        );
    }

    #[test]
    fn fails_for_unknown_description_case() {
        let yaml = r#"
        rules:
          description_case:
            case: title
        "#;
        assert!(matches!(parse_config(yaml), Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn read_scope_case() {
        let yaml = r#"
        rules:
          scope_case:
            case: any
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(
            config.rules.scope_case.rule,
            ScopeCase {
                case: ScopeCaseMode::Any
            }
        );
    }

    #[test]
    fn read_scope_presence() {
        for (value, presence) in [
            ("optional", Presence::Optional),
            ("required", Presence::Required),
            ("forbidden", Presence::Forbidden),
        ] {
            let yaml = format!("rules:\n  scope_presence:\n    scope: {value}\n");
            let config = parse_config(&yaml).unwrap();
            assert_eq!(
                config.rules.scope_presence.rule,
                ScopePresence { scope: presence }
            );
        }
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
    fn fails_for_unknown_option_of_rule() {
        let yaml = r#"
        rules:
          header_length:
            max_lenght: 50
        "#;
        let error = parse_config(yaml).unwrap_err().to_string();
        assert!(
            error.contains(
                "unknown option `max_lenght`, expected one of `enabled`, `severity`, `max_length`"
            ),
            "{error}"
        );
    }

    #[test]
    fn fails_for_unknown_option_of_rule_without_options() {
        let yaml = r#"
        rules:
          type_case:
            severity: warning
            case: lower
        "#;
        assert!(matches!(parse_config(yaml), Err(ConfigError::Parse { .. })));
    }

    #[test]
    fn reads_common_and_rule_options_in_any_order() {
        let yaml = r#"
        rules:
          header_length:
            max_length: 50
            severity: warning
            enabled: false
        "#;
        let config = parse_config(yaml).unwrap();
        assert!(!config.rules.header_length.enabled);
        assert_eq!(config.rules.header_length.severity, Severity::Warning);
        assert_eq!(config.rules.header_length.rule.max_length, 50);
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

    // Only on Linux, because the file systems of macOS and Windows do not allow file names that are
    // not valid Unicode.
    #[cfg(target_os = "linux")]
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

    #[test]
    fn finds_no_config_file_in_empty_directory() {
        let dir = tempfile::TempDir::new().unwrap();

        assert_eq!(find_config(dir.path()).unwrap(), None);
    }

    #[test]
    fn finds_config_file_with_any_of_the_names() {
        for name in CONFIG_FILE_NAMES {
            let dir = tempfile::TempDir::new().unwrap();
            let path = dir.path().join(name);
            std::fs::write(&path, "rules: {}").unwrap();

            assert_eq!(find_config(dir.path()).unwrap(), Some(path), "{name}");
        }
    }

    #[test]
    fn multiple_config_files_are_an_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let paths = ["cococheck.yml", ".cococheck.yaml"].map(|name| dir.path().join(name));
        for path in &paths {
            std::fs::write(path, "rules: {}").unwrap();
        }

        let error = find_config(dir.path()).unwrap_err();

        let message = error.to_string();
        assert!(
            matches!(error, ConfigError::MultipleFiles { paths: ref found } if found == &paths),
            "{error:?}"
        );
        for path in &paths {
            assert!(message.contains(&path.display().to_string()), "{message}");
        }
    }

    #[test]
    fn serialized_config_contains_all_settings() {
        let yaml = Config::default().to_yaml().unwrap();

        for key in [
            "forbidden: []",
            "default_ignores: true",
            "ignore_merge_commits: true",
            "ignores: []",
            "ignore_authors: []",
            "footer_detection: last_paragraph",
            "allowed_types:",
            "type_case:",
            "max_length: 72",
        ] {
            assert!(yaml.contains(key), "'{key}' not found in:\n{yaml}");
        }
    }

    #[test]
    fn serialized_config_can_be_parsed_again() {
        let yaml = r#"
        forbidden: ['^fixup! ']
        default_ignores: false
        ignores:
          - '^WIP\b'
          - '(?m)^\[skip checks\]$'
          - "it's: # not a comment"
          - '- not a list'
        parser:
          footer_detection: trailing_paragraphs
        rules:
          allowed_scopes:
            scopes: [cli, core]
          description_case:
            case: sentence
          header_length:
            enabled: false
            severity: warning
            max_length: 100
        "#;
        let config = parse_config(yaml).unwrap();

        let serialized = config.to_yaml().unwrap();
        let parsed = parse_config(&serialized).unwrap();

        assert_eq!(parsed.to_yaml().unwrap(), serialized);
        let patterns = |config: &Config| {
            config
                .ignores
                .iter()
                .map(|pattern| pattern.as_str().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(patterns(&parsed), patterns(&config));
        assert_eq!(parsed.forbidden[0].as_str(), "^fixup! ");
        assert!(!parsed.default_ignores);
        assert_eq!(
            parsed.parser.footer_detection,
            FooterDetection::TrailingParagraphs
        );
        assert_eq!(parsed.rules.allowed_scopes, config.rules.allowed_scopes);
        assert_eq!(parsed.rules.description_case, config.rules.description_case);
        assert_eq!(parsed.rules.header_length, config.rules.header_length);
    }
}
