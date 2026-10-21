use annotate_snippets::{AnnotationKind, Group, Level, Renderer, Snippet};
use clap::Parser;
use cococheck_core::config::{Config, ConfigError, load_config};
use cococheck_core::git::get_commits;
use cococheck_core::parser::parse_conventional_commit;
use cococheck_core::rules::{RuleError, RuleSet};
use log::{LevelFilter, debug, info};
use std::io::{IsTerminal, Write};
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
    let range = get_commits(&args.input, args.from.as_deref(), args.to.as_deref())?;

    info!(
        "Using commit range {}..{} ({}..{})",
        args.from.as_deref().unwrap_or("<first commit>"),
        args.to.as_deref().unwrap_or("HEAD"),
        range.from,
        range.to,
    );

    let commits = range.commits;

    let rule_set = RuleSet::new(&config);
    let renderer = if std::io::stdout().is_terminal() {
        Renderer::styled()
    } else {
        Renderer::plain()
    };
    let mut failed_commits = 0;

    for commit in &commits {
        let message = &commit.message;

        match parse_conventional_commit(message, &config.parser) {
            Ok(parsed) => {
                let rule_errors = rule_set.check_commit(&parsed);
                let failed = rule_errors.iter().any(|(_, errors)| !errors.is_empty());

                if failed {
                    failed_commits += 1;
                    println!("Commit {} failed:\n", commit.id);
                } else {
                    info!("Commit {} passed.", commit.id);
                }

                debug!("{:#?}", parsed);

                for (rule_name, errors) in &rule_errors {
                    if errors.is_empty() {
                        debug!("  Rule '{}' passed", rule_name);
                    } else {
                        for e in errors {
                            println!("{}\n", render_error(&renderer, rule_name, e, message));
                        }
                    }
                }
            }
            Err(e) => {
                failed_commits += 1;
                println!("Commit {} failed to parse: {}", commit.id, e);
            }
        }
    }

    println!(
        "CoCoCheck finished checking {} commit{}.",
        commits.len(),
        if commits.len() == 1 { "" } else { "s" }
    );

    if failed_commits > 0 {
        println!(
            "{} commit{} failed the checks.",
            failed_commits,
            if failed_commits == 1 { "" } else { "s" }
        );
    }

    Ok(failed_commits == 0)
}

/// Render the `error` reported by the rule named `rule_name` as an annotated snippet of the commit
/// `message`.
fn render_error(renderer: &Renderer, rule_name: &str, error: &RuleError, message: &str) -> String {
    let title = Level::ERROR.primary_title(&error.message).id(rule_name);
    let group = match &error.span {
        Some(span) => title.element(
            Snippet::source(message).annotation(AnnotationKind::Primary.span(span.clone())),
        ),
        None => Group::with_title(title),
    };

    renderer.render(&[group])
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
