// src/utils.rs
// Utility placeholders for ast-pack.

/// Computes a blake3 hash of a file path. Stub returns empty hash.
pub fn hash_file(_path: &std::path::Path) -> blake3::Hash {
    blake3::Hash::from([0u8; 32])
}

/// Dummy function to create a backup of a file. Currently does nothing.
pub fn backup_file(_path: &std::path::Path) -> std::io::Result<()> {
    // In real implementation, copy the file to a .bak version.
    Ok(())
}

