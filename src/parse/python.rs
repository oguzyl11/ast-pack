use tree_sitter::Node;

use super::extract::{
    assign_overloads, body_start, collect_imports, collect_refs, field_blob, field_name, join_path,
    kids, parse_language, preceding_doc, python_docstring, Ctx,
};
use crate::error::PackError;
use crate::model::SymbolKind;

const DEFS: &[&str] = &[
    "function_definition",
    "class_definition",
    "decorated_definition",
];

pub fn extract(source: &str) -> Result<super::rust::Extracted, PackError> {
    let tree = parse_language(source, tree_sitter_python::LANGUAGE.into())?;
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
        "import_statement" | "import_from_statement" => {
            collect_imports(node, ctx.source, &mut ctx.imports);
        }
        "decorated_definition" => {
            let before = ctx.drafts.len();
            for child in kids(node) {
                walk(child, ctx, parent);
            }
            if let Some(draft) = ctx.drafts.get_mut(before) {
                draft.span.start = node.start_byte();
                if draft.doc.is_none() {
                    draft.doc = preceding_doc(node, ctx.source);
                }
            }
        }
        "function_definition" => {
            if let Some(name) = field_name(node, ctx.source) {
                let path = join_path(parent, &name);
                let kind = if parent.is_some() {
                    SymbolKind::Method
                } else {
                    SymbolKind::Function
                };
                let doc =
                    python_docstring(node, ctx.source).or_else(|| preceding_doc(node, ctx.source));
                ctx.add(
                    node,
                    &name,
                    &path,
                    kind,
                    body_start(node),
                    collect_refs(node, ctx.source, DEFS),
                    None,
                    doc,
                );
                if let Some(body) = node.child_by_field_name("body") {
                    for child in kids(body) {
                        walk(child, ctx, Some(&path));
                    }
                }
            }
        }
        "class_definition" => {
            if let Some(name) = field_name(node, ctx.source) {
                let path = join_path(parent, &name);
                let extra = node
                    .child_by_field_name("body")
                    .and_then(|body| field_blob(body, ctx.source, DEFS));
                let doc =
                    python_docstring(node, ctx.source).or_else(|| preceding_doc(node, ctx.source));
                ctx.add(
                    node,
                    &name,
                    &path,
                    SymbolKind::Class,
                    body_start(node),
                    collect_refs(node, ctx.source, DEFS),
                    extra,
                    doc,
                );
                if let Some(body) = node.child_by_field_name("body") {
                    for child in kids(body) {
                        walk(child, ctx, Some(&path));
                    }
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
