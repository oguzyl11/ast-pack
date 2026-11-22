use std::collections::HashMap;

use petgraph::graph::DiGraph;

use super::edge::Edge;

/// Personalized PageRank over the symbol graph.
///
/// Seeds are normalized to sum to 1. An empty seed map falls back to a uniform
/// prior. Dangling nodes (no outgoing weight) redistribute along that prior.
pub struct SymbolGraph {
    graph: DiGraph<String, f32>,
}

impl SymbolGraph {
    pub fn new(ids: &[String], edges: &[Edge]) -> Self {
        let mut graph = DiGraph::new();
        let mut index = HashMap::new();
        for id in ids {
            let idx = graph.add_node(id.clone());
            index.insert(id.clone(), idx);
        }
        let mut merged: HashMap<(usize, usize), f32> = HashMap::new();
        for edge in edges {
            let Some(src) = index.get(&edge.source.fold_id()) else {
                continue;
            };
            let Some(dst) = index.get(&edge.target.fold_id()) else {
                continue;
            };
            if src == dst || edge.weight <= 0.0 {
                continue;
            }
            *merged.entry((src.index(), dst.index())).or_insert(0.0) += edge.weight;
        }
        let mut pairs: Vec<_> = merged.into_iter().collect();
        pairs.sort_by_key(|(k, _)| *k);
        for ((src, dst), weight) in pairs {
            graph.add_edge(
                petgraph::graph::NodeIndex::new(src),
                petgraph::graph::NodeIndex::new(dst),
                weight,
            );
        }
        Self { graph }
    }

    pub fn page_rank(&self, seeds: &HashMap<String, f32>) -> HashMap<String, f32> {
        let n = self.graph.node_count();
        let mut result = HashMap::new();
        if n == 0 {
            return result;
        }
        let mut personal = vec![0.0f64; n];
        let mut seed_sum = 0.0f64;
        for idx in self.graph.node_indices() {
            let id = self.graph.node_weight(idx).unwrap();
            if let Some(weight) = seeds.get(id).copied().filter(|w| *w > 0.0) {
                personal[idx.index()] = f64::from(weight);
                seed_sum += f64::from(weight);
            }
        }
        if seed_sum == 0.0 {
            let uniform = 1.0 / n as f64;
            personal.fill(uniform);
        } else {
            for value in &mut personal {
                *value /= seed_sum;
            }
        }

        let mut outgoing = vec![0.0f64; n];
        let mut incoming: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
        let mut edge_list = Vec::new();
        for edge in self.graph.edge_indices() {
            let (src, dst) = self.graph.edge_endpoints(edge).unwrap();
            let weight = f64::from(*self.graph.edge_weight(edge).unwrap());
            edge_list.push((src.index(), dst.index(), weight));
        }
        edge_list.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        for (src, dst, weight) in edge_list {
            outgoing[src] += weight;
            incoming[dst].push((src, weight));
        }

        const DAMPING: f64 = 0.85;
        const ITERS: usize = 40;
        let mut rank = personal.clone();
        for _ in 0..ITERS {
            let dangling: f64 = (0..n)
                .filter(|&i| outgoing[i] == 0.0)
                .map(|i| rank[i])
                .sum();
            let mut next = vec![0.0f64; n];
            for i in 0..n {
                let mut acc = 0.0;
                for &(src, weight) in &incoming[i] {
                    acc += rank[src] * weight / outgoing[src];
                }
                next[i] = (1.0 - DAMPING) * personal[i] + DAMPING * (acc + dangling * personal[i]);
            }
            rank = next;
        }

        for idx in self.graph.node_indices() {
            let id = self.graph.node_weight(idx).unwrap().clone();
            let quantized = (rank[idx.index()] * 1_000_000.0).round() / 1_000_000.0;
            result.insert(id, quantized as f32);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeKind, SymbolId};

    fn id(name: &str) -> SymbolId {
        SymbolId {
            file: "a.rs".into(),
            symbol_path: name.into(),
            overload: None,
        }
    }

    #[test]
    fn seed_mass_reaches_callees() {
        let ids = vec!["a.rs#main".to_string(), "a.rs#helper".to_string()];
        let edges = vec![Edge {
            source: id("main"),
            target: id("helper"),
            kind: EdgeKind::Call,
            weight: 1.0,
        }];
        let graph = SymbolGraph::new(&ids, &edges);
        let mut seeds = HashMap::new();
        seeds.insert("a.rs#main".to_string(), 1.0);
        let scores = graph.page_rank(&seeds);
        assert!(scores["a.rs#helper"] > 0.0);
        assert!(scores["a.rs#main"] > 0.0);
    }
}
