// src/parse/javascript.rs

use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};
use tree_sitter_javascript::language as javascript_language;
use crate::model::symbol::{Symbol, SymbolId, DetailLevel};
use crate::tokenizer::Tokenizer;

/// Parse a JavaScript or JSX file and extract symbols.
pub fn parse(path: &PathBuf) -> Result<Vec<Symbol>> {
    let source = std::fs::read_to_string(path)?;
    let mut parser = Parser::new();
        parser.set_language(&javascript_language()).expect("failed to load JavaScript grammar");
    let tree = parser.parse(&source, None).ok_or_else(|| anyhow::anyhow!("failed to parse"))?;

    let mut symbols = Vec::new();
    let root = tree.root_node();

    fn walk_node(
        node: Node,
        source: &str,
        path: &PathBuf,
        symbols: &mut Vec<Symbol>,
    ) -> Result<()> {
        match node.kind() {
            "function_declaration" => extract_function(node, source, path, symbols)?,
            "class_declaration" => extract_class(node, source, path, symbols)?,
            "lexical_declaration" => extract_lexical_declaration(node, source, path, symbols)?,
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
    let name_node = node.child_by_field_name("name").ok_or_else(|| anyhow::anyhow!("no name"))?;
    let name = name_node.utf8_text(source.as_bytes())?.to_string();
    let body_node = node.child_by_field_name("body").ok_or_else(|| anyhow::anyhow!("no body"))?;
    let body_range = byte_range(body_node);
    let signature_range = byte_range(node).start..node.end_byte();

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

fn extract_lexical_declaration(
    node: Node,
    source: &str,
    path: &PathBuf,
    symbols: &mut Vec<Symbol>,
) -> Result<()> {
    // Handles const/let variable declarations that may have functions as init
    for child in node.named_children(&mut node.walk()) {
        if child.kind() == "function" || child.kind() == "arrow_function" {
            extract_function(child, source, path, symbols)?;
        }
    }
    Ok(())
}

