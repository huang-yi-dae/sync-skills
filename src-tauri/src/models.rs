// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

/// AI coding tool (e.g., Claude Code, Codex CLI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: i64,
    pub name: String,
    pub global_path: String,
    pub project_rel_path: String,
    pub created_at: String,
    pub updated_at: String,
}

/// A user project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub created_at: String,
}

/// A discovered skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub source_path: String,
    pub content_hash: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Skill with installation status for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillView {
    #[serde(flatten)]
    pub skill: Skill,
    pub installed_tools: Vec<String>,
    pub install_count: usize,
}

/// Result of a scan operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub skills_found: usize,
    pub skills_new: usize,
    pub skills_updated: usize,
    pub errors: Vec<String>,
}

/// A discovered skill during scanning (before DB insert)
#[derive(Debug, Clone)]
pub struct DiscoveredSkill {
    pub name: String,
    pub description: Option<String>,
    pub source_path: String,
    pub content_hash: String,
}
