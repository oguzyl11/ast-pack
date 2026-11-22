use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::PackError;
use crate::parse::parse_source;
use crate::utils::{hash_bytes, line_number, slice};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoldRecord {
    pub file: String,
    pub start: usize,
    pub end: usize,
    pub line_start: usize,
    pub line_end: usize,
    pub file_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FoldIndex {
    version: u32,
    folds: BTreeMap<String, FoldRecord>,
}

pub fn index_path(root: &Path) -> std::path::PathBuf {
    root.join(".ast-pack-cache").join("folds.json")
}

pub fn write_index(root: &Path, folds: BTreeMap<String, FoldRecord>) -> Result<(), PackError> {
    let path = index_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let index = FoldIndex { version: 1, folds };
    let json = serde_json::to_string_pretty(&index)
        .map_err(|err| PackError::msg(format!("fold index: {err}")))?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn read_index(root: &Path) -> Option<BTreeMap<String, FoldRecord>> {
    let text = std::fs::read_to_string(index_path(root)).ok()?;
    let index: FoldIndex = serde_json::from_str(&text).ok()?;
    Some(index.folds)
}

#[derive(Debug, Clone)]
pub struct Unfolded {
    pub text: String,
    pub warnings: Vec<String>,
}

pub fn unfold(root: &Path, ids: &[String]) -> Result<Unfolded, PackError> {
    if ids.is_empty() {
        return Err(PackError::msg("unfold needs at least one fold id"));
    }
    let index = read_index(root);
    let mut text = String::new();
    let mut warnings = Vec::new();
    for id in ids {
        let (file, _) =
            split_fold_id(id).ok_or_else(|| PackError::msg(format!("invalid fold id `{id}`")))?;
        let absolute = root.join(file);
        let bytes = std::fs::read(&absolute)
            .map_err(|_| PackError::msg(format!("could not read {file} for fold `{id}`")))?;
        let source = String::from_utf8(bytes.clone())
            .map_err(|_| PackError::msg(format!("{file} is not valid UTF-8")))?;
        let hash = hash_bytes(&bytes);
        if let Some(record) = index.as_ref().and_then(|map| map.get(id)) {
            if record.file_hash != hash {
                warnings.push(format!(
                    "{id}: file changed since it was folded; showing the current body"
                ));
            }
        }
        let (symbols, _) = parse_source(file, &source)?;
        let Some(sym) = symbols.iter().find(|s| s.id.fold_id() == *id) else {
            return Err(PackError::msg(format!(
                "fold `{id}` was not found in {file}"
            )));
        };
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&format!("# {id}\n"));
        let body = slice(&source, sym.span.start..sym.span.end);
        let mut line = line_number(&source, sym.span.start);
        for (offset, piece) in body.split_inclusive('\n').enumerate() {
            let shown = piece.trim_end_matches(['\n', '\r']);
            let _ = offset;
            text.push_str(&format!("{line:>5}| {shown}\n"));
            if piece.ends_with('\n') {
                line += 1;
            }
        }
    }
    Ok(Unfolded { text, warnings })
}

pub fn split_fold_id(id: &str) -> Option<(&str, &str)> {
    let (file, symbol) = id.split_once('#')?;
    if file.is_empty() || symbol.is_empty() {
        None
    } else {
        Some((file, symbol))
    }
}

pub fn fold_ids_in(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let marker = "⟨fold:";
    let mut rest = text;
    while let Some(start) = rest.find(marker) {
        let after = &rest[start + marker.len()..];
        if let Some(end) = after.find('⟩') {
            let id = &after[..end];
            if split_fold_id(id).is_some() && !ids.iter().any(|e: &String| e == id) {
                ids.push(id.to_string());
            }
            rest = &after[end + '⟩'.len_utf8()..];
        } else {
            break;
        }
    }
    ids
}
