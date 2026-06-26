# Skill Manager - MVP PRD

## 1. 产品概述

Skill Manager 是一款桌面应用，用于统一管理 AI 编码工具（Claude Code、Codex CLI、OpenCode、Gemini CLI、Cline）的 Skill 插件。它作为 Skill 的单一信息源（SSOT），在多个工具之间同步 Skill 文件，解决"同一个 Skill 要手动复制到多个工具目录"的痛点。

**一句话定位**：一处管理，多处生效的 AI Skill 同步中心。

## 2. 目标用户

日常使用多个 AI 编码助手的开发者。他们的典型特征是：同时在 2-5 个 AI 工具之间切换，积累了自己的 Skill 库，但苦于每个工具的 Skill 目录不同，手动同步既繁琐又容易遗漏。

## 3. 核心问题

开发者在多个 AI 编码工具中使用 Skill（基于 SKILL.md 的插件），但每个工具有独立的 Skill 目录（如 `~/.claude/skills/`、`~/.codex/skills/`），导致同一份 Skill 需要手动复制到多个位置。当 Skill 更新时，需要逐一同步，极易遗漏造成版本不一致。

## 4. MVP 范围

MVP 聚焦于**本地 Skill 管理**，不涉及远端 GitHub 拉取。核心价值是：扫描发现 → 集中展示 → 一键同步。

**MVP 包含：** 本地路径扫描、Skill 列表展示、启用/禁用控制、SSOT 双向同步、全局与项目两级管理、操作反馈。

**MVP 不包含：** 远端 GitHub 仓库拉取、Remote/Local 分类 Tab、公开市场与裸仓库区分、多平台适配（GitLab 等）、自动工作区检测、定时同步。

## 5. 功能需求

### 5.1 工具路径配置

用户可对各 AI 编码工具的 Skill 目录进行增/删/改操作。配置分两级：

**全局路径**：预设各工具的常见默认路径（如 `~/.claude/skills/`），用户可覆盖。

**项目路径**：留空时自动继承全局路径推断（`<项目根目录>/<工具的相对路径>`），用户手动填写则以自定义值为准。

路径变更后立即触发对应范围的重新扫描。

**验收标准：**
- 5 个预设工具各自有正确的默认路径
- 用户可修改任意工具的全局路径和项目相对路径
- 修改路径后自动触发扫描，无需手动操作

### 5.2 路径格式容错

支持以下路径格式并正确解析为绝对路径：`~/.claude/skills/`、`C:\Users\xx\.claude\skills\`、`/home/xx/.claude/skills/`。路径末尾有无斜杠均可。路径不存在时给出明确错误提示而非崩溃。

**验收标准：**
- `~` 正确展开为用户主目录
- Windows 反斜杠和 Unix 正斜杠均正常处理
- 不存在的路径显示红色提示"路径不存在"

### 5.3 递归扫描 SKILL.md

对每个配置的工具路径递归扫描目录树。找到 `SKILL.md` 后解析其 YAML front matter（提取 `name` 和 `description`），SKILL.md 所在的整个目录作为一个 Skill 识别，停止本目录的继续递归。未找到 SKILL.md 的目录继续向下递归。同一路径下重复扫描时，已存在的 Skill 不产生重复记录（以 `source_path` 去重）。

**扫描算法：**
```
function scan(dir):
    if dir contains "SKILL.md":
        parse front matter → {name, description}
        compute content_hash(dir)
        register skill(source_path=dir)
        return  // 不再深入子目录
    for subdir in dir.children:
        if subdir is directory and not hidden:
            scan(subdir)
```

**Content Hash 计算：** 递归遍历目录下所有非隐藏文件，按相对路径字典序排列，以 `"相对路径\0内容\0"` 格式依次喂给 SHA-256，输出完整哈希字符串。

**验收标准：**
- 能正确发现嵌套目录中的 SKILL.md
- 隐藏目录（以 `.` 开头）跳过不扫描
- 解析 front matter 失败的目录跳过并记日志（不崩溃）
- SKILL.md 存在但 front matter 缺少 `name` 字段 → 以目录名作为 fallback name
- 重复扫描不产生重复记录
- content_hash 能检测到文件内容的变化

### 5.4 数据库存储

使用 SQLite 嵌入式数据库，5 张核心表，外键约束开启，级联删除。

**ID 哈希方案：** 取目标字符串的 SHA-256，截取前 8 字节，按 little-endian 解析为有符号 64 位 INTEGER。映射表（skill_installations、sync_logs）使用自增 ID。

**数据表：**

`tools`（工具注册表）：存储各 AI 工具的名称和路径配置。ID 为工具名称哈希。

`projects`（项目列表）：用户手动添加的项目。ID 为项目绝对路径哈希。内置全局项目（id=0, name="Global", path="~/.agents/skills/"）。

`skills`（Skill 元数据）：所有发现的 Skill。ID 为 source_path 哈希。MVP 阶段不区分 remote/local。

`skill_installations`（安装关系）：Skill 与工具的多对多关系，通过 project_id 区分全局和项目级。唯一约束 `(skill_id, tool_id, project_id)`。状态为 active/disabled。

`sync_logs`（同步日志）：每次同步操作记录一条，含方向（to_ssot/from_ssot）、状态（success/failed）、错误信息。

**验收标准：**
- 应用重启后数据完整可读
- 级联删除正常工作
- ID 哈希计算正确且一致

### 5.5 本地 Skill 双向同步

本地自写 Skill（目录下存在 `local.md` 标记文件）支持双向同步。

**SSOT 存储结构：**
```
~/.agents/skills/
  ├── remote/                   ← 预留目录，MVP 不使用（远端拉取为 post-MVP 功能）
  └── local/<skill-name>/       ← 本地自写 Skill 的 SSOT 存储位置
```
MVP 阶段所有 Skill 统一存入 `local/` 目录。`remote/` 目录结构预留给 post-MVP 远端拉取功能。

**默认半自动模式：** 扫描发现哈希变化 → UI 标记"有更新" → 用户点击"同步"按钮 → 从改动目录复制到 SSOT（`local/<skill-name>/`） → 从 SSOT 复制到其他所有启用该 Skill 的工具目录。

**可选全自动模式：** 设置中切换，扫描到变化后自动执行同步流程。

**文件操作规范：**
- 复制用标准库 API，不用 shell 命令
- 替换用临时目录（`.tmp-{进程ID}-{纳秒时间戳}`）+ rename 原子操作
- 同名冲突加 `-local` 后缀
- Symlink 失败时回退到 Copy 并记录日志

**`local.md` 标记机制：**
- 用户通过 UI 手动创建本地 Skill 时 → 工具自动在 Skill 目录下生成 `local.md`
- 从非远端目录导入 Skill 时 → 工具自动补 `local.md`
- 扫描时：有 `local.md` → 走双向同步逻辑；没有 → 走单向逻辑（MVP 阶段无远端 Skill，所有无 `local.md` 的 Skill 视为只读展示）

**验收标准：**
- 半自动模式下扫描到变化后 UI 正确标记
- 点击同步后 SSOT 和所有目标工具目录内容一致
- content_hash 同步前后一致
- 全自动模式下无需手动操作

### 5.6 Skill 可视化列表

主界面展示所有已发现的 Skill 列表，每个 Skill 显示：名称、描述、来源路径、已安装工具数。每个 Skill 有启用/禁用开关，禁用后不再同步到该工具。列表在扫描完成后自动刷新。

**验收标准：**
- Skill 卡片正确显示 name、description、来源路径
- 启用/禁用切换立即生效
- 扫描完成后列表自动更新

### 5.7 全局与项目两级管理

界面顶部有"全局"和"项目"两个一级 Tab。全局 Tab 管理所有工具的全局 Skill。项目 Tab 左侧显示项目列表（用户手动添加），右侧与全局界面同构。

项目级安装的 Skill 不反向共享到全局或其他项目。MVP 阶段项目级与全局的关联仅体现在工具路径的继承上（§5.1），远端仓库来源的继承在 post-MVP 实现远端拉取后生效。

**验收标准：**
- 全局和项目 Tab 切换正常
- 项目可手动添加/删除
- 项目级 Skill 不影响全局

### 5.8 操作反馈

所有用户操作（扫描、同步、安装、配置）结束后有明确反馈。成功时显示绿色提示（含数量统计，如"扫描完成，发现 3 个 Skill"），失败时显示红色错误详情。反馈停留至少 3 秒或在消息中心可查。

**验收标准：**
- 每次操作都有视觉反馈
- 成功/失败样式明确区分
- 反馈信息包含具体数据（数量、名称等）

## 6. 非功能需求

### 6.1 性能

全量扫描 50 个 Skill（约 10MB）应在 5 秒内完成。哈希计算使用流式处理，避免一次性加载大文件。

### 6.2 稳定性

文件操作失败时不影响其他 Skill 的同步。所有写操作使用临时目录 + rename 原子替换，避免写入中途失败导致数据损坏。

### 6.3 跨平台

MVP 阶段优先支持 Windows 和 macOS，Linux 作为次优先。路径解析需兼容三种系统的格式。

### 6.4 体积

Tauri v2 打包，目标体积 ~5MB，无需额外运行时依赖。

## 7. 同步触发机制

MVP 不含后台进程、文件系统监听或定时任务。同步仅在以下时机触发：

- **打开主界面**：全量扫描（全局 + 所有项目 + 所有工具路径）
- **"检查更新"按钮**：重新扫描所有已配置路径，对比 content_hash 检测变化
- **全局 Tab 手动刷新**：仅扫描全局范围
- **项目 Tab 手动刷新**：仅扫描当前项目
- **安装/卸载/更新操作**：立即同步对应 Skill
- **工具路径变更**：立即触发对应范围扫描

## 8. 技术架构

```
┌─────────────────────────────────────────────────┐
│                   Tauri v2                       │
│  ┌───────────────────────────────────────────┐  │
│  │              React SPA (前端)               │  │
│  │  - Skill 列表展示                          │  │
│  │  - 全局/项目 Tab 切换                      │  │
│  │  - 工具路径配置 UI                         │  │
│  │  - 操作反馈 Toast                          │  │
│  └──────────────────┬────────────────────────┘  │
│                     │ Tauri IPC                  │
│  ┌──────────────────▼────────────────────────┐  │
│  │            Rust Backend (后端)              │  │
│  │  - 目录递归扫描 + SKILL.md 解析            │  │
│  │  - SHA-256 哈希计算                        │  │
│  │  - 文件复制 / 原子替换                     │  │
│  │  - SQLite 读写                             │  │
│  └───────────────────────────────────────────┘  │
│                     │                            │
│  ┌──────────────────▼────────────────────────┐  │
│  │              SQLite 数据库                  │  │
│  │  tools | projects | skills |               │  │
│  │  skill_installations | sync_logs           │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

## 9. 数据模型

详见 `doc/skill-manager-ddl.sql`。核心实体关系：

- `tools` 1:N `skill_installations`：一个工具下可安装多个 Skill
- `skills` 1:N `skill_installations`：一个 Skill 可安装到多个工具
- `projects` 1:N `skill_installations`：通过 project_id=0 标识全局
- `skills` 1:N `sync_logs`：每个 Skill 的同步历史

## 10. 约束条件

1. **同步策略**：默认 Copy，可选 Symlink（try-symlink → catch → fallback copy）
2. **无后台进程**：不含 watcher/cron/polling，仅显式操作触发
3. **标准库文件操作**：不用 shell 命令，用 Rust 标准库 API
4. **不操作 lock 文件**：只复制文件，不管工具的 lock 文件
5. **SKILL.md 为最小发现单位**：找到 SKILL.md 后停止递归，复制整个目录

## 11. MVP 后置功能

以下功能已讨论确认，但不在 MVP 范围内：

- 远端 GitHub 仓库拉取 Skill（CC Switch 方案）
- Remote / Local 分类 Tab
- 公开市场与裸仓库的区分（source_type 扩展）
- 其他代码托管平台适配（RemoteProvider trait）
- 自动检测工作区目录
- 定时检测同步

## 12. 验收检查清单

| # | 功能 | 验收条件 |
|---|------|---------|
| 1 | 工具路径配置 | 5 个预设工具可修改路径，修改后立即触发扫描 |
| 2 | 路径格式容错 | ~/、C:\、/home/ 三种格式均正确解析 |
| 3 | 递归扫描 | 正确发现 SKILL.md，解析 front matter，路径去重 |
| 4 | 数据库 | 5 张表正常工作，重启数据完整，级联删除正确 |
| 5 | 双向同步 | 本地 Skill 变化能同步到 SSOT 并广播到其他工具 |
| 6 | Skill 列表 | 正确展示所有 Skill，启用/禁用开关生效 |
| 7 | 全局/项目 | Tab 切换正常，项目可增删，项目级不影响全局 |
| 8 | 操作反馈 | 每次操作有成功/失败提示，包含具体数据 |
