// Copyright (c) 2026 Skill Manager Contributors
// SPDX-License-Identifier: MIT

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import type {
  Tool, ToolTemplate, Project, SkillView, ScanResult, SyncResult,
  SkillUpdate, SkillDiff, SyncLog, Settings, Toast, InstallationInfo,
} from "./types";

type Tab = "global" | "projects";
type Panel = "main" | "settings" | "logs";

function App() {
  // Core data
  const [tools, setTools] = useState<Tool[]>([]);
  const [skills, setSkills] = useState<SkillView[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [settings, setSettings] = useState<Settings>({ sync_mode: "semi-auto", prefer_symlink: false });

  // UI state
  const [activeTab, setActiveTab] = useState<Tab>("global");
  const [activePanel, setActivePanel] = useState<Panel>("main");
  const [scanning, setScanning] = useState(false);
  const [syncing, setSyncing] = useState<Set<number>>(new Set());
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [updates, setUpdates] = useState<SkillUpdate[]>([]);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [sortBy, setSortBy] = useState<"name" | "updated_at" | "created_at">("name");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");
  const [selectedProject, setSelectedProject] = useState<number>(0);

  // Tool editing
  const [editingTool, setEditingTool] = useState<number | null>(null);
  const [editGlobalPath, setEditGlobalPath] = useState("");
  const [editRelPath, setEditRelPath] = useState("");
  const [showAddTool, setShowAddTool] = useState(false);
  const [newToolName, setNewToolName] = useState("");
  const [newToolGlobal, setNewToolGlobal] = useState("");
  const [newToolRel, setNewToolRel] = useState("");
  const [discoveredTools, setDiscoveredTools] = useState<ToolTemplate[]>([]);
  const [templates, setTemplates] = useState<ToolTemplate[]>([]);
  const [addingDiscovered, setAddingDiscovered] = useState(false);
  const [scanResult, setScanResult] = useState<ScanResult | null>(null);
  const [showUpdatesModal, setShowUpdatesModal] = useState(false);
  const [selectedUpdateDiff, setSelectedUpdateDiff] = useState<{ update: SkillUpdate; diff: SkillDiff } | null>(null);
  const [loadingDiff, setLoadingDiff] = useState<number | null>(null);

  // Project editing
  const [showAddProject, setShowAddProject] = useState(false);
  const [newProjectName, setNewProjectName] = useState("");
  const [newProjectPath, setNewProjectPath] = useState("");

  // Sync logs
  const [syncLogs, setSyncLogs] = useState<SyncLog[]>([]);

  // Load data on mount
  useEffect(() => {
    loadTools();
    loadSkills();
    loadProjects();
    loadSettings();
    loadDiscovery();
    loadTemplates();
  }, []);

  // Sync selectedProject when tab changes
  useEffect(() => {
    if (activeTab === "global") {
      setSelectedProject((prev) => (prev !== 0 ? 0 : prev));
    } else {
      // Projects tab: auto-select first project if none selected
      setSelectedProject((prev) => {
        if (prev === 0 && projects.length > 0) return projects[0].id;
        if (prev !== 0 && !projects.some((p) => p.id === prev)) {
          // Selected project was deleted, pick first available
          return projects.length > 0 ? projects[0].id : 0;
        }
        return prev;
      });
    }
  }, [activeTab, projects]);

  // Reload skills when project changes
  useEffect(() => {
    loadSkills();
  }, [selectedProject]);

  const addToast = useCallback((type: Toast["type"], message: string) => {
    const id = Date.now() + Math.random();
    setToasts((prev) => [...prev, { type, message, id }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  }, []);

  // ==================== Data Loaders ====================

  async function loadTools() {
    try {
      setTools(await invoke<Tool[]>("list_tools"));
    } catch (e) {
      addToast("error", `Failed to load tools: ${e}`);
    }
  }

  async function loadSkills() {
    try {
      setSkills(await invoke<SkillView[]>("list_skills", { projectId: selectedProject }));
    } catch (e) {
      addToast("error", `Failed to load skills: ${e}`);
    }
  }

  async function loadProjects() {
    try {
      const result = await invoke<Project[]>("list_projects");
      setProjects(result.filter((p) => p.id !== 0)); // Exclude Global from project list
    } catch (e) {
      addToast("error", `Failed to load projects: ${e}`);
    }
  }

  async function loadSettings() {
    try {
      setSettings(await invoke<Settings>("get_settings"));
    } catch (e) {
      addToast("error", `Failed to load settings: ${e}`);
    }
  }

  async function loadSyncLogs() {
    try {
      setSyncLogs(await invoke<SyncLog[]>("get_sync_logs", { skillId: null, limit: 50 }));
    } catch (e) {
      addToast("error", `Failed to load sync logs: ${e}`);
    }
  }

  async function loadDiscovery() {
    try {
      const found = await invoke<ToolTemplate[]>("discover_tools");
      setDiscoveredTools(found);
    } catch {
      // Silent fail — discovery is a nice-to-have
    }
  }

  async function loadTemplates() {
    try {
      setTemplates(await invoke<ToolTemplate[]>("list_tool_templates"));
    } catch {
      // Silent fail
    }
  }

  async function handleAddDiscoveredAll() {
    if (discoveredTools.length === 0) return;
    setAddingDiscovered(true);
    let added = 0;
    for (const t of discoveredTools) {
      try {
        await invoke("add_tool", {
          name: t.name,
          globalPath: t.global_path,
          projectRelPath: t.project_rel_path,
        });
        added++;
      } catch {
        // Skip duplicates silently
      }
    }
    setDiscoveredTools([]);
    setAddingDiscovered(false);
    await loadTools();
    if (added > 0) {
      addToast("success", `Added ${added} tool${added > 1 ? "s" : ""}`);
    }
  }

  function handleSelectTemplate(e: React.ChangeEvent<HTMLSelectElement>) {
    const idx = parseInt(e.target.value, 10);
    if (isNaN(idx) || idx < 0) {
      setNewToolName("");
      setNewToolGlobal("");
      setNewToolRel("");
      return;
    }
    const t = templates[idx];
    if (t) {
      setNewToolName(t.name);
      setNewToolGlobal(t.global_path);
      setNewToolRel(t.project_rel_path);
    }
  }

  // ==================== Actions ====================

  async function handleScan() {
    setScanning(true);
    try {
      let result: ScanResult;
      if (activeTab === "projects" && selectedProject !== 0) {
        // Project-level scan: scan project-specific paths
        result = await invoke<ScanResult>("scan_scope", {
          toolId: null,
          projectId: selectedProject,
        });
      } else {
        // Global scan
        result = await invoke<ScanResult>("full_scan");
      }
      await loadSkills();

      const parts: string[] = [];
      if (result.skills_found > 0) parts.push(`found ${result.skills_found}`);
      if (result.skills_new > 0) parts.push(`${result.skills_new} new`);
      if (result.skills_updated > 0) parts.push(`${result.skills_updated} updated`);

      // Show modal if there are new or updated skills
      if (result.details.length > 0) {
        setScanResult(result);
      } else {
        addToast("success", `Scan complete: ${parts.join(", ") || "no skills found"}`);
      }

      if (result.errors.length > 0) {
        result.errors.forEach((err) => addToast("error", err));
      }

      // Auto-check updates after scan
      if (result.skills_found > 0) {
        await handleCheckUpdates();
      }
    } catch (e) {
      addToast("error", `Scan failed: ${e}`);
    } finally {
      setScanning(false);
    }
  }

  async function handleCheckUpdates() {
    setCheckingUpdates(true);
    try {
      const result = await invoke<SkillUpdate[]>("check_updates");
      setUpdates(result);
      if (result.length > 0) {
        setShowUpdatesModal(true);
      } else {
        addToast("info", "All skills are up to date");
      }
    } catch (e) {
      addToast("error", `Check updates failed: ${e}`);
    } finally {
      setCheckingUpdates(false);
    }
  }

  async function handleViewDiff(update: SkillUpdate) {
    setLoadingDiff(update.skill_id);
    try {
      const diff = await invoke<SkillDiff>("get_skill_diff", { skillId: update.skill_id });
      setSelectedUpdateDiff({ update, diff });
    } catch (e) {
      addToast("error", `Failed to load diff: ${e}`);
    } finally {
      setLoadingDiff(null);
    }
  }

  async function handleUpdateFromDiff(skillId: number) {
    try {
      await invoke("sync_skill", { skillId, projectId: null });
      addToast("success", "Skill updated to SSOT");
      setSelectedUpdateDiff(null);
      // Remove from updates list
      setUpdates((prev) => prev.filter((u) => u.skill_id !== skillId));
      await loadSkills();
    } catch (e) {
      addToast("error", `Sync failed: ${e}`);
    }
  }

  function handleSkipUpdate(skillId: number) {
    setSelectedUpdateDiff(null);
    setUpdates((prev) => prev.filter((u) => u.skill_id !== skillId));
  }

  async function handleToggle(skillId: number, toolId: number, active: boolean) {
    try {
      await invoke("toggle_skill", {
        skillId,
        toolId,
        projectId: selectedProject,
        active,
      });
      await loadSkills();

      // Auto-sync when enabling (if full-auto mode)
      if (active && settings.sync_mode === "full-auto") {
        await handleSyncSkill(skillId);
      }
    } catch (e) {
      addToast("error", `Toggle failed: ${e}`);
    }
  }

  async function handleSyncSkill(skillId: number) {
    setSyncing((prev) => new Set(prev).add(skillId));
    try {
      const result = await invoke<SyncResult>("sync_skill", { skillId, projectId: selectedProject });
      await loadSkills();
      await handleCheckUpdates();

      if (result.errors.length > 0) {
        addToast("error", `Sync ${result.skill_name}: ${result.errors.join(", ")}`);
      } else {
        addToast("success", `Synced ${result.skill_name} to ${result.synced_to} tool(s)`);
      }
    } catch (e) {
      addToast("error", `Sync failed: ${e}`);
    } finally {
      setSyncing((prev) => {
        const next = new Set(prev);
        next.delete(skillId);
        return next;
      });
    }
  }

  async function handleSyncAll() {
    setScanning(true);
    try {
      const results = await invoke<SyncResult[]>("sync_all_pending");
      await loadSkills();
      await handleCheckUpdates();

      const totalSynced = results.reduce((sum, r) => sum + r.synced_to, 0);
      const totalErrors = results.reduce((sum, r) => sum + r.errors.length, 0);

      addToast(
        totalErrors > 0 ? "error" : "success",
        `Sync complete: ${totalSynced} synced, ${totalErrors} errors`,
      );
    } catch (e) {
      addToast("error", `Sync all failed: ${e}`);
    } finally {
      setScanning(false);
    }
  }

  async function handleSaveSettings() {
    try {
      await invoke("update_settings", { newSettings: settings });
      addToast("success", "Settings saved");
    } catch (e) {
      addToast("error", `Failed to save settings: ${e}`);
    }
  }

  // ==================== Tool CRUD ====================

  function startEdit(tool: Tool) {
    setEditingTool(tool.id);
    setEditGlobalPath(tool.global_path);
    setEditRelPath(tool.project_rel_path);
  }

  async function saveEdit(toolId: number) {
    if (!editGlobalPath.trim()) {
      addToast("error", "Global path cannot be empty");
      return;
    }
    const gPath = editGlobalPath.trim();
    if (
      !gPath.startsWith("/") &&
      !gPath.startsWith("~") &&
      !/^[A-Za-z]:[\\/]/.test(gPath) &&
      !gPath.startsWith("\\\\")
    ) {
      addToast("error", "Global path must be absolute (start with /, ~/, drive letter, or \\\\)");
      return;
    }
    try {
      await invoke("update_tool_path", {
        toolId,
        globalPath: gPath,
        projectRelPath: editRelPath.trim(),
      });
      setEditingTool(null);
      await loadTools();
      addToast("success", "Path updated");
    } catch (e) {
      addToast("error", `Failed to update: ${e}`);
    }
  }

  async function handleAddTool() {
    if (!newToolName.trim() || !newToolGlobal.trim()) {
      addToast("error", "Name and global path are required");
      return;
    }
    // Validate global path is absolute
    const gPath = newToolGlobal.trim();
    if (
      !gPath.startsWith("/") &&
      !gPath.startsWith("~") &&
      !/^[A-Za-z]:[\\/]/.test(gPath) &&
      !gPath.startsWith("\\\\")
    ) {
      addToast("error", "Global path must be absolute (start with /, ~/, drive letter, or \\\\)");
      return;
    }
    // Validate relative path doesn't start with .
    const rPath = newToolRel.trim();
    if (rPath.startsWith("./") || rPath.startsWith("../") || rPath === ".") {
      addToast("error", "Project relative path must not start with ./ or ../");
      return;
    }
    try {
      await invoke("add_tool", {
        name: newToolName.trim(),
        globalPath: gPath,
        projectRelPath: rPath,
      });
      setShowAddTool(false);
      setNewToolName("");
      setNewToolGlobal("");
      setNewToolRel("");
      await loadTools();
      addToast("success", `Tool "${newToolName.trim()}" added`);
    } catch (e) {
      addToast("error", `Failed to add tool: ${e}`);
    }
  }

  async function handleDeleteTool(toolId: number, toolName: string) {
    if (!confirm(`Delete tool "${toolName}"? This will remove all related installations and sync logs.`)) return;
    try {
      // Optimistic update: immediately remove tool from skills and tool list
      setSkills((prev) =>
        prev.map((s) => {
          const filtered = s.installed_tools.filter((t) => t.tool_id !== toolId);
          return { ...s, installed_tools: filtered, install_count: filtered.length };
        })
      );
      setTools((prev) => prev.filter((t) => t.id !== toolId));
      if (editingTool === toolId) {
        setEditingTool(null);
      }

      await invoke("delete_tool", { toolId, toolName });
      // Reload to get authoritative state from backend
      await Promise.all([loadTools(), loadSkills()]);
      addToast("success", `Tool "${toolName}" deleted`);
    } catch (e) {
      // Rollback on failure: reload everything
      await Promise.all([loadTools(), loadSkills()]);
      addToast("error", `Failed to delete tool: ${e}`);
    }
  }

  // ==================== Project CRUD ====================

  async function handleAddProject() {
    if (!newProjectName.trim() || !newProjectPath.trim()) {
      addToast("error", "Name and path are required");
      return;
    }
    if (newProjectPath.startsWith("./") || newProjectPath.startsWith("../")) {
      addToast("error", "Please enter an absolute path");
      return;
    }
    try {
      await invoke("add_project", { name: newProjectName, path: newProjectPath });
      setShowAddProject(false);
      setNewProjectName("");
      setNewProjectPath("");
      await loadProjects();
      addToast("success", `Project "${newProjectName}" added`);
    } catch (e) {
      addToast("error", `Failed to add project: ${e}`);
    }
  }

  async function handleDeleteProject(projectId: number, projectName: string) {
    if (!confirm(`Delete project "${projectName}"? All related data will be removed.`)) return;
    try {
      await invoke("delete_project", { projectId });
      if (selectedProject === projectId) {
        // Auto-select next project or fall back to global
        const remaining = projects.filter((p) => p.id !== projectId);
        setSelectedProject(remaining.length > 0 ? remaining[0].id : 0);
      }
      await loadProjects();
      addToast("success", `Project "${projectName}" deleted`);
    } catch (e) {
      addToast("error", `Failed to delete project: ${e}`);
    }
  }

  // ==================== Render Helpers ====================

  function getInstallStatus(skill: SkillView, toolId: number): InstallationInfo | undefined {
    return skill.installed_tools.find((t) => t.tool_id === toolId);
  }

  function hasUpdate(skill: SkillView): boolean {
    return updates.some((u) => u.skill_id === skill.id);
  }

  const filteredSkills = (() => {
    const filtered = searchQuery
      ? skills.filter(
          (s) =>
            s.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
            (s.description && s.description.toLowerCase().includes(searchQuery.toLowerCase())),
        )
      : [...skills];
    filtered.sort((a, b) => {
      let cmp = 0;
      if (sortBy === "name") {
        cmp = a.name.localeCompare(b.name);
      } else if (sortBy === "updated_at") {
        cmp = a.updated_at.localeCompare(b.updated_at);
      } else {
        cmp = a.created_at.localeCompare(b.created_at);
      }
      return sortDir === "desc" ? -cmp : cmp;
    });
    return filtered;
  })();

  // ==================== Panels ====================

  function renderSettingsPanel() {
    return (
      <section className="section">
        <div className="panel-header">
          <h2 className="section-title">Settings</h2>
          <button className="btn btn-secondary" onClick={() => setActivePanel("main")}>
            Back
          </button>
        </div>

        <div className="settings-group">
          <label className="settings-label">Sync Mode</label>
          <select
            className="settings-select"
            value={settings.sync_mode}
            onChange={(e) => setSettings({ ...settings, sync_mode: e.target.value })}
          >
            <option value="semi-auto">Semi-Auto (manual sync)</option>
            <option value="full-auto">Full-Auto (sync on change)</option>
          </select>
          <p className="settings-hint">
            Semi-auto: scan detects changes, you click sync. Full-auto: scan triggers immediate sync.
          </p>
        </div>

        <div className="settings-group">
          <label className="settings-label">
            <input
              type="checkbox"
              checked={settings.prefer_symlink}
              onChange={(e) => setSettings({ ...settings, prefer_symlink: e.target.checked })}
            />
            {" "}Prefer symlinks over copies
          </label>
          <p className="settings-hint">
            Symlinks save disk space but require developer mode on Windows. Falls back to copy on failure.
          </p>
        </div>

        <button className="btn btn-primary" onClick={handleSaveSettings}>
          Save Settings
        </button>
      </section>
    );
  }

  function renderLogsPanel() {
    const actionLabels: Record<string, string> = {
      scan: "Scan",
      sync: "Sync",
      toggle_on: "Enable",
      toggle_off: "Disable",
      add_tool: "Add Tool",
      delete_tool: "Delete Tool",
      add_project: "Add Project",
      delete_project: "Delete Project",
    };

    return (
      <section className="section">
        <div className="panel-header">
          <h2 className="section-title">Activity Logs</h2>
          <div className="panel-actions">
            <button className="btn btn-small" onClick={loadSyncLogs}>Refresh</button>
            <button className="btn btn-secondary" onClick={() => setActivePanel("main")}>Back</button>
          </div>
        </div>

        {syncLogs.length === 0 ? (
          <div className="empty-state">
            <p>No operations recorded yet.</p>
          </div>
        ) : (
          <div className="log-table-wrapper">
            <table className="log-table">
              <thead>
                <tr>
                  <th>Time</th>
                  <th>Action</th>
                  <th>Skill</th>
                  <th>Tool</th>
                  <th>Status</th>
                  <th>Detail</th>
                </tr>
              </thead>
              <tbody>
                {syncLogs.map((log) => (
                  <tr key={log.id} className={`log-row log-${log.status}`}>
                    <td className="log-time">{log.created_at}</td>
                    <td>
                      <span className={`action-badge action-${log.action}`}>
                        {actionLabels[log.action] || log.action}
                      </span>
                      {log.direction && (
                        <span className={`direction-badge dir-${log.direction}`}>
                          {log.direction === "to_ssot" ? "→ SSOT" : "← SSOT"}
                        </span>
                      )}
                    </td>
                    <td>{log.skill_name || "—"}</td>
                    <td>{log.tool_name || (log.action === "delete_tool" || log.action === "add_tool" ? log.detail : "—")}</td>
                    <td>
                      <span className={`status-badge status-${log.status}`}>
                        {log.status}
                      </span>
                    </td>
                    <td className="log-error">{log.detail || "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    );
  }

  // ==================== Main Render ====================

  if (activePanel === "settings") return (
    <main className="container">
      <ToastContainer toasts={toasts} />
      {renderSettingsPanel()}
    </main>
  );

  if (activePanel === "logs") return (
    <main className="container">
      <ToastContainer toasts={toasts} />
      {renderLogsPanel()}
    </main>
  );

  return (
    <main className="container">
      <ToastContainer toasts={toasts} />

      {/* App header */}
      <header className="app-header">
        <div className="brand">
          <span className="brand-icon">⬡</span>
          <span className="brand-name">Skill Manager</span>
        </div>
      </header>

      {/* Top navigation */}
      <nav className="top-nav">
        <div className="tabs">
          <button
            className={`tab ${activeTab === "global" ? "tab-active" : ""}`}
            onClick={() => setActiveTab("global")}
          >
            Global
          </button>
          <button
            className={`tab ${activeTab === "projects" ? "tab-active" : ""}`}
            onClick={() => setActiveTab("projects")}
          >
            Projects
          </button>
        </div>
        <div className="nav-actions">
          <button className="btn btn-small btn-ghost" onClick={() => { loadSyncLogs(); setActivePanel("logs"); }}>
            Logs
          </button>
          <button className="btn btn-small btn-ghost" onClick={() => setActivePanel("settings")}>
            Settings
          </button>
        </div>
      </nav>

      {/* Project sub-navigation */}
      {activeTab === "projects" && (
        <div className="project-nav">
          {projects.length === 0 && (
            <span className="project-empty-hint">No projects yet. Add one to get started.</span>
          )}
          {projects.map((p) => (
            <div key={p.id} className="project-btn-group">
              <button
                className={`project-btn ${selectedProject === p.id ? "project-active" : ""}`}
                onClick={() => setSelectedProject(p.id)}
              >
                {p.name}
              </button>
              <button
                className="project-delete"
                onClick={() => handleDeleteProject(p.id, p.name)}
                title="Delete project"
              >
                ×
              </button>
            </div>
          ))}
          <button
            className="btn btn-small btn-primary"
            onClick={() => setShowAddProject(true)}
          >
            + Project
          </button>
        </div>
      )}

      {/* Add project dialog */}
      {showAddProject && (
        <div className="modal-overlay" onClick={() => setShowAddProject(false)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Add Project</h3>
            <div className="form-group">
              <label>Name:</label>
              <input
                type="text"
                value={newProjectName}
                onChange={(e) => setNewProjectName(e.target.value)}
                className="edit-input"
                placeholder="My Project"
              />
            </div>
            <div className="form-group">
              <label>Absolute path:</label>
              <input
                type="text"
                value={newProjectPath}
                onChange={(e) => setNewProjectPath(e.target.value)}
                className="edit-input"
                placeholder="D:\my-project"
              />
            </div>
            <div className="modal-actions">
              <button className="btn btn-primary" onClick={handleAddProject}>Add</button>
              <button className="btn btn-secondary" onClick={() => setShowAddProject(false)}>Cancel</button>
            </div>
          </div>
        </div>
      )}

      {/* Tool configuration */}
      <section className="section">
        <div className="section-header">
          <h2 className="section-title">Tool Paths</h2>
          <button className="btn btn-small" onClick={() => setShowAddTool(!showAddTool)}>
            {showAddTool ? "Cancel" : "+ Add Tool"}
          </button>
        </div>

        {discoveredTools.length > 0 && (
          <div className="discovery-banner">
            <span className="discovery-text">
              Found {discoveredTools.length} unregistered tool{discoveredTools.length > 1 ? "s" : ""}:{" "}
              {discoveredTools.map((t) => t.name).join(", ")}
            </span>
            <button
              className="btn btn-primary btn-small"
              onClick={handleAddDiscoveredAll}
              disabled={addingDiscovered}
            >
              {addingDiscovered ? "Adding..." : "Add All"}
            </button>
            <button
              className="btn btn-small"
              onClick={() => setDiscoveredTools([])}
            >
              Dismiss
            </button>
          </div>
        )}

        {showAddTool && (
          <div className="add-tool-form">
            {templates.length > 0 && (
              <div className="form-row">
                <select
                  className="template-select"
                  defaultValue=""
                  onChange={handleSelectTemplate}
                >
                  <option value="" disabled>Choose a template...</option>
                  {templates.map((t, i) => (
                    <option key={t.name} value={i}>{t.name}</option>
                  ))}
                </select>
              </div>
            )}
            <div className="form-row">
              <input type="text" value={newToolName} onChange={(e) => setNewToolName(e.target.value)}
                className="edit-input" placeholder="Tool name" />
              <input type="text" value={newToolGlobal} onChange={(e) => setNewToolGlobal(e.target.value)}
                className="edit-input" placeholder="Global path (e.g. ~/.mytool/skills/)" />
              <input type="text" value={newToolRel} onChange={(e) => setNewToolRel(e.target.value)}
                className="edit-input" placeholder="Project rel path (e.g. .mytool/skills/)" />
              <button className="btn btn-primary btn-small" onClick={handleAddTool}>Add</button>
            </div>
          </div>
        )}

        <div className="tool-list">
          {tools.map((tool) => (
            <div key={tool.id} className="tool-item">
              <div className="tool-header">
                <span className="tool-name">{tool.name}</span>
                <div className="tool-actions">
                  <button className="btn btn-small" onClick={() => startEdit(tool)}>Edit</button>
                  <button className="btn btn-small btn-danger" onClick={() => handleDeleteTool(tool.id, tool.name)}>×</button>
                </div>
              </div>
              {editingTool === tool.id ? (
                <div className="tool-edit">
                  <div className="edit-row">
                    <label>Global path:</label>
                    <input type="text" value={editGlobalPath} onChange={(e) => setEditGlobalPath(e.target.value)}
                      className="edit-input" />
                  </div>
                  <div className="edit-row">
                    <label>Project path:</label>
                    <input type="text" value={editRelPath} onChange={(e) => setEditRelPath(e.target.value)}
                      className="edit-input" />
                  </div>
                  <div className="edit-actions">
                    <button onClick={() => saveEdit(tool.id)} className="btn btn-primary btn-small">Save</button>
                    <button onClick={() => setEditingTool(null)} className="btn btn-secondary btn-small">Cancel</button>
                  </div>
                </div>
              ) : (
                <code className="path-text">{tool.global_path}</code>
              )}
            </div>
          ))}
        </div>
      </section>

      {/* Action bar */}
      <section className="section">
        <div className="action-bar">
          <button className="btn btn-primary" onClick={handleScan} disabled={scanning}>
            {scanning ? "Scanning..." : "Scan All"}
          </button>
          <button className="btn btn-secondary" onClick={handleCheckUpdates} disabled={checkingUpdates}>
            {checkingUpdates ? "Checking..." : "Check Updates"}
            {updates.length > 0 && <span className="update-badge">{updates.length}</span>}
          </button>
          <button className="btn btn-secondary" onClick={handleSyncAll} disabled={scanning}>
            Sync All Active
          </button>
          <div className="search-box">
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search skills..."
              className="search-input"
            />
          </div>
          <select
            className="sort-select"
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as typeof sortBy)}
          >
            <option value="name">Name</option>
            <option value="updated_at">Updated</option>
            <option value="created_at">Created</option>
          </select>
          <button
            className="btn btn-small sort-dir-btn"
            onClick={() => setSortDir((d) => (d === "asc" ? "desc" : "asc"))}
            title={sortDir === "asc" ? "Ascending" : "Descending"}
          >
            {sortDir === "asc" ? "A\u2192Z" : "Z\u2192A"}
          </button>
        </div>
      </section>

      {/* Skill grid */}
      <section className="section">
        <h2 className="section-title">
          Skills {filteredSkills.length > 0 && <span className="badge">{filteredSkills.length}</span>}
          {updates.length > 0 && <span className="badge badge-update">{updates.length} updates</span>}
        </h2>

        {scanning && skills.length === 0 ? (
          <div className="skeleton-grid">
            {[1, 2, 3].map((i) => (
              <div key={i} className="skeleton-card" />
            ))}
          </div>
        ) : filteredSkills.length === 0 ? (
          <div className="empty-state">
            {searchQuery ? (
              <p>No matching skills found for "{searchQuery}"</p>
            ) : (
              <>
                <p>No skills discovered yet.</p>
                <p>Configure tool paths above and click "Scan All" to discover skills.</p>
              </>
            )}
          </div>
        ) : (
          <div className="skill-grid">
            {filteredSkills.map((skill) => (
              <div key={skill.id} className={`skill-card ${hasUpdate(skill) ? "skill-has-update" : ""}`}>
                <div className="skill-header">
                  <h3 className="skill-name">{skill.name}</h3>
                  {hasUpdate(skill) && <span className="update-indicator" title="Update available">●</span>}
                  {syncing.has(skill.id) && <span className="sync-spinner">⟳</span>}
                </div>

                {skill.description && <p className="skill-desc">{skill.description}</p>}

                <code className="skill-path">{skill.source_path}</code>

                {/* Tool toggles */}
                <div className="skill-tools">
                  {tools.map((tool) => {
                    const inst = getInstallStatus(skill, tool.id);
                    const isActive = inst?.status === "active";
                    return (
                      <div key={tool.id} className="skill-tool-row">
                        <label className="toggle-label">
                          <input
                            type="checkbox"
                            checked={isActive}
                            onChange={(e) => handleToggle(skill.id, tool.id, e.target.checked)}
                          />
                          <span className="toggle-text">{tool.name}</span>
                        </label>
                        {inst?.synced_at && (
                          <span className="sync-time" title={inst.synced_at}>
                            synced
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>

                {/* Sync button (visible when any tool is active) */}
                {skill.install_count > 0 && (
                  <button
                    className="btn btn-small btn-primary sync-btn"
                    onClick={() => handleSyncSkill(skill.id)}
                    disabled={syncing.has(skill.id)}
                  >
                    {syncing.has(skill.id) ? "Syncing..." : "Sync Now"}
                  </button>
                )}
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Scan Result Modal */}
      {scanResult && (
        <div className="modal-overlay" onClick={() => setScanResult(null)}>
          <div className="modal scan-result-modal" onClick={(e) => e.stopPropagation()}>
            <h3>Scan Results</h3>
            <div className="scan-summary">
              <span className="scan-summary-item">
                Found <strong>{scanResult.skills_found}</strong>
              </span>
              {scanResult.skills_new > 0 && (
                <span className="scan-summary-item scan-new">
                  {scanResult.skills_new} new
                </span>
              )}
              {scanResult.skills_updated > 0 && (
                <span className="scan-summary-item scan-updated">
                  {scanResult.skills_updated} updated
                </span>
              )}
            </div>
            <div className="scan-detail-table-wrapper">
              <table className="scan-detail-table">
                <thead>
                  <tr>
                    <th>Skill</th>
                    <th>Tool</th>
                    <th>Scope</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {scanResult.details.map((d, i) => (
                    <tr key={i}>
                      <td className="scan-skill-name" title={d.source_path}>{d.skill_name}</td>
                      <td className="scan-tool-name">{d.tool_name}</td>
                      <td className="scan-scope">{d.scope}</td>
                      <td>
                        <span className={`scan-status scan-status-${d.status}`}>
                          {d.status}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="modal-actions">
              <button className="btn btn-primary" onClick={() => setScanResult(null)}>Close</button>
            </div>
          </div>
        </div>
      )}

      {/* Updates Modal */}
      {showUpdatesModal && (
        <div className="modal-overlay" onClick={() => { setShowUpdatesModal(false); setSelectedUpdateDiff(null); }}>
          <div className="modal updates-modal" onClick={(e) => e.stopPropagation()}>
            {selectedUpdateDiff ? (
              <>
                <div className="diff-header">
                  <button className="btn btn-small" onClick={() => setSelectedUpdateDiff(null)}>&larr; Back</button>
                  <h3 className="diff-title">{selectedUpdateDiff.update.skill_name}</h3>
                </div>
                <div className="diff-meta">
                  <span className="diff-path" title={selectedUpdateDiff.diff.source_path}>
                    source: {selectedUpdateDiff.diff.source_path}
                  </span>
                  <span className="diff-path" title={selectedUpdateDiff.diff.ssot_path}>
                    ssot: {selectedUpdateDiff.diff.ssot_path}
                  </span>
                </div>
                <div className="diff-files">
                  {selectedUpdateDiff.diff.files.map((file, fi) => (
                    <div key={fi} className="diff-file">
                      <div className={`diff-file-header diff-file-${file.change}`}>
                        <span className="diff-change-badge">{file.change}</span>
                        <span className="diff-file-path">{file.path}</span>
                      </div>
                      {file.hunks.map((hunk, hi) => (
                        <div key={hi} className="diff-hunk">
                          <div className="diff-hunk-header">
                            @@ -{hunk.old_start},{hunk.old_count} +{hunk.new_start},{hunk.new_count} @@
                          </div>
                          <pre className="diff-lines">
                            {hunk.lines.map((line, li) => (
                              <div key={li} className={`diff-line diff-line-${line.op === "+" ? "add" : line.op === "-" ? "del" : "ctx"}`}>
                                <span className="diff-line-op">{line.op === " " ? "\u00a0" : line.op}</span>
                                <span className="diff-line-content">{line.content}</span>
                              </div>
                            ))}
                          </pre>
                        </div>
                      ))}
                    </div>
                  ))}
                  {!selectedUpdateDiff.diff.has_changes && (
                    <div className="diff-no-changes">No file-level differences detected (hash may differ due to metadata).</div>
                  )}
                </div>
                <div className="modal-actions">
                  <button
                    className="btn btn-primary"
                    onClick={() => handleUpdateFromDiff(selectedUpdateDiff.update.skill_id)}
                  >
                    Update to SSOT
                  </button>
                  <button
                    className="btn btn-secondary"
                    onClick={() => handleSkipUpdate(selectedUpdateDiff.update.skill_id)}
                  >
                    Skip
                  </button>
                </div>
              </>
            ) : (
              <>
                <h3>Updates Available ({updates.length})</h3>
                <div className="updates-list">
                  {updates.map((u) => (
                    <div key={u.skill_id} className="update-item">
                      <div className="update-item-info">
                        <span className="update-skill-name">{u.skill_name}</span>
                        <code className="update-path" title={u.source_path}>{u.source_path}</code>
                      </div>
                      <div className="update-item-actions">
                        <button
                          className="btn btn-small"
                          onClick={() => handleViewDiff(u)}
                          disabled={loadingDiff === u.skill_id}
                        >
                          {loadingDiff === u.skill_id ? "Loading..." : "View Diff"}
                        </button>
                        <button
                          className="btn btn-small btn-primary"
                          onClick={() => handleUpdateFromDiff(u.skill_id)}
                        >
                          Update
                        </button>
                        <button
                          className="btn btn-small"
                          onClick={() => handleSkipUpdate(u.skill_id)}
                        >
                          Skip
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
                <div className="modal-actions">
                  <button className="btn btn-secondary" onClick={() => setShowUpdatesModal(false)}>Close</button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </main>
  );
}

function ToastContainer({ toasts }: { toasts: Toast[] }) {
  if (toasts.length === 0) return null;
  return (
    <div className="toast-container">
      {toasts.map((toast) => (
        <div key={toast.id} className={`toast toast-${toast.type}`}>
          {toast.message}
        </div>
      ))}
    </div>
  );
}

export default App;
