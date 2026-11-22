use crate::model::symbol::Symbol;

use anyhow::Result;
use std::path::PathBuf;


/// Placeholder parser – returns an empty symbol list.
pub fn parse(_path: &PathBuf) -> Result<Vec<Symbol>> {
    // In a full implementation this would parse the file using tree‑sitter‑rust.
    Ok(Vec::new())
}
