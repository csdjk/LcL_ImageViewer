# LcL ImageViewer — 主任务板

<!-- project-stage: P1 -->

> 当前阶段：**P1 ACTIVE**。N01新拟态、N02的1秒隐藏和N03两侧玻璃导航均已集成复验；N04鼠标手势已集成复验，当前NEXT为 `IV-P1-01`。

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
| IV-P1-N05 | 直接打开当前图片所在文件夹 | IN_PROGRESS | ChatGPT-AgentDock | 用户明确修复需求 / N04 DONE | `crates/iv-viewer/src/app.rs`, `tools/ui-qa/**`, `docs/ui-qa/打开所在目录验收.md` | 打开当前父目录，不使用/select；中文/空格路径、相对路径测试；实机菜单打开Explorer定位核对；tests/check/构建/main复验 |
| IV-P1-N04 | 左键平移图像、右键拖动窗口 | DONE | ChatGPT-AgentDock | 用户2026-09-14明确授权 / N03 DONE | `crates/iv-viewer/src/{app.rs,backdrop.rs}`, `tools/ui-qa/**`, `docs/ui-qa/鼠标拖拽验收.md`, `README.md` | 左键移图不移窗；右键移窗不移图且拖后不弹菜单；单击菜单/边缘缩放/导航保留；tests/check/release及实机双主题双尺寸验证 |
| IV-P1-N03 | 窗口两侧半透明磨砂切图按钮 | DONE | ChatGPT-AgentDock | 用户追加需求；串行复用 N02 的1秒计时 | `crates/iv-viewer/src/{app.rs,ui.rs,main.rs}`, `tools/ui-qa/**`, `docs/ui-qa/**`, `docs/新拟态UI规范.md` | 左右边缘垂直居中；顶栏移除重复箭头；半透明真实场景模糊；边界禁用；1秒隐藏恢复；tests/check/release及双主题双尺寸实机验收 |
| IV-P1-N02 | 菜单栏 1 秒自动隐藏 | DONE | ChatGPT-AgentDock | 用户最新授权 / IV-P1-N01 DONE | `crates/iv-viewer/src/app.rs`, `tools/ui-qa/autohide-actions.json`, `docs/ui-qa/自动隐藏1秒验收.md` | 等待1秒；淡入120ms/淡出180ms不变；计时边界测试；实机双主题双尺寸隐藏恢复；tests/check/release PASS |
| IV-P1-N01 | 浅色单色系新拟态 UI | DONE | ChatGPT-AgentDock | 用户最新授权 / P0 VALIDATED | `crates/iv-viewer/src/{ui.rs,app.rs}`, `tools/ui-qa/**`, `docs/ui-qa/**`, `docs/新拟态UI规范.md`, `README.md` | 12–16pt 圆角；双主题单色系；多层双向柔影；凸起/凹陷、禁用/焦点；图片功能不变；真实双主题双尺寸/关键交互截图；tests/check/release build PASS |
| IV-P1-01 | 建立可重复的 Windows UI 截图验收工具链 | READY | — | P0 VALIDATED | `tools/ui-qa/**`, `docs/ui-qa/**`; 运行产物写入忽略目录 | 动态发现窗口；记录 commit/输入/主题/状态/逻辑与物理客户区/DPI；可重复捕获至少 880×560 与 1280×860；无固定 HWND/旧机器路径；workspace tests PASS |
| IV-P1-02 | 补齐新拟态 UI 视觉/交互矩阵并闭环范围内缺陷 | BACKLOG | — | IV-P1-01 | `crates/iv-viewer/src/{app.rs,ui.rs,render.rs,image.wgsl,backdrop.rs}`, `docs/ui-qa/**` | 深浅主题、两种视口、关键交互/弹层/长文本有实际截图；对比度与裁切有记录；发现的本轮缺陷修复后重截；workspace tests PASS |
| IV-P1-03 | 验证桌面捕获恢复与性能预算 | BACKLOG | — | IV-P1-01 | `crates/iv-viewer/src/{app.rs,backdrop.rs,render.rs}`, `tools/ui-qa/**`, `docs/ui-qa/**` | 拖动、最小化/恢复、捕获失败可观察且可恢复；性能注明环境、样本数、缓存边界、中位数/P95；无证据不宣称达到 GPU ≤2ms |
| IV-P1-04 | P1 集成与 release-candidate 门禁 | BACKLOG | — | IV-P1-02, IV-P1-03 | integration/build checks, `README.md`, `docs/**` | 所有 P1 验收在 main 复验；release workspace build PASS；格式基线有明确处置；status/roadmap 同步；不执行发布或 push |

## Coordinator NEXT

```text
NOW: IV-P1-N05 IN_PROGRESS
COMPLETED: IV-P1-N01 / IV-P1-N02 / IV-P1-N03 DONE
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

## IV-P1-N02 领取记录

Owner: ChatGPT-AgentDock；Branch: `codex/iv-p1-n02-autohide-1s`；Worktree: `Temp/worktrees/autohide-1s`；Base: `ad607e9bd32baad96f056cc80d152d468bfe28ab`。用户只要求等待改为1秒，不改变样式、动画或图片功能。

## IV-P1-N02 阻塞解除

已随N03串行集成并复验。N03增加显式QA隔离profile，在不关闭用户旧Debug、不覆盖其偏好的前提下完成当前Release双主题/双尺寸隐藏恢复验收，旧阻塞已解除。等待1秒、动画不变，点击也重置等待以支持连续切图。

## IV-P1-N03 执行合同

只修改两侧导航布局/材质、复用原玻璃采样与N02计时，不改变其他新拟态表面和图像语义。N02已有提交由本任务串行合并、共同复验，原工作树冻结不再并行编辑。旧Debug窗口保持运行，不覆盖占用的Debug程序。允许main.rs增加显式QA专用隔离profile，并在tools/ui-qa中只操作新profile和本次启动的PID，避免改写用户正在使用的配置。真实双主题、880×560/1280×860、导航/禁用/悬停/按下/焦点、隐藏恢复、菜单/设置截图；记录DPI与二进制hash；不push或发布。

Owner: ChatGPT-AgentDock；Branch: `codex/iv-p1-n03-side-glass`；Worktree: `Temp/worktrees/side-glass`。已串行合并N02提交，先使用Release避免覆盖用户运行的Debug。

## N03 / N02 集成审查

N03 Worker HEAD `31947ea`，功能源码 `7611014`，串行包含N02提交。Allowed Paths核对通过，21tests/check/release PASS；最终64张实机截图四组标题/边界/隐藏恢复/图片ROI检查通过。隔离profile解除N02旧窗口占用的验收阻塞，用户旧Debug不动；两任务进入main串行集成复验。

## N02 / N03 完成记录

集成提交 `6de1d75f8b9ccb511246f61efc6efeaef3c96f9d`；main tests21/check/release PASS。最终任务源码64张截图、main Release40张截图及标题/像素检查通过。用户旧Debug及其配置保留。完整证据见 `docs/ui-qa/两侧玻璃导航验收.md`；未push。NEXT恢复IV-P1-01，P1不标为VALIDATED。

## N04 执行合同及领取记录

用户明确要求左键拖拽图片、右键拖拽整个窗口。任务READY后由ChatGPT-AgentDock领取，独立分支 `codex/iv-p1-n04-mouse-gestures`，工作树 `Temp/worktrees/mouse-gestures`；基线 `0693288`。画布左键平移，保留中键兼容；右键单击菜单与拖动窗口互斥；原顶栏空白左键拖窗和左键边缘缩放保留。固定版本winit的StartDrag只发WM_NCLBUTTONDOWN，右键采用窗口/鼠标物理坐标差移动，避免虚假左键和位置反馈。只操作本次QA进程/隔离profile，不关闭已有查看器、不改关联。复验并集成后才DONE。

N04分支复验：24tests/check/Debug及Release构建通过，48张双主题双尺寸真实拖拽截图的几何/像素检查通过。进入主线复验，完整范围8个文件。

## N04 完成记录

功能集成 `dec4c0abf81a873277fbc5d22cb2485f5c5fbad5`；main24tests/check/Debug/Release及24张实机复验通过，分支48张截图四组几何/像素检查通过。两种构建已同步更新；保留旧用户窗口，不改系统快捷方式/关联，不push。详细结果见 `docs/ui-qa/鼠标拖拽验收.md`。

## N05 执行合同及领取记录

用户要求在资源管理器中直接打开当前文件所在目录。READY后领取，独立分支 `codex/iv-p1-n05-open-folder`，工作树 `Temp/worktrees/open-folder`，基线 `1fd9489`。只更换菜单文件操作和必要的路径测试/验收，保留旧功能；不修改文件关联、不关闭用户窗口、不push。运行程序占用时使用有明确标识的备用构建路径，避免覆盖或终止运行中的程序。完成后集成本地main、复验并记录实际产物。
