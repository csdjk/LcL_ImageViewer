# LcL ImageViewer — 当前状态

> 更新时间：2026-09-14。本文是当前阶段和唯一 NEXT 的权威来源。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-N01 -->

## 当前阶段

```text
P0 Viewer Foundation      VALIDATED
P1 Glass UI Stabilization ACTIVE
P2 Release Hardening      GATED BY P1 VALIDATED
```

## 本轮用户授权

2026-09-14 用户明确要求：浅色粉彩、单色系明暗变化、12–16px 圆角、多层柔和阴影、无硬边框、可交互元素具有凸起/凹陷效果。本轮新拟态规范优先于旧玻璃视觉方案；保留图像功能、已有显式偏好和历史本地提交。

## 当前真实状态

- 玻璃 UI 实现与测试基线为 `162ee27fb712479a3474ba04ad561bc24ea6a899`；接入工作流时 `main` 工作区干净。当前 HEAD 应始终通过 Git 读取，不在本文自引用工作流配置提交的 hash。
- `main` 包含 4 个尚未 push 的本地功能提交：`cbffda3`、`f3b90b2`、`561734a`、`162ee27`。它们实现了玻璃界面、诊断、高斯模糊、窗口拖拽与工具栏自动隐藏调整；本次另增 1 个工作流配置提交。
- 2026-09-14 在当前 HEAD 执行 `cargo test --workspace`：PASS，共 10 个测试通过、0 失败。
- `cargo fmt --all -- --check`：FAIL，发现 `iv-core`、`iv-shell` 和 `winassoc.rs` 等既有格式差异；本次没有自动格式化或修改 Rust 源码。
- `ui-verify-shots/` 中存在当前本机截图，但缺少统一的 commit、DPI、逻辑/物理客户区、输入、状态和比较元数据，不能据此认定 P1 已完成视觉验收。
- `docs/玻璃磨砂UI优化方案.md` 是基于 `b99efcc` 的原始方案与验收规范；方案之后已有实现提交，当前完成度以本文和 Git 为准。

## 当前 NEXT

```text
NOW: IV-P1-N01 READY — 按用户授权实现浅色单色系新拟态
AFTER STYLE TASK: IV-P1-01 — 完整验收工具链
AFTER: IV-P1-02 / IV-P1-03 blocked by IV-P1-01
```

具体范围见 `docs/task-board.md` 和 `docs/development-plan.md`。

## Git / Test 基线

```text
current main HEAD: 每轮通过 git rev-parse main 读取
implementation/test baseline: 162ee27fb712479a3474ba04ad561bc24ea6a899
origin/main: b99efcc（4 个本地功能提交未 push）
cargo test --workspace: PASS，10 passed，2026-09-14，tested at 162ee27
cargo fmt --all -- --check: FAIL，历史格式差异；不是本次文档改动造成
release build: 本次未执行
UI runtime acceptance: 本次未执行
```

## 当前风险

- 4 个玻璃 UI 提交仅存在本地 `main`；未 push 不等于丢失，但远端尚不能恢复这些提交。
- 当前 UI 截图流程散落在忽略目录且部分脚本依赖固定 HWND，无法作为稳定回归入口。
- P1 的对比度、DPI、桌面捕获恢复和性能预算尚无完整、可重复证据。
- 全仓 rustfmt 基线不干净；后续任务必须避免把大规模格式化混入功能 diff。

## 非阻塞说明

- 当前没有 OPEN 的产品或架构决策。
- push、Release 和安装包分发不阻塞本地 P1 验收，但执行前需要用户明确要求。
