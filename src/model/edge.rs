use serde::{Deserialize, Serialize};

use super::symbol::SymbolId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EdgeKind {
    Call,
    Reference,
    Import,
    Containment,
    TypeUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: SymbolId,
    pub target: SymbolId,
    pub kind: EdgeKind,
    pub weight: f32,
}
