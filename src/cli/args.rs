use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "ast-pack",
    version,
    about = "Fold your codebase. Unfold on demand.",
    override_usage = "ast-pack [PATH] [OPTIONS]\n       ast-pack unfold [--root PATH] <ID>...\n       ast-pack mcp\n       ast-pack setup [--remove]"
)]
pub struct PackArgs {
    /// Directory or source file to pack. Defaults to the current directory.
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: String,

    /// Hard token budget, for example 16000 or 16k. Omit it for an unlimited L1 pack.
    #[arg(long, value_name = "N")]
    pub budget: Option<String>,

    /// Files forced to full source (L0). A directory prefix matches everything under it.
    #[arg(long, value_name = "GLOB")]
    pub focus: Vec<String>,

    /// Symbols that overlap this git diff become full source when the budget allows.
    /// `--diff` alone compares against HEAD.
    #[arg(long, num_args = 0..=1, default_missing_value = "HEAD", value_name = "REF")]
    pub diff: Option<String>,

    /// Boost symbols whose name or doc comment matches these terms.
    #[arg(long, value_name = "TEXT")]
    pub task: Option<String>,

    /// Tokenizer used for the hard budget. `o200k` (default) or `cl100k`.
    #[arg(long, value_name = "TOKENIZER", default_value = "o200k")]
    pub tokenizer: String,

    /// `xml` (default) or `markdown`.
    #[arg(long, value_name = "FORMAT", default_value = "xml")]
    pub format: String,

    /// Write the pack to a file.
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<String>,

    /// Copy the pack to the clipboard. Falls back to stdout when no clipboard is available.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub copy: bool,

    /// Print token totals and the L0–L4 histogram on stderr.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub stats: bool,

    /// Print why each symbol landed on its level.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub explain: bool,
}

#[derive(Debug, Parser)]
pub struct UnfoldArgs {
    /// Repository root the fold ids are relative to.
    #[arg(long, default_value = ".")]
    pub root: String,
    /// Fold ids, for example `src/auth.rs#Session::validate`.
    #[arg(required = true)]
    pub ids: Vec<String>,
}

#[derive(Debug, Parser)]
pub struct SetupArgs {
    /// Restore every file `setup` changed.
    #[arg(long, action = clap::ArgAction::SetTrue)]
    pub remove: bool,
    /// Home directory override. Tests and scripts use this to avoid the real home.
    #[arg(long)]
    pub home: Option<String>,
    /// Project directory override. Defaults to the current directory.
    #[arg(long)]
    pub project: Option<String>,
}
