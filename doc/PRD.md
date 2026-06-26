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

## 13. 验收用例

### 13.1 工具路径配置

| 输入 | 预期输出 |
|------|---------|
| 修改 Claude Code 全局路径为 `~/my-skills/claude/` | 路径保存成功，自动触发扫描该目录 |
| 将 Codex CLI 项目路径留空 | 推断为 `<项目根>/.codex/skills/` |
| 将 OpenCode 项目路径填为 `.custom/skills/` | 使用自定义路径覆盖推断 |
| 删除 Gemini CLI 工具 | 对应 skill_installations 和 sync_logs 级联删除 |

### 13.2 路径格式容错

| 输入 | 预期输出 |
|------|---------|
| `~/.claude/skills/` | 展开为 `C:\Users\xxx\.claude\skills\`（Windows）或 `/home/xxx/.claude/skills/`（Linux） |
| `C:\Users\xx\.claude\skills` （无尾斜杠） | 自动补尾斜杠，正常使用 |
| `/home/xx/.claude/skills/` （Windows 上输入 Unix 路径） | 检测到路径不存在，显示"路径不存在"提示 |
| `\\server\share\skills\` （UNC 路径） | 正常解析使用 |
| `./relative/path` | 拒绝，提示"请输入绝对路径" |

### 13.3 递归扫描

| 输入场景 | 预期输出 |
|---------|---------|
| `~/.claude/skills/` 下有 `skill-a/SKILL.md` | 发现 skill-a，name 取 front matter |
| `skills/nested/deep/skill-b/SKILL.md` | 递归找到 skill-b |
| `skills/.hidden/SKILL.md` | 跳过，不扫描隐藏目录 |
| `skills/broken/SKILL.md`（front matter 格式错误） | 跳过，记日志"解析失败" |
| `skills/no-name/SKILL.md`（front matter 缺 name） | 以目录名 `no-name` 作为 name |
| 同一目录扫描第二次 | 不产生重复记录（source_path 去重） |
| `skills/skill-a/sub/SKILL.md`（嵌套 SKILL.md） | 只识别 skill-a（父目录命中后停止递归） |

### 13.4 本地 Skill 双向同步

| 输入场景 | 预期输出 |
|---------|---------|
| 扫描发现 tool-A 下的 skill-x content_hash 变了 | UI 标记 skill-x "有更新" |
| 半自动模式下点击"同步" skill-x | 复制到 SSOT `local/skill-x/` → 广播到其他启用工具 |
| 同步完成后检查各目录 content_hash | 全部一致 |
| 全自动模式下扫描到变化 | 自动执行同步，无需手动操作 |
| 同步过程中目标目录写入失败 | 该 Skill 标记失败，其他 Skill 不受影响，sync_logs 记一条 failed |

### 13.5 全局与项目管理

| 输入场景 | 预期输出 |
|---------|---------|
| 添加项目路径 `D:\my-project` | 项目列表新增一项，扫描该项目下的工具路径 |
| 添加已存在的项目路径 | 弹窗拒绝"项目已存在" |
| 删除项目 | 对应 skill_installations 和 sync_logs 级联删除 |
| 项目 Tab 中安装 Skill | 仅该项目可见，不影响全局和其他项目 |

## 14. 路径解析边界情况

### 14.1 支持的格式

| 格式 | 示例 | 处理方式 |
|------|------|---------|
| Tilde 展开 | `~/.claude/skills/` | 替换 `~` 为 `$HOME` / `$USERPROFILE` |
| Windows 绝对路径 | `C:\Users\xx\skills\` | 直接使用 |
| Unix 绝对路径 | `/home/xx/skills/` | 直接使用 |
| UNC 路径 | `\\server\share\skills\` | 直接使用（Windows only） |
| 无尾斜杠 | `~/.claude/skills` | 自动补 `/` 或 `\` |

### 14.2 不支持的格式

| 格式 | 处理方式 |
|------|---------|
| 相对路径 `./skills/` | 拒绝，提示"请输入绝对路径" |
| 环境变量 `%APPDATA%\skills\` | MVP 不展开，提示"不支持环境变量，请输入完整路径" |
| 中文/Unicode 路径 `C:\用户\技能\` | 正常支持（Rust String 为 UTF-8） |

### 14.3 符号链接

扫描时**跟随**符号链接（Rust `fs::read_dir` 默认行为），但不将符号链接本身作为 Skill 的 source_path。如果 Skill 目录是符号链接，SSOT 存储使用解析后的真实路径。

## 15. 错误处理策略

### 15.1 文件操作错误

| 操作 | 失败场景 | 处理方式 |
|------|---------|---------|
| 扫描目录 | 目录不存在 | 跳过该工具路径，UI 显示"路径不存在: xxx" |
| 扫描目录 | 权限不足 | 跳过该目录，UI 显示"权限不足: xxx" |
| 读取 SKILL.md | 文件不存在 | 跳过（理论上不会发生，scan 已确认存在） |
| 解析 front matter | YAML 格式错误 | 跳过该 Skill，记 sync_log(failed) |
| 复制到 SSOT | 磁盘空间不足 | 回滚（删除临时目录），UI 显示"磁盘空间不足" |
| 复制到 SSOT | 目标目录锁定 | 回滚，UI 显示"目标文件被占用，请关闭相关程序后重试" |
| rename 原子替换 | 跨卷操作 | 回退为 copy + delete（非原子），失败则回滚 |
| 创建符号链接 | 开发者模式未启用 | 回退到 Copy，记日志"symlink 失败，回退到 copy" |

### 15.2 数据库错误

| 失败场景 | 处理方式 |
|---------|---------|
| 数据库文件损坏 | 启动时检测，提示"数据库损坏，是否重置？"，重置后重新扫描 |
| ID 哈希冲突 | 极小概率事件，冲突时在 UI 显示警告，保留旧记录 |
| 写入失败 | 事务回滚，UI 显示错误信息，不破坏现有数据 |

### 15.3 错误恢复原则

- **单个 Skill 失败不影响其他 Skill**：批量操作中跳过失败项，继续处理剩余项
- **所有写操作可回滚**：先写临时目录，rename 成功后才算完成，失败则删除临时目录
- **错误信息具体可操作**：不显示"未知错误"，而是给出具体原因和建议操作

## 16. UI 交互规范

### 16.1 状态流转

```
应用启动
  │
  ├── 数据库不存在 → 初始化 DB + seed data → 首次全量扫描
  │
  └── 数据库存在 → 加载数据 → 全量扫描（检测变化）
                        │
                        ├── 无变化 → 显示当前 Skill 列表
                        │
                        └── 有变化 → 标记"有更新" → 等待用户操作
                                          │
                                          ├── 点击"同步" → 同步 → 更新列表
                                          └── 忽略 → 保持标记
```

### 16.2 加载态

| 场景 | 展示 |
|------|------|
| 全量扫描中 | Skill 列表区域显示骨架屏（Skeleton），顶部进度条 |
| 同步单个 Skill | 该 Skill 卡片显示旋转加载图标 |
| 添加项目 | 按钮显示 loading 态，禁止重复点击 |

### 16.3 空态

| 场景 | 展示 |
|------|------|
| 首次启动无 Skill | 居中插图 + "尚未发现 Skill，请配置工具路径后扫描" |
| 项目下无 Skill | "该项目下暂无 Skill" + 扫描按钮 |
| 搜索无结果 | "未找到匹配的 Skill" |

### 16.4 操作反馈

| 操作 | 成功反馈 | 失败反馈 |
|------|---------|---------|
| 扫描完成 | Toast: "扫描完成，发现 N 个 Skill" | Toast: "扫描失败: [具体原因]" |
| 同步完成 | Toast: "同步成功，N 个 Skill 已更新" | Toast: "同步失败: [具体原因]" |
| 启用/禁用 | Toggle 即时切换，无 Toast | - |
| 添加项目 | Toast: "项目已添加" | Dialog: "路径无效/已存在" |
| 修改路径 | Toast: "路径已更新，正在重新扫描" | Toast: "路径无效: [原因]" |

Toast 显示时间 ≥ 3 秒，支持手动关闭。

## 17. 技术方案评审

### 17.1 技术栈可行性评估

**Tauri v2 + React + SQLite** 组合成熟度足够：

- **Tauri v2**：2024 年 10 月发布 v2 正式版，支持 Windows/macOS/Linux/移动端。Rust 后端提供原生文件系统和进程操作能力，打包体积 ~5MB。
- **React**：与 Tauri v2 的前端绑定无关（任何 JS 框架均可），React 生态成熟，组件库丰富。
- **SQLite**：Tauri 生态有两种集成路径（见 §17.2），无论哪种都经过社区验证。
- **风险点**：Tauri v2 的 Rust 异步运行时（tokio）与 SQLite 的同步 API 需要适配，rusqlite 需用 `spawn_blocking` 包裹。

### 17.2 SQLite 集成方案对比

| 维度 | tauri-plugin-sql | rusqlite (Rust 直连) |
|------|-----------------|---------------------|
| 访问方式 | 前端 JS 直接调 SQL | Rust 后端操作，通过 Tauri Command 暴露给前端 |
| 异步支持 | 原生 async/await | 需 `spawn_blocking` 包裹同步 API |
| 事务控制 | 有限（plugin 层面封装） | 完全控制（BEGIN/COMMIT/ROLLBACK） |
| Migration | 内置 Migration 机制 | 手动管理或自建 |
| 适合场景 | 简单 CRUD、前端直连 | 复杂查询、批量操作、扫描后批量写入 |
| 本项目适用性 | ⚠️ 扫描+哈希+复制+DB写入在 Rust 侧完成，前端直连 SQL 反而增加复杂度 | ✅ Rust 后端统一处理文件操作和数据库写入，事务控制更灵活 |

**决策：采用 rusqlite**。理由：本项目的核心逻辑（目录扫描、哈希计算、文件复制、DB 写入）全部在 Rust 后端完成，使用 rusqlite 可以直接在同一个函数中完成"扫描→写 DB"的事务性操作，无需跨进程通信。前端只通过 Tauri Command 获取处理后的结果。

### 17.3 Tauri IPC 接口清单

Rust 后端通过 `#[tauri::command]` 暴露以下接口给 React 前端：

#### 工具管理

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `list_tools` | - | `Vec<Tool>` | 获取所有工具列表 |
| `update_tool_path` | `tool_id, global_path, project_rel_path` | `Result<()>` | 修改工具路径，触发重新扫描 |
| `add_tool` | `name, global_path, project_rel_path` | `Result<Tool>` | 添加自定义工具 |
| `delete_tool` | `tool_id` | `Result<()>` | 删除工具（级联删除） |

#### 项目管理

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `list_projects` | - | `Vec<Project>` | 获取所有项目 |
| `add_project` | `name, path` | `Result<Project>` | 添加项目，触发扫描 |
| `delete_project` | `project_id` | `Result<()>` | 删除项目（级联删除） |

#### Skill 管理

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `list_skills` | `project_id?` | `Vec<SkillView>` | 获取 Skill 列表（含安装状态） |
| `toggle_skill` | `skill_id, tool_id, project_id, active` | `Result<()>` | 启用/禁用 Skill |
| `sync_skill` | `skill_id` | `Result<SyncResult>` | 手动同步单个 Skill |
| `check_updates` | `project_id?` | `Vec<SkillUpdate>` | 检查所有 Skill 更新 |

#### 扫描与同步

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `full_scan` | - | `ScanResult` | 全量扫描（打开主界面时调用） |
| `scan_scope` | `project_id?, tool_id?` | `ScanResult` | 按范围扫描（刷新按钮） |
| `sync_all_pending` | - | `Vec<SyncResult>` | 同步所有待同步的 Skill |

#### 同步日志

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `get_sync_logs` | `skill_id?, limit?` | `Vec<SyncLog>` | 查询同步日志 |

#### 设置

| Command | 参数 | 返回 | 说明 |
|---------|------|------|------|
| `get_settings` | - | `Settings` | 获取应用设置 |
| `update_settings` | `Settings` | `Result<()>` | 更新设置（含同步模式切换） |

### 17.4 跨平台文件操作策略

| 操作 | Rust 实现 | 跨平台注意 |
|------|----------|-----------|
| 递归扫描 | `std::fs::read_dir` + 递归 | 隐藏文件判断：Unix 检查 `.` 前缀，Windows 检查 `FILE_ATTRIBUTE_HIDDEN` |
| 路径展开 `~` | `dirs::home_dir()` crate | 三平台均支持 |
| 复制目录 | `std::fs::copy` 逐文件 | 保持权限位（Unix `chmod`），Windows 忽略 |
| 原子替换 | 写临时目录 → `std::fs::rename` | Windows 上 rename 跨卷失败 → 回退 copy+delete |
| Symlink | `std::os::windows::fs::symlink_dir` / `std::os::unix::fs::symlink` | Windows 需开发者模式或管理员权限，失败回退 copy |
| 路径规范化 | 统一用 `PathBuf`，输出时按平台转分隔符 | - |

### 17.5 关键依赖

| Crate | 用途 | 版本要求 |
|-------|------|---------|
| `tauri` | 桌面框架 | ^2.0 |
| `rusqlite` | SQLite 操作 | ^0.31（bundled feature） |
| `serde` / `serde_json` | 序列化 | ^1.0 |
| `sha2` | SHA-256 哈希 | ^0.10 |
| `dirs` | 平台目录（home, config 等） | ^5.0 |
| `yaml-front-matter` | SKILL.md front matter 解析 | ^0.1 |
| `tokio` | 异步运行时（Tauri 自带） | ^1.0 |
| `log` / `env_logger` | 日志 | ^0.4 |
