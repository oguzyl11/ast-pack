pub mod args;

use std::path::PathBuf;

use clap::{CommandFactory, Parser};

use crate::error::PackError;
use crate::fold;
use crate::mode::PackMode;
use crate::pipeline::{self, PackOptions};
use crate::render::Format;
use crate::setup;
use crate::utils::parse_budget;

use args::{PackArgs, SetupArgs, UnfoldArgs};

pub fn execute() -> Result<(), PackError> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("unfold") => {
            args.remove(0);
            unfold_cmd(&args)
        }
        Some("mcp") => crate::mcp::server::run(),
        Some("setup") => {
            args.remove(0);
            setup_cmd(&args)
        }
        Some("-h") | Some("--help") => {
            let mut cmd = PackArgs::command();
            cmd.print_help()
                .map_err(|err| PackError::msg(err.to_string()))?;
            println!();
            Ok(())
        }
        _ => pack_cmd(),
    }
}

fn pack_cmd() -> Result<(), PackError> {
    let args = PackArgs::parse();
    let mut options = PackOptions::new(&args.path);
    options.budget = match &args.budget {
        Some(raw) => Some(parse_budget(raw)?),
        None => None,
    };
    options.focus = args.focus.clone();
    options.diff = args.diff.clone();
    options.task = args.task.clone();
    options.tokenizer = args.tokenizer.clone();
    options.format = Format::parse(&args.format)?;
    options.mode = PackMode::Auto;
    options.compute_full = args.stats;
    options.progress = true;
    let packed = pipeline::pack(&options)?;
    for warning in &packed.warnings {
        eprintln!("warning: {warning}");
    }
    if args.copy {
        match arboard::Clipboard::new().and_then(|mut board| board.set_text(packed.text.clone())) {
            Ok(()) => eprintln!("copied to clipboard"),
            Err(err) => {
                eprintln!("warning: clipboard unavailable ({err}); writing stdout");
                println!("{}", packed.text);
            }
        }
    }
    if let Some(path) = &args.output {
        std::fs::write(path, &packed.text)?;
        eprintln!("wrote {path}");
    }
    if !args.copy && args.output.is_none() {
        println!("{}", packed.text);
    }
    if args.stats {
        let counts = packed.level_counts;
        eprintln!(
            "tokens: {}{}",
            packed.tokens,
            packed
                .tokens_full
                .map(|full| format!(" / full {full}"))
                .unwrap_or_default()
        );
        if let Some(budget) = options.budget {
            eprintln!("budget: {budget}");
        }
        eprintln!(
            "levels: L0={} L1={} L2={} L3={} L4={}",
            counts[0], counts[1], counts[2], counts[3], counts[4]
        );
    }
    if args.explain {
        for decision in &packed.decisions {
            eprintln!(
                "{}  {}  score={:.6}  {}",
                decision.id,
                decision.level.as_str(),
                decision.score,
                decision.reason
            );
        }
    }
    Ok(())
}

fn unfold_cmd(args: &[String]) -> Result<(), PackError> {
    let mut argv = vec!["ast-pack".to_string()];
    argv.extend(args.iter().cloned());
    let parsed = UnfoldArgs::try_parse_from(argv).map_err(|err| {
        err.print().ok();
        PackError::msg("invalid unfold arguments")
    })?;
    let unfolded = fold::unfold(PathBuf::from(&parsed.root).as_path(), &parsed.ids)?;
    for warning in &unfolded.warnings {
        eprintln!("warning: {warning}");
    }
    print!("{}", unfolded.text);
    Ok(())
}

fn setup_cmd(args: &[String]) -> Result<(), PackError> {
    let mut argv = vec!["ast-pack".to_string()];
    argv.extend(args.iter().cloned());
    let parsed = SetupArgs::try_parse_from(argv).map_err(|err| {
        err.print().ok();
        PackError::msg("invalid setup arguments")
    })?;
    let home = match parsed.home {
        Some(path) => PathBuf::from(path),
        None => setup::home_dir()?,
    };
    let project = match parsed.project {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir()?,
    };
    let notes = setup::apply(&home, &project, parsed.remove)?;
    for note in notes {
        println!("{note}");
    }
    Ok(())
}
