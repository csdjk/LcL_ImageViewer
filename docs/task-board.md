# LcL ImageViewer — 主任务板

<!-- project-stage: P1 -->

> 当前阶段：**P1 ACTIVE**。用户于 2026-09-14 明确将 UI 方向改为浅色新拟态；`IV-P1-N01` 已完成并经 main 复验；当前唯一可领取任务为 `IV-P1-01`，继续完善完整验收流程。

## 执行规则

状态：`BACKLOG / READY / IN_PROGRESS / REVIEW / BLOCKED / DONE`。

- 只有 `READY` 可以领取；领取后填写唯一 Owner、branch、worktree 和 Base Commit。
- Worker 遵守 Allowed Paths，不直接修改本文、`status.md` 或 `roadmap.md`。
- 任务分支验收通过后进入 `REVIEW`；合入 `main` 并在 main 复验后才进入 `DONE`。
- 重大产品、数据或架构问题写入 `needs-decision.md`，关联 Task 并写清恢复条件。
- push、Release、安装包分发和注册表修改必须有明确用户授权。

## 现有实现基线

玻璃 UI 的主体实现已经以 `cbffda3`、`f3b90b2`、`561734a`、`162ee27` 进入本地 `main`。这些提交是接入工作流前的既有事实，不回填为虚假的 DONE Task；P1 的任务负责补齐可重复验收、修复验收缺陷并完成阶段门禁。

## 当前任务

| ID | Task | Status | Owner | Depends On | Allowed Paths | Acceptance |
|---|---|---|---|---|---|---|
| IV-P1-N01 | 浅色单色系新拟态 UI | DONE | ChatGPT-AgentDock | 用户最新授权 / P0 VALIDATED | `crates/iv-viewer/src/{ui.rs,app.rs}`, `tools/ui-qa/**`, `docs/ui-qa/**`, `docs/新拟态UI规范.md`, `README.md` | 12–16pt 圆角；双主题单色系；多层双向柔影；凸起/凹陷、禁用/焦点；图片功能不变；真实双主题双尺寸/关键交互截图；tests/check/release build PASS |
| IV-P1-01 | 建立可重复的 Windows UI 截图验收工具链 | READY | — | P0 VALIDATED | `tools/ui-qa/**`, `docs/ui-qa/**`; 运行产物写入忽略目录 | 动态发现窗口；记录 commit/输入/主题/状态/逻辑与物理客户区/DPI；可重复捕获至少 880×560 与 1280×860；无固定 HWND/旧机器路径；workspace tests PASS |
| IV-P1-02 | 补齐新拟态 UI 视觉/交互矩阵并闭环范围内缺陷 | BACKLOG | — | IV-P1-01 | `crates/iv-viewer/src/{app.rs,ui.rs,render.rs,image.wgsl,backdrop.rs}`, `docs/ui-qa/**` | 深浅主题、两种视口、关键交互/弹层/长文本有实际截图；对比度与裁切有记录；发现的本轮缺陷修复后重截；workspace tests PASS |
| IV-P1-03 | 验证桌面捕获恢复与性能预算 | BACKLOG | — | IV-P1-01 | `crates/iv-viewer/src/{app.rs,backdrop.rs,render.rs}`, `tools/ui-qa/**`, `docs/ui-qa/**` | 拖动、最小化/恢复、捕获失败可观察且可恢复；性能注明环境、样本数、缓存边界、中位数/P95；无证据不宣称达到 GPU ≤2ms |
| IV-P1-04 | P1 集成与 release-candidate 门禁 | BACKLOG | — | IV-P1-02, IV-P1-03 | integration/build checks, `README.md`, `docs/**` | 所有 P1 验收在 main 复验；release workspace build PASS；格式基线有明确处置；status/roadmap 同步；不执行发布或 push |

## Coordinator NEXT

```text
NOW: IV-P1-01 READY
COMPLETED: IV-P1-N01 DONE
BLOCKED BY DEPENDENCY: IV-P1-02, IV-P1-03, IV-P1-04
```

领取 `IV-P1-01` 时填写：

```text
Owner: <AGENT_ID>
Branch: codex/iv-p1-01-ui-qa-harness
Worktree: <ABSOLUTE_WORKTREE_PATH>
Base Commit: <CURRENT_MAIN_COMMIT>
```

## IV-P1-N01 领取记录

Owner: ChatGPT-AgentDock

Branch: `codex/iv-p1-n01-neumorphic`

Worktree: `Temp/worktrees/neumorphic`（相对主工作区）

Base Commit: `79e3a589857ee61dbc0ce68361a4e4883382c8df`

## IV-P1-N01 Review

Worker HEAD: `7343033`；功能源码 `62b5163`。15 tests、check、release build PASS；50 张真实桌面原始截图覆盖双主题/双尺寸及关键状态，固定图片内部 ROI 和工具栏恢复比较通过。所有改动符合 Allowed Paths。main 集成后仍须重建和复验；完整多 DPI/性能门禁保留。

## IV-P1-N01 完成记录

集成提交：`635b7dcc195c5c220882cb9589b3762ebc124eff`。main tests15/check/release PASS；main Release22张截图复验和配置恢复通过。工作分支50张截图、图片ROI与工具栏恢复比较见验收文档。P1保持ACTIVE。分支与工作树保留，无push/发布；当前NEXT为IV-P1-01。
