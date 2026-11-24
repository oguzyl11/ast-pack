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
        eprint!(
            "{}",
            format_stats(
                packed.tokens,
                packed.tokens_full,
                options.budget,
                packed.level_counts,
            )
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

/// Stderr block printed by `--stats`.
///
/// Savings use published input prices: Claude Sonnet 4 at $3 / 1M tokens and
/// GPT-4o at $2.50 / 1M tokens.
pub fn format_stats(
    tokens: usize,
    tokens_full: Option<usize>,
    budget: Option<usize>,
    counts: [usize; 5],
) -> String {
    let mut out = format!("tokens: {tokens}");
    if let Some(full) = tokens_full {
        let saved = full.saturating_sub(tokens);
        out.push_str(&format!(" / full {full}\n{}", savings_line(saved)));
    }
    out.push('\n');
    if let Some(budget) = budget {
        out.push_str(&format!("budget: {budget}\n"));
    }
    out.push_str(&format!(
        "levels: L0={} L1={} L2={} L3={} L4={}\n",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    ));
    out
}

/// `est. savings: ~$0.43/req (Claude Sonnet) | ~$0.36/req (GPT-4o)`
pub fn savings_line(saved_tokens: usize) -> String {
    format!(
        "est. savings: ~{}/req (Claude Sonnet) | ~{}/req (GPT-4o)",
        usd_from_saved(saved_tokens, 300),
        usd_from_saved(saved_tokens, 250),
    )
}

/// `cents_per_million` is the model input price in US cents per 1M tokens.
fn usd_from_saved(saved_tokens: usize, cents_per_million: u128) -> String {
    let cents = (saved_tokens as u128 * cents_per_million + 500_000) / 1_000_000;
    format!("${}.{:02}", cents / 100, cents % 100)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn savings_line_uses_sonnet_and_gpt4o_prices() {
        let saved = 146_281usize.saturating_sub(2_633);
        assert_eq!(
            savings_line(saved),
            "est. savings: ~$0.43/req (Claude Sonnet) | ~$0.36/req (GPT-4o)"
        );
        assert_eq!(
            savings_line(1_000_000),
            "est. savings: ~$3.00/req (Claude Sonnet) | ~$2.50/req (GPT-4o)"
        );
        assert_eq!(
            savings_line(0),
            "est. savings: ~$0.00/req (Claude Sonnet) | ~$0.00/req (GPT-4o)"
        );
    }

    #[test]
    fn stats_report_places_savings_under_the_token_line() {
        let report = format_stats(2_633, Some(146_281), Some(4_000), [0, 0, 18, 17, 376]);
        assert!(report.starts_with("tokens: 2633 / full 146281\nest. savings: ~$0.43/req (Claude Sonnet) | ~$0.36/req (GPT-4o)\n"));
        assert!(report.contains("budget: 4000\n"));
        assert!(report.contains("levels: L0=0 L1=0 L2=18 L3=17 L4=376\n"));
    }
}
