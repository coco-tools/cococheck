use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use anyhow::Context;
use clap::{Parser, ValueEnum};
use cococheck_core::config::{Config, ConfigError, find_config, load_config};
use cococheck_core::git::{get_commits, get_commits_for_revisions, message_cleanup};
use cococheck_core::parser::{ParseError, parse_conventional_commit};
use cococheck_core::rules::{RuleResult, RuleSet, Severity};
use log::{LevelFilter, debug, info};
use serde::Serialize;
use std::io::{IsTerminal, Read, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the Git repository
    #[arg(short, long, default_value = ".")]
    input: PathBuf,

    /// Path to the config file (default: cococheck.yml or .cococheck.yml in the repository, if it
    /// exists, or the same with the .yaml extension)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Check only this commit, or the commits in a range like A..B, as in `git show` (default: all
    /// commits)
    #[arg(value_name = "REVISION_RANGE", conflicts_with_all = ["from", "to"])]
    revisions: Option<String>,

    /// Check only the commits after this one (exclusive, like Git's FROM..TO, default: all commits)
    #[arg(long)]
    from: Option<String>,

    /// End commit (inclusive, default: HEAD)
    #[arg(long)]
    to: Option<String>,

    /// Check only the commit message in this file ("-" for stdin), e.g. in a commit-msg hook
    #[arg(long, value_name = "FILE", conflicts_with_all = ["revisions", "from", "to"])]
    message_file: Option<PathBuf>,

    /// Print the effective configuration as YAML, including the defaults of omitted settings, and
    /// exit without checking commits
    #[arg(long, conflicts_with_all = ["revisions", "from", "to", "message_file"])]
    print_config: bool,

    /// The format of the report on standard output
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// Log informational messages, or also debug messages if given twice
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

/// The format of the report.
#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
enum Format {
    /// Human-readable text with annotated snippets of the commit messages.
    Text,

    /// A single JSON document with the results of all checked commits.
    Json,
}

fn main() -> ExitCode {
    let args = Cli::parse();

    let level = match args.verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        _ => LevelFilter::Debug,
    };

    env_logger::Builder::new()
        .filter_level(level)
        .parse_default_env()
        .format(|buf, record| {
            let level = record.level().as_str().to_lowercase();
            writeln!(buf, "{level}: {}", record.args())
        })
        .init();

    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {:#}", e);
            ExitCode::FAILURE
        }
    }
}

/// Check the commit messages selected by `args`. Return whether all messages passed the checks.
fn run(args: &Cli) -> anyhow::Result<bool> {
    let (config, config_path) = load_config_or_default(&args.input, args.config.as_deref())?;

    if args.print_config {
        print_config(&config, config_path.as_deref())?;
        return Ok(true);
    }

    info!("Checking Git repository at: {}", args.input.display());

    let messages = match &args.message_file {
        Some(path) => vec![read_message_file(&args.input, path)?],
        None => collect_commits(args)?,
    };

    let checker = Checker::new(&config);
    let reports = messages.iter().map(|message| checker.check(message));

    let outcomes = match args.format {
        Format::Text => print_text_report(reports),
        Format::Json => print_json_report(reports)?,
    };

    Ok(!outcomes.contains(&Outcome::Failed))
}

/// Print the errors and warnings of the `reports` as text, as soon as each commit is checked, and a
/// summary at the end. Return the outcomes of the commits.
fn print_text_report<'a>(reports: impl Iterator<Item = Report<'a>>) -> Vec<Outcome> {
    let reporter = TextReporter::new();
    let outcomes: Vec<Outcome> = reports
        .map(|report| {
            reporter.print_report(&report);
            report.outcome
        })
        .collect();

    print_summary(&outcomes);
    outcomes
}

/// Print the `reports` as a single JSON document. Return the outcomes of the commits.
fn print_json_report<'a>(
    reports: impl Iterator<Item = Report<'a>>,
) -> anyhow::Result<Vec<Outcome>> {
    let reports: Vec<Report> = reports.collect();
    let json = serde_json::to_string_pretty(&JsonReport::new(&reports))
        .context("failed to serialize the report as JSON")?;
    println!("{json}");

    Ok(reports.iter().map(|report| report.outcome).collect())
}

/// A commit message to check.
struct Message {
    /// The ID of the commit, or `None` if the message was not read from a commit.
    id: Option<String>,
    /// The name that refers to the message in the report.
    name: String,
    text: String,
}

/// Collect the messages of the commits selected by `args`, ordered from oldest to newest.
fn collect_commits(args: &Cli) -> anyhow::Result<Vec<Message>> {
    let range = match &args.revisions {
        Some(revisions) => get_commits_for_revisions(&args.input, revisions)?,
        None => get_commits(&args.input, args.from.as_deref(), args.to.as_deref())?,
    };

    let to = args.to.as_deref().unwrap_or("HEAD");
    match (&args.revisions, &args.from, &range.from) {
        (Some(revisions), _, Some(from_id)) => {
            info!("Using commit range {revisions} ({from_id}..{})", range.to)
        }
        (Some(revision), _, None) => info!("Using commit {revision} ({})", range.to),
        (None, Some(from), Some(from_id)) => {
            info!("Using commit range {from}..{to} ({from_id}..{})", range.to)
        }
        _ => info!("Using all commits up to {to} ({})", range.to),
    }

    let messages = range
        .commits
        .into_iter()
        .map(|commit| Message {
            name: format!("Commit {}", commit.id),
            id: Some(commit.id),
            text: commit.message,
        })
        .collect();

    Ok(messages)
}

/// Read the commit message from the file at `path`, or from standard input if the path is `-`, and
/// clean it up like Git does, as configured for the repository at `repo_path`.
fn read_message_file(repo_path: &Path, path: &Path) -> anyhow::Result<Message> {
    let mut bytes = Vec::new();
    if path == Path::new("-") {
        std::io::stdin()
            .read_to_end(&mut bytes)
            .context("failed to read the commit message from standard input")?;
    } else {
        bytes = std::fs::read(path)
            .with_context(|| format!("failed to read commit message file '{}'", path.display()))?;
    }

    let cleanup = message_cleanup(repo_path);
    debug!("Cleaning up the commit message with {:?}.", cleanup);

    Ok(Message {
        id: None,
        name: "Commit message".to_string(),
        text: cleanup.clean(&String::from_utf8_lossy(&bytes)),
    })
}

/// The outcome of checking a single commit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    /// The commit was not checked, because its message matches an ignore pattern.
    Ignored,

    /// The commit passed all rules.
    Passed,

    /// The commit violated only rules with the severity `warning`.
    PassedWithWarnings,

    /// The commit could not be parsed, or violated a rule with the severity `error`.
    Failed,
}

/// The result of checking a single commit message.
struct Report<'a> {
    message: &'a Message,
    outcome: Outcome,
    /// The reason why the message could not be parsed, if it could not be parsed.
    parse_error: Option<ParseError>,
    /// The results of all enabled rules. It is empty if the message is ignored or could not be
    /// parsed.
    results: Vec<RuleResult>,
}

/// Checks commit messages with the configured parser and rules.
struct Checker<'a> {
    config: &'a Config,
    rule_set: RuleSet,
}

impl<'a> Checker<'a> {
    fn new(config: &'a Config) -> Self {
        Checker {
            config,
            rule_set: RuleSet::new(config),
        }
    }

    /// Check the `message` and return the outcome with the errors and warnings.
    fn check<'m>(&self, message: &'m Message) -> Report<'m> {
        let report = |outcome, parse_error, results| Report {
            message,
            outcome,
            parse_error,
            results,
        };

        if self.config.is_ignored(&message.text) {
            info!("{} is ignored.", message.name);
            return report(Outcome::Ignored, None, vec![]);
        }

        let parsed = match parse_conventional_commit(&message.text, &self.config.parser) {
            Ok(parsed) => parsed,
            Err(e) => return report(Outcome::Failed, Some(e), vec![]),
        };

        debug!("{:#?}", parsed);

        let results = self.rule_set.check_commit(&parsed);
        let has_violations = |severity| {
            results
                .iter()
                .any(|result| result.severity == severity && !result.errors.is_empty())
        };

        let outcome = if has_violations(Severity::Error) {
            Outcome::Failed
        } else if has_violations(Severity::Warning) {
            Outcome::PassedWithWarnings
        } else {
            info!("{} passed.", message.name);
            Outcome::Passed
        };

        for result in results.iter().filter(|result| result.errors.is_empty()) {
            debug!("  Rule '{}' passed", result.rule_name);
        }

        report(outcome, None, results)
    }
}

/// Prints the errors and warnings of the checked commits as human-readable text.
struct TextReporter {
    renderer: Renderer,
}

impl TextReporter {
    fn new() -> Self {
        let renderer = if std::io::stdout().is_terminal() {
            Renderer::styled()
        } else {
            Renderer::plain()
        };

        TextReporter { renderer }
    }

    /// Print the errors and warnings of a commit, if it has any.
    fn print_report(&self, report: &Report) {
        let name = &report.message.name;
        let message = &report.message.text;

        if let Some(e) = &report.parse_error {
            println!("{} failed to parse:\n", name);
            let span = parse_error_span(e);
            self.print_error(Severity::Error, None, &e.to_string(), span, message);
            return;
        }

        match report.outcome {
            Outcome::Failed => println!("{} failed:\n", name),
            Outcome::PassedWithWarnings => println!("{} passed with warnings:\n", name),
            Outcome::Ignored | Outcome::Passed => return,
        }

        for result in &report.results {
            for e in &result.errors {
                let span = e.span.clone();
                self.print_error(
                    result.severity,
                    Some(&result.rule_name),
                    &e.message,
                    span,
                    message,
                );
            }
        }
    }

    fn print_error(
        &self,
        severity: Severity,
        id: Option<&str>,
        text: &str,
        span: Option<Range<usize>>,
        message: &str,
    ) {
        let rendered = render_error(&self.renderer, severity, id, text, span, message);
        println!("{}\n", rendered);
    }
}

/// The report of all checked commits in the JSON format.
#[derive(Serialize)]
struct JsonReport<'a> {
    commits: Vec<JsonCommit<'a>>,
    summary: Summary,
}

#[derive(Serialize)]
struct JsonCommit<'a> {
    id: Option<&'a str>,
    header: &'a str,
    outcome: Outcome,
    parse_error: Option<JsonError>,
    rules: Vec<JsonRule<'a>>,
}

#[derive(Serialize)]
struct JsonRule<'a> {
    rule: &'a str,
    severity: Severity,
    violations: Vec<JsonError>,
}

#[derive(Serialize)]
struct JsonError {
    message: String,
    /// The byte range in the full commit message that the error refers to.
    span: Option<JsonSpan>,
}

#[derive(Serialize)]
struct JsonSpan {
    start: usize,
    end: usize,
}

impl From<Range<usize>> for JsonSpan {
    fn from(range: Range<usize>) -> Self {
        JsonSpan {
            start: range.start,
            end: range.end,
        }
    }
}

impl<'a> JsonReport<'a> {
    fn new(reports: &'a [Report]) -> Self {
        let commits = reports
            .iter()
            .map(|report| JsonCommit {
                id: report.message.id.as_deref(),
                header: report.message.text.lines().next().unwrap_or(""),
                outcome: report.outcome,
                parse_error: report.parse_error.as_ref().map(|e| JsonError {
                    message: e.to_string(),
                    span: parse_error_span(e).map(JsonSpan::from),
                }),
                rules: report
                    .results
                    .iter()
                    .map(|result| JsonRule {
                        rule: &result.rule_name,
                        severity: result.severity,
                        violations: result
                            .errors
                            .iter()
                            .map(|e| JsonError {
                                message: e.message.clone(),
                                span: e.span.clone().map(JsonSpan::from),
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect();

        let outcomes: Vec<Outcome> = reports.iter().map(|report| report.outcome).collect();
        JsonReport {
            commits,
            summary: Summary::new(&outcomes),
        }
    }
}

/// How many commits were checked, ignored, passed with warnings, and failed.
#[derive(Debug, PartialEq, Serialize)]
struct Summary {
    checked: usize,
    ignored: usize,
    passed_with_warnings: usize,
    failed: usize,
}

impl Summary {
    fn new(outcomes: &[Outcome]) -> Self {
        let count = |outcome| outcomes.iter().filter(|&&o| o == outcome).count();
        let ignored = count(Outcome::Ignored);

        Summary {
            checked: outcomes.len() - ignored,
            ignored,
            passed_with_warnings: count(Outcome::PassedWithWarnings),
            failed: count(Outcome::Failed),
        }
    }
}

/// Print how many commits were checked, and how many of them failed or had warnings, and how many
/// commits were ignored.
fn print_summary(outcomes: &[Outcome]) {
    let Summary {
        checked: checked_commits,
        ignored: ignored_commits,
        passed_with_warnings: warned_commits,
        failed: failed_commits,
    } = Summary::new(outcomes);

    println!(
        "CoCoCheck finished checking {} commit{}.",
        checked_commits,
        plural(checked_commits)
    );

    if ignored_commits > 0 {
        println!(
            "{} commit{} ignored.",
            ignored_commits,
            if ignored_commits == 1 {
                " was"
            } else {
                "s were"
            }
        );
    }

    if warned_commits > 0 {
        println!(
            "{} commit{} passed with warnings.",
            warned_commits,
            plural(warned_commits)
        );
    }

    if failed_commits > 0 {
        println!(
            "{} commit{} failed the checks.",
            failed_commits,
            plural(failed_commits)
        );
    }
}

/// Return the suffix for the plural of a noun for `count` items.
fn plural(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

/// Render the error or warning with the `text` as an annotated snippet of the commit `message` that
/// marks the `span`. The `id` is the name of the rule that reported the error, if any.
fn render_error(
    renderer: &Renderer,
    severity: Severity,
    id: Option<&str>,
    text: &str,
    span: Option<Range<usize>>,
    message: &str,
) -> String {
    let level = match severity {
        Severity::Warning => Level::WARNING,
        Severity::Error => Level::ERROR,
    };
    let title = level.primary_title(text);
    let title = match id {
        Some(id) => title.id(id),
        None => title,
    };
    let group = match span {
        Some(span) => {
            title.element(Snippet::source(message).annotation(AnnotationKind::Primary.span(span)))
        }
        None => Group::with_title(title),
    };

    renderer.render(&[group])
}

/// Return the byte range in the commit message that the parse `error` refers to.
fn parse_error_span(error: &ParseError) -> Option<Range<usize>> {
    match error {
        ParseError::InvalidHeader { span, .. }
        | ParseError::MissingBlankLineAfterHeader { span } => Some(span.clone()),
        _ => None,
    }
}

/// Print the effective `config` as YAML, preceded by a comment that names the file at
/// `config_path` that it was loaded from, or says that the defaults apply if it is `None`. The
/// output is a valid configuration file.
fn print_config(config: &Config, config_path: Option<&Path>) -> anyhow::Result<()> {
    let yaml = config
        .to_yaml()
        .context("failed to serialize the configuration as YAML")?;

    match config_path {
        Some(path) => println!(
            "# The effective configuration, loaded from '{}'.",
            path.display()
        ),
        None => {
            println!(
                "# The effective configuration. No config file was found, so the defaults apply."
            )
        }
    }
    print!("{yaml}");

    Ok(())
}

/// Load the config file at `config_path`, or from the repository at `repo_path` if no path is
/// given. Use the default configuration if no path is given and the repository does not contain a
/// config file. An explicitly given config file that does not exist is an error, and so are
/// multiple config files in the repository. Return the configuration with the path of the file it
/// was loaded from, relative to the repository if it was found there, or `None` if the default
/// configuration is used.
fn load_config_or_default(
    repo_path: &Path,
    config_path: Option<&Path>,
) -> Result<(Config, Option<PathBuf>), ConfigError> {
    if let Some(config_path) = config_path {
        let config = load_config(config_path)?;
        info!("Using config file: {}", config_path.display());
        return Ok((config, Some(config_path.to_path_buf())));
    }

    let Some(config_path) = find_config(repo_path)? else {
        info!("No config file found, using default configuration.");
        return Ok((Config::default(), None));
    };

    let config = load_config(&config_path)?;
    let relative_path = config_path
        .strip_prefix(repo_path)
        .unwrap_or(&config_path)
        .to_path_buf();
    info!("Using config file: {}", relative_path.display());
    Ok((config, Some(relative_path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workspace root, which contains a `cococheck.yml`.
    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    /// The CLI crate directory, which does not contain a `cococheck.yml`.
    fn dir_without_config() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn message(id: Option<&str>, text: &str) -> Message {
        Message {
            id: id.map(str::to_string),
            name: "Commit".to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn format_defaults_to_text() {
        let args = Cli::try_parse_from(["cococheck"]).unwrap();
        assert_eq!(args.format, Format::Text);

        let args = Cli::try_parse_from(["cococheck", "--format", "json"]).unwrap();
        assert_eq!(args.format, Format::Json);
    }

    #[test]
    fn json_report_contains_the_results_of_all_commits() {
        let config = cococheck_core::config::parse_config(
            "rules: { header_length: { severity: warning, max_length: 30 } }",
        )
        .unwrap();
        let messages = [
            message(None, "feat: Add a feature\n"),
            message(Some("b2"), "feat: Add a feature that is long\n"),
            message(Some("c3"), "feat: Add a feature that is too long."),
            message(Some("d4"), "Update stuff"),
            message(Some("e5"), "Merge branch 'feature'"),
        ];
        let checker = Checker::new(&config);
        let reports: Vec<Report> = messages.iter().map(|m| checker.check(m)).collect();

        let mut json = serde_json::to_value(JsonReport::new(&reports)).unwrap();

        // Every enabled rule is reported, but only compare the results of the rules that the
        // messages violate, so that the test does not depend on the set of rules.
        let rule_count = checker.rule_set.rules.len();
        assert_eq!(
            json["commits"][0]["rules"].as_array().unwrap().len(),
            rule_count
        );
        for commit in json["commits"].as_array_mut().unwrap() {
            commit["rules"].as_array_mut().unwrap().retain(|rule| {
                ["header_length", "no_description_terminal_punctuation"]
                    .contains(&rule["rule"].as_str().unwrap())
            });
        }

        let header_length_violation = |actual: usize| {
            serde_json::json!({
                "message": format!(
                    "Header exceeds maximum length of 30 characters (actual: {actual})."
                ),
                "span": { "start": 0, "end": actual }
            })
        };
        assert_eq!(
            json,
            serde_json::json!({
                "commits": [
                    {
                        "id": null,
                        "header": "feat: Add a feature",
                        "outcome": "passed",
                        "parse_error": null,
                        "rules": [
                            { "rule": "header_length", "severity": "warning", "violations": [] },
                            {
                                "rule": "no_description_terminal_punctuation",
                                "severity": "error",
                                "violations": []
                            }
                        ]
                    },
                    {
                        "id": "b2",
                        "header": "feat: Add a feature that is long",
                        "outcome": "passed_with_warnings",
                        "parse_error": null,
                        "rules": [
                            {
                                "rule": "header_length",
                                "severity": "warning",
                                "violations": [header_length_violation(32)]
                            },
                            {
                                "rule": "no_description_terminal_punctuation",
                                "severity": "error",
                                "violations": []
                            }
                        ]
                    },
                    {
                        "id": "c3",
                        "header": "feat: Add a feature that is too long.",
                        "outcome": "failed",
                        "parse_error": null,
                        "rules": [
                            {
                                "rule": "header_length",
                                "severity": "warning",
                                "violations": [header_length_violation(37)]
                            },
                            {
                                "rule": "no_description_terminal_punctuation",
                                "severity": "error",
                                "violations": [{
                                    "message": "Description should not end with terminal punctuation (., !, ?).",
                                    "span": { "start": 36, "end": 37 }
                                }]
                            }
                        ]
                    },
                    {
                        "id": "d4",
                        "header": "Update stuff",
                        "outcome": "failed",
                        "parse_error": {
                            "message": "invalid header: expected ': ' after the type",
                            "span": { "start": 6, "end": 7 }
                        },
                        "rules": []
                    },
                    {
                        "id": "e5",
                        "header": "Merge branch 'feature'",
                        "outcome": "ignored",
                        "parse_error": null,
                        "rules": []
                    }
                ],
                "summary": { "checked": 4, "ignored": 1, "passed_with_warnings": 1, "failed": 2 }
            })
        );
    }

    #[test]
    fn verbose_can_be_given_multiple_times() {
        let args = Cli::try_parse_from(["cococheck"]).unwrap();
        assert_eq!(args.verbose, 0);

        let args = Cli::try_parse_from(["cococheck", "--verbose"]).unwrap();
        assert_eq!(args.verbose, 1);

        let args = Cli::try_parse_from(["cococheck", "-vv"]).unwrap();
        assert_eq!(args.verbose, 2);
    }

    #[test]
    fn revisions_conflict_with_from_to_and_message_file() {
        for args in [
            &["cococheck", "HEAD", "--from", "HEAD~1"][..],
            &["cococheck", "HEAD", "--to", "HEAD"],
            &["cococheck", "HEAD", "--message-file", "-"],
        ] {
            let error = Cli::try_parse_from(args).unwrap_err();

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }

    #[test]
    fn explicit_config_file_is_loaded() {
        let config_path = workspace_root().join("cococheck.yml");

        let result = load_config_or_default(&dir_without_config(), Some(&config_path));

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn missing_explicit_config_file_is_an_error() {
        let config_path = dir_without_config().join("cococheck.yml");

        let result = load_config_or_default(&workspace_root(), Some(&config_path));

        assert!(
            matches!(
                result,
                Err(ConfigError::Io { ref source, .. })
                    if source.kind() == std::io::ErrorKind::NotFound
            ),
            "{result:?}"
        );
    }

    #[test]
    fn implicit_config_file_is_loaded_from_the_repository() {
        let result = load_config_or_default(&workspace_root(), None);

        let (_, path) = result.unwrap();
        assert_eq!(path, Some(PathBuf::from("cococheck.yml")));
    }

    #[test]
    fn missing_implicit_config_file_falls_back_to_the_default() {
        let result = load_config_or_default(&dir_without_config(), None);

        let (_, path) = result.unwrap();
        assert_eq!(path, None);
    }

    #[test]
    fn print_config_conflicts_with_the_commit_selection() {
        for args in [
            &["cococheck", "--print-config", "HEAD"][..],
            &["cococheck", "--print-config", "--from", "HEAD~1"],
            &["cococheck", "--print-config", "--to", "HEAD"],
            &["cococheck", "--print-config", "--message-file", "-"],
        ] {
            let error = Cli::try_parse_from(args).unwrap_err();

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }
}
