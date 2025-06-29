use clap::Parser;
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
}
