// src/cli/args.rs
use clap::Parser;

/// Command‑line arguments for `ast-pack`.
#[derive(Debug, Parser)]
#[command(name = "ast-pack", version, about = "Fold your codebase. Unfold on demand.")]
pub struct Args {
    /// Path to a directory or a single file to process.
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: String,

    /// Hard token budget (e.g. 16000 or 16k).
    #[arg(long, value_name = "N")]
    pub budget: Option<String>,

    /// Glob patterns that force full detail (L0).
    #[arg(long, value_name = "GLOB")]
    pub focus: Vec<String>,

    /// Show diff‑based focus symbols (optionally against a ref).
    #[arg(long, value_name = "REF")]
    pub diff: Option<Option<String>>, // None → use HEAD, Some(None) → no ref supplied

    /// Task description – boost symbols that match these terms.
    #[arg(long, value_name = "TEXT")]
    pub task: Option<String>,

    /// Tokenizer model to use.
    #[arg(long, value_name = "TOKENIZER", default_value = "o200k")]
    pub tokenizer: String,

    /// Output format.
    #[arg(long, value_name = "FORMAT", default_value = "xml")]
    pub format: String,

    /// Copy result to clipboard or write to a file.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<String>,

    /// Write to clipboard instead of a file.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub copy: bool,

    /// Print statistics to stderr.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub stats: bool,

    /// Explain scoring for each symbol.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub explain: bool,
}

