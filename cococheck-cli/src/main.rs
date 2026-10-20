use clap::Parser;
use cococheck_core::config;
use cococheck_core::config::{Config, load_config};
use cococheck_core::git::{GitError, get_commits};
use cococheck_core::parser::parse_conventional_commit;
use cococheck_core::rules::RuleSet;
use log::{LevelFilter, debug, error, info, warn};
use std::io::Write;
use std::path::PathBuf;

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

fn main() {
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

    warn!("Starting CoCoCheck...");
    info!("Checking Git repository at: {}", args.input.display());

    let config_path = args.input.join("cococheck.yml");
    let config = match load_config(&config_path) {
        Ok(cfg) => {
            info!(
                "Using config file: {}",
                config_path
                    .strip_prefix(&args.input)
                    .unwrap_or(&config_path)
                    .display()
            );
            cfg
        }
        Err(config::ConfigError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("No config file found, using default configuration.");
            Config::default()
        }
        Err(e) => {
            error!("Error loading config: {}", e);
            std::process::exit(1);
        }
    };

    let range = match get_commits(&args.input, args.from.as_deref(), args.to.as_deref()) {
        Ok(range) => range,
        Err(GitError::OpenRepository(e)) => {
            error!("Error opening repository: {}", e);
            std::process::exit(1);
        }
        Err(GitError::RevisionNotFound {
            name,
            revision,
            source,
        }) => {
            error!(
                "The specified --{} reference '{}' could not be found: {}",
                name,
                revision,
                source.message()
            );
            std::process::exit(1);
        }
        Err(GitError::NoCommits) => {
            error!("The repository does not contain any commits.");
            std::process::exit(1);
        }
        Err(GitError::NotAnAncestor { from, to }) => {
            error!(
                "The specified --from commit '{}' is not an ancestor of the --to commit '{}'.",
                from, to
            );
            std::process::exit(1);
        }
        Err(GitError::Git(e)) => {
            error!("Error collecting commits: {}", e);
            std::process::exit(1);
        }
    };

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
        std::process::exit(1);
    }
}
