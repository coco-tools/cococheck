use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use anyhow::Context;
use clap::Parser;
use cococheck_core::config::{Config, ConfigError, load_config};
use cococheck_core::git::{get_commits, message_cleanup};
use cococheck_core::parser::{ParseError, parse_conventional_commit};
use cococheck_core::rules::{RuleSet, Severity};
use log::{LevelFilter, debug, info};
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

    /// Path to the config file (default: cococheck.yml in the repository, if it exists)
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Check only the commits after this one (exclusive, like Git's FROM..TO, default: all commits)
    #[arg(long)]
    from: Option<String>,

    /// End commit (inclusive, default: HEAD)
    #[arg(long)]
    to: Option<String>,

    /// Check only the commit message in this file ("-" for stdin), e.g. in a commit-msg hook
    #[arg(long, value_name = "FILE", conflicts_with_all = ["from", "to"])]
    message_file: Option<PathBuf>,

    /// Enable info logging
    #[arg(long)]
    info: bool,

    /// Enable debug logging
    #[arg(long)]
    debug: bool,
}

fn main() -> ExitCode {
    let args = Cli::parse();

    let level = if args.debug {
        LevelFilter::Debug
    } else if args.info {
        LevelFilter::Info
    } else {
        LevelFilter::Warn
    };

    env_logger::Builder::new()
        .filter_level(level)
        .format(|buf, record| writeln!(buf, "{}", record.args()))
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
    info!("Checking Git repository at: {}", args.input.display());

    let config = load_config_or_default(&args.input, args.config.as_deref())?;
    let messages = match &args.message_file {
        Some(path) => vec![read_message_file(&args.input, path)?],
        None => collect_commits(args)?,
    };

    let checker = Checker::new(&config);
    let outcomes: Vec<Outcome> = messages
        .iter()
        .map(|message| checker.check(message))
        .collect();

    print_summary(&outcomes);

    Ok(!outcomes.contains(&Outcome::Failed))
}

/// A commit message to check.
struct Message {
    /// The name that refers to the message in the report.
    name: String,
    text: String,
}

/// Collect the messages of the commits in the range selected by `args`, ordered from oldest to
/// newest.
fn collect_commits(args: &Cli) -> anyhow::Result<Vec<Message>> {
    let range = get_commits(&args.input, args.from.as_deref(), args.to.as_deref())?;

    let to = args.to.as_deref().unwrap_or("HEAD");
    match (&args.from, &range.from) {
        (Some(from), Some(from_id)) => {
            info!("Using commit range {from}..{to} ({from_id}..{})", range.to)
        }
        _ => info!("Using all commits up to {to} ({})", range.to),
    }

    let messages = range
        .commits
        .into_iter()
        .map(|commit| Message {
            name: format!("Commit {}", commit.id),
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
        name: "Commit message".to_string(),
        text: cleanup.clean(&String::from_utf8_lossy(&bytes)),
    })
}

/// The outcome of checking a single commit.
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// Checks commit messages with the configured parser and rules, and reports the failures.
struct Checker<'a> {
    config: &'a Config,
    rule_set: RuleSet,
    renderer: Renderer,
}

impl<'a> Checker<'a> {
    fn new(config: &'a Config) -> Self {
        let renderer = if std::io::stdout().is_terminal() {
            Renderer::styled()
        } else {
            Renderer::plain()
        };

        Checker {
            config,
            rule_set: RuleSet::new(config),
            renderer,
        }
    }

    /// Check the `message` and report its errors and warnings.
    fn check(&self, message: &Message) -> Outcome {
        let name = &message.name;
        let message = &message.text;

        if self.config.is_ignored(message) {
            info!("{} is ignored.", name);
            return Outcome::Ignored;
        }

        let parsed = match parse_conventional_commit(message, &self.config.parser) {
            Ok(parsed) => parsed,
            Err(e) => {
                println!("{} failed to parse:\n", name);
                let span = parse_error_span(&e);
                self.print_error(Severity::Error, None, &e.to_string(), span, message);
                return Outcome::Failed;
            }
        };

        debug!("{:#?}", parsed);

        let results = self.rule_set.check_commit(&parsed);
        let has_violations = |severity| {
            results
                .iter()
                .any(|result| result.severity == severity && !result.errors.is_empty())
        };

        let outcome = if has_violations(Severity::Error) {
            println!("{} failed:\n", name);
            Outcome::Failed
        } else if has_violations(Severity::Warning) {
            println!("{} passed with warnings:\n", name);
            Outcome::PassedWithWarnings
        } else {
            info!("{} passed.", name);
            Outcome::Passed
        };

        for result in &results {
            if result.errors.is_empty() {
                debug!("  Rule '{}' passed", result.rule_name);
            }

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

        outcome
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

/// Print how many commits were checked, and how many of them failed or had warnings, and how many
/// commits were ignored.
fn print_summary(outcomes: &[Outcome]) {
    let count = |outcome| outcomes.iter().filter(|&&o| o == outcome).count();
    let ignored_commits = count(Outcome::Ignored);
    let checked_commits = outcomes.len() - ignored_commits;
    let failed_commits = count(Outcome::Failed);
    let warned_commits = count(Outcome::PassedWithWarnings);

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

/// Load the config file at `config_path`, or from the repository at `repo_path` if no path is
/// given. Use the default configuration if no path is given and the repository does not contain a
/// config file. An explicitly given config file that does not exist is an error.
fn load_config_or_default(
    repo_path: &Path,
    config_path: Option<&Path>,
) -> Result<Config, ConfigError> {
    if let Some(config_path) = config_path {
        let config = load_config(config_path)?;
        info!("Using config file: {}", config_path.display());
        return Ok(config);
    }

    let config_path = repo_path.join("cococheck.yml");
    match load_config(&config_path) {
        Ok(config) => {
            info!(
                "Using config file: {}",
                config_path
                    .strip_prefix(repo_path)
                    .unwrap_or(&config_path)
                    .display()
            );
            Ok(config)
        }
        Err(ConfigError::Io { ref source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            info!("No config file found, using default configuration.");
            Ok(Config::default())
        }
        Err(e) => Err(e),
    }
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

        assert!(result.is_ok(), "{result:?}");
    }

    #[test]
    fn missing_implicit_config_file_falls_back_to_the_default() {
        let result = load_config_or_default(&dir_without_config(), None);

        assert!(result.is_ok(), "{result:?}");
    }
}
