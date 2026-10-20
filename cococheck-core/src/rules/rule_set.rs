use crate::config::Config;
use crate::parser::ConventionalCommit;
use crate::rules::{Rule, RuleError};

pub struct RuleSet {
    pub rules: Vec<Box<dyn Rule>>,
}

impl RuleSet {
    pub fn new(config: &Config) -> Self {
        let rules: Vec<Box<dyn Rule>> = vec![
            Box::new(config.rules.allowed_types.clone()) as Box<dyn Rule>,
            Box::new(config.rules.blank_line_before_footer.clone()) as Box<dyn Rule>,
            Box::new(config.rules.body_line_length.clone()) as Box<dyn Rule>,
            Box::new(config.rules.header_length.clone()) as Box<dyn Rule>,
            Box::new(config.rules.no_description_terminal_punctuation.clone()) as Box<dyn Rule>,
        ]
        .into_iter()
        .filter(|rule| rule.is_enabled())
        .collect();

        RuleSet { rules }
    }

    pub fn check_commit(&self, commit: &ConventionalCommit) -> Vec<(String, Vec<RuleError>)> {
        self.rules
            .iter()
            .map(|rule| {
                (
                    rule.name().to_string(),
                    rule.check(commit).err().unwrap_or_default(),
                )
            })
            .collect()
    }
}
