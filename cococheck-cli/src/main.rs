use clap::Parser;
use cococheck_core::config::{Config, ConfigError, load_config};
use cococheck_core::git::get_commits;
use cococheck_core::parser::parse_conventional_commit;
use cococheck_core::rules::RuleSet;
use log::{LevelFilter, debug, error, info, warn};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the Git repository
    #[arg(short, long, default_value = ".")]
    input: PathBuf,

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
        .format(|_, record| writeln!(std::io::stdout(), "{}", record.args()))
        .init();

    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            error!("Error: {:#}", e);
            ExitCode::FAILURE
        }
    }
}

/// Check the commits selected by `args`. Return whether all commits passed the checks.
fn run(args: &Cli) -> anyhow::Result<bool> {
    warn!("Starting CoCoCheck...");
    info!("Checking Git repository at: {}", args.input.display());

    let config = load_config_or_default(&args.input)?;
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
    let mut failed_commits = 0;

    for commit in &commits {
        let message = &commit.message;

        match parse_conventional_commit(message, &config.parser) {
            Ok(parsed) => {
                let rule_errors = rule_set.check_commit(&parsed);
                let failed = rule_errors.iter().any(|(_, errors)| !errors.is_empty());

                if failed {
                    failed_commits += 1;
                    warn!("Commit {} failed:", commit.id);
                    warn!("\n--- MESSAGE ---");
                    warn!("Message: {}", message);
                    warn!("--- END MESSAGE ---\n");
                } else {
                    info!("Commit {} passed.", commit.id);
                }

                debug!("{:#?}", parsed);

                for (rule_name, errors) in &rule_errors {
                    if errors.is_empty() {
                        debug!("  Rule '{}' passed", rule_name);
                    } else {
                        warn!("  Rule '{}' failed:", rule_name);
                        for e in errors {
                            match &e.span {
                                Some(span) => {
                                    warn!("  - {} (span: {}..{})", e.message, span.start, span.end)
                                }
                                None => warn!("  - {}", e.message),
                            }
                        }
                    }
                }
            }
            Err(e) => {
                failed_commits += 1;
                warn!("Failed to parse commit message: {}", e)
            }
        }
    }

    warn!(
        "CoCoCheck finished checking {} commit{}.",
        commits.len(),
        if commits.len() == 1 { "" } else { "s" }
    );

    if failed_commits > 0 {
        error!(
            "{} commit{} failed the checks.",
            failed_commits,
            if failed_commits == 1 { "" } else { "s" }
        );
    }

    Ok(failed_commits == 0)
}

/// Load the config file from the repository at `repo_path`, or use the default configuration if
/// the repository does not contain one.
fn load_config_or_default(repo_path: &Path) -> Result<Config, ConfigError> {
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
