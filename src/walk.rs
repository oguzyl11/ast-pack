// src/walk.rs

// Placeholder walk module – actual implementation lives in the root workspace.
// For now we expose an empty public function so the ast-pack binary can compile.

pub fn walk_repo(_root: &std::path::Path) -> Vec<std::path::PathBuf> {
    // In the full project this would walk the repository respecting .gitignore.
    // Returning an empty vector is sufficient for compilation.
    Vec::new()
}

