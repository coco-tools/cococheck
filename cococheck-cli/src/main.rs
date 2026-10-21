use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use clap::Parser;
use cococheck_core::config::{Config, ConfigError, load_config};
use cococheck_core::git::{CommitInfo, get_commits};
use cococheck_core::parser::{ParseError, parse_conventional_commit};
use cococheck_core::rules::RuleSet;
use log::{LevelFilter, debug, info};
use std::io::{IsTerminal, Write};
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

    /// Start commit (inclusive, default: first commit)
    #[arg(long)]
    from: Option<String>,

    /// End commit (inclusive, default: HEAD)
    #[arg(long)]
    to: Option<String>,

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

/// Check the commits selected by `args`. Return whether all commits passed the checks.
fn run(args: &Cli) -> anyhow::Result<bool> {
    info!("Checking Git repository at: {}", args.input.display());

    let config = load_config_or_default(&args.input, args.config.as_deref())?;
    let commits = collect_commits(args)?;

    let checker = Checker::new(&config);
    let failed_commits = commits
        .iter()
        .filter(|commit| !checker.check(commit))
        .count();

    print_summary(commits.len(), failed_commits);

    Ok(failed_commits == 0)
}

/// Collect the commits in the range selected by `args`, ordered from oldest to newest.
fn collect_commits(args: &Cli) -> anyhow::Result<Vec<CommitInfo>> {
    let range = get_commits(&args.input, args.from.as_deref(), args.to.as_deref())?;

    info!(
        "Using commit range {}..{} ({}..{})",
        args.from.as_deref().unwrap_or("<first commit>"),
        args.to.as_deref().unwrap_or("HEAD"),
        range.from,
        range.to,
    );

    Ok(range.commits)
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

    /// Check the message of the `commit` and report its errors. Return whether the commit passed.
    fn check(&self, commit: &CommitInfo) -> bool {
        let message = &commit.message;

        let parsed = match parse_conventional_commit(message, &self.config.parser) {
            Ok(parsed) => parsed,
            Err(e) => {
                println!("Commit {} failed to parse:\n", commit.id);
                let span = parse_error_span(&e);
                self.print_error(None, &e.to_string(), span, message);
                return false;
            }
        };

        debug!("{:#?}", parsed);

        let rule_errors = self.rule_set.check_commit(&parsed);
        let failed = rule_errors.iter().any(|(_, errors)| !errors.is_empty());

        if failed {
            println!("Commit {} failed:\n", commit.id);
        } else {
            info!("Commit {} passed.", commit.id);
        }

        for (rule_name, errors) in &rule_errors {
            if errors.is_empty() {
                debug!("  Rule '{}' passed", rule_name);
            }

            for e in errors {
                self.print_error(Some(rule_name), &e.message, e.span.clone(), message);
            }
        }

        !failed
    }

    fn print_error(&self, id: Option<&str>, text: &str, span: Option<Range<usize>>, message: &str) {
        println!(
            "{}\n",
            render_error(&self.renderer, id, text, span, message)
        );
    }
}

/// Print how many commits were checked and how many of them failed.
fn print_summary(checked_commits: usize, failed_commits: usize) {
    println!(
        "CoCoCheck finished checking {} commit{}.",
        checked_commits,
        plural(checked_commits)
    );

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

/// Render the error with the `text` as an annotated snippet of the commit `message` that marks the
/// `span`. The `id` is the name of the rule that reported the error, if any.
fn render_error(
    renderer: &Renderer,
    id: Option<&str>,
    text: &str,
    span: Option<Range<usize>>,
    message: &str,
) -> String {
    let title = Level::ERROR.primary_title(text);
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
