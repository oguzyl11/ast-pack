use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PackError;
use crate::model::{Symbol, SymbolId, SymbolKind};

#[derive(Serialize, Deserialize)]
struct CachedSymbol {
    file: String,
    symbol_path: String,
    overload: Option<u32>,
    kind: SymbolKind,
    name: String,
    start: usize,
    end: usize,
    sig_end: usize,
    doc: Option<String>,
    refs: Vec<String>,
    extra: Option<String>,
    line_start: usize,
    line_end: usize,
}

#[derive(Serialize, Deserialize)]
struct CacheBlob {
    version: u32,
    hash: String,
    imports: Vec<String>,
    symbols: Vec<CachedSymbol>,
}

pub fn load(root: &Path, hash: &str, rel: &str) -> Option<(Vec<Symbol>, Vec<String>)> {
    let path = root
        .join(".ast-pack-cache")
        .join("parse")
        .join(format!("{hash}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    let blob: CacheBlob = serde_json::from_str(&text).ok()?;
    if blob.version != 1 || blob.hash != hash {
        return None;
    }
    let mut symbols = Vec::new();
    for cached in blob.symbols {
        symbols.push(Symbol {
            id: SymbolId {
                file: rel.replace('\\', "/"),
                symbol_path: cached.symbol_path,
                overload: cached.overload,
            },
            kind: cached.kind,
            name: cached.name,
            span: cached.start..cached.end,
            sig_end: cached.sig_end,
            doc: cached.doc,
            refs: cached.refs,
            extra: cached.extra,
            line_start: cached.line_start,
            line_end: cached.line_end,
            container: None,
            cost: [0; 4],
        });
    }
    crate::model::resolve::assign_containers(&mut symbols);
    Some((symbols, blob.imports))
}

pub fn store(
    root: &Path,
    hash: &str,
    symbols: &[Symbol],
    imports: &[String],
) -> Result<(), PackError> {
    let dir = root.join(".ast-pack-cache").join("parse");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{hash}.json"));
    let symbols = symbols
        .iter()
        .map(|sym| CachedSymbol {
            file: sym.id.file.clone(),
            symbol_path: sym.id.symbol_path.clone(),
            overload: sym.id.overload,
            kind: sym.kind,
            name: sym.name.clone(),
            start: sym.span.start,
            end: sym.span.end,
            sig_end: sym.sig_end,
            doc: sym.doc.clone(),
            refs: sym.refs.clone(),
            extra: sym.extra.clone(),
            line_start: sym.line_start,
            line_end: sym.line_end,
        })
        .collect();
    let blob = CacheBlob {
        version: 1,
        hash: hash.to_string(),
        imports: imports.to_vec(),
        symbols,
    };
    let json =
        serde_json::to_string(&blob).map_err(|err| PackError::msg(format!("cache: {err}")))?;
    let tmp = dir.join(format!("{hash}.tmp"));
    std::fs::write(&tmp, json)?;
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    std::fs::rename(&tmp, &path)?;
    Ok(())
}
