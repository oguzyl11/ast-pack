// src/parse/java.rs
// Placeholder Java parser implementation.

use anyhow::Result;
use std::path::PathBuf;
use crate::model::symbol::Symbol;
use crate::tokenizer::Tokenizer;

pub fn parse(_path: &PathBuf) -> Result<Vec<Symbol>> {
    // TODO: real Java parsing via tree-sitter.
    Ok(Vec::new())
}

