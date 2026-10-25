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
        rule_set.add(&rules.body_length);
        rule_set.add(&rules.body_line_length);
        rule_set.add(&rules.description_case);
        rule_set.add(&rules.footer_length);
        rule_set.add(&rules.footer_line_length);
        rule_set.add(&rules.header_length);
        rule_set.add(&rules.no_description_terminal_punctuation);
        rule_set.add(&rules.no_trailing_whitespace);
        rule_set.add(&rules.required_footers);
        rule_set.add(&rules.scope_case);
        rule_set.add(&rules.scope_presence);
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
                "allowed_scopes",
                "allowed_types",
                "body_length",
                "body_line_length",
                "description_case",
                "footer_length",
                "footer_line_length",
                "no_description_terminal_punctuation",
                "no_trailing_whitespace",
                "required_footers",
                "scope_case",
                "scope_presence",
                "type_case"
            ]
        );
    }

    #[test]
    fn rule_names_are_the_config_keys() {
        let rule_set = RuleSet::new(&Config::default());
        for (rule, _) in &rule_set.rules {
            let yaml = format!("rules: {{ {}: {{ enabled: false }} }}", rule.name());
            let config = parse_config(&yaml).unwrap();

            assert!(
                RuleSet::new(&config)
                    .rules
                    .iter()
                    .all(|(other, _)| other.name() != rule.name()),
                "rule '{}' is not disabled by its config key",
                rule.name()
            );
        }
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
                ("allowed_scopes".to_string(), Severity::Error, 0),
                ("allowed_types".to_string(), Severity::Error, 0),
                ("blank_line_before_footer".to_string(), Severity::Error, 0),
                ("body_length".to_string(), Severity::Error, 0),
                ("body_line_length".to_string(), Severity::Error, 0),
                ("description_case".to_string(), Severity::Error, 0),
                ("footer_length".to_string(), Severity::Error, 0),
                ("footer_line_length".to_string(), Severity::Error, 0),
                ("header_length".to_string(), Severity::Warning, 1),
                (
                    "no_description_terminal_punctuation".to_string(),
                    Severity::Error,
                    1
                ),
                ("no_trailing_whitespace".to_string(), Severity::Error, 0),
                ("required_footers".to_string(), Severity::Error, 0),
                ("scope_case".to_string(), Severity::Error, 0),
                ("scope_presence".to_string(), Severity::Error, 0),
                ("type_case".to_string(), Severity::Error, 0),
            ]
        );
    }
}
