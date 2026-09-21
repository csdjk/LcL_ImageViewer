# LcL ImageViewer — 当前开发计划

> 本文保存复杂 Task 的执行合同。Task Board 只保留摘要、依赖和状态。

当前用户授权增量：`IV-TA-01`，完整执行合同见 [轻量贴图检查优化计划](轻量贴图检查优化计划.md)。首轮完成 DDS 标识、动画保留帧预算、当前文件自动刷新与视图锁定；其余 TA 工具后续单独实施。

## Task IV-P1-N01 — 新拟态 UI（2026-09-14 用户授权）

Outcome：在现有 egui 桌面实现中统一浅色单色系新拟态样式，不重写功能和渲染架构。

Allowed Paths：`crates/iv-viewer/src/ui.rs`、`crates/iv-viewer/src/app.rs`、`tools/ui-qa/**`、`docs/ui-qa/**`、`docs/新拟态UI规范.md`、`README.md`。Coordinator 在 main 维护协调文档，Worker 不修改协调状态。

实现：统一色板、12/14/16pt 圆角、上左高光和下右柔影、内凹按下/选中、可辨识禁用/焦点；默认浅色并保留已有显式主题；桌面磨砂作为已有可选功能保留，新安装默认关闭。不改变图像原始数据。

验收：workspace tests、cargo check、release 主程序构建；真实 Windows 双主题、880×560/1280×860、默认/悬停/按下/选中/焦点、设置/菜单、自动隐藏与恢复；输出 commit、binary hash、输入、主题、状态、客户区和 DPI 元数据。完整多 DPI、性能及剩余玻璃恢复矩阵未覆盖时如实记录，不宣称整个 P1 VALIDATED。

禁止：解码/取样/关联代码、全仓格式化、覆盖用户改动、push、发布、安装包和注册表操作。业务代码在独立 Task worktree 开发；提交后串行集成本地 main 并复验。

## Task IV-P1-01 — Windows UI 截图验收工具链

### Outcome

项目拥有一个可提交、可重复运行的 UI 验收入口，能够启动或连接当前构建的 `imageview`，动态发现正确窗口，设置验收状态并输出截图与完整元数据，为后续视觉、交互和性能任务提供可信基线。

### Non-goals

- 不调整玻璃材质、布局、颜色或交互行为；
- 不修复验收时发现的 UI 缺陷；
- 不重构 `app.rs`、渲染管线或图片解码；
- 不全仓执行格式化；
- 不 push、不发布 Release、不构建/分发安装器、不修改注册表。

### Gate

- P0 已验证；
- 开始领取时 `main` 工作区干净；
- 保留 `main` 现有 4 个未 push 提交；
- 没有其他 worktree 占用 Allowed Paths。

### Inputs

- `docs/玻璃磨砂UI优化方案.md` 的验收矩阵；
- `Temp/capture_window.py` 与 `Temp/interact_window.py` 的本地实验经验；
- `ui-verify-shots/capture_final.py` 的历史问题：固定 HWND 和旧机器假设，只能参考，不能原样沿用；
- `test_images/` 中可重复生成或明确标识的测试输入。

### Outputs

- `tools/ui-qa/` 下的可提交入口与必要辅助脚本；
- `docs/ui-qa/README.md`，说明依赖、命令、状态、输出结构和失败诊断；
- 忽略目录中的截图、JSON 元数据和日志；
- 一次最小 smoke run 的实际输出路径和结果。

### Allowed Paths

```text
tools/ui-qa/**
docs/ui-qa/**
```

运行产物只能写入已有忽略目录：

```text
ui-verify-shots/**
Temp/**
target/**
```

### Forbidden / shared paths

```text
AGENTS.md
README.md
docs/status.md
docs/roadmap.md
docs/task-board.md
docs/needs-decision.md
Cargo.toml
Cargo.lock
crates/**
tools/register_thumbnail.ps1
tools/unregister_thumbnail.ps1
tools/setup.iss
```

确需越界时停止，由 Coordinator 修改 Task 范围或交给后续任务。

### Implementation order

1. 定义单次 run 的 JSON 元数据 schema 和目录命名，不先写桌面控制代码。
2. 动态枚举进程/窗口并用进程路径、PID、窗口标题和可见性确认目标，禁止固定 HWND。
3. 支持显式传入构建产物、输入图片、主题、目标客户区和状态；没有可靠自动操作时返回明确失败。
4. 捕获客户区并验证非空、尺寸匹配，记录逻辑/物理尺寸、DPI、commit、输入、主题和状态。
5. 实现最小 smoke matrix：深色/浅色、880×560/1280×860、工具栏可见；必要时把不可自动化状态标为人工步骤。
6. 写文档、执行验证并提交任务分支。

### Acceptance

- 不修改项目源代码即可重复运行；
- 动态发现当前 `imageview` 窗口，窗口不存在、多窗口歧义或捕获空白时明确失败；
- 每张图有配套元数据，至少含 commit、binary、input、theme、state、logical client size、physical client size、DPI/scale 和 capture method；
- 成功生成 880×560 与 1280×860 的深浅主题 smoke 证据，或对硬件/桌面限制给出可复现的阻塞证据；
- 脚本不包含固定 HWND、旧机器绝对路径、Token 或账号信息；
- `cargo test --workspace` PASS；
- `git diff --check` PASS，diff 只包含 Allowed Paths。

### Stop conditions

- 无法可靠区分目标窗口；
- 切换 DPI、主题或状态会改变用户系统设置且没有局部可逆方案；
- 捕获方法只能得到桌面背景或空白窗口；
- 需要修改 Rust UI/渲染代码才能继续；
- 发现会改变产品方向或渲染架构的问题。

### Rollback and handoff

- 回滚只需删除本 Task 新增的 `tools/ui-qa/` 与 `docs/ui-qa/` 文件；忽略目录产物不是源码状态。
- Worker handoff 必须给出 Task ID、branch、commit、修改文件、运行命令、截图/元数据路径、通过项和 blocker。

## IV-P1-N14 — 自定义背景调色板

Base `265ff830f578aa5e40845cbcad55008dcd95e24e`；Owner ChatGPT-AgentDock；branch `codex/background-palette`；worktree `Temp/worktrees/background-palette`。将RGB滑条改为二维SV调色板、色相条、快捷色与HEX输入，实时预览并复用原背景持久化语义；自定义独立子面板避免560高窗口溢出。只改菜单和回归资料，不改解码/图像/主题/原生置顶、不升级依赖、不push或打包。完成测试、局部格式、双主题实机与本地main复验。

IV-P1-N14 DONE：主线 `98abd01e4abbdf34b1a70c0fc4c66d55d137153d` 完成110项Rust、18项配置/check/Release及74张实机复验；原图/偏好/置顶保持。代码与验证文档本地中文提交，未发布或更新安装。

## IV-MAC-01 — 用户授权macOS构建

Base f25287e；独立分支 codex/macos-package，worktree Temp/worktrees/macos-package。允许新增最小平台适配、推送任务分支/集成主线和GitHub Actions构建产物；不创建或修改正式Release/Latest、不改用户安装。保留核心解码/渲染，Windows API使用平台条件隔离，macOS以原生打开面板、Cmd+O、系统废纸篓和中文系统字体适配。Windows桌面磨砂及Explorer缩略图不伪装为Mac可用。公开仓库使用标准macos-14 ARM及macos-15-intel runner；未提供开发者证书，不假称公证。DMG拖入Applications，包包含许可、版本与源commit/hash。两架构执行测试和窗口启动截图，Windows回归及main复验；失败保留日志并修复。
