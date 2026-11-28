use crate::config::Config;
use crate::parser::ConventionalCommit;
use crate::pattern::Patterns;
use crate::rules::{Rule, RuleConfig, RuleError, Severity};

pub struct RuleSet {
    /// The enabled rules.
    pub rules: Vec<EnabledRule>,
}

/// An enabled rule with its configured severity and the commits that it does not check.
pub struct EnabledRule {
    pub rule: Box<dyn Rule>,
    pub severity: Severity,
    pub ignores: Patterns,
    pub ignore_authors: Patterns,
}

impl EnabledRule {
    /// Return whether the rule does not check the commit with the `message` and the `author`, given
    /// as `Name <email>`, because they match one of the `ignores` or `ignore_authors` of the rule.
    pub fn is_ignored(&self, message: &str, author: Option<&str>) -> bool {
        self.ignores.matches_message(message)
            || author.is_some_and(|author| self.ignore_authors.matches_author(author))
    }
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
        rule_set.add(&rules.breaking_change);
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
            self.rules.push(EnabledRule {
                rule: Box::new(config.rule.clone()),
                severity: config.severity,
                ignores: config.ignores.clone(),
                ignore_authors: config.ignore_authors.clone(),
            });
        }
    }

    /// Return whether any of the rules ignores commits by their author, so that the author of a
    /// commit must be known to check it.
    pub fn ignores_authors(&self) -> bool {
        self.rules
            .iter()
            .any(|rule| !rule.ignore_authors.is_empty())
    }

    /// Check the `commit` by the `author`, given as `Name <email>`, or `None` if it is unknown,
    /// with all rules that do not ignore it. The ignored rules are omitted from the results.
    pub fn check_commit(
        &self,
        commit: &ConventionalCommit,
        author: Option<&str>,
    ) -> Vec<RuleResult> {
        self.rules
            .iter()
            .filter(|rule| !rule.is_ignored(&commit.message, author))
            .map(|rule| RuleResult {
                rule_name: rule.rule.name().to_string(),
                severity: rule.severity,
                errors: rule.rule.check(commit),
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
        let names: Vec<&str> = rule_set.rules.iter().map(|rule| rule.rule.name()).collect();
        assert_eq!(
            names,
            vec![
                "allowed_scopes",
                "allowed_types",
                "body_length",
                "body_line_length",
                "breaking_change",
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
        for rule in rule_set.rules.iter().map(|rule| &rule.rule) {
            let yaml = format!("rules: {{ {}: {{ enabled: false }} }}", rule.name());
            let config = parse_config(&yaml).unwrap();

            assert!(
                RuleSet::new(&config)
                    .rules
                    .iter()
                    .all(|other| other.rule.name() != rule.name()),
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
            .check_commit(&commit, None)
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
                ("breaking_change".to_string(), Severity::Error, 0),
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

    fn checked_rules(rule_set: &RuleSet, message: &str, author: Option<&str>) -> Vec<String> {
        let commit = parse_conventional_commit(message, &ParserConfig::default()).unwrap();
        rule_set
            .check_commit(&commit, author)
            .into_iter()
            .map(|result| result.rule_name)
            .collect()
    }

    #[test]
    fn skips_only_the_rules_that_ignore_a_message() {
        let yaml = r#"
        rules:
          required_footers:
            tokens: [Signed-off-by]
            ignores: ['^chore: Release$']
        "#;
        let rule_set = RuleSet::new(&parse_config(yaml).unwrap());
        let all_rules = checked_rules(&rule_set, "feat: Add a feature", None);

        let checked = checked_rules(&rule_set, "chore: Release\n", None);

        assert!(all_rules.contains(&"required_footers".to_string()));
        let expected: Vec<String> = all_rules
            .into_iter()
            .filter(|rule| rule != "required_footers")
            .collect();
        assert_eq!(checked, expected);
    }

    #[test]
    fn skips_only_the_rules_that_ignore_an_author() {
        let yaml = r#"
        rules:
          required_footers:
            tokens: [Signed-off-by]
            ignore_authors: ['^renovate\[bot\] ']
          header_length:
            ignore_authors: ['@example\.org>$']
        "#;
        let rule_set = RuleSet::new(&parse_config(yaml).unwrap());
        let checks = |author| {
            let rules = checked_rules(&rule_set, "chore: Update a dependency", author);
            (
                rules.contains(&"required_footers".to_string()),
                rules.contains(&"header_length".to_string()),
            )
        };

        assert!(rule_set.ignores_authors());
        assert_eq!(
            checks(Some("renovate[bot] <bot@renovateapp.com>")),
            (false, true)
        );
        assert_eq!(checks(Some("Jane Doe <jane@example.org>")), (true, false));
        assert_eq!(checks(Some("John Doe <john@example.com>")), (true, true));
        assert_eq!(checks(None), (true, true));
    }

    #[test]
    fn ignores_authors_only_if_an_enabled_rule_does() {
        assert!(!RuleSet::new(&Config::default()).ignores_authors());

        let yaml = r#"
        rules:
          header_length:
            enabled: false
            ignore_authors: ['^renovate\[bot\] ']
        "#;
        assert!(!RuleSet::new(&parse_config(yaml).unwrap()).ignores_authors());
    }
}
