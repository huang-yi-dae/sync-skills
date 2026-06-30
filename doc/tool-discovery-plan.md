# Tool Discovery & Preset Templates

> 功能：内置已知 AI 编码工具的路径注册表，支持自动探测本机已安装的工具，添加工具时提供预置模板选择。

## 1. 目标

1. **已知工具注册表**：在代码中维护一份已知 AI 编码工具的名称 + 默认路径表
2. **自动探测**：应用启动时扫描本机文件系统，发现已安装但尚未注册的工具，提示用户添加
3. **预置模板**：添加工具时提供下拉选择，选中后自动填充名称和路径

## 2. 已知工具列表

| # | 工具名 | global_path | project_rel_path | 备注 |
|---|--------|-------------|------------------|------|
| 1 | Claude Code | `~/.claude/skills/` | `.claude/skills/` | 已有种子 |
| 2 | Codex CLI | `~/.codex/skills/` | `.codex/skills/` | 已有种子 |
| 3 | OpenCode | `~/.opencode/skills/` | `.opencode/skills/` | 已有种子 |
| 4 | Gemini CLI | `~/.gemini/skills/` | `.gemini/skills/` | 已有种子 |
| 5 | Cline | `~/.cline/skills/` | `.cline/skills/` | 已有种子 |
| 6 | Cursor | `~/.cursor/skills/` | `.cursor/skills/` | 新增 |
| 7 | Windsurf | `~/.windsurf/skills/` | `.windsurf/skills/` | 新增 (Codeium) |
| 8 | Aider | `~/.aider/skills/` | `.aider/skills/` | 新增 |
| 9 | Roo Code | `~/.roo/skills/` | `.roo/skills/` | 新增 |
| 10 | Trae | `~/.trae/skills/` | `.trae/skills/` | 新增 |
| 11 | Kiro | `~/.kiro/skills/` | `.kiro/skills/` | 新增 |
| 12 | Augment | `~/.augment/skills/` | `.augment/skills/` | 新增 |
| 13 | Qoder | `~/.qoder/skills/` | `.qoder/skills/` | 新增 |

## 3. 架构设计

### 3.1 新模块 `discovery.rs`

```
src-tauri/src/
  discovery.rs    ← 新增
```

职责：
- `KNOWN_TOOLS` 常量数组，定义所有已知工具
- `ToolTemplate` 结构体（name, global_path, project_rel_path）
- `get_all_templates()` → 返回全部模板列表
- `discover_tools(db)` → 检查哪些工具的 global_path 在本机存在但 DB 中没有

### 3.2 IPC 命令变更

新增 2 个命令：

```rust
// 返回所有已知工具模板（用于前端下拉选择）
#[tauri::command]
fn list_tool_templates() -> Vec<ToolTemplate>

// 自动探测本机已安装但未注册的工具，返回发现的工具列表
#[tauri::command]
fn discover_tools(db: State<DbState>) -> Vec<ToolTemplate>
```

不新增"一键批量添加"命令——用户在前端看到发现结果后，逐个或选择性调用已有的 `add_tool` 即可。保持简单。

### 3.3 数据流

```
启动时:
  App.tsx onMount
    → invoke("discover_tools")
    → 后端: 遍历 KNOWN_TOOLS，expand_path + exists() + 检查 DB
    → 返回未注册的工具列表
    → 前端: 如有发现，显示 toast + 内联提示条

添加工具时:
  用户点击 "+ Add Tool"
    → 前端显示模板下拉 (invoke("list_tool_templates") 缓存)
    → 选择模板 → 自动填充 name / global_path / project_rel_path
    → 用户可修改 → 点击 Add → invoke("add_tool") (已有命令，不变)
```

## 4. 文件变更明细

### 4.1 `src-tauri/src/discovery.rs` (新建，约 80 行)

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolTemplate {
    pub name: String,
    pub global_path: String,
    pub project_rel_path: String,
}

pub const KNOWN_TOOLS: &[(&str, &str, &str)] = &[
    ("Claude Code",  "~/.claude/skills/",   ".claude/skills/"),
    ("Codex CLI",    "~/.codex/skills/",    ".codex/skills/"),
    ("OpenCode",     "~/.opencode/skills/", ".opencode/skills/"),
    ("Gemini CLI",   "~/.gemini/skills/",   ".gemini/skills/"),
    ("Cline",        "~/.cline/skills/",    ".cline/skills/"),
    ("Cursor",       "~/.cursor/skills/",   ".cursor/skills/"),
    ("Windsurf",     "~/.windsurf/skills/", ".windsurf/skills/"),
    ("Aider",        "~/.aider/skills/",    ".aider/skills/"),
    ("Roo Code",     "~/.roo/skills/",      ".roo/skills/"),
    ("Trae",         "~/.trae/skills/",     ".trae/skills/"),
    ("Kiro",         "~/.kiro/skills/",     ".kiro/skills/"),
    ("Augment",      "~/.augment/skills/",  ".augment/skills/"),
    ("Qoder",        "~/.qoder/skills/",    ".qoder/skills/"),
];

pub fn get_all_templates() -> Vec<ToolTemplate> { ... }

pub fn discover_tools(db: &crate::db::Database) -> Vec<ToolTemplate> {
    // 1. 获取 DB 中已有的 tool name 列表
    // 2. 遍历 KNOWN_TOOLS
    // 3. expand_path(global_path) → exists()?
    // 4. 存在且 DB 中没有同名 → 加入结果
}
```

### 4.2 `src-tauri/src/lib.rs` (修改)

- 添加 `mod discovery;`
- 新增 `list_tool_templates` 和 `discover_tools` 两个 IPC 命令
- 注册到 `generate_handler!` 宏

### 4.3 `src/types.ts` (修改)

```typescript
// 新增
export interface ToolTemplate {
  name: string;
  global_path: string;
  project_rel_path: string;
}
```

### 4.4 `src/App.tsx` (修改)

**自动探测：**
- 新增 state: `discoveredTools: ToolTemplate[]`
- `useEffect` 中调用 `discover_tools`
- 有发现时在 Tool Paths section 上方显示提示条：
  `"发现 N 个未注册的工具: Claude Code, Cursor... [添加全部] [忽略]"`
- "添加全部" 逐个调用 `add_tool`，完成后刷新

**模板选择：**
- 新增 state: `templates: ToolTemplate[]`
- `showAddTool` 变为 true 时加载模板
- 表单顶部增加 `<select>` 下拉："选择预置模板..."
- 选中模板后自动填充 `newToolName`、`newToolGlobal`、`newToolRel`
- 用户仍可手动修改

### 4.5 `src/App.css` (修改)

- `.discovery-banner` 样式：提示条背景、边框
- `.template-select` 样式：下拉框与现有 form-row 一致

## 5. 执行步骤

| 步骤 | 文件 | 动作 | 预估行数 |
|------|------|------|----------|
| 1 | `discovery.rs` | 新建文件，写 KNOWN_TOOLS + ToolTemplate + 两个函数 | ~80 |
| 2 | `lib.rs` | 加 `mod discovery;` + 2 个 IPC 命令 + 注册 | ~25 |
| 3 | `types.ts` | 加 ToolTemplate 接口 | ~5 |
| 4 | `App.tsx` | 加 discoveredTools state + 探测 useEffect + 提示条 + 模板下拉 | ~80 |
| 5 | `App.css` | 加 discovery-banner + template-select 样式 | ~30 |

总计约 220 行变更。

## 6. 验证

1. `cargo check` — Rust 编译通过
2. `npx tsc --noEmit` — TypeScript 类型检查通过
3. 功能验证点：
   - 启动后自动探测到本机已有的工具目录
   - 发现新工具时显示提示条
   - 添加工具时下拉可选模板，选中后自动填充
   - 手动输入仍然正常工作（"自定义"模式）
