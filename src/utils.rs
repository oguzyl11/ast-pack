use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::error::PackError;

/// Slice `src` on byte offsets, snapping to UTF-8 char boundaries.
pub fn slice(src: &str, range: Range<usize>) -> &str {
    let mut start = range.start.min(src.len());
    let mut end = range.end.min(src.len()).max(start);
    while start > 0 && !src.is_char_boundary(start) {
        start -= 1;
    }
    while end < src.len() && !src.is_char_boundary(end) {
        end += 1;
    }
    if end < start {
        end = start;
    }
    &src[start..end]
}

pub fn line_number(src: &str, byte: usize) -> usize {
    let byte = byte.min(src.len());
    let mut byte = byte;
    while byte > 0 && !src.is_char_boundary(byte) {
        byte -= 1;
    }
    src[..byte].bytes().filter(|b| *b == b'\n').count() + 1
}

pub fn parse_budget(raw: &str) -> Result<usize, PackError> {
    let trimmed = raw.trim().replace('_', "");
    if trimmed.is_empty() {
        return Err(PackError::msg("budget is empty"));
    }
    let (number, mult) = if let Some(rest) = trimmed.strip_suffix(['k', 'K']) {
        (rest, 1000usize)
    } else {
        (trimmed.as_str(), 1usize)
    };
    let value: usize = number
        .parse()
        .map_err(|_| PackError::msg(format!("invalid budget `{raw}`")))?;
    if value == 0 {
        return Err(PackError::msg("budget must be greater than zero"));
    }
    Ok(value.saturating_mul(mult))
}

pub fn rel_slash(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().replace('\\', "/"))
        .collect::<Vec<_>>()
        .join("/")
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

pub fn hash_file(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hash_bytes(&bytes))
}

/// Copy `path` aside once. A missing original is recorded so `--remove` can delete it.
pub fn backup_once(path: &Path) -> std::io::Result<()> {
    let bak = backup_path(path);
    let absent = absent_path(path);
    if bak.exists() || absent.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        std::fs::copy(path, bak)?;
    } else {
        std::fs::write(absent, b"")?;
    }
    Ok(())
}

pub fn restore_backup(path: &Path) -> std::io::Result<bool> {
    let bak = backup_path(path);
    let absent = absent_path(path);
    if bak.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&bak, path)?;
        let _ = std::fs::remove_file(&bak);
        let _ = std::fs::remove_file(&absent);
        return Ok(true);
    }
    if absent.exists() {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        let _ = std::fs::remove_file(&absent);
        return Ok(true);
    }
    Ok(false)
}

pub fn backup_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.ast-pack-bak", path.display()))
}

pub fn absent_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.ast-pack-absent", path.display()))
}

const STOP: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "your", "how", "what", "when",
    "where", "which", "does", "did", "are", "was", "were", "have", "has", "not", "but", "you",
    "can", "its", "per", "before", "after", "make", "makes", "many", "much", "does", "return",
    "returns",
];

pub fn task_terms(task: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in task.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        let term = raw.trim_matches('_').to_lowercase();
        if term.len() < 3 || STOP.contains(&term.as_str()) {
            continue;
        }
        if !out.contains(&term) {
            out.push(term);
        }
    }
    out
}

pub fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn focus_matches(pattern: &str, rel: &str) -> bool {
    let rel = rel.replace('\\', "/");
    let pattern = pattern.replace('\\', "/");
    if let Ok(glob) = glob::Pattern::new(&pattern) {
        if glob.matches(&rel) {
            return true;
        }
    }
    let prefix = pattern.trim_end_matches('/');
    if prefix.is_empty() {
        return false;
    }
    rel == prefix
        || rel.starts_with(&format!("{prefix}/"))
        || rel.starts_with(&format!("{prefix}."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_budget_suffix() {
        assert_eq!(parse_budget("16k").unwrap(), 16_000);
        assert_eq!(parse_budget("16K").unwrap(), 16_000);
        assert_eq!(parse_budget("32_000").unwrap(), 32_000);
        assert!(parse_budget("0").is_err());
    }

    #[test]
    fn focus_directory_and_file() {
        assert!(focus_matches("src/auth", "src/auth.rs"));
        assert!(focus_matches("src/auth", "src/auth/token.rs"));
        assert!(focus_matches("src/main.rs", "src/main.rs"));
        assert!(!focus_matches("src/auth", "src/auth_extra.rs"));
    }
}
