// src/model/symbol.rs

use std::path::PathBuf;
use std::ops::Range;

/// Unique identifier for a symbol in the repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolId {
    /// Absolute path to the source file containing the symbol.
    pub file: PathBuf,
    /// Human‑readable symbol path (e.g., `MyStruct::my_method`).
    pub symbol_path: String,
    /// Optional overload index (e.g., `~2` for second overload).
    pub overload: Option<String>,
}

impl SymbolId {
    /// Generate a stable fold ID string used by the renderer/MCP.
    pub fn to_fold_id(&self) -> String {
        format!("{}#{}", self.file.display(), self.symbol_path)
    }
}

/// Granularity level for emitted symbol detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailLevel {
    /// Full source body (L0)
    Full,
    /// Signature + doc comment + full type definitions (L1)
    SignatureAndTypes,
    /// Signature only (L2)
    SignatureOnly,
    /// Name only (outline) (L3)
    NameOnly,
    /// Omitted – placeholder (L4)
    Omitted,
    // Legacy aliases kept for compatibility
    L0,
    L1,
    L2,
    L3,
    L4,
}

/// Information extracted for a symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    /// Identifier describing file and symbol.
    pub id: SymbolId,
    /// Byte range of the full body (if retained).
    pub body_range: Range<usize>,
    /// Byte range of the signature (or signature‑like region).
    pub signature_range: Range<usize>,
    /// Optional doc‑comment range.
    pub doc_range: Option<Range<usize>>, // currently unused but kept for future extensions
    /// Token count of the body (used for budgeting).
    pub body_tokens: usize,
    /// Token count of the signature.
    pub sig_tokens: usize,
    /// Desired detail level after budgeting.
    pub level: DetailLevel,
}
