use tree_sitter::Node;

use super::extract::{
    assign_overloads, collect_imports, collect_refs, field_blob, field_name, join_path, kids,
    parse_language, preceding_doc, Ctx,
};
use crate::error::PackError;
use crate::model::SymbolKind;

const DEFS: &[&str] = &[
    "class_declaration",
    "interface_declaration",
    "enum_declaration",
    "method_declaration",
    "constructor_declaration",
    "annotation_type_declaration",
];

const METHOD_KINDS: &[&str] = &["method_declaration", "constructor_declaration"];

pub fn extract(source: &str) -> Result<super::rust::Extracted, PackError> {
    let tree = parse_language(source, tree_sitter_java::LANGUAGE.into())?;
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
        "class_declaration"
        | "interface_declaration"
        | "enum_declaration"
        | "annotation_type_declaration" => {
            if let Some(name) = field_name(node, ctx.source) {
                let path = join_path(parent, &name);
                let kind = match node.kind() {
                    "interface_declaration" => SymbolKind::Interface,
                    "enum_declaration" => SymbolKind::Enum,
                    _ => SymbolKind::Class,
                };
                let extra = node
                    .child_by_field_name("body")
                    .and_then(|body| field_blob(body, ctx.source, METHOD_KINDS));
                let sig_end = node
                    .child_by_field_name("body")
                    .map(|body| body.start_byte())
                    .unwrap_or_else(|| node.end_byte());
                ctx.add(
                    node,
                    &name,
                    &path,
                    kind,
                    sig_end,
                    collect_refs(node, ctx.source, DEFS),
                    extra,
                    preceding_doc(node, ctx.source),
                );
                if let Some(body) = node.child_by_field_name("body") {
                    for child in kids(body) {
                        walk(child, ctx, Some(&path));
                    }
                }
            }
        }
        "method_declaration" | "constructor_declaration" => {
            let name = field_name(node, ctx.source).unwrap_or_else(|| "constructor".into());
            let path = join_path(parent, &name);
            let sig_end = node
                .child_by_field_name("body")
                .map(|body| body.start_byte())
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
            if let Some(body) = node.child_by_field_name("body") {
                for child in kids(body) {
                    walk(child, ctx, Some(&path));
                }
            }
        }
        _ => {
            for child in kids(node) {
                walk(child, ctx, parent);
            }
        }
    }
}
