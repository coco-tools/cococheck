use crate::parser::ConventionalCommit;
use crate::pattern::Patterns;
use regex::regex;
use serde::de::value::{MapAccessDeserializer, StringDeserializer};
use serde::de::{DeserializeSeed, Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, PartialEq)]
pub struct RuleError {
    pub message: String,
    /// The byte range in the full commit message that the error refers to.
    pub span: Option<std::ops::Range<usize>>,
}

pub trait Rule {
    fn name(&self) -> &str;
    fn check(&self, message: &ConventionalCommit) -> Vec<RuleError>;
}

/// How a violation of a rule is treated.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The violation is reported, but the commit passes the checks.
    Warning,

    /// The violation is reported, and the commit fails the checks.
    #[default]
    Error,
}

/// The configuration of a rule: Whether it is enabled, its severity, the rule with its own options,
/// which are flattened into the same YAML mapping as the `enabled` flag, and the commits that the
/// rule does not check.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuleConfig<R> {
    pub enabled: bool,
    pub severity: Severity,
    #[serde(flatten)]
    pub rule: R,
    /// Regular expressions for the messages that the rule does not check, like the global
    /// `ignores`.
    pub ignores: Patterns,
    /// Regular expressions for the authors of the commits that the rule does not check, like the
    /// global `ignore_authors`.
    pub ignore_authors: Patterns,
}

/// The options that all rules have, besides their own options.
const COMMON_OPTIONS: &[&str] = &["enabled", "severity", "ignores", "ignore_authors"];

fn default_enabled() -> bool {
    true
}

/// Deserialize the configuration of a rule by hand, because serde ignores `deny_unknown_fields` for
/// a flattened field: It would silently drop unknown keys, like a misspelled option of a rule.
/// Instead, take the `COMMON_OPTIONS` from the mapping, pass the options of the rule to the rule,
/// and reject all other keys.
impl<'de, R: Deserialize<'de>> Deserialize<'de> for RuleConfig<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RuleConfigVisitor<R>(std::marker::PhantomData<R>);

        impl<'de, R: Deserialize<'de>> Visitor<'de> for RuleConfigVisitor<R> {
            type Value = RuleConfig<R>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a mapping with the configuration of a rule")
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                let mut common = CommonOptions {
                    map,
                    rule_options: struct_fields::<R>(),
                    enabled: None,
                    severity: None,
                    ignores: None,
                    ignore_authors: None,
                };
                let rule = R::deserialize(MapAccessDeserializer::new(&mut common))?;
                Ok(RuleConfig {
                    enabled: common.enabled.unwrap_or_else(default_enabled),
                    severity: common.severity.unwrap_or_default(),
                    rule,
                    ignores: common.ignores.unwrap_or_default(),
                    ignore_authors: common.ignore_authors.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_map(RuleConfigVisitor(std::marker::PhantomData))
    }
}

/// A view of the mapping with the configuration of a rule that consumes the options common to all
/// rules, and only yields the options of the rule itself.
struct CommonOptions<A> {
    map: A,
    rule_options: &'static [&'static str],
    enabled: Option<bool>,
    severity: Option<Severity>,
    ignores: Option<Patterns>,
    ignore_authors: Option<Patterns>,
}

impl<'de, A: MapAccess<'de>> CommonOptions<A> {
    /// Read the next value as a list of regular expressions, like the global `ignores`, and fail for
    /// an invalid one, describing it as a pattern of the given `kind`. A null value is read as an
    /// empty list.
    fn next_patterns(&mut self, kind: &str) -> Result<Patterns, A::Error> {
        let patterns: Option<Vec<String>> = self.map.next_value()?;
        Patterns::compile(&patterns.unwrap_or_default(), kind).map_err(A::Error::custom)
    }
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for &mut CommonOptions<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        while let Some(key) = self.map.next_key::<String>()? {
            match key.as_str() {
                "enabled" => self.enabled = Some(self.map.next_value()?),
                "severity" => self.severity = Some(self.map.next_value()?),
                "ignores" => self.ignores = Some(self.next_patterns("ignore")?),
                "ignore_authors" => {
                    self.ignore_authors = Some(self.next_patterns("ignore author")?);
                }
                _ if self.rule_options.contains(&key.as_str()) => {
                    return seed.deserialize(StringDeserializer::new(key)).map(Some);
                }
                _ => {
                    let expected = COMMON_OPTIONS
                        .iter()
                        .chain(self.rule_options)
                        .map(|option| format!("`{option}`"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(A::Error::custom(format!(
                        "unknown option `{key}`, expected one of {expected}"
                    )));
                }
            }
        }
        Ok(None)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        self.map.next_value_seed(seed)
    }
}

/// Return the names of the fields of the struct `R`, which serde passes to the deserializer for a
/// derived implementation of [Deserialize]. Return an empty list if `R` is not a struct.
fn struct_fields<'de, R: Deserialize<'de>>() -> &'static [&'static str] {
    /// A deserializer that only records the names of the fields of a struct.
    struct FieldsDeserializer<'a>(&'a mut &'static [&'static str]);

    impl<'de> Deserializer<'de> for FieldsDeserializer<'_> {
        type Error = serde::de::value::Error;

        fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, Self::Error> {
            Err(Self::Error::custom("not a struct"))
        }

        fn deserialize_struct<V: Visitor<'de>>(
            self,
            _: &'static str,
            fields: &'static [&'static str],
            _: V,
        ) -> Result<V::Value, Self::Error> {
            *self.0 = fields;
            Err(Self::Error::custom("only the fields are recorded"))
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
            option unit unit_struct newtype_struct seq tuple tuple_struct map enum identifier
            ignored_any
        }
    }

    let mut fields: &'static [&'static str] = &[];
    let _ = R::deserialize(FieldsDeserializer(&mut fields));
    fields
}

/// Return whether the `line` contains a URL, for the rules that skip such lines, because long URLs
/// cannot be wrapped. Accept anything that starts like a URL, instead of trying to match only
/// valid URLs.
fn contains_url(line: &str) -> bool {
    regex!(r"\bhttps?://\S+").is_match(line)
}

/// Return the byte range of the scope in the full commit message, without the parentheses, or
/// `None` if the commit has no scope. The header has the form `type(scope)`, and it is the first
/// line of the message.
fn scope_span(commit: &ConventionalCommit) -> Option<std::ops::Range<usize>> {
    let start = commit.commit_type.len() + 1;
    commit
        .scope
        .as_ref()
        .map(|scope| start..start + scope.len())
}

impl<R: Default> Default for RuleConfig<R> {
    fn default() -> Self {
        RuleConfig {
            enabled: default_enabled(),
            severity: Severity::default(),
            rule: R::default(),
            ignores: Patterns::default(),
            ignore_authors: Patterns::default(),
        }
    }
}

mod rule_set;
pub use rule_set::{EnabledRule, RuleResult, RuleSet};

mod allowed_scopes;
pub use allowed_scopes::AllowedScopes;

mod allowed_types;
pub use allowed_types::AllowedTypes;

mod blank_line_before_footer;
pub use blank_line_before_footer::BlankLineBeforeFooter;

mod body_length;
pub use body_length::BodyLength;

mod body_line_length;
pub use body_line_length::BodyLineLength;

mod breaking_change;
pub use breaking_change::BreakingChange;

mod description_case;
pub use description_case::{DescriptionCase, DescriptionCaseMode};

mod footer_length;
pub use footer_length::FooterLength;

mod footer_line_length;
pub use footer_line_length::FooterLineLength;

mod header_length;
pub use header_length::HeaderLength;

mod no_description_terminal_punctuation;
pub use no_description_terminal_punctuation::NoDescriptionTerminalPunctuation;

mod no_trailing_whitespace;
pub use no_trailing_whitespace::NoTrailingWhitespace;

mod required_footers;
pub use required_footers::RequiredFooters;

mod scope_presence;
pub use scope_presence::{Presence, ScopePresence};

mod scope_case;
pub use scope_case::{ScopeCase, ScopeCaseMode};

mod type_case;
pub use type_case::TypeCase;
