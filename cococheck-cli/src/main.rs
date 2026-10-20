use clap::Parser;
use cococheck_core::config;
use cococheck_core::config::{Config, load_config};
use cococheck_core::parser::parse_conventional_commit;
use cococheck_core::rules::RuleSet;
use git2::{Commit, Oid, Repository, Sort};
use log::{debug, error, info, warn};
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

fn find_first_commit(repo: &Repository) -> Result<Oid, git2::Error> {
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(Sort::REVERSE)?;
    revwalk
        .next()
        .transpose()?
        .ok_or_else(|| git2::Error::from_str("No commits found"))
}

fn commits_between_inclusive(
    repo: &Repository,
    from: Oid,
    to: Oid,
) -> Result<Vec<Commit<'_>>, git2::Error> {
    let from_commit = repo.find_commit(from)?;

    let mut revwalk = repo.revwalk()?;
    revwalk.push(to)?;

    if let Some(parent_oid) = from_commit.parent_ids().next() {
        revwalk.hide(parent_oid)?;
    }

    revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;

    let mut commits = Vec::new();
    for oid in revwalk {
        let commit = repo.find_commit(oid?)?;
        commits.push(commit);
    }

    Ok(commits)
}

fn main() {
    let args = Cli::parse();

    if args.debug {
        unsafe {
            std::env::set_var("RUST_LOG", "debug");
        }
    } else if args.info {
        unsafe {
            std::env::set_var("RUST_LOG", "info");
        }
    } else {
        unsafe {
            std::env::set_var("RUST_LOG", "warn");
        }
    }

    env_logger::Builder::from_default_env()
        .format(|_, record| writeln!(std::io::stdout(), "{}", record.args()))
        .init();

    warn!("Starting CoCoCheck...");
    info!("Checking Git repository at: {}", args.input.display());

    let config_path = args.input.join("cococheck.yml");
    let config = match load_config(config_path.to_str().unwrap()) {
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

    let repo = match Repository::open(&args.input) {
        Ok(repo) => repo,
        Err(e) => {
            error!("Error opening repository: {}", e);
            std::process::exit(1);
        }
    };

    let to_oid = match &args.to {
        Some(s) => match repo.revparse_single(s) {
            Ok(obj) => obj.id(),
            Err(e) => {
                error!(
                    "The specified --to reference '{}' could not be found: {}",
                    s,
                    e.message()
                );
                std::process::exit(1);
            }
        },
        None => repo.head().unwrap().target().unwrap(),
    };
    let from_oid = match &args.from {
        Some(s) => match repo.revparse_single(s) {
            Ok(obj) => obj.id(),
            Err(e) => {
                error!(
                    "The specified --from reference '{}' could not be found: {}",
                    s,
                    e.message()
                );
                std::process::exit(1);
            }
        },
        None => find_first_commit(&repo).expect("No commits in repository"),
    };

    info!(
        "Using commit range {}..{} ({}..{})",
        args.from.as_deref().unwrap_or("<first commit>"),
        args.to.as_deref().unwrap_or("HEAD"),
        from_oid,
        to_oid,
    );

    let commits = match commits_between_inclusive(&repo, from_oid, to_oid) {
        Ok(commits) => {
            if commits.is_empty() {
                error!("The specified commit range is empty (possibly --to is before --from).");
                std::process::exit(1);
            }
            commits
        }
        Err(e) => {
            error!("Error collecting commits: {}", e);
            std::process::exit(1);
        }
    };

    let rule_set = RuleSet::new(&config);
    let mut failed_commits = 0;

    for commit in &commits {
        let message = commit.message().unwrap_or("<no commit message>");

        match parse_conventional_commit(message) {
            Ok(parsed) => {
                let rule_errors = rule_set.check_commit(&parsed);
                let failed = rule_errors.iter().any(|(_, errors)| !errors.is_empty());

                if failed {
                    failed_commits += 1;
                    warn!("Commit {} failed:", commit.id());
                    warn!("\n--- MESSAGE ---");
                    warn!("Message: {}", message);
                    warn!("--- END MESSAGE ---\n");
                } else {
                    info!("Commit {} passed.", commit.id());
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
