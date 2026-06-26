// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: MIT

mod db;
mod hash;
mod models;
mod scanner;

use db::Database;
use models::{ScanResult, Skill, Tool};
use std::sync::Arc;
use tauri::State;

type DbState = Arc<Database>;

// ==================== Tauri Commands ====================

#[tauri::command]
fn list_tools(db: State<DbState>) -> Result<Vec<Tool>, String> {
    db.list_tools()
}

#[tauri::command]
fn add_tool(
    db: State<DbState>,
    name: String,
    global_path: String,
    project_rel_path: String,
) -> Result<Tool, String> {
    db.add_tool(&name, &global_path, &project_rel_path)
}

#[tauri::command]
fn update_tool_path(
    db: State<DbState>,
    tool_id: i64,
    global_path: String,
    project_rel_path: String,
) -> Result<(), String> {
    db.update_tool_path(tool_id, &global_path, &project_rel_path)
}

#[tauri::command]
fn delete_tool(db: State<DbState>, tool_id: i64) -> Result<(), String> {
    db.delete_tool(tool_id)
}

#[tauri::command]
fn list_skills(db: State<DbState>) -> Result<Vec<Skill>, String> {
    db.list_skills()
}

#[tauri::command]
async fn full_scan(db: State<'_, DbState>) -> Result<ScanResult, String> {
    let db = db.inner().clone();

    // Run scan in blocking task (file I/O is synchronous)
    tokio::task::spawn_blocking(move || -> Result<ScanResult, String> {
        let tool_paths = db.get_tool_paths()?;
        let mut all_skills = Vec::new();
        let mut all_errors = Vec::new();
        let mut skills_new = 0usize;
        let mut skills_updated = 0usize;

        for (_tool_id, global_path, _project_rel_path) in &tool_paths {
            let expanded = scanner::expand_path(global_path)?;

            match scanner::scan_directory(&expanded) {
                Ok(skills) => all_skills.extend(skills),
                Err(errs) => all_errors.extend(errs),
            }
        }

        // Upsert all discovered skills to DB
        for skill in &all_skills {
            match db.upsert_skill(
                &skill.name,
                skill.description.as_deref(),
                &skill.source_path,
                &skill.content_hash,
            ) {
                Ok((_, is_new)) => {
                    if is_new {
                        skills_new += 1;
                    } else {
                        skills_updated += 1;
                    }
                }
                Err(e) => {
                    all_errors.push(format!("DB error for {}: {}", skill.name, e));
                }
            }
        }

        Ok(ScanResult {
            skills_found: all_skills.len(),
            skills_new,
            skills_updated,
            errors: all_errors,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn scan_scope(
    db: State<'_, DbState>,
    tool_id: Option<i64>,
) -> Result<ScanResult, String> {
    let db = db.inner().clone();
    let _tool_id = tool_id; // For now, scan all (M1 scope)

    // Same as full_scan for M1
    tokio::task::spawn_blocking(move || -> Result<ScanResult, String> {
        let tool_paths = db.get_tool_paths()?;
        let mut all_skills = Vec::new();
        let mut all_errors = Vec::new();
        let mut skills_new = 0usize;
        let mut skills_updated = 0usize;

        for (_tool_id, global_path, _project_rel_path) in &tool_paths {
            let expanded = scanner::expand_path(global_path)?;

            match scanner::scan_directory(&expanded) {
                Ok(skills) => all_skills.extend(skills),
                Err(errs) => all_errors.extend(errs),
            }
        }

        for skill in &all_skills {
            match db.upsert_skill(
                &skill.name,
                skill.description.as_deref(),
                &skill.source_path,
                &skill.content_hash,
            ) {
                Ok((_, is_new)) => {
                    if is_new {
                        skills_new += 1;
                    } else {
                        skills_updated += 1;
                    }
                }
                Err(e) => {
                    all_errors.push(format!("DB error for {}: {}", skill.name, e));
                }
            }
        }

        Ok(ScanResult {
            skills_found: all_skills.len(),
            skills_new,
            skills_updated,
            errors: all_errors,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// ==================== Application Entry ====================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logger
    env_logger::init();

    // Initialize database
    let db = Database::new().expect("Failed to initialize database");
    let db_state = Arc::new(db);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(db_state)
        .invoke_handler(tauri::generate_handler![
            list_tools,
            add_tool,
            update_tool_path,
            delete_tool,
            list_skills,
            full_scan,
            scan_scope,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
