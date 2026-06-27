# Task Plan: Skill Manager MVP PRD 完善与定稿

## Goal

将设计讨论记录（map.md）和数据库 DDL（skill-manager-ddl.sql）转化为一份完整、可执行的 MVP PRD，消除所有模糊点，为后续实现阶段提供明确的验收标准。

## Current Phase

Implementation complete (M1 + M2 + M3 delivered)

## Phases

### Phase 1: 需求验证与差距分析
- [x] 逐条对照 map.md 和 DDL，确认 PRD.md 覆盖所有已确定的约束和决策
- [x] 识别 map.md 中标记"已讨论但后置"的功能，确认 PRD 边界清晰
- [x] 检查 DDL 与 PRD 数据模型描述的一致性
- [x] 列出所有需要进一步澄清的问题（5 个 gap 已全部修复）
- **Status:** complete

### Phase 2: 功能细化与验收标准补全
- [x] 为每个功能需求补充具体的验收用例（输入→预期输出）
- [x] 细化路径解析的边界情况（符号链接、UNC 路径、中文路径等）
- [x] 明确错误处理策略（每个操作的失败场景和恢复方式）
- [x] 补充 UI 交互细节（状态流转、加载态、空态）
- **Status:** complete

### Phase 3: 技术方案评审
- [x] 评审 Tauri v2 + React + SQLite 技术栈的可行性
- [x] 确定 Rust 后端需要暴露的 IPC 接口清单（17 个 Command）
- [x] 评估 SQLite 在 Tauri 中的集成方式（决策：rusqlite）
- [x] 确认文件操作的跨平台实现策略（6 项操作方案）
- **Status:** complete

### Phase 4: 优先级排序与里程碑划分
- [x] 将 MVP 功能按依赖关系排序
- [x] 划分 3 个开发里程碑（M1 基础能力 / M2 核心同步 / M3 完善体验）
- [x] 识别技术风险和关键路径（5 项风险 + 缓解措施）
- [x] 估算工作量（总计 2-4 周）
- **Status:** complete

### Phase 5: PRD 定稿与交付
- [x] 整合所有修订，生成 PRD 终版（18 节 + 3 附录）
- [x] 补充附录（术语表 12 项、参考资料 6 项、变更日志 6 版）
- [x] 创建 PRD 变更日志（附录 C）
- [x] 交付 PRD.md + task_plan.md + findings.md + progress.md
- **Status:** complete

---

## Implementation Phase

### M1: 基础能力 ✅
- [x] Tauri v2 项目脚手架（create-tauri-app + TypeScript）
- [x] Rust 后端模块：models.rs, hash.rs, db.rs, scanner.rs, lib.rs
- [x] SQLite schema 初始化（5 张表 + 种子数据）
- [x] 7 个 IPC 命令：list_tools, add_tool, update_tool_path, delete_tool, list_skills, full_scan, scan_scope
- [x] React 前端：工具列表编辑、扫描按钮、Skill 卡片网格、Toast 通知
- [x] 暗色主题 CSS
- **Commits:** b055e85

### M2: 核心同步 ✅
- [x] sync.rs 文件操作引擎：copy_directory, atomic_replace, symlink_or_copy
- [x] settings.rs JSON 持久化设置（semi-auto/full-auto 模式）
- [x] DB 扩展：toggle_installation, get_active_installations, insert_sync_log, get_sync_logs, list_skills_with_status
- [x] 9 个新 IPC 命令：toggle_skill, sync_skill, sync_all_pending, check_updates, get/update_settings, list/add/delete_projects, get_sync_logs
- [x] SSOT 路径管理（~/.agents/skills/local/）
- [x] local.md 标记机制
- **Commits:** b94d01f

### M2+M3: 前端完整实现 ✅
- [x] TypeScript 类型扩展：SkillView, SyncResult, SkillUpdate, SyncLog, Settings, Project
- [x] Skill 卡片：工具 toggle 开关、同步按钮、安装计数、更新指示器
- [x] Check Updates 按钮 + 哈希比较
- [x] Settings 面板（同步模式、symlink 偏好）
- [x] Global/Projects 双标签导航
- [x] 项目 CRUD（添加/删除项目 + 模态对话框）
- [x] 同步日志查看器（表格 + 方向/状态徽章）
- [x] Skill 搜索过滤
- [x] 工具完整 CRUD（添加/删除/编辑路径 + 路径校验）
- [x] Toast 三级通知、骨架加载态、空状态
- **Commits:** 1bb79d2

### Bug 修复 ✅
- [x] 项目级扫描使用 project_path + project_rel_path（不再复用全局路径）
- [x] Projects tab 移除 Global 按钮
- [x] Tool 添加/编辑路径校验（绝对路径、UNC、拒绝相对路径）
- [x] Tool 删除后 UI 正确刷新（Promise.all + 清除编辑状态）
- **Commits:** 8549439

### UI 重设计 ✅
- [x] frontend-design skill 指导的完整 UI 重设计
- [x] JetBrains Mono + DM Sans 字体组合
- [x] 琥珀色主调 + 青色辅助色配色方案
- [x] 微噪点纹理、玻璃态 Toast、骨架屏动效、卡片悬浮升起
- [x] 模态框动画、自定义滚动条、焦点发光
- [x] 应用品牌标识（⬡ Skill Manager）
- **Commits:** dc67404

## Key Questions

1. ~~PRD 中的"本地 Skill 双向同步"是否需要在 MVP 中完整实现？~~ → **是，完整实现（§5.5）**
2. ~~路径格式容错是否需要支持 UNC 路径？~~ → **是，支持（§14.1）**
3. ~~UI 框架选择？~~ → **待定，实现阶段决定（M3 阶段）**
4. ~~SQLite 集成方式？~~ → **rusqlite（§17.2 决策）**
5. ~~是否需要原型图？~~ → **MVP 不需要，§16 UI 交互规范足够**

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| MVP 不含远端 GitHub 拉取 | 降低复杂度，聚焦本地同步核心价值 |
| 默认 Copy 而非 Symlink | 避开开发者模式、文件系统格式等跨平台坑 |
| 无后台进程 | 简化架构，符合"显式操作触发"的设计哲学 |
| ID 使用哈希而非自增 | 天然去重，重启后 ID 稳定，避免冲突 |
| 5 张表而非更多 | MVP 阶段不分 remote/local，预留扩展点 |

## Errors Encountered

| Error | Attempt | Resolution |
|-------|---------|------------|
| （暂无） | - | - |

## Notes

- 每完成一个 Phase，更新本文件的 Phase Status 和 progress.md
- 重大决策前重读本文件，防止目标漂移
- 所有发现记录到 findings.md，保持 task_plan.md 精简
- PRD.md 已创建在 doc/PRD.md，本计划的目标是完善它
