// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: MIT

use crate::hash::compute_id_hash;
use crate::models::{InstallationInfo, Project, Skill, SkillView, SyncLog, Tool};
use crate::scanner;
use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;

/// Database wrapper with mutex for thread safety
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Open or create the database at the default location
    pub fn new() -> Result<Self, String> {
        let db_path = Self::db_path()?;

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create DB directory: {}", e))?;
        }

        let conn = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open database: {}", e))?;

        // Enable foreign keys
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| format!("Failed to enable foreign keys: {}", e))?;

        let db = Database {
            conn: Mutex::new(conn),
        };

        db.init_schema()?;
        db.seed_data()?;

        Ok(db)
    }

    fn db_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or("Cannot find home directory")?;
        Ok(home.join(".agents").join("skill-manager.db"))
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS tools (
                id              INTEGER PRIMARY KEY,
                name            TEXT    NOT NULL UNIQUE,
                global_path     TEXT    NOT NULL,
                project_rel_path TEXT   NOT NULL,
                created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS projects (
                id              INTEGER PRIMARY KEY,
                name            TEXT    NOT NULL,
                path            TEXT    NOT NULL UNIQUE,
                created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_projects_path ON projects(path);

            CREATE TABLE IF NOT EXISTS skills (
                id              INTEGER PRIMARY KEY,
                name            TEXT    NOT NULL,
                description     TEXT,
                source_path     TEXT    NOT NULL UNIQUE,
                content_hash    TEXT    NOT NULL,
                created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);

            CREATE TABLE IF NOT EXISTS skill_installations (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_id        INTEGER NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
                tool_id         INTEGER NOT NULL REFERENCES tools(id) ON DELETE CASCADE,
                project_id      INTEGER NOT NULL DEFAULT 0 REFERENCES projects(id) ON DELETE CASCADE,
                status          TEXT    NOT NULL DEFAULT 'disabled'
                    CHECK (status IN ('active', 'disabled')),
                synced_at       TEXT,
                created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT    NOT NULL DEFAULT (datetime('now')),
                UNIQUE (skill_id, tool_id, project_id)
            );

            CREATE INDEX IF NOT EXISTS idx_inst_skill ON skill_installations(skill_id);
            CREATE INDEX IF NOT EXISTS idx_inst_tool ON skill_installations(tool_id);
            CREATE INDEX IF NOT EXISTS idx_inst_project ON skill_installations(project_id);
            CREATE INDEX IF NOT EXISTS idx_inst_status ON skill_installations(status);

            CREATE TABLE IF NOT EXISTS sync_logs (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_id        INTEGER NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
                tool_id         INTEGER NOT NULL REFERENCES tools(id) ON DELETE CASCADE,
                project_id      INTEGER NOT NULL DEFAULT 0 REFERENCES projects(id) ON DELETE CASCADE,
                direction       TEXT    NOT NULL CHECK (direction IN ('to_ssot', 'from_ssot')),
                status          TEXT    NOT NULL CHECK (status IN ('success', 'failed')),
                error_message   TEXT,
                created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_log_skill ON sync_logs(skill_id);
            CREATE INDEX IF NOT EXISTS idx_log_project ON sync_logs(project_id);
            CREATE INDEX IF NOT EXISTS idx_log_direction ON sync_logs(direction);
            CREATE INDEX IF NOT EXISTS idx_log_status ON sync_logs(status);
            CREATE INDEX IF NOT EXISTS idx_log_created ON sync_logs(created_at);
            ",
        )
        .map_err(|e| format!("Failed to create schema: {}", e))?;

        Ok(())
    }

    /// Insert seed data (preset tools and global project)
    fn seed_data(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        // Insert global project
        conn.execute(
            "INSERT OR IGNORE INTO projects (id, name, path) VALUES (0, 'Global', '~/.agents/skills/')",
            [],
        )
        .map_err(|e| format!("Failed to seed global project: {}", e))?;

        // Preset tools with pre-computed hash IDs
        let tools = [
            (-768412307910267356i64, "Claude Code", "~/.claude/skills/", ".claude/skills/"),
            (-5387663353590988835i64, "Codex CLI", "~/.codex/skills/", ".codex/skills/"),
            (8996106060148633658i64, "OpenCode", "~/.opencode/skills/", ".opencode/skills/"),
            (-1843142830140024973i64, "Gemini CLI", "~/.gemini/skills/", ".gemini/skills/"),
            (6180056807951602058i64, "Cline", "~/.cline/skills/", ".cline/skills/"),
        ];

        for (id, name, global_path, project_rel_path) in tools {
            conn.execute(
                "INSERT OR IGNORE INTO tools (id, name, global_path, project_rel_path) VALUES (?1, ?2, ?3, ?4)",
                params![id, name, global_path, project_rel_path],
            )
            .map_err(|e| format!("Failed to seed tool {}: {}", name, e))?;
        }

        Ok(())
    }

    /// List all tools
    pub fn list_tools(&self) -> Result<Vec<Tool>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT id, name, global_path, project_rel_path, created_at, updated_at FROM tools ORDER BY name")
            .map_err(|e| format!("Prepare error: {}", e))?;

        let tools = stmt
            .query_map([], |row| {
                Ok(Tool {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    global_path: row.get(2)?,
                    project_rel_path: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(tools)
    }

    /// Add a new tool
    pub fn add_tool(&self, name: &str, global_path: &str, project_rel_path: &str) -> Result<Tool, String> {
        let id = compute_id_hash(name);
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.execute(
            "INSERT INTO tools (id, name, global_path, project_rel_path) VALUES (?1, ?2, ?3, ?4)",
            params![id, name, global_path, project_rel_path],
        )
        .map_err(|e| format!("Failed to add tool: {}", e))?;

        // Fetch the inserted row to get created_at/updated_at from SQLite
        conn.query_row(
            "SELECT id, name, global_path, project_rel_path, created_at, updated_at FROM tools WHERE id = ?1",
            params![id],
            |row| {
                Ok(Tool {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    global_path: row.get(2)?,
                    project_rel_path: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )
        .map_err(|e| format!("Failed to fetch inserted tool: {}", e))
    }

    /// Update tool path
    pub fn update_tool_path(
        &self,
        tool_id: i64,
        global_path: &str,
        project_rel_path: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.execute(
            "UPDATE tools SET global_path = ?1, project_rel_path = ?2, updated_at = datetime('now') WHERE id = ?3",
            params![global_path, project_rel_path, tool_id],
        )
        .map_err(|e| format!("Failed to update tool: {}", e))?;

        Ok(())
    }

    /// Delete a tool (cascades to installations and sync_logs)
    pub fn delete_tool(&self, tool_id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.execute("DELETE FROM tools WHERE id = ?1", params![tool_id])
            .map_err(|e| format!("Failed to delete tool: {}", e))?;

        Ok(())
    }

    /// Upsert a skill (insert or update on conflict)
    pub fn upsert_skill(
        &self,
        name: &str,
        description: Option<&str>,
        source_path: &str,
        content_hash: &str,
    ) -> Result<(i64, bool), String> {
        let id = compute_id_hash(source_path);
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        // Check if exists
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM skills WHERE source_path = ?1",
                params![source_path],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if exists {
            conn.execute(
                "UPDATE skills SET name = ?1, description = ?2, content_hash = ?3, updated_at = datetime('now') WHERE source_path = ?4",
                params![name, description, content_hash, source_path],
            )
            .map_err(|e| format!("Failed to update skill: {}", e))?;
            Ok((id, false))
        } else {
            conn.execute(
                "INSERT INTO skills (id, name, description, source_path, content_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, name, description, source_path, content_hash],
            )
            .map_err(|e| format!("Failed to insert skill: {}", e))?;
            Ok((id, true))
        }
    }

    /// List all skills
    pub fn list_skills(&self) -> Result<Vec<Skill>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT id, name, description, source_path, content_hash, created_at, updated_at FROM skills ORDER BY name")
            .map_err(|e| format!("Prepare error: {}", e))?;

        let skills = stmt
            .query_map([], |row| {
                Ok(Skill {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_path: row.get(3)?,
                    content_hash: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(skills)
    }

    /// Get tool's resolved paths (global_path, project_rel_path)
    pub fn get_tool_paths(&self) -> Result<Vec<(i64, String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT id, global_path, project_rel_path FROM tools")
            .map_err(|e| format!("Prepare error: {}", e))?;

        let paths = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(paths)
    }

    /// Get skill's existing content_hash
    pub fn get_skill_hash(&self, source_path: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let result = conn.query_row(
            "SELECT content_hash FROM skills WHERE source_path = ?1",
            params![source_path],
            |row| row.get(0),
        );

        match result {
            Ok(hash) => Ok(Some(hash)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query error: {}", e)),
        }
    }

    // ==================== M2: Sync & Installation ====================

    /// Get a single skill by ID
    pub fn get_skill_by_id(&self, skill_id: i64) -> Result<Skill, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.query_row(
            "SELECT id, name, description, source_path, content_hash, created_at, updated_at FROM skills WHERE id = ?1",
            params![skill_id],
            |row| {
                Ok(Skill {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_path: row.get(3)?,
                    content_hash: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .map_err(|e| format!("Failed to get skill: {}", e))
    }

    /// List skills with installation status (SkillView)
    pub fn list_skills_with_status(&self, _project_id: i64) -> Result<Vec<SkillView>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        // Get all skills
        let mut skill_stmt = conn
            .prepare("SELECT id, name, description, source_path, content_hash, created_at, updated_at FROM skills ORDER BY name")
            .map_err(|e| format!("Prepare error: {}", e))?;

        let skills: Vec<Skill> = skill_stmt
            .query_map([], |row| {
                Ok(Skill {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_path: row.get(3)?,
                    content_hash: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        // For each skill, get installation info
        let mut views = Vec::new();
        for skill in skills {
            let mut inst_stmt = conn
                .prepare(
                    "SELECT si.tool_id, t.name, si.status, si.synced_at
                     FROM skill_installations si
                     JOIN tools t ON t.id = si.tool_id
                     WHERE si.skill_id = ?1 AND si.project_id = 0",
                )
                .map_err(|e| format!("Prepare error: {}", e))?;

            let installations: Vec<InstallationInfo> = inst_stmt
                .query_map(params![skill.id], |row| {
                    Ok(InstallationInfo {
                        tool_id: row.get(0)?,
                        tool_name: row.get(1)?,
                        status: row.get(2)?,
                        synced_at: row.get(3)?,
                    })
                })
                .map_err(|e| format!("Query error: {}", e))?
                .filter_map(|r| r.ok())
                .collect();

            let install_count = installations.len();

            // Check if skill has an update (content_hash differs from any active installation's synced hash)
            let has_update = false; // Will be set by check_updates logic

            views.push(SkillView {
                skill,
                installed_tools: installations,
                install_count,
                has_update,
            });
        }

        Ok(views)
    }

    /// Toggle skill installation status (enable/disable)
    pub fn toggle_installation(
        &self,
        skill_id: i64,
        tool_id: i64,
        project_id: i64,
        active: bool,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        let status = if active { "active" } else { "disabled" };

        conn.execute(
            "INSERT INTO skill_installations (skill_id, tool_id, project_id, status)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(skill_id, tool_id, project_id) DO UPDATE SET
                status = ?4,
                updated_at = datetime('now')",
            params![skill_id, tool_id, project_id, status],
        )
        .map_err(|e| format!("Failed to toggle installation: {}", e))?;

        Ok(())
    }

    /// Get all active installations for a skill (for sync targeting)
    pub fn get_active_installations(&self, skill_id: i64, project_id: i64) -> Result<Vec<(i64, String)>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT si.tool_id, t.global_path
                 FROM skill_installations si
                 JOIN tools t ON t.id = si.tool_id
                 WHERE si.skill_id = ?1 AND si.project_id = ?2 AND si.status = 'active'",
            )
            .map_err(|e| format!("Prepare error: {}", e))?;

        let results = stmt
            .query_map(params![skill_id, project_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    /// Update synced_at timestamp for an installation
    pub fn update_synced_at(&self, skill_id: i64, tool_id: i64, project_id: i64) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.execute(
            "UPDATE skill_installations SET synced_at = datetime('now'), updated_at = datetime('now')
             WHERE skill_id = ?1 AND tool_id = ?2 AND project_id = ?3",
            params![skill_id, tool_id, project_id],
        )
        .map_err(|e| format!("Failed to update synced_at: {}", e))?;

        Ok(())
    }

    /// Insert a sync log entry
    pub fn insert_sync_log(
        &self,
        skill_id: i64,
        tool_id: i64,
        project_id: i64,
        direction: &str,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.execute(
            "INSERT INTO sync_logs (skill_id, tool_id, project_id, direction, status, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![skill_id, tool_id, project_id, direction, status, error_message],
        )
        .map_err(|e| format!("Failed to insert sync log: {}", e))?;

        Ok(())
    }

    /// Query sync logs with optional filters
    pub fn get_sync_logs(&self, skill_id: Option<i64>, limit: Option<i64>) -> Result<Vec<SyncLog>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let limit = limit.unwrap_or(100);

        let query = match skill_id {
            Some(sid) => format!(
                "SELECT sl.id, sl.skill_id, s.name, sl.tool_id, t.name, sl.project_id,
                        sl.direction, sl.status, sl.error_message, sl.created_at
                 FROM sync_logs sl
                 LEFT JOIN skills s ON s.id = sl.skill_id
                 LEFT JOIN tools t ON t.id = sl.tool_id
                 WHERE sl.skill_id = {}
                 ORDER BY sl.created_at DESC LIMIT {}",
                sid, limit
            ),
            None => format!(
                "SELECT sl.id, sl.skill_id, s.name, sl.tool_id, t.name, sl.project_id,
                        sl.direction, sl.status, sl.error_message, sl.created_at
                 FROM sync_logs sl
                 LEFT JOIN skills s ON s.id = sl.skill_id
                 LEFT JOIN tools t ON t.id = sl.tool_id
                 ORDER BY sl.created_at DESC LIMIT {}",
                limit
            ),
        };

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| format!("Prepare error: {}", e))?;

        let logs = stmt
            .query_map([], |row| {
                Ok(SyncLog {
                    id: row.get(0)?,
                    skill_id: row.get(1)?,
                    skill_name: row.get(2)?,
                    tool_id: row.get(3)?,
                    tool_name: row.get(4)?,
                    project_id: row.get(5)?,
                    direction: row.get(6)?,
                    status: row.get(7)?,
                    error_message: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(logs)
    }

    /// Check for skill updates by comparing DB hashes with current file hashes
    pub fn get_all_skills_for_update_check(&self) -> Result<Vec<(i64, String, String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT id, name, source_path, content_hash FROM skills")
            .map_err(|e| format!("Prepare error: {}", e))?;

        let skills = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(skills)
    }

    // ==================== M3: Project Management ====================

    /// List all projects
    pub fn list_projects(&self) -> Result<Vec<Project>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT id, name, path, created_at FROM projects ORDER BY id")
            .map_err(|e| format!("Prepare error: {}", e))?;

        let projects = stmt
            .query_map([], |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(projects)
    }

    /// Add a new project
    pub fn add_project(&self, name: &str, path: &str) -> Result<Project, String> {
        let id = compute_id_hash(path);
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        // Check if path already exists
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM projects WHERE path = ?1",
                params![path],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if exists {
            return Err("Project with this path already exists".to_string());
        }

        conn.execute(
            "INSERT INTO projects (id, name, path) VALUES (?1, ?2, ?3)",
            params![id, name, path],
        )
        .map_err(|e| format!("Failed to add project: {}", e))?;

        // Fetch the inserted row
        conn.query_row(
            "SELECT id, name, path, created_at FROM projects WHERE id = ?1",
            params![id],
            |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        )
        .map_err(|e| format!("Failed to fetch inserted project: {}", e))
    }

    /// Delete a project (cascades to installations and sync_logs)
    pub fn delete_project(&self, project_id: i64) -> Result<(), String> {
        if project_id == 0 {
            return Err("Cannot delete the Global project".to_string());
        }

        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.execute("DELETE FROM projects WHERE id = ?1", params![project_id])
            .map_err(|e| format!("Failed to delete project: {}", e))?;

        Ok(())
    }

    /// Get a project's path by ID
    pub fn get_project_path(&self, project_id: i64) -> Result<String, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.query_row(
            "SELECT path FROM projects WHERE id = ?1",
            params![project_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to get project path: {}", e))
    }

    /// Get tool paths scoped to a project (project_path + tool.project_rel_path)
    pub fn get_project_tool_paths(&self, project_path: &str) -> Result<Vec<(i64, String, String)>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT id, global_path, project_rel_path FROM tools")
            .map_err(|e| format!("Prepare error: {}", e))?;

        let project_base = scanner::expand_path(project_path)?;
        let paths = stmt
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                let _global: String = row.get(1)?;
                let rel: String = row.get(2)?;
                Ok((id, _global, rel))
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .map(|(id, _global, rel)| {
                // Construct full path: project_path / project_rel_path
                let full_path = if rel.is_empty() {
                    project_base.to_string_lossy().to_string()
                } else {
                    project_base.join(&rel).to_string_lossy().to_string()
                };
                (id, full_path, rel)
            })
            .collect();

        Ok(paths)
    }
}
