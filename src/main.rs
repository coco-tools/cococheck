use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use anyhow::Context;
use clap::{Parser, ValueEnum};
use cococheck_core::config::{Config, ConfigError, find_config, load_config};
use cococheck_core::git::{
    author_ident, find_latest_tag, get_commits, get_commits_for_revisions, message_cleanup,
};
use cococheck_core::parser::{ParseError, parse_conventional_commit};
use cococheck_core::rules::{RuleResult, RuleSet, Severity};
use log::{LevelFilter, debug, info, warn};
use serde::Serialize;
use std::io::{IsTerminal, Read, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the Git repository
    #[arg(short, long, value_name = "PATH", default_value = ".")]
    repository: PathBuf,

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

    /// Check only the commits after the latest tag that is reachable from the end commit, or all
    /// commits if there is no such tag
    #[arg(long, conflicts_with_all = ["revisions", "from"])]
    from_latest_tag: bool,

    /// Check only the commit message in this file ("-" for stdin), e.g. in a commit-msg hook
    #[arg(
        long,
        value_name = "FILE",
        conflicts_with_all = ["revisions", "from", "to", "from_latest_tag"]
    )]
    message_file: Option<PathBuf>,

    /// Print the effective configuration as YAML, including the defaults of omitted settings, and
    /// exit without checking commits
    #[arg(
        long,
        conflicts_with_all = ["revisions", "from", "to", "from_latest_tag", "message_file"]
    )]
    print_config: bool,

    /// The format of the report on standard output
    #[arg(long, value_enum, default_value_t = Format::Text)]
    format: Format,

    /// When to color the text report
    #[arg(long, value_enum, value_name = "WHEN", default_value_t = ColorChoice::Auto)]
    color: ColorChoice,

    /// Fail the checks for commits that only violate rules with the severity `warning`
    #[arg(long)]
    deny_warnings: bool,

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

/// When to color the text report.
#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
enum ColorChoice {
    /// Color the report if standard output is a terminal or the `CLICOLOR_FORCE` environment
    /// variable is set, unless the `NO_COLOR` environment variable is set.
    Auto,

    /// Always color the report.
    Always,

    /// Never color the report.
    Never,
}

impl ColorChoice {
    /// Return whether to color the output, using `env` to look up environment variables. For
    /// `auto`, follow the conventions of <https://no-color.org/> and
    /// <https://bixense.com/clicolors/>: `NO_COLOR` disables colors if it is set to a non-empty
    /// value, which takes precedence over `CLICOLOR_FORCE`, which enables colors if it is set to a
    /// value other than `0`. Otherwise, color the output if it is a terminal.
    fn use_color(self, env: impl Fn(&str) -> Option<String>, is_terminal: bool) -> bool {
        match self {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => {
                let is_set = |name, off: &str| env(name).is_some_and(|value| value != off);
                if is_set("NO_COLOR", "") {
                    false
                } else if is_set("CLICOLOR_FORCE", "0") {
                    true
                } else {
                    is_terminal
                }
            }
        }
    }
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

    let result = run(&args);
    if let Err(e) = &result {
        eprintln!("error: {:#}", e);
    }
    exit_code(&result)
}

/// The exit status if any commit failed the checks.
const EXIT_FAILED: u8 = 1;

/// The exit status if an error occurred. clap uses the same status for invalid arguments.
const EXIT_ERROR: u8 = 2;

/// Return the exit status for the `result` of [run]: `0` if all commits passed, [EXIT_FAILED] if
/// any commit failed, and [EXIT_ERROR] if an error occurred, so that a caller can tell failed
/// commits apart from errors.
fn exit_code(result: &anyhow::Result<bool>) -> ExitCode {
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(EXIT_FAILED),
        Err(_) => ExitCode::from(EXIT_ERROR),
    }
}

/// Check the commit messages selected by `args`. Return whether all messages passed the checks.
fn run(args: &Cli) -> anyhow::Result<bool> {
    let (config, config_path) = load_config_or_default(&args.repository, args.config.as_deref())?;

    if args.print_config {
        print_config(&config, config_path.as_deref())?;
        return Ok(true);
    }

    info!("Checking Git repository at: {}", args.repository.display());

    let messages = match &args.message_file {
        Some(path) => vec![read_message_file(&args.repository, path, &config)?],
        None => collect_commits(args)?,
    };

    let checker = Checker::new(&config, args.deny_warnings);
    let reports = messages.iter().map(|message| checker.check(message));

    let outcomes = match args.format {
        Format::Text => {
            let color = args.color.use_color(
                |name| std::env::var(name).ok(),
                std::io::stdout().is_terminal(),
            );
            print_text_report(reports, color)
        }
        Format::Json => print_json_report(reports)?,
    };

    Ok(!outcomes.contains(&Outcome::Failed))
}

/// Print the errors and warnings of the `reports` as text, as soon as each commit is checked, and a
/// summary at the end. Color the report if `color` is `true`. Return the outcomes of the commits.
fn print_text_report<'a>(reports: impl Iterator<Item = Report<'a>>, color: bool) -> Vec<Outcome> {
    let reporter = TextReporter::new(color);
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
    /// Whether the message is the message of a merge commit.
    is_merge: bool,
    /// The author of the commit as `Name <email>`, or `None` if it is unknown.
    author: Option<String>,
}

/// Collect the messages of the commits selected by `args`, ordered from oldest to newest.
fn collect_commits(args: &Cli) -> anyhow::Result<Vec<Message>> {
    let to = args.to.as_deref().unwrap_or("HEAD");

    let from = if args.from_latest_tag {
        let tag = find_latest_tag(&args.repository, args.to.as_deref())?;
        match &tag {
            Some(tag) => info!("Found latest tag {tag}."),
            None => info!("No tag is reachable from {to}, checking all commits."),
        }
        tag
    } else {
        args.from.clone()
    };

    let range = match &args.revisions {
        Some(revisions) => get_commits_for_revisions(&args.repository, revisions)?,
        None => {
            // Resolve a tag as a tag, even if a branch has the same name.
            let from_revision = match (&from, args.from_latest_tag) {
                (Some(tag), true) => Some(format!("refs/tags/{tag}")),
                _ => from.clone(),
            };
            get_commits(
                &args.repository,
                from_revision.as_deref(),
                args.to.as_deref(),
            )?
        }
    };

    match (&args.revisions, &from, &range.from) {
        (Some(revisions), _, Some(from_id)) => {
            info!("Using commit range {revisions} ({from_id}..{})", range.to)
        }
        (Some(revision), _, None) => info!("Using commit {revision} ({})", range.to),
        (None, Some(from), Some(from_id)) => {
            info!("Using commit range {from}..{to} ({from_id}..{})", range.to)
        }
        _ => info!("Using all commits up to {to} ({})", range.to),
    }

    let total = range.commits.len();
    let messages = range
        .commits
        .into_iter()
        .enumerate()
        .map(|(index, commit)| Message {
            name: format!(
                "Commit {}/{total} {}",
                index + 1,
                commit_label(&commit.short_id, &commit.message)
            ),
            id: Some(commit.id),
            text: commit.message,
            is_merge: commit.is_merge,
            author: Some(commit.author),
        })
        .collect();

    Ok(messages)
}

/// The maximum number of characters of a commit title that is shown in the name of a commit.
const MAX_TITLE_CHARS: usize = 72;

/// Return a label that lets a human recognize a commit, made of the `short_id` and the title of the
/// commit `message`, which is cropped to [`MAX_TITLE_CHARS`] characters.
fn commit_label(short_id: &str, message: &str) -> String {
    let title = message.lines().next().unwrap_or_default().trim();
    if title.is_empty() {
        return short_id.to_string();
    }

    let title = match title.char_indices().nth(MAX_TITLE_CHARS) {
        Some(_) => {
            let (end, _) = title.char_indices().nth(MAX_TITLE_CHARS - 1).unwrap();
            format!("{}…", title[..end].trim_end())
        }
        None => title.to_string(),
    };

    format!("{short_id} \"{title}\"")
}

/// Read the commit message from the file at `path`, or from standard input if the path is `-`, and
/// clean it up like Git does, as configured for the repository at `repo_path`. The commit does not
/// exist yet, so if the `config` ignores authors, determine the author like Git does for a new
/// commit.
fn read_message_file(repo_path: &Path, path: &Path, config: &Config) -> anyhow::Result<Message> {
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

    // Running Git is only necessary if the author is needed.
    let author = if config.ignore_authors.is_empty() {
        None
    } else {
        author_ident(repo_path)
            .inspect_err(|e| warn!("{e}, so the commit cannot be ignored by its author"))
            .ok()
    };

    Ok(Message {
        id: None,
        name: "Commit message".to_string(),
        text: cleanup.clean(&String::from_utf8_lossy(&bytes)),
        // The commit does not exist yet, so its parents are unknown.
        is_merge: false,
        author,
    })
}

/// The outcome of checking a single commit.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    /// The commit was not checked, because its message or author matches an ignore pattern, or it
    /// is an ignored merge commit.
    Ignored,

    /// The commit passed all rules.
    Passed,

    /// The commit violated only rules with the severity `warning`.
    PassedWithWarnings,

    /// The commit message matches a forbidden pattern, could not be parsed, violated a rule with the
    /// severity `error`, or violated a rule with the severity `warning` and warnings are denied.
    Failed,
}

/// The result of checking a single commit message.
struct Report<'a> {
    message: &'a Message,
    outcome: Outcome,
    /// The violation of a forbidden pattern, if the message matches one.
    forbidden: Option<Forbidden>,
    /// The reason why the message could not be parsed, if it could not be parsed.
    parse_error: Option<ParseError>,
    /// The results of all enabled rules. It is empty if the message is forbidden, ignored, or could
    /// not be parsed.
    results: Vec<RuleResult>,
}

/// The violation of a forbidden pattern by a commit message.
struct Forbidden {
    message: String,
    /// The byte range of the match in the commit message.
    span: Range<usize>,
}

/// Checks commit messages with the configured parser and rules.
struct Checker<'a> {
    config: &'a Config,
    rule_set: RuleSet,
    /// Whether a commit that only violates rules with the severity `warning` fails.
    deny_warnings: bool,
}

impl<'a> Checker<'a> {
    fn new(config: &'a Config, deny_warnings: bool) -> Self {
        Checker {
            config,
            rule_set: RuleSet::new(config),
            deny_warnings,
        }
    }

    /// Check the `message` and return the outcome with the errors and warnings.
    fn check<'m>(&self, message: &'m Message) -> Report<'m> {
        let report = |outcome, parse_error, results| Report {
            message,
            outcome,
            forbidden: None,
            parse_error,
            results,
        };

        // Forbidden patterns are checked first, so that they also apply to ignored messages, like
        // the messages of fixup commits.
        if let Some(found) = self.config.find_forbidden(&message.text) {
            return Report {
                forbidden: Some(Forbidden {
                    message: format!(
                        "Message matches the forbidden pattern '{}'.",
                        found.pattern.as_str()
                    ),
                    span: found.span,
                }),
                ..report(Outcome::Failed, None, vec![])
            };
        }

        let is_ignored_author = |author: &str| self.config.is_ignored_author(author);
        if self.config.is_ignored(&message.text)
            || (self.config.ignore_merge_commits && message.is_merge)
            || message.author.as_deref().is_some_and(is_ignored_author)
        {
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
            if self.deny_warnings {
                Outcome::Failed
            } else {
                Outcome::PassedWithWarnings
            }
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
    fn new(color: bool) -> Self {
        let renderer = if color {
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

        if let Some(forbidden) = &report.forbidden {
            println!("{} failed:\n", name);
            let span = Some(forbidden.span.clone());
            self.print_error(Severity::Error, None, &forbidden.message, span, message);
            return;
        }

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
    forbidden_pattern: Option<JsonError>,
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
                forbidden_pattern: report.forbidden.as_ref().map(|forbidden| JsonError {
                    message: forbidden.message.clone(),
                    span: Some(JsonSpan::from(forbidden.span.clone())),
                }),
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
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// The source directory, which does not contain a `cococheck.yml`.
    fn dir_without_config() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
    }

    fn message(id: Option<&str>, text: &str) -> Message {
        Message {
            id: id.map(str::to_string),
            name: "Commit".to_string(),
            text: text.to_string(),
            is_merge: false,
            author: None,
        }
    }

    #[test]
    fn commit_label_contains_the_short_id_and_the_title() {
        assert_eq!(
            commit_label("1a2b3c4", "feat: Add a feature\n\nSome body.\n"),
            "1a2b3c4 \"feat: Add a feature\""
        );
    }

    #[test]
    fn commit_label_crops_long_titles() {
        let title = "feat: Add a feature with a title that is much too long to show it in full in the report";
        assert_eq!(
            commit_label("1a2b3c4", title),
            "1a2b3c4 \"feat: Add a feature with a title that is much too long to show it in fu…\""
        );

        let exact = "x".repeat(MAX_TITLE_CHARS);
        assert_eq!(
            commit_label("1a2b3c4", &exact),
            format!("1a2b3c4 \"{exact}\"")
        );
    }

    #[test]
    fn commit_label_crops_titles_at_character_boundaries() {
        let title = "ä".repeat(MAX_TITLE_CHARS + 1);
        let expected = format!("1a2b3c4 \"{}…\"", "ä".repeat(MAX_TITLE_CHARS - 1));
        assert_eq!(commit_label("1a2b3c4", &title), expected);
    }

    #[test]
    fn commit_label_omits_an_empty_title() {
        assert_eq!(commit_label("1a2b3c4", ""), "1a2b3c4");
        assert_eq!(commit_label("1a2b3c4", "  \nbody"), "1a2b3c4");
    }

    #[test]
    fn exit_code_tells_failed_commits_apart_from_errors() {
        assert_eq!(exit_code(&Ok(true)), ExitCode::SUCCESS);
        assert_eq!(exit_code(&Ok(false)), ExitCode::from(1));
        assert_eq!(exit_code(&Err(anyhow::anyhow!("error"))), ExitCode::from(2));
    }

    #[test]
    fn exit_code_for_errors_matches_clap() {
        let error = Cli::try_parse_from(["cococheck", "--unknown"]).unwrap_err();
        assert_eq!(error.exit_code(), i32::from(EXIT_ERROR));
    }

    #[test]
    fn ignores_commits_by_author() {
        let config =
            cococheck_core::config::parse_config(r"ignore_authors: ['^renovate\[bot\] ']").unwrap();
        let checker = Checker::new(&config, false);
        let by = |author: &str| Message {
            author: Some(author.to_string()),
            ..message(Some("a1"), "Update dependency")
        };

        assert_eq!(
            checker
                .check(&by("renovate[bot] <bot@renovateapp.com>"))
                .outcome,
            Outcome::Ignored
        );
        assert_eq!(
            checker.check(&by("Jane Doe <jane@example.org>")).outcome,
            Outcome::Failed
        );
        assert_eq!(
            checker
                .check(&message(Some("a1"), "Update dependency"))
                .outcome,
            Outcome::Failed
        );
    }

    #[test]
    fn ignores_merge_commits_with_any_message() {
        let merge = Message {
            is_merge: true,
            ..message(Some("a1"), "Update stuff")
        };

        let config = Config::default();
        assert_eq!(
            Checker::new(&config, false).check(&merge).outcome,
            Outcome::Ignored
        );

        let config = cococheck_core::config::parse_config("ignore_merge_commits: false").unwrap();
        assert_eq!(
            Checker::new(&config, false).check(&merge).outcome,
            Outcome::Failed
        );

        // Forbidden patterns also apply to ignored merge commits.
        let config = cococheck_core::config::parse_config("forbidden: ['stuff']").unwrap();
        assert_eq!(
            Checker::new(&config, false).check(&merge).outcome,
            Outcome::Failed
        );
    }

    #[test]
    fn format_defaults_to_text() {
        let args = Cli::try_parse_from(["cococheck"]).unwrap();
        assert_eq!(args.format, Format::Text);

        let args = Cli::try_parse_from(["cococheck", "--format", "json"]).unwrap();
        assert_eq!(args.format, Format::Json);
    }

    #[test]
    fn repository_defaults_to_the_current_directory() {
        let args = Cli::try_parse_from(["cococheck"]).unwrap();
        assert_eq!(args.repository, PathBuf::from("."));

        for args in [
            ["cococheck", "-r", "repo"],
            ["cococheck", "--repository", "repo"],
        ] {
            let args = Cli::try_parse_from(args).unwrap();
            assert_eq!(args.repository, PathBuf::from("repo"));
        }
    }

    #[test]
    fn json_report_contains_the_results_of_all_commits() {
        let config = cococheck_core::config::parse_config(
            "{ forbidden: ['^fixup! '], rules: { header_length: { severity: warning, max_length: 30 } } }",
        )
        .unwrap();
        let messages = [
            message(None, "feat: Add a feature\n"),
            message(Some("b2"), "feat: Add a feature that is long\n"),
            message(Some("c3"), "feat: Add a feature that is too long."),
            message(Some("d4"), "Update stuff"),
            message(Some("e5"), "Merge branch 'feature'"),
            message(Some("f6"), "fixup! feat: Add a feature"),
        ];
        let checker = Checker::new(&config, false);
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
                        "forbidden_pattern": null,
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
                        "forbidden_pattern": null,
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
                        "forbidden_pattern": null,
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
                                    "message": "Description should not end with terminal punctuation '.'.",
                                    "span": { "start": 36, "end": 37 }
                                }]
                            }
                        ]
                    },
                    {
                        "id": "d4",
                        "header": "Update stuff",
                        "outcome": "failed",
                        "forbidden_pattern": null,
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
                        "forbidden_pattern": null,
                        "parse_error": null,
                        "rules": []
                    },
                    {
                        "id": "f6",
                        "header": "fixup! feat: Add a feature",
                        "outcome": "failed",
                        "forbidden_pattern": {
                            "message": "Message matches the forbidden pattern '^fixup! '.",
                            "span": { "start": 0, "end": 7 }
                        },
                        "parse_error": null,
                        "rules": []
                    }
                ],
                "summary": { "checked": 5, "ignored": 1, "passed_with_warnings": 1, "failed": 3 }
            })
        );
    }

    #[test]
    fn from_latest_tag_conflicts_with_other_commit_selections() {
        for args in [
            &["cococheck", "--from-latest-tag", "HEAD~1.."][..],
            &["cococheck", "--from-latest-tag", "--from", "HEAD~1"],
            &["cococheck", "--from-latest-tag", "--message-file", "-"],
        ] {
            let error = Cli::try_parse_from(args).unwrap_err();

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }

        let args = Cli::try_parse_from(["cococheck", "--from-latest-tag", "--to", "HEAD"]).unwrap();
        assert!(args.from_latest_tag);
    }

    #[test]
    fn color_defaults_to_auto() {
        let args = Cli::try_parse_from(["cococheck"]).unwrap();
        assert_eq!(args.color, ColorChoice::Auto);

        let args = Cli::try_parse_from(["cococheck", "--color", "never"]).unwrap();
        assert_eq!(args.color, ColorChoice::Never);
    }

    #[test]
    fn color_choice_follows_the_terminal_and_the_environment() {
        let env = |vars: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                vars.iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.to_string())
            }
        };

        for (choice, vars, is_terminal, expected) in [
            (ColorChoice::Auto, &[][..], true, true),
            (ColorChoice::Auto, &[], false, false),
            (ColorChoice::Auto, &[("NO_COLOR", "1")], true, false),
            (ColorChoice::Auto, &[("NO_COLOR", "")], true, true),
            (ColorChoice::Auto, &[("CLICOLOR_FORCE", "1")], false, true),
            (ColorChoice::Auto, &[("CLICOLOR_FORCE", "0")], false, false),
            (
                ColorChoice::Auto,
                &[("NO_COLOR", "1"), ("CLICOLOR_FORCE", "1")],
                false,
                false,
            ),
            (ColorChoice::Always, &[("NO_COLOR", "1")], false, true),
            (ColorChoice::Never, &[("CLICOLOR_FORCE", "1")], true, false),
        ] {
            assert_eq!(
                choice.use_color(env(vars), is_terminal),
                expected,
                "{choice:?} with {vars:?}, terminal: {is_terminal}"
            );
        }
    }

    #[test]
    fn denied_warnings_fail_the_commit() {
        let config = cococheck_core::config::parse_config(
            "rules: { header_length: { severity: warning, max_length: 10 } }",
        )
        .unwrap();
        let too_long = message(None, "feat: Add a feature\n");
        let valid = message(None, "feat: Add\n");

        let checker = Checker::new(&config, false);
        assert_eq!(
            checker.check(&too_long).outcome,
            Outcome::PassedWithWarnings
        );
        assert_eq!(checker.check(&valid).outcome, Outcome::Passed);

        let checker = Checker::new(&config, true);
        assert_eq!(checker.check(&too_long).outcome, Outcome::Failed);
        assert_eq!(checker.check(&valid).outcome, Outcome::Passed);
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
            &["cococheck", "--print-config", "--from-latest-tag"],
            &["cococheck", "--print-config", "--message-file", "-"],
        ] {
            let error = Cli::try_parse_from(args).unwrap_err();

            assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
        }
    }
}
