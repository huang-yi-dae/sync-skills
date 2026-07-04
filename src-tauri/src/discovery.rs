// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: AGPL-3.0-only

use crate::db::Database;
use crate::scanner::expand_path;
use serde::Serialize;

/// A known AI coding tool template with default paths.
#[derive(Debug, Clone, Serialize)]
pub struct ToolTemplate {
    pub name: String,
    pub global_path: String,
    pub project_rel_path: String,
}

/// Registry of known AI coding tools and their default skill directories.
pub const KNOWN_TOOLS: &[(&str, &str, &str)] = &[
    ("Claude Code", "~/.claude/skills/", ".claude/skills/"),
    ("Codex CLI", "~/.codex/skills/", ".codex/skills/"),
    ("OpenCode", "~/.opencode/skills/", ".opencode/skills/"),
    ("Gemini CLI", "~/.gemini/skills/", ".gemini/skills/"),
    ("Cline", "~/.cline/skills/", ".cline/skills/"),
    ("Cursor", "~/.cursor/skills/", ".cursor/skills/"),
    ("Windsurf", "~/.windsurf/skills/", ".windsurf/skills/"),
    ("Aider", "~/.aider/skills/", ".aider/skills/"),
    ("Roo Code", "~/.roo/skills/", ".roo/skills/"),
    ("Trae", "~/.trae/skills/", ".trae/skills/"),
    ("Kiro", "~/.kiro/skills/", ".kiro/skills/"),
    ("Augment", "~/.augment/skills/", ".augment/skills/"),
    ("Qoder", "~/.qoder/skills/", ".qoder/skills/"),
];

/// Return all known tool templates (for frontend dropdown).
pub fn get_all_templates() -> Vec<ToolTemplate> {
    KNOWN_TOOLS
        .iter()
        .map(|(name, global, rel)| ToolTemplate {
            name: name.to_string(),
            global_path: global.to_string(),
            project_rel_path: rel.to_string(),
        })
        .collect()
}

/// Discover known tools whose global_path exists on disk but are not yet
/// registered in the database. Returns templates for newly found tools.
pub fn discover_tools(db: &Database) -> Result<Vec<ToolTemplate>, String> {
    let existing = db.list_tools()?;
    let existing_names: Vec<String> = existing.iter().map(|t| t.name.to_lowercase()).collect();

    let mut found = Vec::new();

    for (name, global_path, project_rel_path) in KNOWN_TOOLS {
        // Skip if already registered (case-insensitive match)
        if existing_names.contains(&name.to_lowercase()) {
            continue;
        }

        // Check if the directory exists on disk
        let expanded = match expand_path(global_path) {
            Ok(p) => p,
            Err(_) => continue,
        };

        if expanded.is_dir() {
            found.push(ToolTemplate {
                name: name.to_string(),
                global_path: global_path.to_string(),
                project_rel_path: project_rel_path.to_string(),
            });
        }
    }

    Ok(found)
}
