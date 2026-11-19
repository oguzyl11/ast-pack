// src/main.rs
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
use std::path::PathBuf;

fn main() {
    // Parse CLI arguments
    let args = Args::parse();

    // If the user wants to run the MCP server, start it and exit.
    if args.mcp {
        mcp::server::run_server().expect("MCP server failed");
        return;
    }

    // Determine the root path to process.
    let root = PathBuf::from(&args.path);

    // 1️⃣ Walk the repository (respect .gitignore, skip binaries etc.)
    let files = walk::walker::walk_repo(&root);

    // 2️⃣ Parse each file to extract symbols (parallelism can be added later).
    let mut all_symbols = Vec::new();
    for file in files {
        if let Ok(mut syms) = parse::lib::parse_file(&file) {
            all_symbols.append(&mut syms);
        }
    }

    // 3️⃣ Build the (empty) edge list – real edge extraction will be added later.
    let edges = Vec::new();

    // 4️⃣ Prepare PageRank seeds based on CLI flags.
    let mut seeds = std::collections::HashMap::new();
    // Focus symbols (full detail) – we treat any file matching the focus globs as a seed.
    for pattern in &args.focus {
        if let Ok(glob_pat) = glob::Pattern::new(pattern) {
            for sym in &all_symbols {
                if glob_pat.matches_path(&sym.id.file) {
                    seeds.insert(sym.id.clone(), 1.0_f32);
                }
            }
        }
    }
    // Diff and task seeds could be added here later.

    // 5️⃣ Resolve the token budget.
    let budget = args.budget.as_deref().and_then(|b| {
        if b.ends_with('k') || b.ends_with('K') {
            b[..b.len() - 1].parse::<usize>().ok().map(|v| v * 1000)
        } else {
            b.parse::<usize>().ok()
        }
    }).unwrap_or(16_000);

    // 6️⃣ Run the budget solver.
    let plan = model::solver::solve_budget(all_symbols.clone(), edges, budget, seeds)
        .expect("Failed to solve budget");

    // 7️⃣ Render the final output.
    let tokenizer = tokenizer::token::Tokenizer::new(&args.tokenizer).expect("Invalid tokenizer");
    let renderer = render::renderer::Renderer::new(tokenizer, &args.format);
    let output = renderer.render(&root, &all_symbols, &plan.levels)
        .expect("Rendering failed");

    // 8️⃣ Output according to --copy / -o flags.
    if args.copy {
        let mut board = arboard::Clipboard::new().expect("Failed to open clipboard");
        board.set_text(output).expect("Failed to copy to clipboard");
        eprintln!("Result copied to clipboard.");
    } else if let Some(file) = args.output {
        std::fs::write(&file, output).expect("Failed to write output file");
        eprintln!("Result written to {}", file);
    } else {
        println!("{}", output);
    }

    // 9️⃣ Print statistics if requested.
    if args.stats {
        eprintln!("Tokens used: {} / {}", plan.tokens_used, plan.tokens_full);
    }
}
