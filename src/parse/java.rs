// src/parse/java.rs
// Placeholder Java parser implementation.

use anyhow::Result;
use std::path::PathBuf;
use crate::model::symbol::Symbol;

/// Parse a Java file (placeholder) – returns an empty symbol list.
pub fn parse(_path: &PathBuf) -> Result<Vec<Symbol>> {
    // TODO: real Java parsing via tree-sitter.
    Ok(Vec::new())
}
