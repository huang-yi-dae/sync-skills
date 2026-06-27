// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: MIT

mod db;
mod hash;
mod models;
mod scanner;
mod settings;
mod sync;

use db::Database;
use models::{
    Project, ScanResult, SkillUpdate, SkillView, SyncLog, SyncResult, Tool,
};
use settings::Settings;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

type DbState = Arc<Database>;

// ==================== Shared Helpers ====================

/// Core scan logic: scan a list of (tool_id, global_path) pairs, upsert to DB.
fn scan_tool_paths(
    db: &Database,
    tool_paths: &[(i64, String, String)],
) -> Result<ScanResult, String> {
    let mut all_skills = Vec::new();
    let mut all_errors = Vec::new();
    let mut skills_new = 0usize;
    let mut skills_updated = 0usize;

    for (_tool_id, global_path, _project_rel_path) in tool_paths {
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
}

/// Core sync logic: sync a skill to all its active installation targets.
fn do_sync_skill(
    db: &Database,
    skill_id: i64,
    prefer_symlink: bool,
) -> Result<SyncResult, String> {
    let skill = db.get_skill_by_id(skill_id)?;
    let source = PathBuf::from(&skill.source_path);

    if !source.exists() {
        return Err(format!("Source path does not exist: {}", skill.source_path));
    }

    // Ensure SSOT directory exists
    sync::ensure_ssot_dir()?;
    let ssot_target = sync::ssot_path(&skill.name)?;

    // Step 1: Copy to SSOT (use atomic_replace if target already exists)
    let to_ssot_result = if prefer_symlink {
        sync::symlink_or_copy(&source, &ssot_target)
    } else if ssot_target.exists() {
        sync::atomic_replace(&source, &ssot_target).map(|_| "atomic_replace".to_string())
    } else {
        sync::copy_directory(&source, &ssot_target).map(|_| "copy".to_string())
    };

    match &to_ssot_result {
        Ok(method) => {
            log::info!("Synced {} to SSOT via {}", skill.name, method);
            // Create local.md marker in SSOT
            let _ = sync::create_local_marker(&ssot_target);
        }
        Err(e) => {
            log::error!("Failed to sync {} to SSOT: {}", skill.name, e);
            return Ok(SyncResult {
                skill_id,
                skill_name: skill.name.clone(),
                synced_to: 0,
                errors: vec![e.clone()],
            });
        }
    }

    // Step 2: Copy from SSOT to all active installation targets
    let installations = db.get_active_installations(skill_id, 0)?;
    let mut synced_to = 0usize;
    let mut errors = Vec::new();

    for (tool_id, tool_global_path) in &installations {
        let expanded = match scanner::expand_path(tool_global_path) {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("Path expansion failed for tool {}: {}", tool_id, e));
                db.insert_sync_log(
                    skill_id,
                    *tool_id,
                    0,
                    "from_ssot",
                    "failed",
                    Some(&e),
                )?;
                continue;
            }
        };

        let target_dir = expanded.join(&skill.name);

        let result = if prefer_symlink {
            sync::symlink_or_copy(&ssot_target, &target_dir)
        } else if target_dir.exists() {
            sync::atomic_replace(&ssot_target, &target_dir).map(|_| "atomic_replace".to_string())
        } else {
            sync::copy_directory(&ssot_target, &target_dir).map(|_| "copy".to_string())
        };

        match result {
            Ok(method) => {
                log::info!(
                    "Synced {} to tool {} via {}",
                    skill.name,
                    tool_id,
                    method
                );
                db.update_synced_at(skill_id, *tool_id, 0)?;
                db.insert_sync_log(skill_id, *tool_id, 0, "from_ssot", "success", None)?;
                synced_to += 1;
            }
            Err(e) => {
                log::error!(
                    "Failed to sync {} to tool {}: {}",
                    skill.name,
                    tool_id,
                    e
                );
                errors.push(format!("Tool {}: {}", tool_id, e));
                db.insert_sync_log(
                    skill_id,
                    *tool_id,
                    0,
                    "from_ssot",
                    "failed",
                    Some(&e),
                )?;
            }
        }
    }

    Ok(SyncResult {
        skill_id,
        skill_name: skill.name,
        synced_to,
        errors,
    })
}

// ==================== Tool Commands ====================

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

// ==================== Skill Commands ====================

#[tauri::command]
fn list_skills(db: State<DbState>, project_id: Option<i64>) -> Result<Vec<SkillView>, String> {
    let pid = project_id.unwrap_or(0);
    db.list_skills_with_status(pid)
}

// ==================== Scan Commands ====================

#[tauri::command]
async fn full_scan(db: State<'_, DbState>) -> Result<ScanResult, String> {
    let db = db.inner().clone();

    tokio::task::spawn_blocking(move || -> Result<ScanResult, String> {
        let tool_paths = db.get_tool_paths()?;
        scan_tool_paths(&db, &tool_paths)
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

    tokio::task::spawn_blocking(move || -> Result<ScanResult, String> {
        let all_tool_paths = db.get_tool_paths()?;

        let tool_paths = match tool_id {
            Some(tid) => all_tool_paths
                .into_iter()
                .filter(|(id, _, _)| *id == tid)
                .collect::<Vec<_>>(),
            None => all_tool_paths,
        };

        scan_tool_paths(&db, &tool_paths)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// ==================== Sync Commands (M2) ====================

#[tauri::command]
fn toggle_skill(
    db: State<DbState>,
    skill_id: i64,
    tool_id: i64,
    project_id: i64,
    active: bool,
) -> Result<(), String> {
    db.toggle_installation(skill_id, tool_id, project_id, active)
}

#[tauri::command]
async fn sync_skill(
    db: State<'_, DbState>,
    skill_id: i64,
) -> Result<SyncResult, String> {
    let db = db.inner().clone();
    let settings = Settings::load();

    tokio::task::spawn_blocking(move || -> Result<SyncResult, String> {
        do_sync_skill(&db, skill_id, settings.prefer_symlink)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn sync_all_pending(db: State<'_, DbState>) -> Result<Vec<SyncResult>, String> {
    let db = db.inner().clone();
    let settings = Settings::load();

    tokio::task::spawn_blocking(move || -> Result<Vec<SyncResult>, String> {
        // Get all skills with active installations
        let skills = db.list_skills()?;
        let mut results = Vec::new();

        for skill in &skills {
            let installations = db.get_active_installations(skill.id, 0)?;
            if !installations.is_empty() {
                match do_sync_skill(&db, skill.id, settings.prefer_symlink) {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        results.push(SyncResult {
                            skill_id: skill.id,
                            skill_name: skill.name.clone(),
                            synced_to: 0,
                            errors: vec![e],
                        });
                    }
                }
            }
        }

        Ok(results)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn check_updates(
    db: State<'_, DbState>,
    _project_id: Option<i64>,
) -> Result<Vec<SkillUpdate>, String> {
    let db = db.inner().clone();

    tokio::task::spawn_blocking(move || -> Result<Vec<SkillUpdate>, String> {
        let skills = db.get_all_skills_for_update_check()?;
        let mut updates = Vec::new();

        for (skill_id, skill_name, source_path, old_hash) in &skills {
            let path = PathBuf::from(source_path);
            if !path.exists() {
                continue;
            }

            match hash::compute_content_hash(&path) {
                Ok(new_hash) => {
                    if new_hash != *old_hash {
                        updates.push(SkillUpdate {
                            skill_id: *skill_id,
                            skill_name: skill_name.clone(),
                            source_path: source_path.clone(),
                            old_hash: old_hash.clone(),
                            new_hash,
                        });
                    }
                }
                Err(_) => {
                    // Hash computation failed, skip
                    continue;
                }
            }
        }

        Ok(updates)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

// ==================== Settings Commands (M2) ====================

#[tauri::command]
fn get_settings() -> Settings {
    Settings::load()
}

#[tauri::command]
fn update_settings(new_settings: Settings) -> Result<(), String> {
    new_settings.save()
}

// ==================== Project Commands (M3) ====================

#[tauri::command]
fn list_projects(db: State<DbState>) -> Result<Vec<Project>, String> {
    db.list_projects()
}

#[tauri::command]
fn add_project(db: State<DbState>, name: String, path: String) -> Result<Project, String> {
    db.add_project(&name, &path)
}

#[tauri::command]
fn delete_project(db: State<DbState>, project_id: i64) -> Result<(), String> {
    db.delete_project(project_id)
}

// ==================== Sync Log Commands (M3) ====================

#[tauri::command]
fn get_sync_logs(
    db: State<DbState>,
    skill_id: Option<i64>,
    limit: Option<i64>,
) -> Result<Vec<SyncLog>, String> {
    db.get_sync_logs(skill_id, limit)
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
            // Tools
            list_tools,
            add_tool,
            update_tool_path,
            delete_tool,
            // Skills
            list_skills,
            // Scan
            full_scan,
            scan_scope,
            // Sync (M2)
            toggle_skill,
            sync_skill,
            sync_all_pending,
            check_updates,
            // Settings (M2)
            get_settings,
            update_settings,
            // Projects (M3)
            list_projects,
            add_project,
            delete_project,
            // Sync Logs (M3)
            get_sync_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
