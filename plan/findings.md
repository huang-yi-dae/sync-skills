# Findings & Decisions: Skill Manager MVP

## Requirements

### 核心需求（来自 map.md 第三节）

- **3.1 工具路径配置**：全局 + 项目两级，修改后立即触发扫描
- **3.2 路径留空推断**：项目路径留空时继承全局路径
- **3.3 路径格式容错**：支持 `~/`、`C:\`、`/home/` 三种格式
- **3.4 递归扫描 SKILL.md**：找到 SKILL.md 后停止递归，目录去重
- **3.5 数据库存储**：5 张表，ID 哈希方案，出厂预设数据
- **3.6 本地 Skill 双向同步**：半自动（默认）/ 全自动两种模式
- **3.7 Skill 可视化列表**：名称 + 描述 + 来源 + 已安装数 + 启用/禁用
- **3.8 操作反馈**：成功绿色 / 失败红色，含数量统计，停留 ≥3s

### 约束条件（来自 map.md 第一节）

- 同步策略：默认 Copy，可选 Symlink（try → catch → fallback）
- 触发时机：无后台进程，仅显式操作触发
- 文件操作：标准库 API，不用 shell 命令，原子替换用 tmp+rename
- 发现机制：递归扫描 SKILL.md，解析 front matter，目录为最小复制单位
- 路径配置：两级（全局 + 项目），项目继承全局可覆盖
- Lock 文件：不操作，不管工具的 lock 文件

### 后置功能（MVP 不含）

- 远端 GitHub 仓库拉取
- Remote / Local 分类 Tab
- 公开市场与裸仓库区分
- 多平台适配（GitLab 等）
- 自动工作区检测
- 定时同步

## Research Findings

### Phase 1 差距分析结果

交叉验证 map.md（6 项约束 + 7 项决策）vs PRD，发现 5 个差距并全部修复：

1. **§5.7 项目级继承描述**：原 PRD 提到"继承全局远程仓库配置"，但 MVP 不含远端拉取。修正为：MVP 阶段项目与全局的关联仅体现在工具路径继承上。
2. **§5.5 SSOT 目录结构**：原描述含混（remote/ 和 local/ 混用）。修正为：MVP 阶段所有 Skill 统一存入 `local/`，`remote/` 预留。
3. **§5.5 缺失机制**：`local.md` 创建时机、Symlink fallback 日志、临时目录命名规范均未说明。补充完整。
4. **§7 触发机制遗漏**：map.md 提到的"检查更新"按钮未列入同步触发入口。补充。
5. **§5.3 边界情况**：隐藏目录跳过、front matter 缺少 name 的 fallback 均未说明。补充。

**覆盖验证：** map.md 第三节 8 项 MVP 需求、第一节 6 项约束条件、DDL 5 张表定义均已在 PRD 中完整覆盖，无遗漏。

### 设计讨论已解决的关键决策

- **公开市场 vs 裸仓库**：MVP 不区分，预留 `source_type: SkillSource` 字段，只留 `Unknown` variant
- **远程 GitHub 拉取流程**：参考 CC Switch 设计，ZIP 下载 → 解压 → 扫描 SKILL.md → 选择性安装。分支回退：请求分支 → main → master。更新检测用 SHA-256 content_hash 对比
- **多平台支持**：MVP 只做 GitHub，预留 `RemoteProvider` trait 抽象
- **Skill 生命周期**：远端单向同步（SSOT → 工具），本地双向同步。用 `local.md` 标记本地 Skill
- **SSOT 存储结构**：`~/.agents/skills/remote/` 和 `~/.agents/skills/local/`，同名冲突加 `-local` 后缀
- **全局 vs 项目级**：项目 Tab 与全局同构，手动添加项目，Skill 来源单向继承（全局 → 项目）
- **Lock 文件**：主流工具（Claude Code、Codex CLI、OpenCode、Gemini CLI、Cline）均不用 lock 文件管理 Skill，启动时扫描目录即可

### 技术栈确认

| 层面 | 选型 | 理由 |
|------|------|------|
| 桌面框架 | Tauri v2 | 体积小（~5MB），原生性能，与 CC Switch 一致 |
| 前端 | React | 与 Next.js 生态共享，技能可迁移 |
| 数据库 | SQLite | 量级够用、零配置、嵌入式 |
| 后端 | Rust | Tauri 自带，处理文件扫描/哈希/DB |
| 打包 | Tauri builder | 输出原生 exe，无 Node 运行时依赖 |

### DDL 分析

- 5 张表：tools, projects, skills, skill_installations, sync_logs
- ID 哈希：SHA-256 前 8 字节，little-endian，有符号 64 位 INTEGER
- 预设数据：Global 项目（id=0）+ 5 个默认工具
- 外键：CASCADE 删除，PRAGMA foreign_keys = ON
- skill_installations 唯一约束 (skill_id, tool_id, project_id)
- sync_logs 记录方向（to_ssot/from_ssot）和状态（success/failed）

## Technical Decisions

| Decision | Rationale |
|----------|-----------|
| Copy 做默认同步策略 | Skill 文件小（20-200KB/个），用空间换稳定性 |
| Symlink 做可选开关 | 省空间，但需开发者模式，让用户自行决定 |
| SHA-256 前 8 字节做 ID | 天然去重，重启后 ID 稳定，无需额外唯一性检查 |
| 无后台进程 | 简化架构，避免 watcher/cron 跨平台复杂度 |
| 标准库文件操作 | 跨平台一致，不依赖外部进程，rename 原子操作 |
| 临时目录 + rename 替换 | 避免写入中途失败导致数据损坏 |
| 5 张表不分 remote/local | MVP 简化，预留扩展点 |
| 全局项目 id=0 | 固定值非哈希，作为特殊内置项目 |
| 项目级不反向共享到全局 | 避免项目间 Skill 泄漏 |

## Issues Encountered

| Issue | Resolution |
|-------|------------|
| （暂无，计划刚启动） | - |

## Resources

- 设计讨论文档：`doc/map.md`
- 数据库 DDL：`doc/skill-manager-ddl.sql`
- PRD 初稿：`doc/PRD.md`
- Tauri v2 官方文档：https://v2.tauri.app/
- Tauri SQLite 插件：https://github.com/tauri-apps/plugins-workspace/tree/v2/packages/sql
- SKILL.md 格式规范：（参考 QoderWork Skills 格式）

## Visual/Browser Findings

（暂无，尚未进行浏览器研究）

---

*Update this file after every 2 view/browser/search operations*
*This prevents visual information from being lost*
