mod config;
mod parser;
mod rules;

use crate::config::{Config, load_config};
use crate::parser::{ConventionalCommit, parse_conventional_commit};
use crate::rules::Rule;
use clap::Parser;
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

fn run_rules(commit: &ConventionalCommit, config: &Config) {
    let rules: Vec<Box<dyn Rule>> = vec![
        Box::new(config.rules.body_line_length.clone()) as Box<dyn Rule>,
        Box::new(config.rules.header_length.clone()) as Box<dyn Rule>,
    ]
    .into_iter()
    .filter(|rule| rule.is_enabled())
    .collect();

    for rule in rules {
        match rule.check(commit) {
            Ok(_) => info!("Rule '{}' passed", rule.name()),
            Err(errors) => {
                warn!("Rule '{}' failed:", rule.name());
                for e in errors {
                    match e.span {
                        Some(span) => {
                            warn!("- {} (span: {}..{})", e.message, span.start, span.end)
                        }
                        None => warn!("- {}", e.message),
                    }
                }
            }
        }
    }
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

    info!("Starting CoCoCheck...");
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
            return;
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
            return;
        }
    };

    for commit in &commits {
        let message = commit.message().unwrap_or("<no commit message>");
        info!("Commit: {}", commit.summary().unwrap_or("<no summary>"));
        debug!("ref: {}", commit.id());

        match parse_conventional_commit(message) {
            Ok(parsed) => {
                debug!("{:#?}", parsed);
                run_rules(&parsed, &config);
            }
            Err(e) => warn!("Failed to parse commit message: {}", e),
        }
    }

    info!("CoCoCheck finished checking {} commits.", commits.len());
}
