use crate::parser::ParserConfig;
use crate::rules::{
    AllowedTypes, BlankLineBeforeFooter, BodyLineLength, HeaderLength,
    NoDescriptionTerminalPunctuation,
};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default, deserialize_with = "null_as_default")]
    pub parser: ParserConfig,

    #[serde(default, deserialize_with = "null_as_default")]
    pub rules: RulesConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulesConfig {
    #[serde(default, deserialize_with = "null_as_default")]
    pub allowed_types: AllowedTypes,

    #[serde(default, deserialize_with = "null_as_default")]
    pub blank_line_before_footer: BlankLineBeforeFooter,

    #[serde(default, deserialize_with = "null_as_default")]
    pub body_line_length: BodyLineLength,

    #[serde(default, deserialize_with = "null_as_default")]
    pub header_length: HeaderLength,

    #[serde(default, deserialize_with = "null_as_default")]
    pub no_description_terminal_punctuation: NoDescriptionTerminalPunctuation,
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

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_saphyr::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "Failed to read configuration file: {}", e),
            ConfigError::Parse(e) => write!(f, "Failed to parse configuration: {}", e),
        }
    }
}

pub fn parse_config(content: &str) -> Result<Config, ConfigError> {
    serde_saphyr::from_str(content).map_err(ConfigError::Parse)
}

pub fn load_config(path: &Path) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
    parse_config(&content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::FooterDetection;
    use pretty_assertions::assert_eq;

    #[test]
    fn read_default_config_for_empty_config() {
        let config = parse_config("").unwrap();
        assert_eq!(config.rules.allowed_types, AllowedTypes::default());
        assert_eq!(
            config.rules.blank_line_before_footer,
            BlankLineBeforeFooter::default()
        );
        assert_eq!(config.rules.body_line_length, BodyLineLength::default());
        assert_eq!(config.rules.header_length, HeaderLength::default());
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            NoDescriptionTerminalPunctuation::default()
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
        assert!(matches!(parse_config(yaml), Err(ConfigError::Parse(_))));
    }

    #[test]
    fn read_default_value_for_empty_rules() {
        let yaml = r#"
        rules: {}
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(config.rules.allowed_types, AllowedTypes::default());
        assert_eq!(
            config.rules.blank_line_before_footer,
            BlankLineBeforeFooter::default()
        );
        assert_eq!(config.rules.body_line_length, BodyLineLength::default());
        assert_eq!(config.rules.header_length, HeaderLength::default());
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            NoDescriptionTerminalPunctuation::default()
        );
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
            BodyLineLength {
                enabled: false,
                max_length: 100
            }
        );
        assert_eq!(config.rules.header_length, HeaderLength::default());
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
            BodyLineLength {
                enabled: true,
                ..Default::default()
            }
        );
        assert_eq!(
            config.rules.header_length,
            HeaderLength {
                max_length: 100,
                ..Default::default()
            }
        );
    }

    #[test]
    fn read_default_value_for_null_rules() {
        let config = parse_config("rules:").unwrap();
        assert_eq!(config.rules.allowed_types, AllowedTypes::default());
        assert_eq!(config.rules.header_length, HeaderLength::default());
    }

    #[test]
    fn read_default_value_for_null_rule() {
        let yaml = r#"
        rules:
          header_length:
          no_description_terminal_punctuation:
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(config.rules.header_length, HeaderLength::default());
        assert_eq!(
            config.rules.no_description_terminal_punctuation,
            NoDescriptionTerminalPunctuation::default()
        );
    }

    #[test]
    fn fails_for_unknown_rule() {
        let yaml = r#"
        rules:
          unknown_rule:
            enabled: true
        "#;
        assert!(matches!(parse_config(yaml), Err(ConfigError::Parse(_))));
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
            NoDescriptionTerminalPunctuation::default()
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
}
