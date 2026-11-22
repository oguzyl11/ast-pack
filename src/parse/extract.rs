use std::ops::Range;

use tree_sitter::Node;

use crate::model::SymbolKind;

#[derive(Debug, Clone)]
pub struct Draft {
    pub name: String,
    pub symbol_path: String,
    pub kind: SymbolKind,
    pub span: Range<usize>,
    pub sig_end: usize,
    pub doc: Option<String>,
    pub refs: Vec<String>,
    pub extra: Option<String>,
    pub overload: Option<u32>,
}

pub struct Ctx<'a> {
    pub source: &'a str,
    pub drafts: Vec<Draft>,
    pub imports: Vec<String>,
}

impl<'a> Ctx<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            drafts: Vec::new(),
            imports: Vec::new(),
        }
    }

    pub fn add(
        &mut self,
        node: Node,
        name: &str,
        path: &str,
        kind: SymbolKind,
        sig_end: usize,
        refs: Vec<String>,
        extra: Option<String>,
        doc: Option<String>,
    ) {
        let start = node.start_byte();
        let end = node.end_byte().max(start);
        let sig_end = sig_end.clamp(start, end);
        self.drafts.push(Draft {
            name: name.to_string(),
            symbol_path: path.to_string(),
            kind,
            span: start..end,
            sig_end,
            doc,
            refs,
            extra,
            overload: None,
        });
    }
}

pub fn kids(node: Node) -> Vec<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).collect()
}

pub fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    crate::utils::slice(source, node.start_byte()..node.end_byte())
}

pub fn is_ident(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "type_identifier"
            | "property_identifier"
            | "field_identifier"
            | "shorthand_property_identifier"
            | "shorthand_property_identifier_pattern"
    )
}

pub fn last_ident(node: Node, source: &str) -> Option<String> {
    if is_ident(node.kind()) {
        let text = node_text(node, source).trim();
        if text.is_empty() {
            return None;
        }
        return Some(text.to_string());
    }
    for child in kids(node).into_iter().rev() {
        if let Some(name) = last_ident(child, source) {
            return Some(name);
        }
    }
    None
}

pub fn field_name(node: Node, source: &str) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|n| last_ident(n, source))
}

pub fn join_path(parent: Option<&str>, name: &str) -> String {
    match parent {
        Some(parent) if !parent.is_empty() => format!("{parent}::{name}"),
        _ => name.to_string(),
    }
}

pub fn body_start(node: Node) -> usize {
    node.child_by_field_name("body")
        .map(|n| n.start_byte())
        .unwrap_or_else(|| node.end_byte())
}

pub fn preceding_doc(node: Node, source: &str) -> Option<String> {
    let mut current = node;
    let mut lines = Vec::new();
    loop {
        let Some(prev) = current.prev_sibling() else {
            break;
        };
        let kind = prev.kind();
        if matches!(
            kind,
            "attribute_item"
                | "decorator"
                | "decorators"
                | "modifiers"
                | "annotation"
                | "marker_annotation"
                | "annotation_type_declaration"
        ) {
            current = prev;
            continue;
        }
        if kind.contains("comment") {
            let text = node_text(prev, source).trim();
            if !text.is_empty() {
                lines.push(text.to_string());
            }
            current = prev;
            if lines.len() >= 8 {
                break;
            }
            continue;
        }
        break;
    }
    if lines.is_empty() {
        None
    } else {
        lines.reverse();
        Some(lines.join("\n"))
    }
}

pub fn python_docstring(node: Node, source: &str) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let first = body.named_child(0)?;
    let expr = if first.kind() == "expression_statement" {
        first.named_child(0)?
    } else {
        first
    };
    if expr.kind() == "string" {
        Some(node_text(expr, source).trim().to_string())
    } else {
        None
    }
}

const CALL_KINDS: &[&str] = &["call", "call_expression", "method_invocation"];

pub fn collect_refs(node: Node, source: &str, def_kinds: &[&str]) -> Vec<String> {
    let mut refs = Vec::new();
    fn walk(node: Node, source: &str, def_kinds: &[&str], root: bool, sig_end: usize, refs: &mut Vec<String>) {
        if !root && def_kinds.contains(&node.kind()) {
            return;
        }
        if CALL_KINDS.contains(&node.kind()) {
            let target = node
                .child_by_field_name("function")
                .or_else(|| node.child_by_field_name("name"));
            if let Some(target) = target {
                if let Some(name) = last_ident(target, source) {
                    if !is_noise(&name) {
                        refs.push(name);
                    }
                }
            }
        }
        if node.start_byte() < sig_end && is_ident(node.kind()) && node.end_byte() <= sig_end {
            let name = node_text(node, source).trim();
            if !name.is_empty() && !is_noise(name) {
                refs.push(format!("type:{name}"));
            }
            return;
        }
        for child in kids(node) {
            walk(child, source, def_kinds, false, sig_end, refs);
        }
    }
    let sig_end = body_start(node);
    walk(node, source, def_kinds, true, sig_end, &mut refs);
    refs.sort();
    refs.dedup();
    refs
}

fn is_noise(name: &str) -> bool {
    matches!(
        name,
        "self" | "Self" | "super" | "this" | "crate" | "std" | "true" | "false" | "None" | "null"
    )
}

pub fn collect_imports(node: Node, source: &str, imports: &mut Vec<String>) {
    fn walk(node: Node, source: &str, imports: &mut Vec<String>) {
        if is_ident(node.kind()) {
            let name = node_text(node, source).trim();
            if !name.is_empty() && !is_noise(name) && name != "from" {
                imports.push(name.to_string());
            }
            return;
        }
        for child in kids(node) {
            walk(child, source, imports);
        }
    }
    walk(node, source, imports);
    imports.sort();
    imports.dedup();
}

pub fn field_blob(body: Node, source: &str, skip_kinds: &[&str]) -> Option<String> {
    let mut parts = Vec::new();
    for child in kids(body) {
        if skip_kinds.contains(&child.kind()) || child.kind().contains("comment") {
            continue;
        }
        let text = node_text(child, source).trim();
        if !text.is_empty() {
            parts.push(text.to_string());
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

pub fn assign_overloads(drafts: &mut [Draft]) {
    let mut groups: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for (i, draft) in drafts.iter().enumerate() {
        groups.entry(draft.symbol_path.clone()).or_default().push(i);
    }
    for indexes in groups.values_mut() {
        indexes.sort_by(|&a, &b| {
            drafts[a]
                .span
                .start
                .cmp(&drafts[b].span.start)
                .then(drafts[a].span.end.cmp(&drafts[b].span.end))
        });
        if indexes.len() > 1 {
            for (offset, index) in indexes.iter().enumerate() {
                if offset > 0 {
                    drafts[*index].overload = Some((offset + 1) as u32);
                }
            }
        }
    }
}

pub fn parse_language(
    source: &str,
    language: tree_sitter::Language,
) -> Result<tree_sitter::Tree, crate::error::PackError> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language)
        .map_err(|err| crate::error::PackError::msg(format!("grammar: {err}")))?;
    parser
        .parse(source, None)
        .ok_or_else(|| crate::error::PackError::msg("parser returned no tree"))
}
