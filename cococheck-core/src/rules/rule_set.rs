use crate::config::Config;
use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleConfig, RuleError};

pub struct RuleSet {
    pub rules: Vec<Box<dyn Rule>>,
}

impl RuleSet {
    pub fn new(config: &Config) -> Self {
        let rules = &config.rules;
        let mut rule_set = RuleSet { rules: vec![] };
        rule_set.add(&rules.allowed_types);
        rule_set.add(&rules.blank_line_before_footer);
        rule_set.add(&rules.body_line_length);
        rule_set.add(&rules.header_length);
        rule_set.add(&rules.no_description_terminal_punctuation);
        rule_set.add(&rules.required_footers);
        rule_set
    }

    fn add<R: Rule + Clone + 'static>(&mut self, config: &RuleConfig<R>) {
        if config.enabled {
            self.rules.push(Box::new(config.rule.clone()));
        }
    }

    pub fn check_commit(&self, commit: &ConventionalCommit) -> Vec<(String, Vec<RuleError>)> {
        self.rules
            .iter()
            .map(|rule| (rule.name().to_string(), rule.check(commit)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_config;
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
        let names: Vec<&str> = rule_set.rules.iter().map(|rule| rule.name()).collect();
        assert_eq!(
            names,
            vec![
                "AllowedTypes",
                "BodyLineLength",
                "NoDescriptionTerminalPunctuation",
                "RequiredFooters"
            ]
        );
    }
}
