// src/parse/typescript.rs

use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser, Tree};
use tree_sitter_typescript::language as typescript_language;
use crate::model::symbol::{Symbol, SymbolId, DetailLevel};
use crate::tokenizer::token::Tokenizer;

/// Parse a TypeScript or TSX file and extract symbols.
pub fn parse(path: &PathBuf) -> Result<Vec<Symbol>> {
    let source = std::fs::read_to_string(path)?;
    let mut parser = Parser::new();
    parser.set_language(typescript_language())?;
    let tree = parser.parse(&source, None).ok_or_else(|| anyhow::anyhow!("failed to parse"))?;

    let mut symbols = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();

    fn walk_node(
        node: Node,
        source: &str,
        path: &PathBuf,
        symbols: &mut Vec<Symbol>,
    ) -> Result<()> {
        match node.kind() {
            "function_declaration" => extract_function(node, source, path, symbols)?,
            "class_declaration" => extract_class(node, source, path, symbols)?,
            "interface_declaration" => extract_interface(node, source, path, symbols)?,
            "enum_declaration" => extract_enum(node, source, path, symbols)?,
            _ => {}
        }
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

fn extract_function(
    node: Node,
    source: &str,
    path: &PathBuf,
    symbols: &mut Vec<Symbol>,
) -> Result<()> {
    // function_declaration: name, parameters, body
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

fn extract_class(
    node: Node,
    source: &str,
    path: &PathBuf,
    symbols: &mut Vec<Symbol>,
) -> Result<()> {
    // class_declaration: name, body
    let name_node = node.child_by_field_name("name").ok_or_else(|| anyhow::anyhow!("no name"))?;
    let name = name_node.utf8_text(source.as_bytes())?.to_string();
    let body_node = node.child_by_field_name("body").ok_or_else(|| anyhow::anyhow!("no body"))?;
    let body_range = byte_range(body_node);
    let signature_range = byte_range(node).start..node.end_byte(); // whole class as signature

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

fn extract_interface(
    node: Node,
    source: &str,
    path: &PathBuf,
    symbols: &mut Vec<Symbol>,
) -> Result<()> {
    // treat like class but without implementation body
    let name_node = node.child_by_field_name("name").ok_or_else(|| anyhow::anyhow!("no name"))?;
    let name = name_node.utf8_text(source.as_bytes())?.to_string();
    let signature_range = byte_range(node).start..node.end_byte();
    let body_range = signature_range.clone(); // no separate body

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

fn extract_enum(
    node: Node,
    source: &str,
    path: &PathBuf,
    symbols: &mut Vec<Symbol>,
) -> Result<()> {
    let name_node = node.child_by_field_name("name").ok_or_else(|| anyhow::anyhow!("no name"))?;
    let name = name_node.utf8_text(source.as_bytes())?.to_string();
    let signature_range = byte_range(node).start..node.end_byte();
    let body_range = signature_range.clone();
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

