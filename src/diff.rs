use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::path::Path;
use std::process::Command;

use crate::error::PackError;

pub fn changed_lines(
    root: &Path,
    git_ref: &str,
) -> Result<HashMap<String, Vec<RangeInclusive<usize>>>, PackError> {
    let output = Command::new("git")
        .args([
            "-c",
            "core.quotepath=false",
            "diff",
            "-U0",
            "--no-color",
            git_ref,
        ])
        .current_dir(root)
        .output()
        .map_err(|err| PackError::msg(format!("could not run git: {err}")))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(PackError::msg(format!(
            "git diff {git_ref} failed: {}",
            err.trim()
        )));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_unified_diff(&text))
}

pub fn parse_unified_diff(text: &str) -> HashMap<String, Vec<RangeInclusive<usize>>> {
    let mut map: HashMap<String, Vec<RangeInclusive<usize>>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            let path = rest.trim().trim_matches('"');
            if path == "/dev/null" {
                current = None;
            } else {
                let path = path.strip_prefix("b/").unwrap_or(path).replace('\\', "/");
                current = Some(path);
            }
            continue;
        }
        if line.starts_with("@@") {
            let Some(file) = current.clone() else {
                continue;
            };
            if let Some(range) = hunk_new_lines(line) {
                map.entry(file).or_default().push(range);
            }
        }
    }
    map
}

fn hunk_new_lines(line: &str) -> Option<RangeInclusive<usize>> {
    let plus = line.split_whitespace().find(|part| part.starts_with('+'))?;
    let plus = plus.trim_start_matches('+');
    if plus == "" || plus.starts_with('+') {
        return None;
    }
    let mut parts = plus.split(',');
    let start: usize = parts.next()?.parse().ok()?;
    let count: usize = parts.next().unwrap_or("1").parse().ok()?;
    if start == 0 {
        return None;
    }
    let end = if count == 0 { start } else { start + count - 1 };
    Some(start..=end)
}

pub fn overlaps(line_start: usize, line_end: usize, ranges: &[RangeInclusive<usize>]) -> bool {
    ranges
        .iter()
        .any(|range| line_start <= *range.end() && *range.start() <= line_end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_hunk_line_range() {
        let diff = "\
diff --git a/src/a.rs b/src/a.rs
--- a/src/a.rs
+++ b/src/a.rs
@@ -10 +11,2 @@
+one
+two
";
        let map = parse_unified_diff(diff);
        assert_eq!(map["src/a.rs"], vec![11..=12]);
    }
}
