# Session 记录：Skill 管理工具设计讨论

---

## 一、已确定的约束

> 以下内容在讨论中达成一致，作为后续设计的约束条件。

### 1. 同步策略：默认 Copy，可选 Symlink

- Skill 文件体积很小（一个 Skill 约 20-200KB，50 个约 10MB），用空间换稳定性是划算的交易
- **Copy 做默认**（稳，避开开发者模式、文件系统格式、工具的 symlink bug 等所有坑）
- **Symlink 做可选手动开关**（省空间，由用户自行决定是否启用）
- 如果用户选了 Symlink，则先尝试创建 symlink，失败则回退到 Copy 并打一条日志。**不前置检测**文件系统类型或开发者模式——直接 try-symlink，catch 到错误就走 Copy

### 2. 同步触发时机：无后台常驻检测

- **没有**文件系统监听（watcher/inotify）
- **没有**定时轮询（timer/cron/interval）
- **没有**后台常驻进程
- 只在明确的操作时触发：
  - **打开主界面 → 全量扫描**：全局 + 所有项目 + 所有工具路径，发现变化就同步
  - **全局 Tab 手动刷新**：仅扫描全局范围的 Skill
  - **项目 Tab 手动刷新**：仅扫描对应项目下的 Skill
  - 安装/卸载/更新 → 立即同步对应 Skill
- 添加/删除/修改某个工具的配置（如路径变更）→ 立即同步

### 3. 本地文件操作实现：用标准库，不用系统命令

- **复制目录**：标准库 API 逐个文件拷贝（不走 `cp` / `xcopy` / `Copy-Item`）
- **删除目录**：标准库递归删除（不走 `Remove-Item`）
- **替换（更新用）**：先拷到临时目录（`.tmp-进程ID-纳秒时间戳`），再 `rename` 原子替换
- 理由：跨平台一致、不依赖外部进程、`rename` 是原子操作

### 4. 远程仓库发现机制：递归扫描 SKILL.md

- 对任何仓库都一样：递归扫描目录
- 找到 `SKILL.md` → 解析 front matter 拿 name + description → 算一个 Skill，**停掉本目录的递归**
- **不解析** `marketplace.json` / `plugin-manifest.json` / `registry.json` 等清单文件
- 没有找到 SKILL.md → 继续进子目录找
- 复制的最小单位是**目录**（SKILL.md 所在的整个目录），`references/`、`scripts/` 等附属文件全跟着走

### 5. 每个工具支持自定义路径（两级配置）

- 工具的路径配置分两级：**系统级（全局）** 和 **项目级**
- **系统级路径**：默认部署到各工具的默认目录（如 `~/.claude/skills/`），可 override
- **项目级路径**：默认继承系统级路径做推断；留空就用全局，自己填则按自定义路径处理
- 工具的自定义路径支持增/删/改，变更后立即触发同步

### 6. 不操作 lock 文件

- 同步时只 Copy 文件，不管工具自己的 lock 文件
- 不写 lock 文件、不解析 lock 文件作为同步依据

---

## 二、已讨论并确定的内容

> 以下内容在讨论中提出，已达成确定结论。

### （已解决）公开市场与裸仓库的区分

**结论：MVP 阶段不做区分，预留扩展点。**

具体方案：
- Skill 结构体中预留 `source_type: SkillSource` 字段
- 枚举只留一个 `Unknown` variant，具体来源变体（`Marketplace` / `BareRepo` / `LocalUser` 等）等需要时再设计
- 来源判断机制（如 `.skill-market.yaml` 文件标识或用户手动标记）也在正式设计时再定

### （已解决）远程 GitHub 拉取流程

**结论：参考 CC Switch 实现，MVP 阶段照搬其设计。**

具体方案（CC Switch 已验证的流程）：
- **发现**：`https://github.com/{owner}/{name}/archive/refs/heads/{branch}.zip` 下载 ZIP → 解压到临时目录 → 递归扫描 SKILL.md → 返回 `DiscoverableSkill` 列表
- **分支回退**：请求的 branch → main → master，依次尝试
- **选择性安装**：前端展示 Skill 列表（卡片形式，含名称/描述/来源），用户勾选后调用 `install_skill_unified` 只装选中的单个 Skill
- **更新检测**：`check_skill_updates` 重新下载 ZIP → 扫描 → 用 SHA-256 计算 `content_hash`（递归遍历目录下所有非隐藏文件，按相对路径字典序排列，`"相对路径\0内容\0"` 依次喂给 SHA-256）→ 与 DB 中记录的 hash 对比
- **更新触发**：无后台检测，仅用户手动点击"检查更新"
- **仓库管理**：`SkillRepo { owner, name, branch, enabled }`，内置默认列表，用户可增删启用/禁用

**与用户设想的对照**：
| 用户设想 | CC Switch 做法 |
|---------|----------------|
| 输入 URL → 扫描出 Skill 列表 | ✅ 一样 |
| 选择性勾选安装 | ✅ 一样 |
| 安装到 SSOT → 同步到各工具 | ✅ 一样 |
| 打开检查 / 定时 / 手动检查更新 | ✅ 有打开检查和手动，暂没有定时 |
| 管理界面有更新按钮 | ✅ 有 |

### （已解决）拓展到其他代码托管平台

**结论：MVP 阶段只做 GitHub，预留 adapter 抽象层。**

设计方案：
- 定义 `RemoteProvider` trait / 接口，GitHub 作为第一个实现
- 每个平台一个独立的 adapter 文件（如 `providers/github.rs`、`providers/gitlab.rs`），对外暴露统一格式的结果
- 类似 CC Switch 中 API 供应商的设计思路——每个供应商一个 adapter，上层调用方无需感知具体平台的差异
- 国内平台的适配在 MVP 之后按需添加

### （已解决）Skill 完整生命周期管理

**结论：分为远端 Skill 和本地自写 Skill 两条路径，处理方式不同。**

#### 远端 Skill（从 GitHub 等仓库拉取）
- **单向同步**：SSOT → 各工具，不反向
- 用户在各工具目录下修改远端 Skill → 工具检测到变化，但不会往回同步
- 更新时重新从远端下载 → 覆盖 SSOT → 覆盖各工具

#### 本地自写 Skill
- **双向同步**：任一工具改了 → 推回 SSOT → 广播到其他启用该 Skill 的工具
- 检测方式：本地扫描时检查哈希变化，发现变化则同步
- 触发时机：无后台检测，集成在「检查更新」按钮中，按钮同时检查远端和本地
- 冲突处理：本地自写 Skill 不存在多人同时修改的场景，以最后修改为准

#### 本地 Skill 的标识机制：`local.md`
- 工具自动管理 `local.md` 文件，用户不需要感知
- 检测逻辑：检查 Skill 目录下是否存在 `local.md`
- 创建本地 Skill 时 → 工具自动生成 `local.md`
- 从非远端目录导入 Skill → 工具自动补 `local.md`
- 扫描时：有 `local.md` → 双向同步逻辑；没有 → 单向逻辑

#### SSOT 存储结构（防同名冲突）
```
~/.agents/skills/
  ├── remote/<skill-name>/    ← 远端拉来的
  └── local/<skill-name>/     ← 本地自写的
```
同步到 Agent 工具目录时保持扁平结构，同名冲突加 `-local` 后缀：
```
~/.claude/skills/
  ├── my-skill/               ← 远端版本
  └── my-skill-local/         ← 本地版本
```
同名冲突的具体处理方案留到实际出现时再细化。

### （已解决）全局 vs 项目级 Skill

**结论：增加「项目」Tab，界面与全局同构，项目列表用户手动管理，Skill 来源单向继承。**

#### UI 布局
```
┌──────────────────────────────────────────┐
│  [全局]  [项目]                          │  ← 顶部一级 Tab
│                                          │
│  ┌────────┐  ┌────────────────────────┐  │
│  │ 项目1   │  │                        │  │
│  │ 项目2   │  │   和全局完全一样的     │  │
│  │ 项目3   │  │   界面布局             │  │
│  │         │  │   远程 | 本地 | 设置   │  │
│  │  + 添加 │  │                        │  │
│  └────────┘  └────────────────────────┘  │
│       ↑               ↑                  │
│  左侧项目列表       主内容区              │
│  （点哪个显示哪个）  和全局同构            │
└──────────────────────────────────────────┘
```

#### 项目列表管理
- **MVP 阶段**：用户手动添加项目（指定项目路径即可）
- **预留拓展**：未来可自动检测 CodeX、Claude Code 等工具的工作区目录
- 左侧「+ 添加」按钮手动添加

#### 项目级 Skill 的存储位置
```
<project-path>/.agents/skills/
  ├── remote/<skill-name>/    ← 远端拉来的
  └── local/<skill-name>/     ← 本地自写的
```
同步到 Agent 工具时依然保持 Agent 工具路径下的扁平结构。

#### Skill 来源继承规则
- **单向继承**：项目级可以继承全局的远程仓库来源配置
- **项目级不会反向共享到全局**（项目级安装的 Skill 只在项目内可见）
- 即：全局配置了哪些 GitHub 仓库 → 所有项目默认可用；项目自己装的 Skill → 不影响其他项目也不影响全局

#### 检测触发
- 无论用户切换到全局还是某个项目的页面，**所有（全局 + 所有已添加的项目）全部执行检测**
- 检测内容：远端是否有更新 + 本地目录是否有变化

### （已解决）跨工具 lock 文件管理

**结论：主流工具均不使用 lock 文件来管理 Skill，本工具也无需维护。自己用数据库记录安装状态和哈希即可。**

对于主流 AI 编码工具（Claude Code、Codex CLI、OpenCode、Gemini CLI 等），Skill 的发现机制都是**启动时扫描 SKILL.md 目录**，不存在 lock 文件或注册表：
- Claude Code：扫描 `~/.claude/skills/` + `.claude/skills/`
- Codex CLI：扫描 `~/.codex/skills/` + `.agents/skills/`
- 其他工具同理

**唯一的例外**是 `~/.agents/.skill-lock.json`，但它只作为来源信息的元数据文件存在，工具不依赖它来决定是否加载 Skill。

**对 MVP 的影响：**
- Skill 放在正确目录就能用，lock 文件不存在不影响任何功能
- CC Switch 用自己维护的 SQLite 数据库记录安装状态和 `content_hash` 就足够了
- 无需往工具目录里写 lock 文件
- lock 文件只在更新检测时需要（通过数据库中的 hash 判断本地是否和远端一致）

### （已解决）发现源的管理

**结论：直接沿用 CC Switch 的设计。**

具体方案：
- **内置 4 个默认 GitHub 仓库**（如 anthropics/skills 等），预置在代码中
- **仓库管理**：用户在 UI 上可「增 / 删 / 启用 / 禁用」仓库
- **数据结构**：
  ```rust
  SkillRepo {
      owner: String,    // GitHub 用户/组织名
      name: String,     // 仓库名称
      branch: String,   // 分支 (默认 "main")
      enabled: bool,    // 是否启用
  }
  ```
- **禁用的仓库**：不会被扫描或检查更新
- **仓库增删改后**：只需在下次「检查更新」时重新扫描即可，无需立即重建索引

---

## 三、MVP 实现要求

> 以下为第一阶段的 MVP 功能清单及验收标准。完成即通过，不设中间档。

### 3.1 工具路径配置（全局 + 项目）

用户可手动添加/删除/修改 AI 编码工具的 Skill 目录路径。全局路径默认预设各工具的常见路径（如 `~/.claude/skills/`、`~/.codex/skills/`），每个工具至少一条默认。路径修改后立即触发对应范围的扫描。

### 3.2 路径留空自动推断

项目级路径留空时，自动继承全局路径进行推断。用户填写项目路径后，以此为准覆盖全局推断。

### 3.3 路径格式容错

以下三种格式均能正确解析为绝对路径使用：`~/.claude/skills/`、`C:\Users\xx\.claude\skills\`、`/home/xx/.claude/skills/`。路径末尾有无斜杠均可。路径不存在时给出明确提示而非崩溃。

### 3.4 递归扫描 SKILL.md

对每个配置的工具路径递归扫描；找到 `SKILL.md` 后成功解析 front matter（name + description），SKILL.md 所在的整个目录作为一个 Skill 识别；没找到 SKILL.md 的目录继续往下递归；同一路径下重复扫描时，已存在的 Skill 不产生重复记录（用路径去重）。

### 3.5 数据库存储

数据库至少包含以下 5 张表。ID 优先使用哈希值，映射表用自增。重启后数据完整可读。

**ID 哈希方案：** 取字符串的 SHA-256，取前 8 字节，按 little-endian 解析为有符号 64 位 INTEGER。详见 `D:\Develop\Wiki\skill-manager-ddl.sql`。

**出厂预设数据：**
- `projects` 表内置全局项目（id=0，name="Global"，path="~/.agents/skills/"）
- `tools` 表预设 5 个工具的默认路径：
  - Claude Code → `~/.claude/skills/` / `.claude/skills/`
  - Codex CLI → `~/.codex/skills/` / `.codex/skills/`
  - OpenCode → `~/.opencode/skills/` / `.opencode/skills/`
  - Gemini CLI → `~/.gemini/skills/` / `.gemini/skills/`
  - Cline → `~/.cline/skills/` / `.cline/skills/`

#### tools — 工具注册表

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | 工具名称的哈希值 |
| `name` | TEXT | 用户输入的工具名，如 "Claude Code" |
| `global_path` | TEXT | 系统级默认路径，如 `~/.claude/skills/` |
| `project_rel_path` | TEXT | 项目相对路径，如 `.claude/skills/`（用于推断项目路径） |
| `created_at` | TEXT | 创建时间 |
| `updated_at` | TEXT | 更新时间 |

路径推断规则：
- 全局路径 → 直接用 `global_path`
- 项目路径 → `<project.path> / <tool.project_rel_path>`

#### projects — 用户添加的项目

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | 项目绝对路径的哈希值 |
| `name` | TEXT | 用户起的别名 |
| `path` | TEXT UNIQUE | 项目根目录绝对路径，重复时弹窗拒绝 |

**全局作为一个特殊的内置项目**存在，id 为固定值（如 0），name="Global"，path 指向系统级 SSOT 目录 `~/.agents/skills/`。

#### skills — Skill 元数据

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | 路径哈希（source_path 的哈希值） |
| `name` | TEXT | SKILL.md front matter 的 name |
| `description` | TEXT | SKILL.md front matter 的 description |
| `source_path` | TEXT | SSOT 目录下的绝对路径 |
| `content_hash` | TEXT | 目录全量文件的 SHA-256，用于检测内容变化 |
| `created_at` | TEXT | 创建时间 |
| `updated_at` | TEXT | 更新时间 |

MVP 阶段不分 remote / local，所有 Skill 一视同仁。

#### skill_installations — Skill 与工具安装关系

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | 自增 |
| `skill_id` | INTEGER FK | → skills.id |
| `tool_id` | INTEGER FK | → tools.id |
| `project_id` | INTEGER FK | → projects.id；全局项目用固定 id（如 0） |
| `status` | TEXT | `active` / `disabled` |
| `synced_at` | TEXT | 最后一次同步时间 |

唯一约束 `(skill_id, tool_id, project_id)`。全局和项目共用同一张表，通过 `project_id` 区分。

#### sync_logs — 同步操作日志

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | 自增 |
| `skill_id` | INTEGER FK | → skills.id |
| `tool_id` | INTEGER FK | → tools.id |
| `project_id` | INTEGER FK | → projects.id |
| `direction` | TEXT | `to_ssot` / `from_ssot` |
| `status` | TEXT | `success` / `failed` |
| `error_message` | TEXT | 失败时记录原因（可为 null） |
| `created_at` | TEXT | 同步发生时间 |

每次实际的同步操作记录一条，用于追溯同步链路和排查失败原因。

**DDL 文件：** `D:\Develop\Wiki\skill-manager-ddl.sql` — 包含完整的建表 SQL（含索引、外键、CHECK 约束）及 seed data INSERT 语句。

### 3.6 本地 Skill 双向传播

本地自写 Skill（有 `local.md` 标记的）支持双向同步。默认半自动模式：扫描到变化后做标记 → 用户点击「同步」按钮后执行：①从改动目录复制到 SSOT（`local/<skill-name>/`）→ ②从 SSOT 复制到其他所有启用该 Skill 的工具目录。设置中可切换为全自动模式（扫到变化后自动执行）。同步前后用 `content_hash` 确认一致性。同名本地/远端 Skill 在工具目录用 `-local` 后缀区分。

### 3.7 Skill 可视化列表

主界面展示本地 Skill 列表（名称 + 描述 + 来源 + 已安装工具数）。每个 Skill 有启用/禁用开关（禁用后不再同步到该工具）。列表在扫描完成后自动刷新。

### 3.8 操作反馈

所有用户操作（扫描、同步、安装、配置）结束后都有明确反馈：成功有绿色提示（含数量统计，如"扫描完成，发现 3 个 Skill"），失败有红色错误详情（含错误码或错误信息）。反馈停留时间至少 3 秒或在消息中心可查。

---

> **MVP 阶段不包含的功能（已讨论但后置）：**
> - 远端 GitHub 仓库拉取 Skill
> - Remote / Local 分类 Tab
> - 公开市场与裸仓库的区分
> - 其他代码托管平台适配
> - 自动检测工作区目录
> - 定时检测同步

---

## 四、技术选型

**方案：Tauri v2 + React（SPA）+ SQLite**

| 层面 | 选型 | 理由 |
|------|------|------|
| 桌面框架 | **Tauri v2** | 和 CC Switch 一致；打包体积小（~5MB）；原生性能 |
| 前端 | **React** | 与你正在学的 Next.js 共享 React 生态（组件、状态管理、hooks），技能直接迁移 |
| 数据库 | **SQLite** | 量级够用、零配置、嵌入式、进程内读写 |
| 后端 | **Rust** | Tauri 自带 Rust 后端，处理文件扫描、哈希计算、SQLite 读写等底层操作 |
| 打包 | Tauri builder | 输出原生 exe，无需 Node 运行时依赖 |
