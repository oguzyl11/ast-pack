// ast-pack/src/main.rs
mod cli;
mod walk;
mod tokenizer;
mod parse;
mod model;
mod render;
mod mcp;
mod utils;

use crate::cli::args::Args;
use clap::Parser;

fn main() {
    // Parse CLI arguments
    let args = Args::parse();
    // TODO: wire up the rest of the pipeline.
    println!("ast-pack version {}", env!("CARGO_PKG_VERSION"));
    let _ = args; // silence unused warning
}
