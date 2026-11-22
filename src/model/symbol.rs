use std::ops::Range;

use serde::{Deserialize, Serialize};

/// Stable identity of a symbol. Fold ids use a repo-relative path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId {
    pub file: String,
    pub symbol_path: String,
    pub overload: Option<u32>,
}

impl SymbolId {
    pub fn fold_id(&self) -> String {
        match self.overload {
            Some(n) => format!("{}#{}~{n}", self.file, self.symbol_path),
            None => format!("{}#{}", self.file, self.symbol_path),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DetailLevel {
    L0,
    L1,
    L2,
    L3,
    L4,
}

impl DetailLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L0 => "L0",
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
            Self::L4 => "L4",
        }
    }

    pub fn index(self) -> Option<usize> {
        match self {
            Self::L0 => Some(0),
            Self::L1 => Some(1),
            Self::L2 => Some(2),
            Self::L3 => Some(3),
            Self::L4 => None,
        }
    }

    pub fn finer(self) -> Self {
        match self {
            Self::L4 => Self::L3,
            Self::L3 => Self::L2,
            Self::L2 => Self::L1,
            Self::L1 => Self::L0,
            Self::L0 => Self::L0,
        }
    }

    pub fn coarser(self) -> Self {
        match self {
            Self::L0 => Self::L1,
            Self::L1 => Self::L2,
            Self::L2 => Self::L3,
            Self::L3 => Self::L4,
            Self::L4 => Self::L4,
        }
    }

    /// Relative value of showing this level. Used as the numerator of the greedy ratio.
    pub fn value(self) -> f32 {
        match self {
            Self::L0 => 1.0,
            Self::L1 => 0.75,
            Self::L2 => 0.45,
            Self::L3 => 0.15,
            Self::L4 => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Impl,
    TypeAlias,
    Const,
    Module,
}

impl SymbolKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::TypeAlias => "type",
            Self::Const => "const",
            Self::Module => "module",
        }
    }

    /// L1 keeps the whole definition: struct fields, interface members, enums, aliases.
    pub fn is_full_at_l1(self) -> bool {
        matches!(
            self,
            Self::Struct | Self::Enum | Self::Interface | Self::Trait | Self::TypeAlias
        )
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub id: SymbolId,
    pub kind: SymbolKind,
    pub name: String,
    pub span: Range<usize>,
    pub sig_end: usize,
    pub doc: Option<String>,
    pub refs: Vec<String>,
    /// Field declarations kept when a class itself is folded but its fields should remain.
    pub extra: Option<String>,
    pub line_start: usize,
    pub line_end: usize,
    /// Nearest strictly containing symbol, if any.
    pub container: Option<String>,
    /// Token cost of the rendered snippet at L0, L1, L2, L3. L4 costs nothing.
    pub cost: [usize; 4],
}

impl Symbol {
    pub fn cost_of(&self, level: DetailLevel) -> usize {
        level.index().map(|i| self.cost[i]).unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct Decision {
    pub id: String,
    pub level: DetailLevel,
    pub score: f32,
    pub reason: String,
}
