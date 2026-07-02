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
                core_hash       TEXT    NOT NULL DEFAULT '',
                created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_skills_core_hash ON skills(core_hash) WHERE core_hash != '';

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
            ",
        )
        .map_err(|e| format!("Failed to create schema: {}", e))?;

        // Migrate sync_logs: check if old schema exists (has 'direction' CHECK but no 'action' column)
        let needs_migration: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('sync_logs') WHERE name = 'action'")
            .and_then(|mut stmt| {
                stmt.query_row([], |row| row.get::<_, i64>(0))
            })
            .map(|count| count == 0) // action column missing → needs migration
            .unwrap_or(true); // table doesn't exist → will be created fresh

        if needs_migration {
            // Check if old table exists
            let old_exists: bool = conn
                .prepare("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sync_logs'")
                .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
                .map(|c| c > 0)
                .unwrap_or(false);

            if old_exists {
                // Migrate: rename old table, create new, copy data, drop old
                conn.execute_batch(
                    "
                    ALTER TABLE sync_logs RENAME TO sync_logs_old;
                    ",
                )
                .map_err(|e| format!("Migration rename failed: {}", e))?;
            }
            // else: no old table, just create fresh below

            // Create new sync_logs with expanded schema
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS sync_logs (
                    id              INTEGER PRIMARY KEY AUTOINCREMENT,
                    skill_id        INTEGER REFERENCES skills(id) ON DELETE SET NULL,
                    tool_id         INTEGER REFERENCES tools(id) ON DELETE SET NULL,
                    project_id      INTEGER NOT NULL DEFAULT 0 REFERENCES projects(id) ON DELETE CASCADE,
                    action          TEXT    NOT NULL DEFAULT 'sync',
                    direction       TEXT,
                    status          TEXT    NOT NULL CHECK (status IN ('success', 'failed')),
                    detail          TEXT,
                    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
                );

                CREATE INDEX IF NOT EXISTS idx_log_skill ON sync_logs(skill_id);
                CREATE INDEX IF NOT EXISTS idx_log_project ON sync_logs(project_id);
                CREATE INDEX IF NOT EXISTS idx_log_action ON sync_logs(action);
                CREATE INDEX IF NOT EXISTS idx_log_status ON sync_logs(status);
                CREATE INDEX IF NOT EXISTS idx_log_created ON sync_logs(created_at);
                ",
            )
            .map_err(|e| format!("Migration create failed: {}", e))?;

            if old_exists {
                // Copy data from old table
                conn.execute_batch(
                    "
                    INSERT INTO sync_logs (skill_id, tool_id, project_id, action, direction, status, detail, created_at)
                    SELECT skill_id, tool_id, project_id, 'sync', direction, status, error_message, created_at
                    FROM sync_logs_old;

                    DROP TABLE sync_logs_old;
                    ",
                )
                .map_err(|e| format!("Migration copy failed: {}", e))?;
            }
        }

        // Migrate: normalize source_path in skills table (strip \\?\ prefix on Windows)
        #[cfg(target_os = "windows")]
        {
            conn.execute_batch(
                "UPDATE skills SET source_path = SUBSTR(source_path, 5)
                 WHERE source_path LIKE '\\\\?\\%';",
            )
            .map_err(|e| format!("Source path migration failed: {}", e))?;
        }

        // Migrate: detect old (pre-JS-safe) tool IDs and clear stale data.
        // Old IDs were full i64 values that exceed JS Number.MAX_SAFE_INTEGER.
        // If any old seed tool is found, wipe all data so seed_data() can
        // re-populate with new JS-safe IDs.
        {
            let old_seed_ids: [i64; 5] = [
                -768412307910267356,  // old Claude Code
                -5387663353590988835, // old Codex CLI
                8996106060148633658,  // old OpenCode
                -1843142830140024973, // old Gemini CLI
                6180056807951602058,  // old Cline
            ];
            let placeholders = old_seed_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query = format!("SELECT COUNT(*) FROM tools WHERE id IN ({})", placeholders);
            let count: i64 = conn
                .query_row(&query, rusqlite::params_from_iter(old_seed_ids.iter()), |row| row.get(0))
                .unwrap_or(0);
            if count > 0 {
                // Old IDs detected — clear all data (FK cascades handle dependencies)
                conn.execute_batch(
                    "DELETE FROM skill_installations;
                     DELETE FROM sync_logs;
                     DELETE FROM skills;
                     DELETE FROM tools;
                     DELETE FROM projects;",
                )
                .map_err(|e| format!("Stale data migration failed: {}", e))?;
            }
        }

        // Migrate: add core_hash column to skills table if missing
        {
            let has_core_hash: bool = conn
                .prepare("SELECT COUNT(*) FROM pragma_table_info('skills') WHERE name = 'core_hash'")
                .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, i64>(0)))
                .map(|count| count > 0)
                .unwrap_or(false);

            if !has_core_hash {
                conn.execute_batch(
                    "ALTER TABLE skills ADD COLUMN core_hash TEXT NOT NULL DEFAULT '';"
                ).map_err(|e| format!("Failed to add core_hash column: {}", e))?;

                // Backfill: compute core_hash for existing skills from their SKILL.md
                let skills_to_backfill: Vec<(i64, String)> = {
                    let mut stmt = conn
                        .prepare("SELECT id, source_path FROM skills WHERE core_hash = ''")
                        .map_err(|e| format!("Backfill query failed: {}", e))?;
                    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                        .map_err(|e| format!("Backfill query_map failed: {}", e))?;
                    let result: Vec<(i64, String)> = rows.filter_map(|r| r.ok()).collect();
                    result
                };

                for (id, source_path) in skills_to_backfill {
                    let skill_md = std::path::Path::new(&source_path).join("SKILL.md");
                    if let Ok(hash) = crate::hash::compute_core_hash(&skill_md) {
                        let _ = conn.execute(
                            "UPDATE skills SET core_hash = ?1 WHERE id = ?2",
                            params![hash, id],
                        );
                    }
                }

                // Create unique index on core_hash (excluding empty strings)
                let _ = conn.execute_batch(
                    "CREATE UNIQUE INDEX IF NOT EXISTS idx_skills_core_hash ON skills(core_hash) WHERE core_hash != '';"
                );
            }
        }

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

        // Preset tools with pre-computed hash IDs (JS-safe, < 2^53)
        let tools = [
            (6206827997457956i64, "Claude Code", "~/.claude/skills/", ".claude/skills/"),
            (7648999998865373i64, "Codex CLI", "~/.codex/skills/", ".codex/skills/"),
            (6921203917123642i64, "OpenCode", "~/.opencode/skills/", ".opencode/skills/"),
            (3333017081878387i64, "Gemini CLI", "~/.gemini/skills/", ".gemini/skills/"),
            (1118119199281546i64, "Cline", "~/.cline/skills/", ".cline/skills/"),
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

    /// Upsert a skill with two-level dedup:
    /// 1. source_path (primary): same location → same skill, even if content changed
    /// 2. core_hash (secondary): same content in different location → same skill
    /// Returns (skill_id, is_new).
    pub fn upsert_skill(
        &self,
        name: &str,
        description: Option<&str>,
        source_path: &str,
        content_hash: &str,
        core_hash: &str,
    ) -> Result<(i64, bool), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        // 1. Check by source_path first (same location = same skill, even if edited)
        let existing_by_path: Option<i64> = conn
            .query_row(
                "SELECT id FROM skills WHERE source_path = ?1",
                params![source_path],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing_by_path {
            // Check if another skill already owns this core_hash
            let conflict_id: Option<i64> = conn
                .query_row(
                    "SELECT id FROM skills WHERE core_hash = ?1 AND core_hash != '' AND id != ?2",
                    params![core_hash, id],
                    |row| row.get(0),
                )
                .ok();

            if let Some(other_id) = conflict_id {
                // Another skill has the same SKILL.md content.
                // Merge: delete the duplicate, keep this one (by source_path).
                conn.execute("DELETE FROM skills WHERE id = ?1", params![other_id])
                    .map_err(|e| format!("Failed to delete duplicate skill: {}", e))?;
            }

            conn.execute(
                "UPDATE skills SET name = ?1, description = ?2, content_hash = ?3,
                 core_hash = ?4, updated_at = datetime('now')
                 WHERE id = ?5",
                params![name, description, content_hash, core_hash, id],
            )
            .map_err(|e| format!("Failed to update skill: {}", e))?;
            return Ok((id, false));
        }

        // 2. Check by core_hash (same content in a different directory = same skill)
        let existing_by_core: Option<i64> = conn
            .query_row(
                "SELECT id FROM skills WHERE core_hash = ?1 AND core_hash != ''",
                params![core_hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing_by_core {
            // Same skill content found at a different location — reuse existing record
            // Don't overwrite source_path (keep the original location as canonical)
            conn.execute(
                "UPDATE skills SET name = ?1, description = ?2, content_hash = ?3,
                 updated_at = datetime('now')
                 WHERE id = ?4",
                params![name, description, content_hash, id],
            )
            .map_err(|e| format!("Failed to update skill: {}", e))?;
            return Ok((id, false));
        }

        // 3. Brand new skill — insert
        let id = compute_id_hash(source_path);
        conn.execute(
            "INSERT INTO skills (id, name, description, source_path, content_hash, core_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, description, source_path, content_hash, core_hash],
        )
        .map_err(|e| format!("Failed to insert skill: {}", e))?;
        Ok((id, true))
    }

    /// List all skills
    pub fn list_skills(&self) -> Result<Vec<Skill>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let mut stmt = conn
            .prepare("SELECT id, name, description, source_path, content_hash, core_hash, created_at, updated_at FROM skills ORDER BY name")
            .map_err(|e| format!("Prepare error: {}", e))?;

        let skills = stmt
            .query_map([], |row| {
                Ok(Skill {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_path: row.get(3)?,
                    content_hash: row.get(4)?,
                    core_hash: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
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
    #[allow(dead_code)] // Reserved for future use
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
            "SELECT id, name, description, source_path, content_hash, core_hash, created_at, updated_at FROM skills WHERE id = ?1",
            params![skill_id],
            |row| {
                Ok(Skill {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    source_path: row.get(3)?,
                    content_hash: row.get(4)?,
                    core_hash: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            },
        )
        .map_err(|e| format!("Failed to get skill: {}", e))
    }

    /// Look up a skill by its core_hash (SKILL.md content hash).
    /// Returns the skill ID if found, None otherwise.
    pub fn get_skill_id_by_core_hash(&self, core_hash: &str) -> Result<Option<i64>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        let result = conn.query_row(
            "SELECT id FROM skills WHERE core_hash = ?1 AND core_hash != ''",
            params![core_hash],
            |row| row.get(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(format!("Query error: {}", e)),
        }
    }

    /// List skills with installation status (SkillView)
    /// Filters skills by source_path prefix:
    /// - project_id=0 (Global): skills under any tool's global_path
    /// - project_id>0 (Project): skills under the project's directory
    pub fn list_skills_with_status(&self, project_id: i64) -> Result<Vec<SkillView>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        // Step 1: Determine path prefix(es) for filtering (normalized)
        let path_prefixes: Vec<String> = if project_id == 0 {
            // Global: skills whose source_path is under any tool's global_path
            let mut stmt = conn
                .prepare("SELECT global_path FROM tools")
                .map_err(|e| format!("Prepare error: {}", e))?;
            let paths: Vec<String> = stmt
                .query_map([], |row| row.get(0))
                .map_err(|e| format!("Query error: {}", e))?
                .filter_map(|r| r.ok())
                .filter_map(|p: String| scanner::expand_path(&p).ok())
                .map(|p| scanner::normalize_path(&p))
                .collect();
            paths
        } else {
            // Project: skills whose source_path is under the project's path
            let project_path: String = conn
                .query_row("SELECT path FROM projects WHERE id = ?1", params![project_id], |row| row.get(0))
                .map_err(|e| format!("Failed to get project path: {}", e))?;
            let expanded = scanner::expand_path(&project_path)
                .map_err(|e| format!("Failed to expand project path: {}", e))?;
            vec![scanner::normalize_path(&expanded)]
        };

        // Step 2: Get skills filtered by path prefix
        let skills: Vec<Skill> = if path_prefixes.is_empty() {
            // No tools configured or no project path — return empty
            Vec::new()
        } else {
            let mut all_skills: Vec<Skill> = Vec::new();
            for prefix in &path_prefixes {
                let pattern = format!("{}%", prefix);
                let mut stmt = conn
                    .prepare(
                        "SELECT id, name, description, source_path, content_hash, core_hash, created_at, updated_at
                         FROM skills WHERE source_path LIKE ?1 ORDER BY name",
                    )
                    .map_err(|e| format!("Prepare error: {}", e))?;

                let matched: Vec<Skill> = stmt
                    .query_map(params![pattern], |row| {
                        Ok(Skill {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            description: row.get(2)?,
                            source_path: row.get(3)?,
                            content_hash: row.get(4)?,
                            core_hash: row.get(5)?,
                            created_at: row.get(6)?,
                            updated_at: row.get(7)?,
                        })
                    })
                    .map_err(|e| format!("Query error: {}", e))?
                    .filter_map(|r| r.ok())
                    .collect();

                all_skills.extend(matched);
            }
            // Deduplicate by skill id (in case multiple prefixes match the same skill)
            all_skills.sort_by_key(|s| s.id);
            all_skills.dedup_by_key(|s| s.id);
            all_skills
        };

        // Step 3: For each skill, get installation info for the given project_id
        let mut views = Vec::new();
        for skill in skills {
            let mut inst_stmt = conn
                .prepare(
                    "SELECT si.tool_id, t.name, si.status, si.synced_at
                     FROM skill_installations si
                     JOIN tools t ON t.id = si.tool_id
                     WHERE si.skill_id = ?1 AND si.project_id = ?2",
                )
                .map_err(|e| format!("Prepare error: {}", e))?;

            let installations: Vec<InstallationInfo> = inst_stmt
                .query_map(params![skill.id, project_id], |row| {
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

            let install_count = installations.iter().filter(|i| i.status == "active").count();

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

    /// Auto-detect installation: insert an active record only if no record exists yet.
    /// Used during scan to mark skills that physically exist in a tool's directory.
    /// Does NOT override existing records (preserves user's explicit toggle-off).
    pub fn ensure_installation(
        &self,
        skill_id: i64,
        tool_id: i64,
        project_id: i64,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        conn.execute(
            "INSERT OR IGNORE INTO skill_installations (skill_id, tool_id, project_id, status)
             VALUES (?1, ?2, ?3, 'active')",
            params![skill_id, tool_id, project_id],
        )
        .map_err(|e| format!("Failed to ensure installation: {}", e))?;
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

    /// Get active installation target paths, resolved per project scope.
    /// - project_id == 0 (Global): returns tool's global_path (e.g. ~/.claude/skills/)
    /// - project_id > 0 (Project): returns project_path + tool's project_rel_path
    pub fn get_active_installation_paths(
        &self,
        skill_id: i64,
        project_id: i64,
    ) -> Result<Vec<(i64, String)>, String> {
        if project_id == 0 {
            // Global: use global_path directly
            return self.get_active_installations(skill_id, 0);
        }

        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        // Get project base path
        let project_path: String = conn
            .query_row("SELECT path FROM projects WHERE id = ?1", params![project_id], |row| row.get(0))
            .map_err(|e| format!("Failed to get project path: {}", e))?;
        let project_base = scanner::expand_path(&project_path)?;

        let mut stmt = conn
            .prepare(
                "SELECT si.tool_id, t.project_rel_path
                 FROM skill_installations si
                 JOIN tools t ON t.id = si.tool_id
                 WHERE si.skill_id = ?1 AND si.project_id = ?2 AND si.status = 'active'",
            )
            .map_err(|e| format!("Prepare error: {}", e))?;

        let results = stmt
            .query_map(params![skill_id, project_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("Query error: {}", e))?
            .filter_map(|r| r.ok())
            .map(|(tool_id, rel_path)| {
                let full_path = if rel_path.is_empty() {
                    scanner::normalize_path(&project_base)
                } else {
                    scanner::normalize_path(&project_base.join(&rel_path))
                };
                (tool_id, full_path)
            })
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

    /// Refresh stored hashes for a skill after sync (clears update indicator)
    pub fn update_skill_hashes(
        &self,
        skill_id: i64,
        content_hash: &str,
        core_hash: &str,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        conn.execute(
            "UPDATE skills SET content_hash = ?1, core_hash = ?2, updated_at = datetime('now')
             WHERE id = ?3",
            params![content_hash, core_hash, skill_id],
        )
        .map_err(|e| format!("Failed to update skill hashes: {}", e))?;
        Ok(())
    }

    /// Update only content_hash (leave core_hash untouched).
    pub fn update_content_hash(&self, skill_id: i64, content_hash: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        conn.execute(
            "UPDATE skills SET content_hash = ?1, updated_at = datetime('now')
             WHERE id = ?2",
            params![content_hash, skill_id],
        )
        .map_err(|e| format!("Failed to update content hash: {}", e))?;
        Ok(())
    }

    /// Insert a sync log entry (for sync operations with direction)
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
            "INSERT INTO sync_logs (skill_id, tool_id, project_id, action, direction, status, detail)
             VALUES (?1, ?2, ?3, 'sync', ?4, ?5, ?6)",
            params![skill_id, tool_id, project_id, direction, status, error_message],
        )
        .map_err(|e| format!("Failed to insert sync log: {}", e))?;

        Ok(())
    }

    /// Insert an action log entry (for non-sync operations: scan, toggle, add, delete, etc.)
    pub fn insert_action_log(
        &self,
        action: &str,
        skill_id: Option<i64>,
        tool_id: Option<i64>,
        project_id: i64,
        status: &str,
        detail: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        conn.execute(
            "INSERT INTO sync_logs (skill_id, tool_id, project_id, action, status, detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![skill_id, tool_id, project_id, action, status, detail],
        )
        .map_err(|e| format!("Failed to insert action log: {}", e))?;

        Ok(())
    }

    /// Get a tool's name by ID (for logging before deletion)
    pub fn get_tool_name(&self, tool_id: i64) -> Result<String, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        conn.query_row(
            "SELECT name FROM tools WHERE id = ?1",
            params![tool_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to get tool name: {}", e))
    }

    /// Get a project's name by ID (for logging before deletion)
    pub fn get_project_name(&self, project_id: i64) -> Result<String, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;
        conn.query_row(
            "SELECT name FROM projects WHERE id = ?1",
            params![project_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to get project name: {}", e))
    }

    /// Query sync logs with optional filters
    pub fn get_sync_logs(&self, skill_id: Option<i64>, limit: Option<i64>) -> Result<Vec<SyncLog>, String> {
        let conn = self.conn.lock().map_err(|e| format!("Lock error: {}", e))?;

        let limit = limit.unwrap_or(100);

        let query = match skill_id {
            Some(sid) => format!(
                "SELECT sl.id, sl.skill_id, s.name, sl.tool_id, t.name, sl.project_id,
                        sl.action, sl.direction, sl.status, sl.detail, sl.created_at
                 FROM sync_logs sl
                 LEFT JOIN skills s ON s.id = sl.skill_id
                 LEFT JOIN tools t ON t.id = sl.tool_id
                 WHERE sl.skill_id = {}
                 ORDER BY sl.created_at DESC LIMIT {}",
                sid, limit
            ),
            None => format!(
                "SELECT sl.id, sl.skill_id, s.name, sl.tool_id, t.name, sl.project_id,
                        sl.action, sl.direction, sl.status, sl.detail, sl.created_at
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
                    action: row.get(6)?,
                    direction: row.get(7)?,
                    status: row.get(8)?,
                    detail: row.get(9)?,
                    created_at: row.get(10)?,
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
                    scanner::normalize_path(&project_base)
                } else {
                    scanner::normalize_path(&project_base.join(&rel))
                };
                (id, full_path, rel)
            })
            .collect();

        Ok(paths)
    }
}
