// src/model/graph.rs
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::pagerank;
use std::collections::HashMap;
use anyhow::Result;

use crate::model::{
    symbol::{Symbol, SymbolId},
    edge::{Edge, EdgeKind},
};

/// Wrapper around a directed graph of symbols.
pub struct SymbolGraph {
    pub graph: DiGraph<Symbol, Edge>,
    pub idx_map: HashMap<SymbolId, NodeIndex>,
}

impl SymbolGraph {
    /// Build a graph from a list of symbols and edges.
    pub fn new(symbols: Vec<Symbol>, edges: Vec<Edge>) -> Self {
        let mut graph = DiGraph::new();
        let mut idx_map = HashMap::new();

        // Insert symbols as nodes.
        for sym in symbols {
            let idx = graph.add_node(sym.clone());
            idx_map.insert(sym.id.clone(), idx);
        }

        // Insert edges, ignoring missing nodes.
        for edge in edges {
            if let (Some(&src_idx), Some(&dst_idx)) = (
                idx_map.get(&edge.source),
                idx_map.get(&edge.target),
            ) {
                graph.add_edge(src_idx, dst_idx, edge);
            }
        }

        Self { graph, idx_map }
    }

    /// Run personalized PageRank.
    ///
    /// `seeds` maps a `SymbolId` to a seed weight (e.g. focus = 1.0, diff = 0.8, task = 0.5).
    /// If `seeds` is empty the function falls back to a uniform distribution.
    pub fn page_rank(&self, seeds: &HashMap<SymbolId, f32>) -> Result<HashMap<SymbolId, f32>> {
        let mut personalization = vec![0.0_f64; self.graph.node_count()];
        for (sid, &weight) in seeds {
            if let Some(&idx) = self.idx_map.get(sid) {
                personalization[idx.index()] = weight as f64;
            }
        }

        // Uniform distribution when no seeds.
        if personalization.iter().all(|&v| v == 0.0) {
            let uniform = 1.0 / self.graph.node_count() as f64;
            for v in &mut personalization {
                *v = uniform;
            }
        }

        // PageRank parameters: damping 0.85, max 20 iterations.
        let scores = pagerank(&self.graph, 0.85, Some(&personalization), None, 20)?;

        // Convert back to SymbolId -> score.
        let mut result = HashMap::new();
        for (idx, score) in scores.iter().enumerate() {
            let node = self.graph.node_weight(NodeIndex::new(idx)).unwrap();
            result.insert(node.id.clone(), *score as f32);
        }
        Ok(result)
    }
}

