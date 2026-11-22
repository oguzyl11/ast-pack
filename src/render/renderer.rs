// src/render/renderer.rs
// Removed unused imports
use crate::model::{symbol::{Symbol, SymbolId, DetailLevel}};

use crate::tokenizer::Tokenizer;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::ops::Range;
use std::path::Path;

/// Renderer responsible for turning a `PackPlan` into XML or Markdown output.
pub struct Renderer {
    tokenizer: Tokenizer,
    format: String, // "xml" or "markdown"
}

impl Renderer {
    pub fn new(tokenizer: Tokenizer, format: &str) -> Self {
        Self {
            tokenizer,
            format: format.to_lowercase(),
        }
    }

    /// Render the whole project given the root path, the list of symbols and the
    /// selected detail levels (the "plan"). Returns the full text.
    pub fn render(
        &self,
        _root: &Path,
        symbols: &[Symbol],
        plan: &HashMap<SymbolId, DetailLevel>,
    ) -> Result<String> {
        // 1️⃣ Legend (fixed 3‑line block)
        let mut output = String::new();
        output.push_str(&self.legend());
        output.push('\n');

        // 2️⃣ Group symbols by file for deterministic ordering
        let mut files: HashMap<String, Vec<&Symbol>> = HashMap::new();
        for sym in symbols {
            let path_str = sym.id.file.to_string_lossy().into_owned();
            files.entry(path_str).or_default().push(sym);
        }
        let mut file_keys: Vec<_> = files.keys().cloned().collect();
        file_keys.sort();

        // 3️⃣ Render each file in alphabetical order
        for file in file_keys {
            output.push_str(&format!("File: {}\n", file));
            let mut syms = files.get(&file).unwrap().clone();
            // Sort symbols by start of signature for deterministic output
            syms.sort_by_key(|s| s.signature_range.start);
            for sym in syms {
                let level = plan
                    .get(&sym.id)
                    .copied()
                    .unwrap_or(DetailLevel::Omitted);
                match level {
                    DetailLevel::Full => {
                        let body = self.read_slice(&sym.id.file, &sym.body_range)?;
                        output.push_str(&self.fold_marker(&sym.id, &body, sym.body_range.len()));
                        output.push('\n');
                    }
                    DetailLevel::SignatureAndTypes => {
                        let sig = self.read_slice(&sym.id.file, &sym.signature_range)?;
                        output.push_str(&format!("{} …\n", sig));
                    }
                    DetailLevel::SignatureOnly => {
                        let sig = self.read_slice(&sym.id.file, &sym.signature_range)?;
                        output.push_str(&format!("{} …\n", sig));
                    }
                    DetailLevel::NameOnly => {
                        output.push_str(&format!("{} …\n", sym.id.symbol_path));
                    }
                    _ => { /* other levels – omitted */ }
                }
            }
            output.push('\n');
        }

        Ok(output)
    }

    fn legend(&self) -> String {
        let marker = "⟨fold:PATH#SYMBOL⟩";
        format!(
            "Legend:\n- Fold marker {} indicates a collapsed region.\n- Use `ast-pack unfold <id>` to retrieve the hidden body.\n- Token budget is enforced on the final rendered text.",
            marker
        )
    }

    /// Read the file and return the UTF‑8 slice for the given byte range.
    fn read_slice(&self, path: &Path, range: &Range<usize>) -> Result<String> {
        let content = fs::read_to_string(path)?;
        // Ensure range is within bounds (byte‑wise). If not, fallback to empty.
        let bytes = content.as_bytes();
        let end = std::cmp::min(range.end, bytes.len());
        let start = std::cmp::min(range.start, end);
        let slice = &bytes[start..end];
        Ok(String::from_utf8_lossy(slice).to_string())
    }

    /// Produce a single line with a fold marker that contains the original snippet
    /// head and the marker with line count.
    fn fold_marker(&self, id: &SymbolId, body: &str, line_count: usize) -> String {
        // Use the first non‑empty line as a short preview.
        let preview = body.lines().next().unwrap_or("[empty]");
        format!(
            "{} {{ /* ⟨fold:{}⟩ {} lines */ }}",
            preview,
            id.to_fold_id(),
            line_count
        )
    }
}
