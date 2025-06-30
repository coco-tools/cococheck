mod parser;
mod rules;

use crate::parser::{ConventionalCommit, parse_conventional_commit};
use crate::rules::Rule;
use clap::Parser;
use git2::Repository;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the Git repository
    #[arg(short, long, default_value = ".")]
    input: PathBuf,
}

fn run_rules(commit: &ConventionalCommit) {
    let rules: Vec<Box<dyn Rule>> = vec![
        Box::new(rules::BodyLineLength::default()) as Box<dyn Rule>,
        Box::new(rules::HeaderLength::default()) as Box<dyn Rule>,
    ]
    .into_iter()
    .filter(|rule| rule.is_enabled())
    .collect();

    for rule in rules {
        match rule.check(commit) {
            Ok(_) => println!("Rule '{}' passed", rule.name()),
            Err(errors) => {
                eprintln!("Rule '{}' failed:", rule.name());
                for e in errors {
                    match e.span {
                        Some(span) => {
                            eprintln!("- {} (span: {}..{})", e.message, span.start, span.end)
                        }
                        None => eprintln!("- {}", e.message),
                    }
                }
            }
        }
    }
}

fn main() {
    let args = Cli::parse();

    println!("Starting CoCoCheck...");
    println!("Analyzing Git repository at: {}", args.input.display());

    let repo = match Repository::open(&args.input) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("Error opening repository: {}", e);
            return;
        }
    };

    let mut revwalk = match repo.revwalk() {
        Ok(walk) => walk,
        Err(e) => {
            eprintln!("Error creating revwalk: {}", e);
            return;
        }
    };

    if let Err(e) = revwalk.push_head() {
        eprintln!("Error pushing head: {}", e);
        return;
    }

    for (_, oid_result) in revwalk.enumerate().take(10) {
        match oid_result {
            Ok(oid) => {
                let commit = match repo.find_commit(oid) {
                    Ok(commit) => commit,
                    Err(e) => {
                        eprintln!("Error finding commit: {}", e);
                        continue;
                    }
                };

                let message = commit.message().unwrap_or("<no commit message>");
                println!("Commit: {}", oid);

                match parse_conventional_commit(message) {
                    Ok(parsed) => {
                        println!("{:#?}", parsed);
                        run_rules(&parsed);
                    }
                    Err(e) => eprintln!("Failed to parse commit message: {}", e),
                }
            }
            Err(e) => {
                eprintln!("Error iterating commits: {}", e);
            }
        }
    }

    println!("CoCoCheck finished.");
}
