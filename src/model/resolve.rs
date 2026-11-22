use std::collections::HashMap;

use super::edge::{Edge, EdgeKind};
use super::symbol::{Symbol, SymbolKind};

const NOISE: &[&str] = &[
    "crate", "self", "super", "std", "java", "lang", "String", "Option", "Result", "Vec", "Some",
    "None", "Ok", "Err", "true", "false", "int", "str", "bool", "void", "any", "i32", "i64", "u32",
    "u64", "usize", "isize", "f32", "f64", "Self", "this", "error", "fmt", "Debug", "Clone",
];

pub fn assign_containers(symbols: &mut [Symbol]) {
    let mut order: Vec<usize> = (0..symbols.len()).collect();
    order.sort_by(|a, b| {
        symbols[*a]
            .span
            .start
            .cmp(&symbols[*b].span.start)
            .then(symbols[*b].span.end.cmp(&symbols[*a].span.end))
            .then(symbols[*a].id.fold_id().cmp(&symbols[*b].id.fold_id()))
    });
    let mut stack: Vec<usize> = Vec::new();
    let mut parent: Vec<Option<String>> = vec![None; symbols.len()];
    for i in order {
        while let Some(&top) = stack.last() {
            if symbols[top].span.end <= symbols[i].span.start {
                stack.pop();
            } else {
                break;
            }
        }
        if let Some(&top) = stack.last() {
            let outer = &symbols[top];
            let inner = &symbols[i];
            let strict = outer.span.start < inner.span.start || outer.span.end > inner.span.end;
            if strict && outer.span.start <= inner.span.start && outer.span.end >= inner.span.end {
                parent[i] = Some(outer.id.fold_id());
            }
        }
        stack.push(i);
    }
    for (i, container) in parent.into_iter().enumerate() {
        symbols[i].container = container;
    }
}

pub fn build_edges(symbols: &[Symbol], imports_by_file: &HashMap<String, Vec<String>>) -> Vec<Edge> {
    let mut by_name: HashMap<&str, Vec<&Symbol>> = HashMap::new();
    for sym in symbols {
        by_name.entry(sym.name.as_str()).or_default().push(sym);
    }

    let mut edges = Vec::new();
    for sym in symbols {
        if let Some(parent) = &sym.container {
            if let Some(source) = symbols.iter().find(|s| s.id.fold_id() == *parent) {
                edges.push(Edge {
                    source: source.id.clone(),
                    target: sym.id.clone(),
                    kind: EdgeKind::Containment,
                    weight: 0.25,
                });
            }
        }
        push_named(
            &mut edges,
            sym,
            &sym.refs,
            &by_name,
            EdgeKind::Call,
            true,
        );
    }

    for (file, imports) in imports_by_file {
        let tops: Vec<&Symbol> = symbols
            .iter()
            .filter(|s| s.id.file == *file && !s.id.symbol_path.contains("::"))
            .collect();
        for name in imports {
            let Some(targets) = by_name.get(name.as_str()) else {
                continue;
            };
            let targets: Vec<&&Symbol> = targets
                .iter()
                .filter(|t| t.id.file != *file)
                .collect();
            if targets.is_empty() || targets.len() > 12 {
                continue;
            }
            let weight = 1.0 / targets.len() as f32;
            for source in &tops {
                for target in &targets {
                    edges.push(Edge {
                        source: source.id.clone(),
                        target: target.id.clone(),
                        kind: EdgeKind::Import,
                        weight,
                    });
                }
            }
        }
    }

    // Type-name hits inside a signature (the bytes before the body).
    for sym in symbols {
        if matches!(sym.kind, SymbolKind::Module) {
            continue;
        }
        let type_names: Vec<String> = sym
            .refs
            .iter()
            .filter_map(|r| r.strip_prefix("type:").map(|s| s.to_string()))
            .collect();
        push_named(&mut edges, sym, &type_names, &by_name, EdgeKind::TypeUsage, false);
    }

    edges.sort_by(|a, b| {
        a.source
            .fold_id()
            .cmp(&b.source.fold_id())
            .then(a.target.fold_id().cmp(&b.target.fold_id()))
            .then(a.kind.cmp(&b.kind))
    });
    edges.dedup_by(|a, b| a.source == b.source && a.target == b.target && a.kind == b.kind);
    edges
}

fn push_named(
    edges: &mut Vec<Edge>,
    source: &Symbol,
    names: &[String],
    by_name: &HashMap<&str, Vec<&Symbol>>,
    kind: EdgeKind,
    skip_type_prefix: bool,
) {
    for name in names {
        if skip_type_prefix && name.starts_with("type:") {
            continue;
        }
        if NOISE.contains(&name.as_str()) {
            continue;
        }
        let Some(targets) = by_name.get(name.as_str()) else {
            continue;
        };
        if targets.is_empty() || targets.len() > 12 {
            continue;
        }
        let weight = 1.0 / targets.len() as f32;
        for target in targets {
            if target.id == source.id {
                continue;
            }
            edges.push(Edge {
                source: source.id.clone(),
                target: target.id.clone(),
                kind,
                weight,
            });
        }
    }
}
