// src/model/solver.rs
use anyhow::Result;
use serde::{Serialize, Deserialize};

use std::collections::HashMap;

use crate::model::{
    symbol::{Symbol, SymbolId, DetailLevel},
    edge::Edge,
    graph::SymbolGraph,
};

/// Final packing plan – which detail level each symbol receives.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackPlan {
    /// SymbolId → chosen detail level.
    pub levels: HashMap<SymbolId, DetailLevel>,
    /// Tokens used by the rendered output (after applying the plan).
    pub tokens_used: usize,
    /// Tokens of a full‑dump (useful for `--stats`).
    pub tokens_full: usize,
}

/// Greedy budget solver.
///
/// * `symbols` – all extracted symbols with token counts already calculated.
/// * `edges`   – relationship edges between symbols.
/// * `budget`  – hard token budget (e.g. 16 000).
/// * `seeds`   – optional seed weights for PageRank (focus, diff, task).
pub fn solve_budget(
    symbols: Vec<Symbol>,
    edges: Vec<Edge>,
    budget: usize,
    seeds: HashMap<SymbolId, f32>,
) -> Result<PackPlan> {
    // 1️⃣ Build graph and compute PageRank scores.
    let graph = SymbolGraph::new(symbols.clone(), edges);
    let scores = graph.page_rank(&seeds)?;

    // 2️⃣ Initialise everything at L4 (omitted).
    let mut plan = PackPlan {
        levels: HashMap::new(),
        tokens_used: 0,
        tokens_full: symbols.iter().map(|s| s.body_tokens).sum(),
    };
    for sym in &symbols {
        plan.levels.insert(sym.id.clone(), DetailLevel::Omitted);
    }

    // 3️⃣ Greedy upgrade loop (placeholder – real logic added later).
    //    Compute incremental gain for moving to a finer detail level and
    //    pick the highest gain‑per‑cost until the token budget is exhausted.
    //    For now we simply return the empty‑plan.

    Ok(plan)
}

