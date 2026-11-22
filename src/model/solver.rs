use std::collections::{HashMap, HashSet};

use crate::error::PackError;
use crate::mode::PackMode;
use crate::model::graph::SymbolGraph;
use crate::model::symbol::{Decision, DetailLevel, Symbol};

pub struct SolveRequest<'a> {
    pub symbols: &'a [Symbol],
    pub scores: &'a HashMap<String, f32>,
    pub budget: Option<usize>,
    pub focus: &'a HashSet<String>,
    pub diff: &'a HashSet<String>,
    pub seeded: &'a HashSet<String>,
    pub mode: PackMode,
}

pub fn rank_symbols(
    symbols: &[Symbol],
    edges: &[crate::model::Edge],
    seeds: &HashMap<String, f32>,
) -> HashMap<String, f32> {
    let mut ids: Vec<String> = symbols.iter().map(|s| s.id.fold_id()).collect();
    ids.sort();
    ids.dedup();
    SymbolGraph::new(&ids, edges).page_rank(seeds)
}

pub fn solve(
    req: SolveRequest<'_>,
    mut measure: impl FnMut(&HashMap<String, DetailLevel>) -> usize,
) -> Result<(HashMap<String, DetailLevel>, Vec<Decision>), PackError> {
    let mut levels: HashMap<String, DetailLevel> = HashMap::new();
    let mut reasons: HashMap<String, String> = HashMap::new();
    for sym in req.symbols {
        levels.insert(sym.id.fold_id(), DetailLevel::L4);
        reasons.insert(sym.id.fold_id(), "omitted".to_string());
    }

    match req.mode {
        PackMode::Full => {
            for sym in req.symbols {
                levels.insert(sym.id.fold_id(), DetailLevel::L0);
                reasons.insert(sym.id.fold_id(), "full dump".to_string());
            }
            swallow(&mut levels, req.symbols);
            let decided = decisions(req.symbols, &levels, req.scores, &reasons);
            return Ok((levels, decided));
        }
        PackMode::Names => {
            for sym in req.symbols {
                levels.insert(sym.id.fold_id(), DetailLevel::L3);
                reasons.insert(sym.id.fold_id(), "name map".to_string());
            }
            let decided = decisions(req.symbols, &levels, req.scores, &reasons);
            return Ok((levels, decided));
        }
        PackMode::Auto => {}
    }

    if req.budget.is_none() {
        for sym in req.symbols {
            let id = sym.id.fold_id();
            if req.focus.contains(&id) || req.diff.contains(&id) {
                levels.insert(id.clone(), DetailLevel::L0);
                reasons.insert(
                    id,
                    if req.focus.contains(&sym.id.fold_id()) {
                        "pinned by --focus".to_string()
                    } else {
                        "changed lines (--diff)".to_string()
                    },
                );
            } else {
                levels.insert(id.clone(), DetailLevel::L1);
                reasons.insert(id, "default L1 (no budget)".to_string());
            }
        }
        swallow(&mut levels, req.symbols);
        let decided = decisions(req.symbols, &levels, req.scores, &reasons);
        return Ok((levels, decided));
    }

    let budget = req.budget.unwrap();
    for id in levels.values_mut() {
        *id = DetailLevel::L4;
    }
    let header = measure(&levels);
    if header > budget {
        return Err(PackError::BudgetTooSmall {
            needed: header,
            budget,
        });
    }

    for sym in req.symbols {
        let id = sym.id.fold_id();
        if req.focus.contains(&id) {
            levels.insert(id.clone(), DetailLevel::L0);
            reasons.insert(id, "pinned by --focus".to_string());
        }
    }
    swallow(&mut levels, req.symbols);
    let focus_tokens = measure(&levels);
    if focus_tokens > budget {
        return Err(PackError::FocusExceedsBudget {
            needed: focus_tokens,
            budget,
        });
    }

    // Changed symbols prefer a full body, but unlike --focus they may be folded back.
    let mut diff_ids: Vec<String> = req.diff.iter().cloned().collect();
    diff_ids.sort_by(|a, b| {
        score(req.scores, b)
            .partial_cmp(&score(req.scores, a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(b))
    });
    for id in diff_ids {
        if !levels.contains_key(&id) || req.focus.contains(&id) {
            continue;
        }
        if swallowed_id(&id, &levels, req.symbols) {
            continue;
        }
        let previous = levels[&id];
        levels.insert(id.clone(), DetailLevel::L0);
        swallow(&mut levels, req.symbols);
        if measure(&levels) > budget {
            levels.insert(id.clone(), previous);
            swallow(&mut levels, req.symbols);
        } else {
            reasons.insert(id, "changed lines (--diff)".to_string());
        }
    }

    // Greedy upgrades by score gain per extra token, using the real renderer as the meter
    // only for the final fit. Intermediate choices use snippet costs so large repos stay fast.
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > req.symbols.len().saturating_mul(4).saturating_add(8) {
            break;
        }
        let used = estimate(&levels, req.symbols, header);
        let remaining = budget.saturating_sub(used.min(budget));
        let mut best: Option<(String, f32)> = None;
        for sym in req.symbols {
            let id = sym.id.fold_id();
            if req.focus.contains(&id) || swallowed_id(&id, &levels, req.symbols) {
                continue;
            }
            let current = levels[&id];
            if current == DetailLevel::L0 {
                continue;
            }
            let next = current.finer();
            let extra = sym.cost_of(next).saturating_sub(sym.cost_of(current));
            if extra > remaining && extra > 0 {
                continue;
            }
            let gain = score(req.scores, &id) * (next.value() - current.value());
            let ratio = gain / (extra as f32).max(1.0);
            let replace = match &best {
                None => true,
                Some((bid, br)) => ratio > *br + 1e-9 || ((ratio - br).abs() <= 1e-9 && id < *bid),
            };
            if replace && gain > 0.0 {
                best = Some((id, ratio));
            }
        }
        let Some((id, _)) = best else {
            break;
        };
        let current = levels[&id];
        levels.insert(id.clone(), current.finer());
        let reason = if req.seeded.contains(&id) && !req.diff.contains(&id) {
            "task term match"
        } else {
            "score / token cost"
        };
        reasons.insert(id, reason.to_string());
        swallow(&mut levels, req.symbols);
    }

    fit_budget(
        &mut levels,
        req.symbols,
        req.focus,
        budget,
        &mut measure,
        &mut reasons,
    )?;
    let decided = decisions(req.symbols, &levels, req.scores, &reasons);
    Ok((levels, decided))
}

fn fit_budget(
    levels: &mut HashMap<String, DetailLevel>,
    symbols: &[Symbol],
    focus: &HashSet<String>,
    budget: usize,
    measure: &mut impl FnMut(&HashMap<String, DetailLevel>) -> usize,
    reasons: &mut HashMap<String, String>,
) -> Result<(), PackError> {
    let mut guard = 0;
    loop {
        let used = measure(levels);
        if used <= budget {
            return Ok(());
        }
        guard += 1;
        if guard > 48 {
            return Err(PackError::BudgetTooSmall {
                needed: used,
                budget,
            });
        }
        let overflow = used - budget;
        let mut order: Vec<&Symbol> = symbols
            .iter()
            .filter(|s| {
                let id = s.id.fold_id();
                !focus.contains(&id)
                    && levels.get(&id).copied().unwrap_or(DetailLevel::L4) != DetailLevel::L4
                    && !swallowed_id(&id, levels, symbols)
            })
            .collect();
        order.sort_by(|a, b| {
            priority(a, levels)
                .partial_cmp(&priority(b, levels))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.id.fold_id().cmp(&a.id.fold_id()))
        });
        let mut saved = 0usize;
        let mut progressed = false;
        for sym in order {
            let id = sym.id.fold_id();
            let current = levels[&id];
            if current == DetailLevel::L4 {
                continue;
            }
            let before = sym.cost_of(current);
            let next = current.coarser();
            levels.insert(id.clone(), next);
            swallow(levels, symbols);
            saved += before.saturating_sub(sym.cost_of(next));
            reasons.insert(id, "downgraded to fit the budget".to_string());
            progressed = true;
            if saved >= overflow.saturating_add(32) {
                break;
            }
        }
        if !progressed {
            return Err(PackError::BudgetTooSmall {
                needed: used,
                budget,
            });
        }
    }
}

fn priority(sym: &Symbol, levels: &HashMap<String, DetailLevel>) -> f32 {
    let level = levels[&sym.id.fold_id()];
    let cost = sym.cost_of(level).max(1) as f32;
    level.value() / cost
}

fn score(scores: &HashMap<String, f32>, id: &str) -> f32 {
    scores.get(id).copied().unwrap_or(0.0).max(1e-6)
}

fn estimate(levels: &HashMap<String, DetailLevel>, symbols: &[Symbol], header: usize) -> usize {
    let mut total = header;
    let mut files = HashSet::new();
    for sym in symbols {
        let id = sym.id.fold_id();
        let level = levels.get(&id).copied().unwrap_or(DetailLevel::L4);
        if level == DetailLevel::L4 || swallowed_id(&id, levels, symbols) {
            continue;
        }
        files.insert(sym.id.file.clone());
        total += sym.cost_of(level);
    }
    total += files.len() * 12;
    total
}

pub fn swallow(levels: &mut HashMap<String, DetailLevel>, symbols: &[Symbol]) {
    let snapshot = levels.clone();
    for sym in symbols {
        if swallowed_id(&sym.id.fold_id(), &snapshot, symbols) {
            levels.insert(sym.id.fold_id(), DetailLevel::L4);
        }
    }
}

fn swallowed_id(id: &str, levels: &HashMap<String, DetailLevel>, symbols: &[Symbol]) -> bool {
    let Some(sym) = symbols.iter().find(|s| s.id.fold_id() == id) else {
        return false;
    };
    let mut container = sym.container.as_deref();
    while let Some(parent_id) = container {
        let expanded = levels.get(parent_id).copied().unwrap_or(DetailLevel::L4);
        let parent_kind = symbols
            .iter()
            .find(|s| s.id.fold_id() == parent_id)
            .map(|s| s.kind);
        let swallows = expanded == DetailLevel::L0
            || (expanded == DetailLevel::L1 && parent_kind.is_some_and(|k| k.is_full_at_l1()));
        if swallows {
            return true;
        }
        container = symbols
            .iter()
            .find(|s| s.id.fold_id() == parent_id)
            .and_then(|s| s.container.as_deref());
    }
    false
}

fn decisions(
    symbols: &[Symbol],
    levels: &HashMap<String, DetailLevel>,
    scores: &HashMap<String, f32>,
    reasons: &HashMap<String, String>,
) -> Vec<Decision> {
    let mut out: Vec<Decision> = symbols
        .iter()
        .map(|sym| {
            let id = sym.id.fold_id();
            Decision {
                id: id.clone(),
                level: levels.get(&id).copied().unwrap_or(DetailLevel::L4),
                score: scores.get(&id).copied().unwrap_or(0.0),
                reason: reasons
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| "omitted".into()),
            }
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SymbolId;
    use crate::model::SymbolKind;
    use std::ops::Range;

    fn symbol(name: &str, cost: [usize; 4]) -> Symbol {
        Symbol {
            id: SymbolId {
                file: "a.rs".into(),
                symbol_path: name.into(),
                overload: None,
            },
            kind: SymbolKind::Function,
            name: name.into(),
            span: Range { start: 0, end: 10 },
            sig_end: 5,
            doc: None,
            refs: Vec::new(),
            extra: None,
            line_start: 1,
            line_end: 4,
            container: None,
            cost,
        }
    }

    #[test]
    fn greedy_stays_inside_budget() {
        let symbols = vec![
            symbol("alpha", [80, 40, 15, 4]),
            symbol("beta", [80, 40, 15, 4]),
        ];
        let mut scores = HashMap::new();
        scores.insert("a.rs#alpha".into(), 0.9);
        scores.insert("a.rs#beta".into(), 0.1);
        let focus = HashSet::new();
        let diff = HashSet::new();
        let seeded = HashSet::new();
        let req = SolveRequest {
            symbols: &symbols,
            scores: &scores,
            budget: Some(40),
            focus: &focus,
            diff: &diff,
            seeded: &seeded,
            mode: PackMode::Auto,
        };
        let (levels, _) = solve(req, |_| 0).unwrap();
        let used: usize = symbols
            .iter()
            .map(|s| s.cost_of(levels[&s.id.fold_id()]))
            .sum();
        assert!(used <= 40);
        assert!(
            levels[&"a.rs#alpha".to_string()].value() >= levels[&"a.rs#beta".to_string()].value()
        );
    }
}
