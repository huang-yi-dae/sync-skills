// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: MIT

export interface Tool {
  id: number;
  name: string;
  global_path: string;
  project_rel_path: string;
  created_at: string;
  updated_at: string;
}

export interface Project {
  id: number;
  name: string;
  path: string;
  created_at: string;
}

export interface InstallationInfo {
  tool_id: number;
  tool_name: string;
  status: string;
  synced_at: string | null;
}

export interface SkillView {
  id: number;
  name: string;
  description: string | null;
  source_path: string;
  content_hash: string;
  created_at: string;
  updated_at: string;
  installed_tools: InstallationInfo[];
  install_count: number;
  has_update: boolean;
}

export interface ScanResult {
  skills_found: number;
  skills_new: number;
  skills_updated: number;
  errors: string[];
}

export interface SyncResult {
  skill_id: number;
  skill_name: string;
  synced_to: number;
  errors: string[];
}

export interface SkillUpdate {
  skill_id: number;
  skill_name: string;
  source_path: string;
  old_hash: string;
  new_hash: string;
}

export interface SyncLog {
  id: number;
  skill_id: number;
  skill_name: string | null;
  tool_id: number;
  tool_name: string | null;
  project_id: number;
  direction: string;
  status: string;
  error_message: string | null;
  created_at: string;
}

export interface Settings {
  sync_mode: string;
  prefer_symlink: boolean;
}

export interface Toast {
  type: "success" | "error" | "info";
  message: string;
  id: number;
}
