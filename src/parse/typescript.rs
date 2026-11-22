// src/parse/typescript.rs

use anyhow::Result;
use std::path::PathBuf;
use crate::model::symbol::Symbol;

/// Placeholder parser – returns an empty symbol list.
pub fn parse(_path: &PathBuf) -> Result<Vec<Symbol>> {
    // In a full implementation this would parse the file using tree‑sitter‑typescript.
    Ok(Vec::new())
}
