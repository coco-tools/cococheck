use crate::rules::{AllowedTypes, BodyLineLength, HeaderLength};
use serde::Deserialize;
use std::fmt;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub rules: RuleSet,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSet {
    #[serde(default)]
    pub allowed_types: AllowedTypes,

    #[serde(default)]
    pub body_line_length: BodyLineLength,

    #[serde(default)]
    pub header_length: HeaderLength,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_yaml::Error),
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
    serde_yaml::from_str(content).map_err(ConfigError::Parse)
}

pub fn load_config(path: &str) -> Result<Config, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
    parse_config(&content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn read_default_config_for_empty_config() {
        let config = parse_config("").unwrap();
        assert_eq!(config.rules.allowed_types, AllowedTypes::default());
        assert_eq!(config.rules.body_line_length, BodyLineLength::default());
        assert_eq!(config.rules.header_length, HeaderLength::default());
    }

    #[test]
    fn read_default_value_for_empty_rules() {
        let yaml = r#"
        rules: {}
        "#;
        let config = parse_config(yaml).unwrap();
        assert_eq!(config.rules.allowed_types, AllowedTypes::default());
        assert_eq!(config.rules.body_line_length, BodyLineLength::default());
        assert_eq!(config.rules.header_length, HeaderLength::default());
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
}
