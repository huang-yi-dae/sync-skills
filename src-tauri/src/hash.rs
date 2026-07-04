// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: AGPL-3.0-only

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// Compute an ID hash from a string.
/// Takes SHA-256, first 8 bytes, little-endian i64, masked to fit within
/// JavaScript's Number.MAX_SAFE_INTEGER (2^53 - 1) to prevent precision loss
/// when IDs are serialized as JSON numbers across the Tauri IPC boundary.
pub fn compute_id_hash(input: &str) -> i64 {
    let hash = Sha256::digest(input.as_bytes());
    let bytes: [u8; 8] = hash[..8].try_into().unwrap();
    let raw = u64::from_le_bytes(bytes);
    // Mask to 53 bits: ensures the result is within JS safe integer range
    let masked = raw & 0x001F_FFFF_FFFF_FFFF;
    masked as i64
}

/// Compute content hash for a directory.
/// Recursively traverses non-hidden files, sorts by relative path,
/// feeds "relative_path\0content\0" to SHA-256.
pub fn compute_content_hash(dir: &Path) -> Result<String, String> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    collect_files(dir, dir, &mut files)?;

    // Sort by relative path (lexicographic)
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel_path, content) in files {
        hasher.update(rel_path.as_bytes());
        hasher.update(b"\0");
        hasher.update(&content);
        hasher.update(b"\0");
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute the core identity hash of a skill by hashing only its SKILL.md file.
/// Unlike compute_content_hash (which hashes the entire directory), this is stable
/// across copies: the same skill in different tool directories has the same core_hash.
/// This follows Git's content-addressable model — identity = content, not location.
pub fn compute_core_hash(skill_md_path: &Path) -> Result<String, String> {
    let content = fs::read(skill_md_path)
        .map_err(|e| format!("Failed to read {:?}: {}", skill_md_path, e))?;
    let hash = Sha256::digest(&content);
    Ok(format!("{:x}", hash))
}

fn collect_files(
    base: &Path,
    current: &Path,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    let entries = fs::read_dir(current).map_err(|e| format!("Failed to read dir {:?}: {}", current, e))?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Dir entry error: {}", e))?;
        let path = entry.path();

        // Skip hidden files/dirs
        if is_hidden(&path) {
            continue;
        }

        // Skip sync metadata files (injected by sync engine, not part of skill content)
        if let Some(name) = path.file_name() {
            if name == "local.md" {
                continue;
            }
        }

        if path.is_dir() {
            collect_files(base, &path, files)?;
        } else if path.is_file() {
            let rel_path = path
                .strip_prefix(base)
                .map_err(|e| format!("Strip prefix error: {}", e))?
                .to_string_lossy()
                .replace('\\', "/"); // Normalize to forward slashes

            let content = fs::read(&path)
                .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;
            files.push((rel_path, content));
        }
    }

    Ok(())
}

/// Check if a path component is hidden (starts with '.')
/// On Windows, also check FILE_ATTRIBUTE_HIDDEN
fn is_hidden(path: &Path) -> bool {
    // Check filename starts with '.'
    if let Some(name) = path.file_name() {
        if name.to_string_lossy().starts_with('.') {
            return true;
        }
    }

    // On Windows, check file attribute
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(metadata) = path.metadata() {
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            return metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_id_hash() {
        let hash = compute_id_hash("Claude Code");
        // Just verify it returns a consistent value
        let hash2 = compute_id_hash("Claude Code");
        assert_eq!(hash, hash2);

        // Different inputs produce different hashes
        let hash3 = compute_id_hash("Codex CLI");
        assert_ne!(hash, hash3);
    }
}
