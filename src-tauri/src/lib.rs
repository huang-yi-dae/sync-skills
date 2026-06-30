// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: MIT

mod db;
mod discovery;
mod hash;
mod models;
mod scanner;
mod settings;
mod sync;

use db::Database;
use discovery::ToolTemplate;
use models::{
    Project, ScanDetail, ScanResult, SkillUpdate, SkillView, SyncLog, SyncResult, Tool,
};
use settings::Settings;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

type DbState = Arc<Database>;

// ==================== Shared Helpers ====================

/// Core scan logic: scan a list of (tool_id, global_path) pairs, upsert to DB.
/// Deduplicates skills by name: same skill in different tool dirs → one card, multiple installations.
fn scan_tool_paths(
    db: &Database,
    tool_paths: &[(i64, String, String)],
    project_id: i64,
) -> Result<ScanResult, String> {
    let mut all_skills: Vec<(i64, models::DiscoveredSkill)> = Vec::new(); // (tool_id, skill)
    let mut all_errors = Vec::new();
    let mut skills_new = 0usize;
    let mut skills_updated = 0usize;
    let mut details: Vec<ScanDetail> = Vec::new();

    // Pre-build tool_id → tool_name map
    let all_tools = db.list_tools().unwrap_or_default();
    let tool_name_map: std::collections::HashMap<i64, String> =
        all_tools.into_iter().map(|t| (t.id, t.name)).collect();

    // Determine scope label
    let scope = if project_id == 0 {
        "Global".to_string()
    } else {
        db.get_project_name(project_id).unwrap_or_else(|_| format!("Project#{}", project_id))
    };

    for (tool_id, global_path, _project_rel_path) in tool_paths {
        let expanded = match scanner::expand_path(global_path) {
            Ok(p) => p,
            Err(e) => {
                all_errors.push(format!("Path '{}' expand failed: {}", global_path, e));
                continue;
            }
        };

        // Skip non-existent directories silently (not an error — tool may not be installed)
        if !expanded.exists() {
            continue;
        }

        match scanner::scan_directory(&expanded) {
            Ok(skills) => {
                for skill in skills {
                    all_skills.push((*tool_id, skill));
                }
            }
            Err(errs) => all_errors.extend(errs),
        }
    }

    // Upsert skills and auto-create installation records.
    // upsert_skill handles dedup: source_path first (location identity),
    // then core_hash (content identity across directories).
    for (tool_id, skill) in &all_skills {
        let tname = tool_name_map.get(tool_id).cloned().unwrap_or_else(|| format!("Tool#{}", tool_id));

        let skill_id = match db.upsert_skill(
            &skill.name,
            skill.description.as_deref(),
            &skill.source_path,
            &skill.content_hash,
            &skill.core_hash,
        ) {
            Ok((id, is_new)) => {
                let status = if is_new {
                    skills_new += 1;
                    "new"
                } else {
                    skills_updated += 1;
                    "updated"
                };
                details.push(ScanDetail {
                    skill_name: skill.name.clone(),
                    tool_name: tname.clone(),
                    scope: scope.clone(),
                    status: status.to_string(),
                    source_path: skill.source_path.clone(),
                });
                id
            }
            Err(e) => {
                all_errors.push(format!("DB error for {}: {}", skill.name, e));
                continue;
            }
        };

        // Auto-detect: skill physically exists in this tool's directory,
        // so mark as active installation (only if no record exists yet).
        let _ = db.ensure_installation(skill_id, *tool_id, project_id);
    }

    // Log scan operation
    let status = if all_errors.is_empty() { "success" } else { "failed" };
    let log_detail = if all_errors.is_empty() {
        format!("found={}, new={}, updated={}", all_skills.len(), skills_new, skills_updated)
    } else {
        all_errors.join("; ")
    };
    let _ = db.insert_action_log(
        "scan",
        None,
        None,
        project_id,
        status,
        Some(&log_detail),
    );

    Ok(ScanResult {
        skills_found: all_skills.len(),
        skills_new,
        skills_updated,
        errors: all_errors,
        details,
    })
}

/// Core sync logic: sync a skill to all its active installation targets.
fn do_sync_skill(
    db: &Database,
    skill_id: i64,
    project_id: i64,
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

    // Step 1: Copy to SSOT (replace if target already exists)
    let to_ssot_result = if prefer_symlink {
        sync::symlink_or_copy(&source, &ssot_target)
    } else if ssot_target.exists() {
        sync::replace_directory(&source, &ssot_target).map(|_| "replace".to_string())
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

    // Step 2: Copy from SSOT to all active installation targets (project-aware paths)
    let installations = db.get_active_installation_paths(skill_id, project_id)?;
    let mut synced_to = 0usize;
    let mut errors = Vec::new();

    for (tool_id, target_path) in &installations {
        let expanded = match scanner::expand_path(target_path) {
            Ok(p) => p,
            Err(e) => {
                errors.push(format!("Path expansion failed for tool {}: {}", tool_id, e));
                db.insert_sync_log(
                    skill_id,
                    *tool_id,
                    project_id,
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
            sync::replace_directory(&ssot_target, &target_dir).map(|_| "replace".to_string())
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
                db.update_synced_at(skill_id, *tool_id, project_id)?;
                db.insert_sync_log(skill_id, *tool_id, project_id, "from_ssot", "success", None)?;
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
                    project_id,
                    "from_ssot",
                    "failed",
                    Some(&e),
                )?;
            }
        }
    }

    // After successful sync, refresh stored hashes from source
    // so check_updates doesn't flag it as "needs update" anymore
    if synced_to > 0 || ssot_target.exists() {
        if let Ok(new_content_hash) = hash::compute_content_hash(&source) {
            let skill_md = source.join("SKILL.md");
            let new_core_hash = hash::compute_core_hash(&skill_md).unwrap_or_default();
            let _ = db.update_skill_hashes(skill_id, &new_content_hash, &new_core_hash);
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
    let tool = db.add_tool(&name, &global_path, &project_rel_path)?;
    let _ = db.insert_action_log("add_tool", None, Some(tool.id), 0, "success", Some(&name));
    Ok(tool)
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
fn delete_tool(db: State<DbState>, tool_id: i64, tool_name: Option<String>) -> Result<(), String> {
    let name = tool_name.unwrap_or_else(|| {
        db.get_tool_name(tool_id).unwrap_or_else(|_| format!("id={}", tool_id))
    });
    db.delete_tool(tool_id)?;
    let _ = db.insert_action_log("delete_tool", None, None, 0, "success", Some(&name));
    Ok(())
}

#[tauri::command]
fn list_tool_templates() -> Vec<ToolTemplate> {
    discovery::get_all_templates()
}

#[tauri::command]
fn discover_tools(db: State<DbState>) -> Result<Vec<ToolTemplate>, String> {
    discovery::discover_tools(&db)
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
        scan_tool_paths(&db, &tool_paths, 0)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn scan_scope(
    db: State<'_, DbState>,
    tool_id: Option<i64>,
    project_id: Option<i64>,
) -> Result<ScanResult, String> {
    let db = db.inner().clone();

    tokio::task::spawn_blocking(move || -> Result<ScanResult, String> {
        let pid = project_id.unwrap_or(0);

        // Determine scan paths based on scope
        let all_tool_paths = if pid != 0 {
            // Project-level scan: use project path + tool's project_rel_path
            let project_path = db.get_project_path(pid)?;
            db.get_project_tool_paths(&project_path)?
        } else {
            // Global scan: use global paths
            db.get_tool_paths()?
        };

        let tool_paths = match tool_id {
            Some(tid) => all_tool_paths
                .into_iter()
                .filter(|(id, _, _)| *id == tid)
                .collect::<Vec<_>>(),
            None => all_tool_paths,
        };

        scan_tool_paths(&db, &tool_paths, pid)
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
    db.toggle_installation(skill_id, tool_id, project_id, active)?;

    // Log the toggle action
    let action = if active { "toggle_on" } else { "toggle_off" };
    let _ = db.insert_action_log(action, Some(skill_id), Some(tool_id), project_id, "success", None);

    Ok(())
}

#[tauri::command]
async fn sync_skill(
    db: State<'_, DbState>,
    skill_id: i64,
    project_id: Option<i64>,
) -> Result<SyncResult, String> {
    let db = db.inner().clone();
    let settings = Settings::load();
    let pid = project_id.unwrap_or(0);

    tokio::task::spawn_blocking(move || -> Result<SyncResult, String> {
        do_sync_skill(&db, skill_id, pid, settings.prefer_symlink)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
async fn sync_all_pending(db: State<'_, DbState>) -> Result<Vec<SyncResult>, String> {
    let db = db.inner().clone();
    let settings = Settings::load();

    tokio::task::spawn_blocking(move || -> Result<Vec<SyncResult>, String> {
        let skills = db.list_skills()?;
        let projects = db.list_projects()?;
        let mut results = Vec::new();

        // For each project (including global id=0), check active installations
        for project in &projects {
            for skill in &skills {
                let installations = db.get_active_installations(skill.id, project.id)?;
                if !installations.is_empty() {
                    match do_sync_skill(&db, skill.id, project.id, settings.prefer_symlink) {
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
    let project = db.add_project(&name, &path)?;
    let _ = db.insert_action_log("add_project", None, None, project.id, "success", Some(&format!("{} ({})", name, path)));
    Ok(project)
}

#[tauri::command]
fn delete_project(db: State<DbState>, project_id: i64) -> Result<(), String> {
    // Log before deletion so we can capture the project name
    let project_name = db.get_project_name(project_id).unwrap_or_else(|_| format!("id={}", project_id));
    db.delete_project(project_id)?;
    let _ = db.insert_action_log("delete_project", None, None, 0, "success", Some(&project_name));
    Ok(())
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
            list_tool_templates,
            discover_tools,
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
