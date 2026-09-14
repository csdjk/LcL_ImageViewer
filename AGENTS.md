# LcL ImageViewer — Agent 协作入口

所有开发 Agent 第一次接手时按顺序阅读：

1. `README.md` — 产品、运行方式与文档导航。
2. `docs/status.md` — 当前真实状态和唯一 NEXT。
3. `docs/roadmap.md` — 阶段顺序与 Stage Gate。
4. `docs/task-board.md` — Task、Owner、依赖、Allowed Paths 与 Acceptance。
5. `docs/development-plan.md` — 当前复杂 Task 的执行合同。
6. `docs/needs-decision.md` — 等待用户决定的问题与恢复条件。
7. 当前 Task 指向的设计、代码和验收资料；玻璃 UI 主线还应阅读 `docs/玻璃磨砂UI优化方案.md`。

聊天记录和历史截图不能覆盖磁盘、Git 与可重复测试的当前事实。

## 权威职责

```text
docs/status.md            当前阶段、已验证事实、唯一 NEXT
docs/roadmap.md           阶段顺序、目标与 Stage Gate
docs/task-board.md        Task、Owner、依赖、范围与 Acceptance
docs/development-plan.md  当前复杂 Task 的完整执行合同
docs/needs-decision.md    必须由用户决定的问题
Git                       代码、branch、worktree、commit 的真实状态
Tests / 运行截图          完成证据
```

## 协作与 Git

- 一个 Task 对应一个 Owner、branch 和独立 worktree。
- Coordinator 只领取 `READY` Task，领取前核对依赖、Allowed Paths、Acceptance、`main` HEAD、工作区和其他 worktree。
- Worker 不直接在 `main` 开发，不修改 `status.md`、`roadmap.md` 或 `task-board.md` 等全局协调状态。
- Integrator 串行审查任务提交，合入 `main` 后重新运行必要验收，再更新全局状态。
- Coordinator/Integrator 可在干净的 `main` 上维护协调文档；业务代码仍必须进入 Task worktree。
- 不覆盖、reset、stash、删除或改写用户与其他 Agent 的未提交工作和本地提交。
- 每个完成的任务都要有 Git commit；只有用户明确要求时才 push、发布 Release、构建/分发安装包或修改文件关联。

当前接入工作流时，`main` 比 `origin/main` 超前 4 个本地提交。这是既有状态，不能自动丢弃、重排或推送。

## Rust 与验证

- 项目是 Rust workspace：`iv-core`、`iv-viewer`、`iv-shell`；主 GUI bin 为 `imageview`。
- 常规验证：`cargo test --workspace`。修改 Rust 后按 Task 范围增加 `cargo check`、相关测试和 release build。
- `cargo fmt --all -- --check` 当前存在历史格式差异；不得在无专门 Task 时全仓格式化。新改动保持局部格式正确，并用 diff 检查避免扩大范围。
- 如果系统 PATH 没有 Cargo，可使用任务局部的临时 Rust 工具链；不要修改全局 PATH，也不要把机器私有工具链路径写进仓库。

## UI 与桌面验收

- UI 修改必须运行真实 Windows 桌面应用并取得当前构建截图，不能用旧截图或 HTML 样稿代替。
- 每张验收截图应记录 commit、输入图、主题、页面/交互状态、逻辑与物理客户区、DPI/缩放率和裁剪方式。
- 至少覆盖受影响的深浅主题、`880×560`、`1280×860`，以及默认/悬停/按下/选中/焦点、设置、右键菜单、自动隐藏和恢复状态。
- 检查安全边距、内边距、对齐、文字基线与裁切、长中英文、层级、溢出和可读性。弹层还要检查标题、正文、图标、关闭按钮和操作区。
- 只有存在目标参考图时才执行同条件像素相似度门禁；没有目标图或缺少同条件截图时不得宣称达到 98%。
- `ui-verify-shots/` 与 `Temp/` 是忽略的本地证据目录。提交物应保存可重复命令和文字结果，截图路径要在交付中报告。

## DONE 与停止条件

Task 只有在实现满足 Acceptance、相关测试通过、任务分支已提交、Integrator 合入 `main`、`main` 复验通过且状态文档同步后才是 `DONE`。

遇到以下情况停止并保留可恢复状态：

- 没有 READY 或可恢复任务；
- 与其他 Agent 的 branch/worktree 或 Allowed Paths 冲突；
- Git 状态无法安全继续；
- 需要改变产品方向、核心数据语义、重要架构或重大技术路线；
- UI 验收缺少可运行环境、指定状态、输入或可靠截图；
- 发布、push、注册表或安装器操作没有明确授权。

需要用户决定时创建 `ND-xxx`，关联 Task，标记 `BLOCKED`，写清方案、影响和恢复条件后停止。
