use tree_sitter::Node;

use super::extract::{
    assign_overloads, collect_imports, collect_refs, field_name, join_path, kids, last_ident,
    parse_language, preceding_doc, Ctx,
};
use crate::error::PackError;
use crate::model::SymbolKind;

const DEFS: &[&str] = &[
    "function_declaration",
    "method_declaration",
    "type_declaration",
    "type_spec",
];

pub fn extract(source: &str) -> Result<super::rust::Extracted, PackError> {
    let tree = parse_language(source, tree_sitter_go::LANGUAGE.into())?;
    let mut ctx = Ctx::new(source);
    walk(tree.root_node(), &mut ctx, None);
    assign_overloads(&mut ctx.drafts);
    Ok(super::rust::Extracted {
        drafts: ctx.drafts,
        imports: ctx.imports,
    })
}

fn walk(node: Node, ctx: &mut Ctx, parent: Option<&str>) {
    match node.kind() {
        "import_declaration" => collect_imports(node, ctx.source, &mut ctx.imports),
        "function_declaration" => {
            if let Some(name) = field_name(node, ctx.source) {
                let path = join_path(parent, &name);
                let sig_end = node
                    .child_by_field_name("body")
                    .map(|n| n.start_byte())
                    .unwrap_or_else(|| node.end_byte());
                ctx.add(
                    node,
                    &name,
                    &path,
                    SymbolKind::Function,
                    sig_end,
                    collect_refs(node, ctx.source, DEFS),
                    None,
                    preceding_doc(node, ctx.source),
                );
            }
        }
        "method_declaration" => {
            let recv = node
                .child_by_field_name("receiver")
                .and_then(|n| last_ident(n, ctx.source))
                .unwrap_or_else(|| "Receiver".into());
            if let Some(name) = field_name(node, ctx.source) {
                let path = format!("{recv}::{name}");
                let sig_end = node
                    .child_by_field_name("body")
                    .map(|n| n.start_byte())
                    .unwrap_or_else(|| node.end_byte());
                ctx.add(
                    node,
                    &name,
                    &path,
                    SymbolKind::Method,
                    sig_end,
                    collect_refs(node, ctx.source, DEFS),
                    None,
                    preceding_doc(node, ctx.source),
                );
            }
        }
        "type_spec" => {
            if let Some(name) = field_name(node, ctx.source) {
                let path = join_path(parent, &name);
                let ty = node.child_by_field_name("type");
                let kind = match ty.map(|n| n.kind()) {
                    Some("interface_type") => SymbolKind::Interface,
                    Some("struct_type") => SymbolKind::Struct,
                    _ => SymbolKind::TypeAlias,
                };
                let sig_end = ty
                    .map(|n| n.start_byte())
                    .unwrap_or_else(|| node.end_byte());
                ctx.add(
                    node,
                    &name,
                    &path,
                    kind,
                    sig_end,
                    collect_refs(node, ctx.source, DEFS),
                    None,
                    preceding_doc(node, ctx.source),
                );
            }
        }
        _ => {
            for child in kids(node) {
                walk(child, ctx, parent);
            }
        }
    }
}
