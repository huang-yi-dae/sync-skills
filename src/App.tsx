import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import type { Tool, Skill, ScanResult, Toast } from "./types";

function App() {
  const [tools, setTools] = useState<Tool[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [scanning, setScanning] = useState(false);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [editingTool, setEditingTool] = useState<number | null>(null);
  const [editGlobalPath, setEditGlobalPath] = useState("");
  const [editRelPath, setEditRelPath] = useState("");

  // Load data on mount
  useEffect(() => {
    loadTools();
    loadSkills();
  }, []);

  const addToast = useCallback((type: "success" | "error", message: string) => {
    const id = Date.now();
    setToasts((prev) => [...prev, { type, message, id }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id));
    }, 4000);
  }, []);

  async function loadTools() {
    try {
      const result = await invoke<Tool[]>("list_tools");
      setTools(result);
    } catch (e) {
      addToast("error", `Failed to load tools: ${e}`);
    }
  }

  async function loadSkills() {
    try {
      const result = await invoke<Skill[]>("list_skills");
      setSkills(result);
    } catch (e) {
      addToast("error", `Failed to load skills: ${e}`);
    }
  }

  async function handleScan() {
    setScanning(true);
    try {
      const result = await invoke<ScanResult>("full_scan");
      await loadSkills();

      const parts: string[] = [];
      if (result.skills_found > 0) parts.push(`found ${result.skills_found} skills`);
      if (result.skills_new > 0) parts.push(`${result.skills_new} new`);
      if (result.skills_updated > 0) parts.push(`${result.skills_updated} updated`);
      if (result.errors.length > 0) parts.push(`${result.errors.length} errors`);

      addToast("success", `Scan complete: ${parts.join(", ") || "no skills found"}`);

      if (result.errors.length > 0) {
        result.errors.forEach((err) => addToast("error", err));
      }
    } catch (e) {
      addToast("error", `Scan failed: ${e}`);
    } finally {
      setScanning(false);
    }
  }

  function startEdit(tool: Tool) {
    setEditingTool(tool.id);
    setEditGlobalPath(tool.global_path);
    setEditRelPath(tool.project_rel_path);
  }

  async function saveEdit(toolId: number) {
    try {
      await invoke("update_tool_path", {
        toolId,
        globalPath: editGlobalPath,
        projectRelPath: editRelPath,
      });
      setEditingTool(null);
      await loadTools();
      addToast("success", "Path updated");
    } catch (e) {
      addToast("error", `Failed to update path: ${e}`);
    }
  }

  function cancelEdit() {
    setEditingTool(null);
  }

  return (
    <main className="container">
      {/* Toast notifications */}
      <div className="toast-container">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast toast-${toast.type}`}>
            {toast.message}
          </div>
        ))}
      </div>

      {/* Tab bar */}
      <div className="tabs">
        <button className="tab tab-active">Global</button>
        <button className="tab tab-disabled" disabled>
          Projects
        </button>
      </div>

      {/* Tool configuration section */}
      <section className="section">
        <h2 className="section-title">Tool Paths</h2>
        <div className="tool-list">
          {tools.map((tool) => (
            <div key={tool.id} className="tool-item">
              <div className="tool-name">{tool.name}</div>
              {editingTool === tool.id ? (
                <div className="tool-edit">
                  <div className="edit-row">
                    <label>Global path:</label>
                    <input
                      type="text"
                      value={editGlobalPath}
                      onChange={(e) => setEditGlobalPath(e.target.value)}
                      className="edit-input"
                    />
                  </div>
                  <div className="edit-row">
                    <label>Project path:</label>
                    <input
                      type="text"
                      value={editRelPath}
                      onChange={(e) => setEditRelPath(e.target.value)}
                      className="edit-input"
                    />
                  </div>
                  <div className="edit-actions">
                    <button onClick={() => saveEdit(tool.id)} className="btn btn-primary">
                      Save
                    </button>
                    <button onClick={cancelEdit} className="btn btn-secondary">
                      Cancel
                    </button>
                  </div>
                </div>
              ) : (
                <div className="tool-paths">
                  <code className="path-text">{tool.global_path}</code>
                  <button onClick={() => startEdit(tool)} className="btn btn-small">
                    Edit
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      </section>

      {/* Action buttons */}
      <section className="section">
        <div className="action-bar">
          <button onClick={handleScan} className="btn btn-primary" disabled={scanning}>
            {scanning ? "Scanning..." : "Scan All"}
          </button>
        </div>
      </section>

      {/* Skill list */}
      <section className="section">
        <h2 className="section-title">
          Skills {skills.length > 0 && <span className="badge">{skills.length}</span>}
        </h2>
        {skills.length === 0 ? (
          <div className="empty-state">
            <p>No skills discovered yet.</p>
            <p>Configure tool paths above and click "Scan All" to discover skills.</p>
          </div>
        ) : (
          <div className="skill-grid">
            {skills.map((skill) => (
              <div key={skill.id} className="skill-card">
                <h3 className="skill-name">{skill.name}</h3>
                {skill.description && (
                  <p className="skill-desc">{skill.description}</p>
                )}
                <code className="skill-path">{skill.source_path}</code>
              </div>
            ))}
          </div>
        )}
      </section>
    </main>
  );
}

export default App;
