// src/parse/lib.rs
use std::path::PathBuf;
use anyhow::Result;

/// Result of parsing a file – a vector of extracted `Symbol`s.
pub type ParseResult = Vec<crate::model::symbol::Symbol>;

/// Parse a single file according to its language. Returns an empty vec if the
/// language is unsupported or the file cannot be parsed.
pub fn parse_file(path: &PathBuf) -> Result<ParseResult> {
    // Detect language by file extension and dispatch.
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    match ext {
        "rs" => crate::parse::rust::parse(path),
        "ts" | "tsx" => crate::parse::typescript::parse(path),
        "js" | "jsx" => crate::parse::javascript::parse(path),
        "py" => crate::parse::python::parse(path),
        "go" => crate::parse::go::parse(path),
        "java" => crate::parse::java::parse(path),
        _ => Ok(Vec::new()),
    }
}

