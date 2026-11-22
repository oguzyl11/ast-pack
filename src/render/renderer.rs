use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::model::symbol::{DetailLevel, Symbol, SymbolKind};
use crate::utils::{slice, xml_escape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Xml,
    Markdown,
}

impl Format {
    pub fn parse(raw: &str) -> Result<Self, crate::error::PackError> {
        match raw.to_ascii_lowercase().as_str() {
            "xml" => Ok(Self::Xml),
            "markdown" | "md" => Ok(Self::Markdown),
            other => Err(crate::error::PackError::msg(format!(
                "unknown format `{other}` (expected xml or markdown)"
            ))),
        }
    }

    fn lang(ext: &str) -> &'static str {
        match ext {
            "rs" => "rust",
            "py" => "python",
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "go" => "go",
            "java" => "java",
            _ => "",
        }
    }
}

pub fn snippet(sym: &Symbol, level: DetailLevel, source: &str, format: Format) -> String {
    let raw = match level {
        DetailLevel::L4 => return String::new(),
        DetailLevel::L0 => render_full(sym, source),
        DetailLevel::L1 if sym.kind.is_full_at_l1() => render_full(sym, source),
        DetailLevel::L1 => render_folded(sym, source, true),
        DetailLevel::L2 => render_folded(sym, source, false),
        DetailLevel::L3 => format!("{} {}", sym.name, marker(sym, source)),
    };
    match format {
        Format::Xml => xml_escape(&raw),
        Format::Markdown => raw,
    }
}

fn render_full(sym: &Symbol, source: &str) -> String {
    let body = slice(source, sym.span.start..sym.span.end)
        .trim_end()
        .to_string();
    if let Some(doc) = &sym.doc {
        if !body.contains(doc.as_str()) {
            return format!("{doc}\n{body}");
        }
    }
    body
}

fn render_folded(sym: &Symbol, source: &str, with_doc: bool) -> String {
    let mut parts = Vec::new();
    if with_doc {
        if let Some(doc) = &sym.doc {
            parts.push(doc.clone());
        }
    }
    let signature = slice(source, sym.span.start..sym.sig_end)
        .trim()
        .trim_end_matches('{')
        .trim_end()
        .to_string();
    let signature = if signature.is_empty() {
        sym.name.clone()
    } else {
        signature
    };
    parts.push(format!("{signature} {}", marker(sym, source)));
    if let Some(extra) = &sym.extra {
        if !extra.trim().is_empty() {
            parts.push(extra.trim().to_string());
        }
    }
    parts.join("\n")
}

fn marker(sym: &Symbol, source: &str) -> String {
    let lines = folded_lines(sym, source);
    format!("{{ /* ⟨fold:{}⟩ {lines} lines */ }}", sym.id.fold_id())
}

fn folded_lines(sym: &Symbol, source: &str) -> usize {
    if sym.span.end <= sym.span.start {
        return 1;
    }
    let end_byte = sym.span.end.saturating_sub(1);
    let end_line = crate::utils::line_number(source, end_byte);
    end_line.saturating_sub(sym.line_start).saturating_add(1)
}

pub struct RenderInput<'a> {
    pub files: &'a [String],
    pub texts: &'a HashMap<String, String>,
    pub symbols: &'a [Symbol],
    pub levels: &'a HashMap<String, DetailLevel>,
    pub format: Format,
}

pub fn render(input: RenderInput<'_>) -> String {
    match input.format {
        Format::Xml => render_xml(&input),
        Format::Markdown => render_markdown(&input),
    }
}

fn legend() -> &'static str {
    "\
Folded code is not deleted. A marker ⟨fold:PATH#SYMBOL⟩ is a stable id.
Open it with `ast-pack unfold PATH#SYMBOL` or the MCP tool `unfold`.
L0 full source. L1 signature, doc comment, and full type definitions. L2 signature. L3 name. L4 omitted.
The file stays in the tree either way."
}

fn render_xml(input: &RenderInput<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<ast-pack version=\"{}\">\n<legend>\n{}\n</legend>\n<tree>\n",
        env!("CARGO_PKG_VERSION"),
        xml_escape(legend())
    ));
    for file in input.files {
        out.push_str(&xml_escape(file));
        out.push('\n');
    }
    out.push_str("</tree>\n");
    for file in input.files {
        let visible = visible_symbols(input, file);
        if visible.is_empty() {
            continue;
        }
        out.push_str(&format!("<file path=\"{}\">\n", xml_escape(file)));
        let source = input.texts.get(file).map(String::as_str).unwrap_or("");
        for sym in visible {
            let level = level_of(input, sym);
            let body = snippet(sym, level, source, Format::Xml);
            out.push_str(&format!(
                "<symbol id=\"{}\" kind=\"{}\" level=\"{}\" lines=\"{}-{}\">\n{}\n</symbol>\n",
                xml_escape(&sym.id.fold_id()),
                sym.kind.as_str(),
                level.as_str(),
                sym.line_start,
                sym.line_end,
                body
            ));
        }
        out.push_str("</file>\n");
    }
    out.push_str("</ast-pack>\n");
    out
}

fn render_markdown(input: &RenderInput<'_>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "<!-- ast-pack {} -->\n{}\n\n## Tree\n\n",
        env!("CARGO_PKG_VERSION"),
        legend()
    ));
    for file in input.files {
        out.push_str("- ");
        out.push_str(file);
        out.push('\n');
    }
    out.push('\n');
    for file in input.files {
        let visible = visible_symbols(input, file);
        if visible.is_empty() {
            continue;
        }
        out.push_str("## ");
        out.push_str(file);
        out.push_str("\n\n");
        let ext = Path::new(file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let source = input.texts.get(file).map(String::as_str).unwrap_or("");
        let mut body = String::new();
        for sym in visible {
            let level = level_of(input, sym);
            body.push_str(&snippet(sym, level, source, Format::Markdown));
            body.push_str("\n\n");
        }
        let fence = fence_for(&body);
        out.push_str(&fence);
        let lang = Format::lang(ext);
        if !lang.is_empty() {
            out.push_str(lang);
        }
        out.push('\n');
        out.push_str(body.trim_end());
        out.push('\n');
        out.push_str(&fence);
        out.push_str("\n\n");
    }
    out
}

fn fence_for(body: &str) -> String {
    let mut size = 3;
    while body.contains(&"`".repeat(size)) {
        size += 1;
    }
    "`".repeat(size)
}

fn level_of(input: &RenderInput<'_>, sym: &Symbol) -> DetailLevel {
    input
        .levels
        .get(&sym.id.fold_id())
        .copied()
        .unwrap_or(DetailLevel::L4)
}

fn visible_symbols<'a>(input: &'a RenderInput<'_>, file: &str) -> Vec<&'a Symbol> {
    let mut syms: Vec<&Symbol> = input
        .symbols
        .iter()
        .filter(|sym| sym.id.file == file && level_of(input, sym) != DetailLevel::L4)
        .filter(|sym| !swallowed(sym, input))
        .collect();
    syms.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then(a.span.end.cmp(&b.span.end))
            .then(a.id.fold_id().cmp(&b.id.fold_id()))
    });
    syms
}

fn swallowed(sym: &Symbol, input: &RenderInput<'_>) -> bool {
    let mut container = sym.container.as_deref();
    while let Some(parent_id) = container {
        let level = input
            .levels
            .get(parent_id)
            .copied()
            .unwrap_or(DetailLevel::L4);
        let kind = input
            .symbols
            .iter()
            .find(|s| s.id.fold_id() == parent_id)
            .map(|s| s.kind);
        if level == DetailLevel::L0
            || (level == DetailLevel::L1 && kind.is_some_and(SymbolKind::is_full_at_l1))
        {
            return true;
        }
        container = input
            .symbols
            .iter()
            .find(|s| s.id.fold_id() == parent_id)
            .and_then(|s| s.container.as_deref());
    }
    false
}

pub fn count_levels(symbols: &[Symbol], levels: &HashMap<String, DetailLevel>) -> [usize; 5] {
    let mut counts = [0usize; 5];
    let mut seen = HashSet::new();
    for sym in symbols {
        let id = sym.id.fold_id();
        if !seen.insert(id.clone()) {
            continue;
        }
        let slot = match levels.get(&id).copied().unwrap_or(DetailLevel::L4) {
            DetailLevel::L0 => 0,
            DetailLevel::L1 => 1,
            DetailLevel::L2 => 2,
            DetailLevel::L3 => 3,
            DetailLevel::L4 => 4,
        };
        counts[slot] += 1;
    }
    counts
}

pub fn empty_levels(symbols: &[Symbol], level: DetailLevel) -> HashMap<String, DetailLevel> {
    symbols
        .iter()
        .map(|sym| (sym.id.fold_id(), level))
        .collect()
}

pub fn level_map_from_pairs(
    pairs: impl IntoIterator<Item = (String, DetailLevel)>,
) -> HashMap<String, DetailLevel> {
    pairs.into_iter().collect()
}

pub fn sorted_files(files: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    let mut files: Vec<String> = files.into_iter().map(|f| f.as_ref().to_string()).collect();
    files.sort();
    files.dedup();
    files
}
