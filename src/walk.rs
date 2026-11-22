use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

const SKIP_DIRS: &[&str] = &[
    "target",
    "node_modules",
    ".ast-pack-cache",
    "dist",
    "build",
    ".git",
    "vendor",
];

const SOURCE_EXT: &[&str] = &["rs", "ts", "tsx", "js", "jsx", "py", "go", "java"];

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub absolute: PathBuf,
    pub relative: String,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub files: Vec<SourceFile>,
}

pub fn load_workspace(path: &Path) -> Result<Workspace, crate::error::PackError> {
    if !path.exists() {
        return Err(crate::error::PackError::msg(format!(
            "path does not exist: {}",
            path.display()
        )));
    }
    if path.is_file() {
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let relative = path
            .file_name()
            .map(|s| s.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| "file".into());
        if !is_source(path) {
            return Err(crate::error::PackError::msg(
                "file is not a supported source language",
            ));
        }
        return Ok(Workspace {
            root,
            files: vec![SourceFile {
                absolute: path.to_path_buf(),
                relative,
            }],
        });
    }

    let mut files = Vec::new();
    let walker = WalkBuilder::new(path)
        .hidden(true)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .parents(true)
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !SKIP_DIRS.contains(&name.as_ref())
        })
        .build();
    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|k| k.is_file()) {
            continue;
        }
        let abs = entry.path();
        if !is_source(abs) {
            continue;
        }
        let relative = crate::utils::rel_slash(path, abs);
        files.push(SourceFile {
            absolute: abs.to_path_buf(),
            relative,
        });
    }
    files.sort_by(|a, b| a.relative.cmp(&b.relative));
    files.dedup_by(|a, b| a.relative == b.relative);
    Ok(Workspace {
        root: path.to_path_buf(),
        files,
    })
}

pub fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| SOURCE_EXT.contains(&ext))
}
