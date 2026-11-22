use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;

use rayon::prelude::*;

use crate::cache;
use crate::diff;
use crate::error::PackError;
use crate::fold::{self, FoldRecord};
use crate::mode::PackMode;
use crate::model::resolve::build_edges;
use crate::model::solver::{self, SolveRequest};
use crate::model::symbol::{Decision, DetailLevel, Symbol};
use crate::parse::parse_source;
use crate::render::renderer::{self, RenderInput};
use crate::render::Format;
use crate::tokenizer::Tokenizer;
use crate::utils::{focus_matches, hash_bytes, task_terms};
use crate::walk::{self, SourceFile};

pub use crate::mode::PackMode as Mode;

#[derive(Debug, Clone)]
pub struct PackOptions {
    pub root: PathBuf,
    pub budget: Option<usize>,
    pub focus: Vec<String>,
    pub diff: Option<String>,
    pub task: Option<String>,
    pub tokenizer: String,
    pub format: Format,
    pub mode: PackMode,
    pub cache: bool,
    pub write_folds: bool,
    pub compute_full: bool,
    pub progress: bool,
}

impl PackOptions {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            budget: None,
            focus: Vec::new(),
            diff: None,
            task: None,
            tokenizer: "o200k".into(),
            format: Format::Xml,
            mode: PackMode::Auto,
            cache: true,
            write_folds: true,
            compute_full: false,
            progress: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FoldSummary {
    pub id: String,
    pub level: String,
    pub cost: usize,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone)]
pub struct PackOutput {
    pub text: String,
    pub tokens: usize,
    pub tokens_full: Option<usize>,
    pub folds: Vec<FoldSummary>,
    pub decisions: Vec<Decision>,
    pub warnings: Vec<String>,
    pub level_counts: [usize; 5],
}

struct ParsedFile {
    relative: String,
    source: String,
    hash: String,
    symbols: Vec<Symbol>,
    imports: Vec<String>,
}

pub fn pack(options: &PackOptions) -> Result<PackOutput, PackError> {
    if options.progress {
        eprintln!("scanning {}", options.root.display());
    }
    let workspace = walk::load_workspace(&options.root)?;
    if options.progress {
        eprintln!("{} source files", workspace.files.len());
    }
    let tokenizer = Tokenizer::new(&options.tokenizer)?;
    let mut warnings = Vec::new();

    let parsed: Vec<Result<ParsedFile, PackError>> = workspace
        .files
        .par_iter()
        .map(|file| parse_file(&workspace.root, file, options.cache))
        .collect();

    let mut files = Vec::new();
    let mut texts = HashMap::new();
    let mut symbols = Vec::new();
    let mut imports_by_file = HashMap::new();
    let mut hashes = HashMap::new();
    for item in parsed {
        match item {
            Ok(file) => {
                hashes.insert(file.relative.clone(), file.hash.clone());
                texts.insert(file.relative.clone(), file.source.clone());
                imports_by_file.insert(file.relative.clone(), file.imports.clone());
                symbols.extend(file.symbols);
                files.push(file.relative);
            }
            Err(err) => warnings.push(err.to_string()),
        }
    }
    files.sort();
    symbols.sort_by(|a, b| {
        a.id.file
            .cmp(&b.id.file)
            .then(a.span.start.cmp(&b.span.start))
            .then(a.id.fold_id().cmp(&b.id.fold_id()))
    });
    if options.progress {
        eprintln!("{} symbols", symbols.len());
    }

    for sym in &mut symbols {
        let source = texts.get(&sym.id.file).map(String::as_str).unwrap_or("");
        for (index, level) in [
            DetailLevel::L0,
            DetailLevel::L1,
            DetailLevel::L2,
            DetailLevel::L3,
        ]
        .into_iter()
        .enumerate()
        {
            sym.cost[index] =
                tokenizer.count(&renderer::snippet(sym, level, source, options.format));
        }
    }

    let edges = build_edges(&symbols, &imports_by_file);
    let focus = focus_ids(&symbols, &options.focus);
    let diff_ids = match &options.diff {
        Some(git_ref) => diff_ids_for(&workspace.root, &symbols, git_ref)?,
        None => HashSet::new(),
    };
    let terms = options.task.as_deref().map(task_terms).unwrap_or_default();
    let mut seeds = HashMap::new();
    let mut seeded = HashSet::new();
    for sym in &symbols {
        let id = sym.id.fold_id();
        let mut weight = 0.0f32;
        if focus.contains(&id) {
            weight = weight.max(1.0);
        }
        if diff_ids.contains(&id) {
            weight = weight.max(0.8);
        }
        if !terms.is_empty() && task_matches(sym, &terms) {
            weight = weight.max(0.5);
            seeded.insert(id.clone());
        }
        if weight > 0.0 {
            seeds.insert(id, weight);
        }
    }
    let scores = solver::rank_symbols(&symbols, &edges, &seeds);

    let measure = |levels: &HashMap<String, DetailLevel>| {
        let text = renderer::render(RenderInput {
            files: &files,
            texts: &texts,
            symbols: &symbols,
            levels,
            format: options.format,
        });
        tokenizer.count(&text)
    };

    let (levels, decisions) = solver::solve(
        SolveRequest {
            symbols: &symbols,
            scores: &scores,
            budget: options.budget,
            focus: &focus,
            diff: &diff_ids,
            seeded: &seeded,
            mode: options.mode,
        },
        measure,
    )?;

    let text = renderer::render(RenderInput {
        files: &files,
        texts: &texts,
        symbols: &symbols,
        levels: &levels,
        format: options.format,
    });
    let tokens = tokenizer.count(&text);
    if let Some(budget) = options.budget {
        if options.mode == PackMode::Auto && tokens > budget {
            return Err(PackError::BudgetTooSmall {
                needed: tokens,
                budget,
            });
        }
    }

    let tokens_full = if options.compute_full {
        let mut full_levels = HashMap::new();
        for sym in &symbols {
            full_levels.insert(sym.id.fold_id(), DetailLevel::L0);
        }
        solver::swallow(&mut full_levels, &symbols);
        let full = renderer::render(RenderInput {
            files: &files,
            texts: &texts,
            symbols: &symbols,
            levels: &full_levels,
            format: options.format,
        });
        Some(tokenizer.count(&full))
    } else {
        None
    };

    if options.write_folds {
        let mut index = BTreeMap::new();
        for sym in &symbols {
            let Some(hash) = hashes.get(&sym.id.file) else {
                continue;
            };
            index.insert(
                sym.id.fold_id(),
                FoldRecord {
                    file: sym.id.file.clone(),
                    start: sym.span.start,
                    end: sym.span.end,
                    line_start: sym.line_start,
                    line_end: sym.line_end,
                    file_hash: hash.clone(),
                },
            );
        }
        fold::write_index(&workspace.root, index)?;
    }

    let mut folds: Vec<FoldSummary> = symbols
        .iter()
        .map(|sym| FoldSummary {
            id: sym.id.fold_id(),
            level: levels
                .get(&sym.id.fold_id())
                .copied()
                .unwrap_or(DetailLevel::L4)
                .as_str()
                .to_string(),
            cost: sym.cost_of(DetailLevel::L0),
            line_start: sym.line_start,
            line_end: sym.line_end,
        })
        .collect();
    folds.sort_by(|a, b| a.id.cmp(&b.id));
    let level_counts = renderer::count_levels(&symbols, &levels);

    Ok(PackOutput {
        text,
        tokens,
        tokens_full,
        folds,
        decisions,
        warnings,
        level_counts,
    })
}

fn parse_file(
    root: &std::path::Path,
    file: &SourceFile,
    cache_on: bool,
) -> Result<ParsedFile, PackError> {
    let bytes = std::fs::read(&file.absolute)?;
    let source = String::from_utf8(bytes.clone())
        .map_err(|_| PackError::msg(format!("{} is not valid UTF-8", file.relative)))?;
    let hash = hash_bytes(&bytes);
    if cache_on {
        if let Some((symbols, imports)) = cache::load(root, &hash, &file.relative) {
            return Ok(ParsedFile {
                relative: file.relative.clone(),
                source,
                hash,
                symbols,
                imports,
            });
        }
    }
    let (symbols, imports) = parse_source(&file.relative, &source)?;
    if cache_on {
        let _ = cache::store(root, &hash, &symbols, &imports);
    }
    Ok(ParsedFile {
        relative: file.relative.clone(),
        source,
        hash,
        symbols,
        imports,
    })
}

fn focus_ids(symbols: &[Symbol], patterns: &[String]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for sym in symbols {
        if patterns
            .iter()
            .any(|pattern| focus_matches(pattern, &sym.id.file))
        {
            ids.insert(sym.id.fold_id());
        }
    }
    ids
}

fn diff_ids_for(
    root: &std::path::Path,
    symbols: &[Symbol],
    git_ref: &str,
) -> Result<HashSet<String>, PackError> {
    let changed = diff::changed_lines(root, git_ref)?;
    let mut ids = HashSet::new();
    for sym in symbols {
        let Some(ranges) = changed.get(&sym.id.file) else {
            continue;
        };
        if diff::overlaps(sym.line_start, sym.line_end, ranges) {
            ids.insert(sym.id.fold_id());
        }
    }
    Ok(ids)
}

fn task_matches(sym: &Symbol, terms: &[String]) -> bool {
    let doc = sym.doc.as_deref().unwrap_or("");
    let hay = format!("{} {} {doc}", sym.name, sym.id.symbol_path).to_lowercase();
    terms.iter().any(|term| hay.contains(term))
}

pub fn outline(root: &std::path::Path) -> Result<serde_json::Value, PackError> {
    let mut options = PackOptions::new(root);
    options.mode = PackMode::Names;
    options.cache = true;
    options.write_folds = false;
    options.budget = None;
    let packed = pack(&options)?;
    let mut tree = Vec::new();
    for fold in packed.folds {
        tree.push(serde_json::json!({
            "id": fold.id,
            "symbol": fold.id.split_once('#').map(|(_, s)| s).unwrap_or(&fold.id),
            "level": fold.level,
            "token_cost": fold.cost,
            "line_start": fold.line_start,
            "line_end": fold.line_end,
        }));
    }
    Ok(serde_json::json!({ "tree": tree }))
}
