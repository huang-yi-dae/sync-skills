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
    pub core_hash: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Skill with installation status for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillView {
    #[serde(flatten)]
    pub skill: Skill,
    pub installed_tools: Vec<InstallationInfo>,
    pub install_count: usize,
    pub has_update: bool,
}

/// Installation info for a single tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationInfo {
    pub tool_id: i64,
    pub tool_name: String,
    pub status: String,
    pub synced_at: Option<String>,
}

/// Result of a sync operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub skill_id: i64,
    pub skill_name: String,
    pub synced_to: usize,
    pub errors: Vec<String>,
}

/// Update info for a skill (used by check_updates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillUpdate {
    pub skill_id: i64,
    pub skill_name: String,
    pub source_path: String,
    pub old_hash: String,
    pub new_hash: String,
}

/// A sync log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLog {
    pub id: i64,
    pub skill_id: Option<i64>,
    pub skill_name: Option<String>,
    pub tool_id: Option<i64>,
    pub tool_name: Option<String>,
    pub project_id: i64,
    pub action: String,
    pub direction: Option<String>,
    pub status: String,
    pub detail: Option<String>,
    pub created_at: String,
}

/// Detail of a single skill found during scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanDetail {
    pub skill_name: String,
    pub tool_name: String,
    pub scope: String,       // "Global" or project name
    pub status: String,      // "new" or "updated"
    pub source_path: String,
}

/// Result of a scan operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub skills_found: usize,
    pub skills_new: usize,
    pub skills_updated: usize,
    pub errors: Vec<String>,
    pub details: Vec<ScanDetail>,
}

/// A discovered skill during scanning (before DB insert)
#[derive(Debug, Clone)]
pub struct DiscoveredSkill {
    pub name: String,
    pub description: Option<String>,
    pub source_path: String,
    pub content_hash: String,
    pub core_hash: String,
}
