// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: MIT

use crate::hash::compute_id_hash;
use crate::models::{Skill, Tool};
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
}
