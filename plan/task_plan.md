# Task Plan: Skill Manager MVP PRD 完善与定稿

## Goal

将设计讨论记录（map.md）和数据库 DDL（skill-manager-ddl.sql）转化为一份完整、可执行的 MVP PRD，消除所有模糊点，为后续实现阶段提供明确的验收标准。

## Current Phase

Phase 4

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
- [ ] 将 MVP 功能按依赖关系排序
- [ ] 划分 2-3 个开发里程碑（每个里程碑可独立交付验证）
- [ ] 识别技术风险和关键路径
- [ ] 估算工作量（粗略 T-shirt sizing）
- **Status:** pending

### Phase 5: PRD 定稿与交付
- [ ] 整合所有修订，生成 PRD 终版
- [ ] 补充附录（术语表、参考资料、设计决策记录）
- [ ] 创建 PRD 变更日志
- [ ] 交付 PRD.md + task_plan.md + findings.md + progress.md
- **Status:** pending

## Key Questions

1. PRD 中的"本地 Skill 双向同步"是否需要在 MVP 中完整实现，还是可以先做单向同步？
2. 路径格式容错是否需要支持符号链接和 UNC 路径（`\\server\share\...`）？
3. UI 框架选择：纯 React 还是需要引入 UI 组件库（shadcn/ui、Ant Design）？
4. SQLite 集成方式：tauri-plugin-sql（异步、开箱即用）还是 rusqlite（更灵活、需手动管理）？
5. 是否需要为 PRD 添加原型图或线框图？

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
