// src/parse/rust.rs
use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_rust::language as rust_language;
use crate::model::symbol::{Symbol, SymbolId, DetailLevel};
use crate::tokenizer::token::Tokenizer;

/// Extract symbols from a Rust source file.
/// Returns a vector of `Symbol` where `body_range` and `signature_range`
/// are byte offsets into the original source string.
pub fn parse(path: &PathBuf) -> Result<Vec<Symbol>> {
    let source = std::fs::read_to_string(path)?;
    let mut parser = Parser::new();
    parser.set_language(rust_language())?;
    let tree = parser.parse(&source, None).ok_or_else(|| anyhow::anyhow!("failed to parse"))?;

    let mut symbols = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();

    // Walk the tree and collect function, struct, enum, impl items.
    fn walk_node(node: Node, source: &str, path: &PathBuf, symbols: &mut Vec<Symbol>) -> Result<()> {
        match node.kind() {
            "function_item" => extract_function(node, source, path, symbols)?,
            "struct_item" => extract_struct(node, source, path, symbols)?,
            "enum_item" => extract_enum(node, source, path, symbols)?,
            "impl_item" => extract_impl(node, source, path, symbols)?,
            _ => {}
        }
        // Recurse into children
        for child in node.children(&mut node.walk()) {
            walk_node(child, source, path, symbols)?;
        }
        Ok(())
    }

    walk_node(root, &source, path, &mut symbols)?;
    Ok(symbols)
}

fn byte_range(node: Node) -> std::ops::Range<usize> {
    let start = node.start_byte();
    let end = node.end_byte();
    start..end
}

fn extract_function(node: Node, source: &str, path: &PathBuf, symbols: &mut Vec<Symbol>) -> Result<()> {
    // function_item -> (visibility?) (async?) (const?) "fn" name generic_params? parameters
    // body is a block node child.
    let name_node = node.child_by_field_name("name").ok_or_else(|| anyhow::anyhow!("no name"))?;
    let name = name_node.utf8_text(source.as_bytes())?.to_string();
    let signature_node = node.child_by_field_name("parameters").unwrap_or(node);
    let signature_range = byte_range(node).start..signature_node.end_byte();
    let body_node = node.child_by_field_name("body").ok_or_else(|| anyhow::anyhow!("no body"))?;
    let body_range = byte_range(body_node);

    let id = SymbolId {
        file: path.clone(),
        symbol_path: name,
        overload: None,
    };

    // Token counts – use a simple tokenizer placeholder (no model information).
    let tokenizer = Tokenizer::new("o200k").unwrap(); // default model
    let body_tokens = tokenizer.count(&source[body_range.clone()]);
    let sig_tokens = tokenizer.count(&source[signature_range.clone()]);

    symbols.push(Symbol {
        id,
        body_range,
        signature_range,
        doc_range: None,
        body_tokens,
        sig_tokens,
        level: DetailLevel::Omitted,
    });
    Ok(())
}

fn extract_struct(node: Node, source: &str, path: &PathBuf, symbols: &mut Vec<Symbol>) -> Result<()> {
    let name_node = node.child_by_field_name("name").ok_or_else(|| anyhow::anyhow!("no name"))?;
    let name = name_node.utf8_text(source.as_bytes())?.to_string();
    let signature_range = byte_range(node).start..node.end_byte(); // whole struct as signature
    let body_range = byte_range(node); // treat whole as body (no separate)

    let id = SymbolId {
        file: path.clone(),
        symbol_path: name,
        overload: None,
    };
    let tokenizer = Tokenizer::new("o200k").unwrap();
    let body_tokens = tokenizer.count(&source[body_range.clone()]);
    let sig_tokens = tokenizer.count(&source[signature_range.clone()]);
    symbols.push(Symbol {
        id,
        body_range,
        signature_range,
        doc_range: None,
        body_tokens,
        sig_tokens,
        level: DetailLevel::Omitted,
    });
    Ok(())
}

fn extract_enum(node: Node, source: &str, path: &PathBuf, symbols: &mut Vec<Symbol>) -> Result<()> {
    let name_node = node.child_by_field_name("name").ok_or_else(|| anyhow::anyhow!("no name"))?;
    let name = name_node.utf8_text(source.as_bytes())?.to_string();
    let signature_range = byte_range(node).start..node.end_byte();
    let body_range = byte_range(node);
    let id = SymbolId {
        file: path.clone(),
        symbol_path: name,
        overload: None,
    };
    let tokenizer = Tokenizer::new("o200k").unwrap();
    let body_tokens = tokenizer.count(&source[body_range.clone()]);
    let sig_tokens = tokenizer.count(&source[signature_range.clone()]);
    symbols.push(Symbol {
        id,
        body_range,
        signature_range,
        doc_range: None,
        body_tokens,
        sig_tokens,
        level: DetailLevel::Omitted,
    });
    Ok(())
}

fn extract_impl(node: Node, source: &str, path: &PathBuf, symbols: &mut Vec<Symbol>) -> Result<()> {
    // For simplicity, treat each method inside impl as a separate symbol.
    for child in node.children(&mut node.walk()) {
        if child.kind() == "function_item" {
            extract_function(child, source, path, symbols)?;
        }
    }
    Ok(())
}

