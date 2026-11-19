// src/model/edge.rs
use serde::{Deserialize, Serialize};
use crate::model::symbol::SymbolId;

/// Types of relationships between symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    Call,
    Reference,
    Import,
    Containment,
    TypeUsage,
}

/// Weighted edge used for PageRank.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: SymbolId,
    pub target: SymbolId,
    pub kind: EdgeKind,
    pub weight: f32, // 0.0 – 1.0 (fractional weight for ambiguous matches)
}

