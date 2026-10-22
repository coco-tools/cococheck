use crate::config::Config;
use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleConfig, RuleError, Severity};

pub struct RuleSet {
    /// The enabled rules with their configured severities.
    pub rules: Vec<(Box<dyn Rule>, Severity)>,
}

/// The result of checking a commit with a single rule.
#[derive(Debug, PartialEq)]
pub struct RuleResult {
    pub rule_name: String,
    pub severity: Severity,
    /// The violations of the rule. The rule passed if this is empty.
    pub errors: Vec<RuleError>,
}

impl RuleSet {
    pub fn new(config: &Config) -> Self {
        let rules = &config.rules;
        let mut rule_set = RuleSet { rules: vec![] };
        rule_set.add(&rules.allowed_scopes);
        rule_set.add(&rules.allowed_types);
        rule_set.add(&rules.blank_line_before_footer);
        rule_set.add(&rules.body_line_length);
        rule_set.add(&rules.description_case);
        rule_set.add(&rules.header_length);
        rule_set.add(&rules.no_description_terminal_punctuation);
        rule_set.add(&rules.required_footers);
        rule_set.add(&rules.type_case);
        rule_set
    }

    fn add<R: Rule + Clone + 'static>(&mut self, config: &RuleConfig<R>) {
        if config.enabled {
            self.rules
                .push((Box::new(config.rule.clone()), config.severity));
        }
    }

    pub fn check_commit(&self, commit: &ConventionalCommit) -> Vec<RuleResult> {
        self.rules
            .iter()
            .map(|(rule, severity)| RuleResult {
                rule_name: rule.name().to_string(),
                severity: *severity,
                errors: rule.check(commit),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_config;
    use crate::parser::{ParserConfig, parse_conventional_commit};
    use pretty_assertions::assert_eq;

    #[test]
    fn contains_only_enabled_rules() {
        let yaml = r#"
        rules:
          blank_line_before_footer:
            enabled: false
          header_length:
            enabled: false
        "#;
        let rule_set = RuleSet::new(&parse_config(yaml).unwrap());
        let names: Vec<&str> = rule_set.rules.iter().map(|(rule, _)| rule.name()).collect();
        assert_eq!(
            names,
            vec![
                "AllowedScopes",
                "AllowedTypes",
                "BodyLineLength",
                "DescriptionCase",
                "NoDescriptionTerminalPunctuation",
                "RequiredFooters",
                "TypeCase"
            ]
        );
    }

    #[test]
    fn reports_the_configured_severity() {
        let yaml = r#"
        rules:
          header_length:
            severity: warning
            max_length: 10
          no_description_terminal_punctuation:
            severity: error
        "#;
        let rule_set = RuleSet::new(&parse_config(yaml).unwrap());
        let commit =
            parse_conventional_commit("feat: Add a feature.", &ParserConfig::default()).unwrap();

        let severities: Vec<(String, Severity, usize)> = rule_set
            .check_commit(&commit)
            .into_iter()
            .map(|result| (result.rule_name, result.severity, result.errors.len()))
            .collect();
        assert_eq!(
            severities,
            vec![
                ("AllowedScopes".to_string(), Severity::Error, 0),
                ("AllowedTypes".to_string(), Severity::Error, 0),
                ("BlankLineBeforeFooter".to_string(), Severity::Error, 0),
                ("BodyLineLength".to_string(), Severity::Error, 0),
                ("DescriptionCase".to_string(), Severity::Error, 0),
                ("HeaderLength".to_string(), Severity::Warning, 1),
                (
                    "NoDescriptionTerminalPunctuation".to_string(),
                    Severity::Error,
                    1
                ),
                ("RequiredFooters".to_string(), Severity::Error, 0),
                ("TypeCase".to_string(), Severity::Error, 0),
            ]
        );
    }
}
