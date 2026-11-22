use std::path::{Path, PathBuf};

use crate::error::PackError;
use crate::utils::{backup_once, restore_backup};

enum TargetKind {
    Json,
    Codex,
}

struct Target {
    path: PathBuf,
    kind: TargetKind,
}

pub fn apply(home: &Path, project: &Path, remove: bool) -> Result<Vec<String>, PackError> {
    let exe =
        std::env::current_exe().map_err(|err| PackError::msg(format!("current exe: {err}")))?;
    let mut notes = Vec::new();
    for target in targets(home, project) {
        if remove {
            if restore_backup(&target.path)? {
                notes.push(format!("restored {}", target.path.display()));
            } else {
                notes.push(format!("nothing to restore at {}", target.path.display()));
            }
            continue;
        }
        backup_once(&target.path)?;
        match target.kind {
            TargetKind::Json => write_mcp_json(&target.path, &exe)?,
            TargetKind::Codex => write_codex(&target.path, &exe)?,
        }
        notes.push(format!("registered {}", target.path.display()));
    }
    Ok(notes)
}

fn targets(home: &Path, project: &Path) -> Vec<Target> {
    vec![
        Target {
            path: project.join(".cursor").join("mcp.json"),
            kind: TargetKind::Json,
        },
        Target {
            path: project.join(".mcp.json"),
            kind: TargetKind::Json,
        },
        Target {
            path: home.join(".cursor").join("mcp.json"),
            kind: TargetKind::Json,
        },
        Target {
            path: home.join(".claude.json"),
            kind: TargetKind::Json,
        },
        Target {
            path: home.join(".codex").join("config.toml"),
            kind: TargetKind::Codex,
        },
        Target {
            path: home.join(".gemini").join("settings.json"),
            kind: TargetKind::Json,
        },
    ]
}

fn write_mcp_json(path: &Path, exe: &Path) -> Result<(), PackError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut value = if path.exists() {
        let text = std::fs::read_to_string(path)?;
        if text.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&text).map_err(|err| {
                PackError::msg(format!("{} is not valid JSON ({err})", path.display()))
            })?
        }
    } else {
        serde_json::json!({})
    };
    if !value.is_object() {
        return Err(PackError::msg(format!(
            "{} is not a JSON object",
            path.display()
        )));
    }
    value
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    value["mcpServers"]["ast-pack"] = serde_json::json!({
        "command": exe,
        "args": ["mcp"],
    });
    let pretty = serde_json::to_string_pretty(&value)
        .map_err(|err| PackError::msg(format!("json: {err}")))?;
    std::fs::write(path, pretty + "\n")?;
    Ok(())
}

fn write_codex(path: &Path, exe: &Path) -> Result<(), PackError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let existing = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };
    let command = exe
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    let section = format!("[mcp_servers.ast-pack]\ncommand = \"{command}\"\nargs = [\"mcp\"]\n");
    let stripped = strip_toml_section(&existing, "[mcp_servers.ast-pack]");
    let mut out = stripped.trim_end().to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&section);
    std::fs::write(path, out)?;
    Ok(())
}

fn strip_toml_section(text: &str, header: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed == header {
            skipping = true;
            continue;
        }
        if skipping && trimmed.starts_with('[') && trimmed.ends_with(']') {
            skipping = false;
        }
        if skipping {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

pub fn home_dir() -> Result<PathBuf, PackError> {
    if let Some(path) = std::env::var_os("AST_PACK_HOME") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("USERPROFILE") {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = std::env::var_os("HOME") {
        return Ok(PathBuf::from(path));
    }
    Err(PackError::msg("could not locate the user home directory"))
}
