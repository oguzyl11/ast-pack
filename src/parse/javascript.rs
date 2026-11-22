use tree_sitter::Node;

use super::extract::{
    assign_overloads, collect_imports, collect_refs, field_blob, field_name, join_path, kids,
    parse_language, preceding_doc, Ctx,
};
use crate::error::PackError;
use crate::model::SymbolKind;

const DEFS: &[&str] = &[
    "function_declaration",
    "class_declaration",
    "method_definition",
    "generator_function_declaration",
    "variable_declarator",
];

pub fn extract(source: &str) -> Result<super::rust::Extracted, PackError> {
    let tree = parse_language(source, tree_sitter_javascript::LANGUAGE.into())?;
    let mut ctx = Ctx::new(source);
    walk(tree.root_node(), &mut ctx, None);
    assign_overloads(&mut ctx.drafts);
    Ok(super::rust::Extracted {
        drafts: ctx.drafts,
        imports: ctx.imports,
    })
}

pub fn extract_typescript(source: &str, tsx: bool) -> Result<super::rust::Extracted, PackError> {
    let language = if tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    let tree = parse_language(source, language)?;
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
        "import_statement" => {
            collect_imports(node, ctx.source, &mut ctx.imports);
        }
        "function_declaration" | "generator_function_declaration" => {
            add_fn(node, ctx, parent, SymbolKind::Function);
        }
        "class_declaration" | "abstract_class_declaration" => add_class(node, ctx, parent),
        "interface_declaration" => add_type(node, ctx, parent, SymbolKind::Interface),
        "type_alias_declaration" => add_type(node, ctx, parent, SymbolKind::TypeAlias),
        "enum_declaration" => add_type(node, ctx, parent, SymbolKind::Enum),
        "method_definition" | "method_signature" | "abstract_method_signature" => {
            add_fn(node, ctx, parent, SymbolKind::Method);
        }
        "variable_declarator" => add_var_fn(node, ctx, parent),
        "export_statement" => {
            let before = ctx.drafts.len();
            for child in kids(node) {
                walk(child, ctx, parent);
            }
            if let Some(draft) = ctx.drafts.get_mut(before) {
                draft.span.start = node.start_byte();
            }
        }
        _ => {
            for child in kids(node) {
                walk(child, ctx, parent);
            }
        }
    }
}

fn add_fn(node: Node, ctx: &mut Ctx, parent: Option<&str>, kind: SymbolKind) {
    let Some(name) = field_name(node, ctx.source) else {
        return;
    };
    let path = join_path(parent, &name);
    let kind = if parent.is_some() {
        SymbolKind::Method
    } else {
        kind
    };
    let sig_end = node
        .child_by_field_name("body")
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
    if let Some(body) = node.child_by_field_name("body") {
        for child in kids(body) {
            walk(child, ctx, Some(&path));
        }
    }
}

fn add_class(node: Node, ctx: &mut Ctx, parent: Option<&str>) {
    let Some(name) = field_name(node, ctx.source) else {
        return;
    };
    let path = join_path(parent, &name);
    let extra = node.child_by_field_name("body").and_then(|body| {
        field_blob(
            body,
            ctx.source,
            &[
                "method_definition",
                "method_signature",
                "abstract_method_signature",
                "class_declaration",
            ],
        )
    });
    let sig_end = node
        .child_by_field_name("body")
        .map(|n| n.start_byte())
        .unwrap_or_else(|| node.end_byte());
    ctx.add(
        node,
        &name,
        &path,
        SymbolKind::Class,
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

fn add_type(node: Node, ctx: &mut Ctx, parent: Option<&str>, kind: SymbolKind) {
    let Some(name) = field_name(node, ctx.source) else {
        return;
    };
    let path = join_path(parent, &name);
    ctx.add(
        node,
        &name,
        &path,
        kind,
        node.end_byte(),
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

fn add_var_fn(node: Node, ctx: &mut Ctx, parent: Option<&str>) {
    let Some(value) = node.child_by_field_name("value") else {
        return;
    };
    if !matches!(value.kind(), "arrow_function" | "function" | "function_expression" | "generator_function") {
        for child in kids(node) {
            walk(child, ctx, parent);
        }
        return;
    }
    let Some(name) = field_name(node, ctx.source) else {
        return;
    };
    let path = join_path(parent, &name);
    let sig_end = value
        .child_by_field_name("body")
        .map(|n| n.start_byte())
        .unwrap_or_else(|| value.start_byte());
    ctx.add(
        node,
        &name,
        &path,
        if parent.is_some() {
            SymbolKind::Method
        } else {
            SymbolKind::Function
        },
        sig_end,
        collect_refs(node, ctx.source, DEFS),
        None,
        preceding_doc(node, ctx.source),
    );
}
