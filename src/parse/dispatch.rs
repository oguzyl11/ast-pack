use std::ops::Range;

use crate::error::PackError;
use crate::model::{Symbol, SymbolId};
use crate::utils::{line_number, slice};

pub mod extract;
pub mod go;
pub mod java;
pub mod javascript;
pub mod python;
pub mod rust;
pub mod typescript;

pub fn language_of(rel: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    Some(match ext {
        "rs" => "rust",
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" => "typescript",
        "tsx" => "tsx",
        "go" => "go",
        "java" => "java",
        _ => return None,
    })
}

pub fn parse_source(rel: &str, source: &str) -> Result<(Vec<Symbol>, Vec<String>), PackError> {
    let extracted = match language_of(rel) {
        Some("rust") => rust::extract(source)?,
        Some("python") => python::extract(source)?,
        Some("javascript") => javascript::extract(source)?,
        Some("typescript") => typescript::extract(source)?,
        Some("tsx") => javascript::extract_typescript(source, true)?,
        Some("go") => go::extract(source)?,
        Some("java") => java::extract(source)?,
        _ => {
            return Ok((Vec::new(), Vec::new()));
        }
    };
    let mut symbols = Vec::new();
    if extracted.drafts.is_empty() && !source.trim().is_empty() {
        let name = std::path::Path::new(rel)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into());
        symbols.push(make_symbol(
            rel,
            &name,
            &name,
            crate::model::SymbolKind::Module,
            0..source.len(),
            source.len(),
            None,
            Vec::new(),
            None,
            None,
            source,
        ));
        return Ok((symbols, extracted.imports));
    }
    for draft in extracted.drafts {
        symbols.push(make_symbol(
            rel,
            &draft.name,
            &draft.symbol_path,
            draft.kind,
            draft.span.clone(),
            draft.sig_end,
            draft.doc,
            draft.refs,
            draft.extra,
            draft.overload,
            source,
        ));
    }
    crate::model::resolve::assign_containers(&mut symbols);
    Ok((symbols, extracted.imports))
}

fn make_symbol(
    rel: &str,
    name: &str,
    path: &str,
    kind: crate::model::SymbolKind,
    span: Range<usize>,
    sig_end: usize,
    doc: Option<String>,
    refs: Vec<String>,
    extra: Option<String>,
    overload: Option<u32>,
    source: &str,
) -> Symbol {
    let end = span.end.min(source.len()).max(span.start.min(source.len()));
    let start = span.start.min(end);
    let line_start = line_number(source, start);
    let line_end = line_number(source, end.saturating_sub(1));
    let _ = slice(source, start..end);
    Symbol {
        id: SymbolId {
            file: rel.replace('\\', "/"),
            symbol_path: path.to_string(),
            overload,
        },
        kind,
        name: name.to_string(),
        span: start..end,
        sig_end: sig_end.clamp(start, end),
        doc,
        refs,
        extra,
        line_start,
        line_end,
        container: None,
        cost: [0; 4],
    }
}

pub fn debug_sexp(rel: &str, source: &str) -> String {
    let language = match language_of(rel) {
        Some("rust") => tree_sitter_rust::LANGUAGE.into(),
        Some("python") => tree_sitter_python::LANGUAGE.into(),
        Some("javascript") => tree_sitter_javascript::LANGUAGE.into(),
        Some("typescript") => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        Some("tsx") => tree_sitter_typescript::LANGUAGE_TSX.into(),
        Some("go") => tree_sitter_go::LANGUAGE.into(),
        Some("java") => tree_sitter_java::LANGUAGE.into(),
        _ => return String::new(),
    };
    match extract::parse_language(source, language) {
        Ok(tree) => tree.root_node().to_sexp(),
        Err(err) => err.to_string(),
    }
}
