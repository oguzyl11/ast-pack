use tree_sitter::Node;

use super::extract::{
    assign_overloads, body_start, collect_imports, collect_refs, field_name, join_path, kids,
    parse_language, preceding_doc, Ctx,
};
use crate::error::PackError;
use crate::model::SymbolKind;

pub struct Extracted {
    pub drafts: Vec<super::extract::Draft>,
    pub imports: Vec<String>,
}

const DEFS: &[&str] = &[
    "function_item",
    "function_signature_item",
    "struct_item",
    "enum_item",
    "union_item",
    "trait_item",
    "impl_item",
    "type_item",
    "const_item",
    "mod_item",
];

pub fn extract(source: &str) -> Result<Extracted, PackError> {
    let tree = parse_language(source, tree_sitter_rust::LANGUAGE.into())?;
    let mut ctx = Ctx::new(source);
    walk(tree.root_node(), &mut ctx, None);
    assign_overloads(&mut ctx.drafts);
    Ok(Extracted {
        drafts: ctx.drafts,
        imports: ctx.imports,
    })
}

fn walk(node: Node, ctx: &mut Ctx, parent: Option<&str>) {
    match node.kind() {
        "use_declaration" => {
            collect_imports(node, ctx.source, &mut ctx.imports);
        }
        "impl_item" => {
            let ty = node
                .child_by_field_name("type")
                .and_then(|n| super::extract::last_ident(n, ctx.source))
                .unwrap_or_else(|| "Impl".into());
            let path = join_path(parent, &format!("{ty}::impl"));
            let doc = preceding_doc(node, ctx.source);
            let refs = collect_refs(node, ctx.source, DEFS);
            ctx.add(
                node,
                &ty,
                &path,
                SymbolKind::Impl,
                body_start(node),
                refs,
                None,
                doc,
            );
            if let Some(body) = node.child_by_field_name("body") {
                let method_parent = join_path(parent, &ty);
                for child in kids(body) {
                    walk(child, ctx, Some(&method_parent));
                }
            }
        }
        "mod_item" => {
            if let Some(name) = field_name(node, ctx.source) {
                let path = join_path(parent, &name);
                if node.child_by_field_name("body").is_some() {
                    ctx.add(
                        node,
                        &name,
                        &path,
                        SymbolKind::Module,
                        body_start(node),
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
            }
        }
        "function_item" | "function_signature_item" => add_named(node, ctx, parent, SymbolKind::Function, DEFS),
        "struct_item" | "union_item" => add_named(node, ctx, parent, SymbolKind::Struct, DEFS),
        "enum_item" => add_named(node, ctx, parent, SymbolKind::Enum, DEFS),
        "trait_item" => add_named(node, ctx, parent, SymbolKind::Trait, DEFS),
        "type_item" => add_named(node, ctx, parent, SymbolKind::TypeAlias, DEFS),
        "const_item" => add_named(node, ctx, parent, SymbolKind::Const, DEFS),
        _ => {
            for child in kids(node) {
                walk(child, ctx, parent);
            }
        }
    }
}

fn add_named(node: Node, ctx: &mut Ctx, parent: Option<&str>, kind: SymbolKind, defs: &[&str]) {
    let Some(name) = field_name(node, ctx.source) else {
        for child in kids(node) {
            walk(child, ctx, parent);
        }
        return;
    };
    let path = join_path(parent, &name);
    let kind = if parent.is_some() && kind == SymbolKind::Function {
        SymbolKind::Method
    } else {
        kind
    };
    ctx.add(
        node,
        &name,
        &path,
        kind,
        body_start(node),
        collect_refs(node, ctx.source, defs),
        None,
        preceding_doc(node, ctx.source),
    );
    if let Some(body) = node.child_by_field_name("body") {
        let next_parent = if matches!(kind, SymbolKind::Function | SymbolKind::Method | SymbolKind::Trait | SymbolKind::Module) {
            Some(path)
        } else {
            parent.map(|p| p.to_string())
        };
        for child in kids(body) {
            walk(child, ctx, next_parent.as_deref());
        }
    }
}
