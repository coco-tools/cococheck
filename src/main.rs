mod parser;

use clap::Parser;
use git2::Repository;
use parser::parse_conventional_commit;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the Git repository
    #[arg(short, long, default_value = ".")]
    input: PathBuf,
}

fn main() {
    let args = Cli::parse();

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
                    Ok(parsed) => println!("{:#?}", parsed),
                    Err(e) => eprintln!("Failed to parse commit message: {}", e),
                }
            }
            Err(e) => {
                eprintln!("Error iterating commits: {}", e);
            }
        }
    }
}
