# LcL ImageViewer — 主任务板

<!-- project-stage: P1 -->

> 当前阶段：**P1 ACTIVE**。N01–N08均已集成复验；当前NEXT为 `IV-P1-01`。

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
| IV-MAC-01 | macOS双架构GitHub打包 | IN_PROGRESS | ChatGPT-AgentDock | 用户明确要求macOS安装包 | `crates/iv-viewer/**`, `crates/iv-core/tests/**`, `Cargo.lock`, `.github/workflows/**`, `tools/macos/**`, `README.md`, `docs/releases/macos.md` | 双架构云端编译/测试/启动及DMG校验；Windows无回归；仅工作流产物不发新Latest |
| IV-REL-050 | 发布置顶调色板与图文说明v0.5.0 | DONE | ChatGPT-AgentDock | 用户本轮授权 / N13及调色板已完成 | `Cargo.toml`, `Cargo.lock`, `tools/**`, `README.md`, `CHANGELOG.md`, `docs/screenshots/**`, `docs/releases/**`, `docs/screenshot-*.jpg` | 全部功能配当前实机图、动画播放暂停逐帧GIF；版本/测试/构建/包体验证；main与新标签及Latest正式发布和下载hash；保留当前安装 |
| IV-P1-N14 | 自定义背景调色板 | DONE | ChatGPT-AgentDock | 用户本轮要求 / N13 DONE | `crates/iv-viewer/src/{app.rs,main.rs,color_palette.rs}`, `tools/ui-qa/**`, `README.md`, `docs/ui-qa/**` | 二维SV色板/色相条/快捷色/HEX，实时预览；黑白灰不丢色相，旧偏好与原图不变；测试/构建/实机/本地合入，不发布 |
| IV-P1-N13 | 顶部置顶与背景颜色菜单 | DONE | ChatGPT-AgentDock | 用户本轮要求 / REL-040 DONE | `crates/iv-viewer/src/{app.rs,ui.rs,background.rs,main.rs}`, `crates/iv-viewer/Cargo.toml`, `Cargo.lock`, `tools/ui-qa/**`, `docs/ui-qa/**`, `README.md` | 真正窗口置顶可切换；背景/棋盘格顶部入口及偏好兼容；窄宽布局、原图像素、动画与菜单回归；测试/构建/实机后中文本地合入，不发布 |
| IV-REL-040 | 发布AVIF与浏览增强正式版v0.4.0 | DONE | ChatGPT-AgentDock | 用户明确授权 / FMT-02及N10–N12 DONE | `Cargo.toml`, `Cargo.lock`, `tools/{setup.iss,package_release.py,build_windows_release.py}`, `tools/tests/**`, `README.md`, `CHANGELOG.md`, `docs/releases/**` | 版本统一、完整许可/运行依赖；101项Rust与安装契约、Release实机、隔离安装/卸载、ZIP与hash；推送main及新tag、正式Release与公开下载核验；不改变当前安装 |
| IV-FMT-02 | 动态AVIF完整播放 | DONE | ChatGPT-AgentDock | 用户明确追加要求 / FMT-01 DONE | `crates/iv-core/src/{avif.rs,decode.rs}`, `crates/iv-core/tests/**`, `crates/iv-viewer/src/app.rs`, `tools/ui-qa/**`, `docs/formats/**`, `README.md` | 完整帧/时长/Alpha；自动循环/暂停/逐帧/进度；有界总帧内存，预览只取首帧；测试/双构建/实机/main复验；中文提交不发布 |
| IV-FMT-01 | AVIF图片查看与缩略图支持 | DONE | ChatGPT-AgentDock | 用户明确要求 / 当前main | `crates/iv-core/**`, `crates/iv-viewer/src/{app.rs,directory.rs,winassoc.rs}`, `crates/iv-shell/**`, `Cargo.lock`, `tools/{setup.iss,register_thumbnail.ps1,unregister_thumbnail.ps1}`, `tools/tests/**`, `tools/ui-qa/**`, `docs/formats/**`, `README.md` | 内容识别、静态/透明AVIF、打开/切图/预览；有界解码及坏文件测试；测试/双构建/实机；中文提交本地合入，不发布/安装/改关联 |
| IV-P1-N12 | 顶部文件名显示浏览根目录相对路径 | DONE | ChatGPT-AgentDock | 用户要求 / N11 DONE | `crates/iv-viewer/src/{directory.rs,app.rs}`, `tools/ui-qa/**`, `docs/ui-qa/**`, `README.md` | 根目录文件名/子目录相对路径；原省略和完整悬停；无额外IO；单元测试、双构建及实机；中文提交本地合入 |
| IV-P1-N11 | 子文件夹浏览与中文提交规范 | DONE | ChatGPT-AgentDock | 用户本轮明确要求 / N10 DONE | `AGENTS.md`, `crates/iv-viewer/src/{directory.rs,app.rs,ui.rs}`, `tools/ui-qa/**`, `docs/ui-qa/**`, `README.md` | 固定根只向下/跳过重解析点；后台取消/进度/资源上限；左右键与按钮共用；删除保留范围；中文提交；测试/双构建/实机/main复验，不发布 |
| IV-P1-N10 | WebP/PSD缩略图、全画布棋盘格与图片AABB | DONE | ChatGPT-AgentDock | 用户本轮要求 / 当前main | `crates/iv-viewer/src/**`, `crates/iv-shell/**`, `crates/iv-core/src/{decode.rs,psd_composite.rs}`, `crates/iv-core/tests/**`, `tools/{setup.iss,register_thumbnail.ps1,unregister_thumbnail.ps1}`, `tools/tests/**`, `tools/ui-qa/**`, `README.md`, `docs/ui-qa/**` | 标准缩略图GUID/WebP注册与COM图像验证；透明画布连续；边界高亮随图变换；tests/check/双构建及实机；不改实时注册/安装/远端 |
| IV-DOC-02 | 面向用户精简 README | DONE | ChatGPT-AgentDock | 用户本轮要求 / DOC-01已推送 | `README.md` | RGBA简述不写教程；仅功能/配图/下载/操作；无开发/性能/版本过程；文档检查和本地合入，不push |
| IV-DOC-01 | README 当前功能与 RGBA 重点说明 | DONE | ChatGPT-AgentDock | 用户明确要求修改提交 / REL-031 DONE | `README.md` | 当前功能而非版本历史；通道/像素说明与代码一致；引用/Markdown/diff检查；本地提交合入，不push或发布 |
| IV-REL-031 | 发布加载优化及安装器修复 v0.3.1 | DONE | ChatGPT-AgentDock | 用户本轮明确要求 / PERF-01、INSTALL-01/02 DONE | `Cargo.toml`, `Cargo.lock`, `tools/setup.iss`, `tools/tests/**`, `README.md`, `CHANGELOG.md`, `docs/releases/**` | 版本统一；54+11测试、隔离注册与目录验证、正式EXE实机；包hash/启动；push main+新tag，正式Release附件回读；不改当前安装 |
| IV-PERF-01 | 图片加载与启动耗时优化 | DONE | ChatGPT-AgentDock | 用户本轮明确要求 / 当前main | `crates/iv-viewer/src/{main.rs,app.rs,loader.rs,render.rs,perf.rs,directory.rs}`, `crates/iv-core/src/decode.rs`, `crates/iv-core/tests/**`, `crates/iv-core/examples/**`, `tools/perf/**`, `docs/performance/**`, `README.md` | 同样本同Release基线；调度/扫描/拷贝优化；像素一致；tests/check/双构建及实际窗口验证；不改关联/安装/远端 |
| IV-INSTALL-02 | 新安装默认D盘目录 | DONE | ChatGPT-AgentDock | 用户要求 / INSTALL-01 DONE | `tools/setup.iss`, `tools/tests/**`, `README.md`, `docs/releases/default-install-directory.md` | D盘优先/无D回退/升级保留/目录可选；真实Inno测试与安装包构建；不安装、不改关联、不发布 |
| IV-INSTALL-01 | 修复安装器打开方式空注册键 | DONE | ChatGPT-AgentDock | 用户安装后打开方式缺失 / REL-030 DONE | `tools/setup.iss`, `tools/tests/**`, `docs/releases/openwith-installer-fix.md`, `README.md` | 显式ValueType、关联刷新；旧版复现/修正版隔离真实安装和卸载回归；本地修正版包，不改真实关联、不push或覆盖公开附件 |
| IV-REL-030 | 更新README、实机配图并发布v0.3.0 | DONE | ChatGPT-AgentDock | 用户2026-09-15明确授权打包发布 / N09 DONE | `README.md`, `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `tools/**`, `docs/screenshots/**`, `docs/screenshot-*.jpg`, `docs/releases/**` | 文档准确；公开截图无隐私；tests/check/release workspace；包内容/hash/启动检查；推送main和新标签后GitHub Release附件核验 |
| IV-P1-N09 | 设置弹窗独立拖动（纠正拖动对象） | DONE | ChatGPT-AgentDock | 用户明确纠正 / N08 DONE | `crates/iv-viewer/src/{app.rs,ui.rs,backdrop.rs}`, `README.md`, `tools/ui-qa/**`, `docs/ui-qa/设置弹窗独立拖动验收.md` | 标题只移动内部弹窗，主窗口固定；控件不误拖；边界可见；画布右键拖窗保留；tests/check/双构建与实机验证 |
| IV-P1-N08 | 设置页紧凑列表与标题拖窗 | DONE | ChatGPT-AgentDock | 用户最新授权；N07已集成待串行复验 | `crates/iv-viewer/src/{app.rs,ui.rs,backdrop.rs}`, `README.md`, `tools/ui-qa/**`, `docs/ui-qa/**` | 无分类无大卡片；保留所有实际设置；标题左右键移整窗、控件不误拖；tests/check/双构建/双主题双尺寸实机及main复验 |
| IV-P1-N07 | 右键菜单去掉分类与分组空白 | DONE | ChatGPT-AgentDock | 用户最新需求 / N06 DONE | `crates/iv-viewer/src/{app.rs,ui.rs}`, `tools/ui-qa/**`, `docs/ui-qa/无分类菜单验收.md` | 连续单列无分类；功能/快捷键保留；tests/check/构建及双主题双尺寸实机验证 |
| IV-P1-N06 | 精简右键菜单、A/D切图及Delete回收站删除 | DONE | ChatGPT-AgentDock | 用户最新菜单需求及上一条Delete修复 / N05 DONE | `crates/iv-viewer/{Cargo.toml,src/app.rs,src/main.rs,src/recycle.rs}`, `Cargo.lock`, `README.md`, `tools/ui-qa/**`, `docs/ui-qa/菜单快捷键验收.md` | 菜单精简；A/D与原方向键；输入焦点/组合键保护；确认后仅回收站删除、取消/失败保留；tests/check/双构建及隔离实机回归 |
| IV-P1-N05 | 直接打开当前图片所在文件夹 | DONE | ChatGPT-AgentDock | 用户明确修复需求 / N04 DONE | `crates/iv-viewer/src/app.rs`, `tools/ui-qa/**`, `docs/ui-qa/打开所在目录验收.md` | 打开当前父目录，不使用/select；中文/空格路径、相对路径测试；实机菜单打开Explorer定位核对；tests/check/构建/main复验 |
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
NOW: IV-P1-01 READY
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

## N05 完成记录

集成 `9ba63ec28ae226af8722a2844e0998b4f3b0251c`；main29tests/check/双构建通过。分支4组、main Release2组实机菜单点击及实际Explorer父目录核验PASS，DPI96。菜单更名“打开所在文件夹”，直接传父目录，不用/select。修复版 `target/folder-fix/release/imageview.exe`，Debug已更新。原 `target/release/imageview.exe` 仍被用户运行占用，未覆盖或终止；本轮修复版位于上述备用路径。详见 `docs/ui-qa/打开所在目录验收.md`。无push/发布；NEXT恢复IV-P1-01。

## N06 执行合同

用户要求精简菜单及A/D切图；上一条Delete删除修复尚未执行，本轮一并完成。READY后领取，Owner ChatGPT-AgentDock，分支 `codex/iv-p1-n06-menu-keys`，工作树 `Temp/worktrees/menu-keys`，Base `dda28fddd2cbc60c9c86860170b248a4b6f29ee3`。保留文件操作、属性、像素检查、设置；移除重复的切图/视图/通道/主题/动画/Mip控制入口，功能仍由顶栏/快捷键提供。删除只在显式确认后通过IFileOperation移入回收站，拒绝非回收模式，不提供永久删除；阻止输入框、弹层和Ctrl/Alt/Shift组合误触。只回收本轮生成的测试文件，不操作用户原图。保留既有提交与用户窗口，主线验收后DONE，不push/发布。

N06 Review：工作树b45b3a5，源码5130492，35tests/check/双构建通过，100张最终实机截图四组PASS；已核对每组3张测试图的回收站字节hash。范围9文件核对通过；主线复验后DONE。

## N06 完成记录

功能集成 `85bb27e077d62356f2e6957f6fd2e0b0834d0e6f`；main35tests/check/双构建PASS；分支100张及主线Release50张实机回归，A/D、精简菜单、确认/取消/回收站原始hash、后继/空态校验PASS。常规Debug/Release均更新，用户旧folder-fix副本未关闭或覆盖；使用隔离配置、只回收新建测试图，无push/发布。详见菜单快捷键验收.md。NEXT恢复IV-P1-01。

## N07 领取记录

READY后由ChatGPT-AgentDock串行领取；Base `2ba8e09700b4da9a917c324d48ca68612ff0c4b3`，分支`codex/iv-p1-n07-flat-menu`，工作树`Temp/worktrees/flat-menu`。只移除菜单分类/分组留白并统一行高，不改操作回调及快捷键。旧N06工作树已集成且冻结。验收仅对测试图打开/取消弹层，不回收或删除文件；保持用户窗口，不改关联，不push。

N07 Review：Worker `515dc89b1266d952a0b3e57c4dcdd7314f85931c`，35tests/check/Debug通过；4组40张实际截图、含/不含像素行菜单和原快捷键检查通过，输入文件保持不变。范围核对后串行合入main复验。

## N08 执行合同

READY后串行领取。Base `2452ffacd7d6d81f5d0fe018c1b2c8ced164dbdd`；branch `codex/iv-p1-n08-compact-settings`，worktree `Temp/worktrees/compact-settings`。移除设置分类卡片/装饰说明，保留实际设置、错误诊断和持久化键；固定设置面板随原生窗口移动，标题和顶部空白为独立拖动区，关闭/开关/滑条区域不启动拖窗。N07已在main且原工作树冻结，本轮一并补齐主线实机复验，不回滚旧功能。测试只改隔离QA profile，不执行注册/注销/默认应用等系统写入，不关闭用户窗口，不push/发布。

N08范围补充：设置标题左键复用已有物理坐标WindowDrag，只新增按键标识与独立状态，避免系统StartDrag吞掉起拖前位移；原右键行为及系统关联不变。

N08 Review：源码3c89b47；37tests/check/双构建通过，双主题双尺寸104张最终截图及几何/配置回归PASS。范围7文件已检查，串行集成后补齐N07/N08主线收尾。

## N07 收尾完成

原功能集成 `2452ffa` 保留。本轮在main `d3299f57c1c0680beb2cfc4eec884a4239d62445` 重建并补齐未完成的主线复验：专门入口 `flat-menu-main-finish-dark-880` 10张，含/不含像素行、属性、Delete确认取消、A/D和隐藏恢复PASS；N08主线52张同样覆盖双主题无分类菜单。实际查看截图无分类标题和组间空白；测试文件未变。N07为DONE。

## N08 完成记录

Worker源码 `3c89b47`，验收提交 `f1ba462`；功能集成 `d3299f57c1c0680beb2cfc4eec884a4239d62445`。37tests/check/Debug/Release均PASS；分支104张及main52张真实设置回归通过，标题三种拖动准确跟手，正文/开关/两条滑条零窗口位移，参数保存、主题、关闭、菜单、A/D及删除确认取消通过。常规Debug/Release已更新；没有系统关联写入、push或发布。完整证据见 `docs/ui-qa/紧凑设置页验收.md`。NEXT恢复IV-P1-01；P1仍ACTIVE。

## N09 领取记录

用户澄清移动的是查看器内的设置弹窗，不是操作系统窗口。独立分支 `codex/iv-p1-n09-popup-drag`，工作树 `Temp/worktrees/popup-drag`，Base `a7ae49daf317d142791058d1dcf2b7cd2ef16e35`。N08工作树冻结；仅修正设置标题拖动和说明，保留画布右键原生拖窗。当前会话记住弹窗位置并限制在客户区。验证只使用新建测试图/隔离配置，不删除用户文件、不改系统关联。

N09 Review：40tests/check/Debug及四组64张实机验证通过；标题仅改变弹窗位置，原生外框固定。工作分支已提交，准备串行main复验。

## N09 完成记录

功能集成 `7d333f665eac9fbba3ac1beac72e7a58e13cfe0c`；main40tests/check/双构建及32张实机复验通过，分支64张截图通过。设置标题只拖动内部弹窗、原生主窗口保持不动，覆盖N08旧语义；画布右键原生拖窗保留。边界、控件、重开位置通过，未操作用户原图/关联，无push/发布。NEXT恢复IV-P1-01。

## IV-REL-030 发布合同

用户明确要求更新README、配图、打包并发布新版本。当前远端最新v0.2.0，发布v0.3.0。Base `5de7a66f465afb700fa52810a809c301bbda43d2`；独立branch `codex/release-v0.3.0`，worktree `Temp/worktrees/release-v0.3.0`。允许此次推送main与新标签及创建GitHub Release，不强推、不改写旧版本。只发布构建产物、文档和已审查源码；不上传Temp、用户偏好、私密桌面或机器文件。仅发布前必要验证，不把未完成的P1全DPI/性能门禁标为通过；发布说明如实列明覆盖范围。保持现有窗口和安装目录不变，不执行注册/卸载系统集成。

IV-REL-030 Review：Worker 0ef558d。40tests/check/release workspace、PE x64 GUI及DLL加载导出、Inno编译与ZIP内容hash通过；README本地链接/图片已核对，5张为本版本实际窗口截图，范围审查通过。开始main复验与正式打包；发布授权仅本轮。

## IV-REL-030 完成

v0.3.0正式发布：https://github.com/csdjk/LcL_ImageViewer/releases/tag/v0.3.0；tag源码`c8ceef0e6bc4555020e4ea7b4fdbdce8a7c951c4`。main及新标签已推送，三附件上传且公开下载hash均一致，安装器和ZIP通过本地验证；README五张配图为实际v0.3.0。发布权限来自本轮用户明确要求，不强推，不覆盖旧版本。NEXT恢复IV-P1-01，P1整体门禁继续保留。

## IV-INSTALL-01 执行合同

Base `8fc0d51921cbcedce4aaf6827970409ef92ba6a9`；用户安装v0.3.0且已选thumbs,assoc，实机注册键为空，命令残留旧安装位置。本轮修正安装脚本遗漏ValueType和关联刷新，必要共享键只清除自有值；不重写GUI/图像/默认关联。独立分支`codex/install-openwith-fix`，工作树`Temp/worktrees/install-openwith-fix`；原发布工作树冻结。真实安装测试仅允许隔离的HKCU\Software\LcL\InstallerQA随机命名空间及项目Temp，禁止修改实时Software\Classes/UserChoice/RegisteredApplications、用户安装或快捷方式。沿用已发布0.3.0主程序生成单独标注修正版的本地安装包，不重新发布或覆盖公开0.3.0附件。

IV-INSTALL-01 Review：6项契约测试通过，真实Inno隔离复现旧脚本缺值和失效命令，修正版71个值与共享值卸载保护通过；用户真实关联快照不变。GUI/解码代码无改动，准予本地主线集成，不发布。

IV-INSTALL-01 完成：本地集成`6ce3ea621bf81de8105af033c8f1accd489c44f1`，40+6测试及真实隔离注册回归通过；本地openwith-fix安装包已编译及校验，未修改实时关联、安装目录或公开Release。用户可从实际安装程序的设置页手动注册；记录见`docs/releases/openwith-installer-fix.md`。

## IV-INSTALL-02 合同

Base `42d3393662b5a07d80b5fe6c9b82dfdf50423c39`，独立branch `codex/install-default-d`，worktree `Temp/worktrees/install-default-d`。默认首次安装D:\Program Files\LcL ImageViewer，D盘不存在回退localappdata\Programs；升级沿用旧目录，目录选择页始终显示；保留INSTALL-01全部修复。只生成新的本地安装包，不覆盖已发布附件，不移动或重装用户当前程序，不修改实时关联。测试仅运行无负载/无注册动作的隔离目录选择探针，源码无GUI改动。

IV-INSTALL-02 Review: 11 read-only installer checks pass; actual Inno initialization selected the D-drive directory and passed both branch assertions. Wizard screenshot not validated. Existing association fixes retained; no application code changes.

IV-INSTALL-02 completed: integration `d084227f9f1b44b22586689c17e8e0bb504cefa4`, 11 checks and production compilation/version checks passed. Resolver evidence and screenshot limitation documented. Artifact `dist/default-d/LcL-ImageViewer-Setup-v0.3.0-d-drive-win64.exe` includes prior Open With fixes. User install and public release unchanged.

## IV-PERF-01 执行合同

Base `83fb9e7002e6f33b82db732895644cccd1220f58`；独立分支 `codex/perf-image-load`，工作树 `Temp/worktrees/perf-image-load`。先加入可选分阶段计时并保留同构建优化前基线，然后在同一实现上优化最新请求优先、有界预读、异步目录、启动并行与无损像素复制。禁止降低图片分辨率/画质、丢Alpha/HDR/Mip/帧；保持旧用户设置与所有安装器修复。不改变默认关联、当前安装，不push/发布。只使用项目测试素材和新建输入，性能区分进程启动/请求/解码/上传与提交绘制，不冒充屏幕实际呈现时间。

IV-PERF-01 Review：功能源码f316300，任务提交`2454fa4b2d308b93eb0c5bcd5ba63d6d67e166b5`；54项tests/check/Release、20次新进程优化前后同样本对比、10组真实通道ROI逐像素一致和41状态交互回归通过。范围内修正静态WebP原错误；新策略不抢占正在执行的单张解码，无有损降级。允许串行本地主线集成和复验，不push/发布。

IV-PERF-01 完成：集成`9eaae72e950831f524d582183aafdb3425b677fc`，54+11测试/check/双构建与主线16状态回归、8次同输入新进程复测通过，二进制hash与记录一致。优化前后基线为每场景5次；不把CPU提交解释为呈现延迟。未修改安装或远端，NEXT恢复IV-P1-01。


## IV-REL-031 发布合同

Base `dfb45debaf33409c17677395d2eba9d7cd73edc5`；branch `codex/release-v0.3.1`；worktree `Temp/worktrees/release-v0.3.1`。用户明确要求更新安装包并发布；原GitHub远端最新v0.3.0，本次使用v0.3.1，不覆盖旧tag/附件。允许将已有19个未推送提交与本次版本文档推送main、新标签及Release。整合加载优化、静态WebP修复、D盘默认与打开方式注册修复。不迁移当前安装、不操作实时关联、不强推、不发布私密文件。验证使用新建图片/隔离profile，注册测试仅在独立HKCU InstallerQA命名空间，目录探针不执行安装。P1长期门禁不改为通过。


IV-REL-031 Review：Worker `22bf57c`；54项Rust测试/check、11项安装器检查通过；真实Inno旧空键复现/修正版71值/共享值卸载及默认D目录解析通过，实时关联未改动。范围7文件符合合同，无GUI或解码行为新增修改。主线重建0.3.1后补齐载荷安装hash、实际窗口及最终包验证。


IV-REL-031 主线构建验收：aa70310；54+11检查/双构建、Inno隔离71值及EXE/DLL安装hash/卸载、实际D盘解析、双主题26状态通过，等待正式包和远端附件回读。


IV-REL-031 DONE：v0.3.1于2026-09-15T13:44:36Z正式发布并置Latest，源码tag `97fcf8e4309d8fe3d2e17c5c47cd216729735e34`，main和新标签均推送；三附件公开下载SHA256一致。54+11检查、隔离Inno安装hash/注册卸载、D盘解析和26状态GUI及ZIP启动通过。未修改用户当前安装。详见v0.3.1-validation.md。NEXT恢复IV-P1-01。


## IV-DOC-01 执行合同

Base `1d5cb0b3da7213a6019687f2af5be7f6608b71ca`；branch `codex/readme-rgba`；worktree `Temp/worktrees/readme-rgba`。用户要求直接修改README并提交：只描述当前功能，突出RGBA单通道/Alpha/像素读取，去掉版本叙事。Worker仅改README，复用已有真实配图并准确标注为界面示例；不重绘图片、不改Rust/版本/安装器、不重建安装包。核对app.rs快捷键、image.wgsl通道语义和像素读取来源，校验引用/Markdown/范围；Integrator合入main再验证，同步协调记录。此轮仅本地提交，不push，既有发布不变。

IV-DOC-01 Review：Worker `09a19d96af0686f065e8e475fa9e0868e459d735`，任务分支只改README。已核对快捷键、Shader单通道与像素读取来源；无版本更新段落，六张既有配图和文档/锚点引用及Markdown/HTML检查通过。允许本地主线集成，不push、不改代码或发布包。

IV-DOC-01 DONE：README提交 `09a19d96af0686f065e8e475fa9e0868e459d735`，本地集成 `f873c642c75afcf01434b363921717e5c75b0731`。主页按当前功能组织，RGBA/Alpha/像素检查优先，无逐版本更新段落；说明O与5/C、键盘A与Alpha按钮区别。主线Markdown/HTML、10个本地引用、6张图片、锚点/通道映射及diff检查PASS；54项workspace测试通过。只改README及协调文档，Rust/图片/版本/安装器与Debug、Release二进制均未变；没有push或重新发布。证据位于忽略目录 `Temp/readme-rgba-review/`。NEXT恢复IV-P1-01。


## IV-DOC-02 执行合同

Base `e953f321f331ae86259fd434972c317fd960d208`；branch `codex/readme-user-page`；worktree `Temp/worktrees/readme-user-page`。Worker只改README，将通道说明缩为简介和快捷键，删除原理/案例教程、源码构建、注册脚本、实现细节、测试/协作信息与重复说明。保留用户可用功能、实机配图、下载/安装、鼠标和快捷键，复用既有图不重新截图。Integrator核对功能和引用、合入main后复验，同步状态；不改代码/版本/安装包、不push或发布。旧工作树冻结。

IV-DOC-02 Review / DONE：README提交 `8e458797f6460f9f6d0883f4cbc481f95fc19207`，仅一份产品文档改变并快进合入main。正文208→107行、字符量减少59.2%；RGBA简述+快捷键，保留功能/配图/格式/下载/操作，移除技术实现、构建脚本和过量教程。分支和主线Markdown/HTML、锚点、6处本地引用、4张图片、快捷键映射与diff检查通过。代码/图片/版本/安装器和现有Debug/Release哈希未变；文档任务未重建程序，无push/发布。证据 `Temp/readme-user-review`；NEXT恢复IV-P1-01。


## IV-P1-N10 合同

Base `f094a58b882dc8131794b977217828c973f486b2`；独立分支`codex/preview-canvas-bounds`，工作树`Temp/worktrees/preview-canvas-bounds`。用户要求WebP/PSD预览图、透明图全画布棋盘格、顶部AABB高亮开关。已只读确认本机PSD等缩略图键存在多余右花括号，WebP未注册；优先修复真实缺陷。图片边界为完整矩形（非非透明像素包围盒），随平移/缩放/Mip，默认关且记住偏好；不改变原像素或原图。缩略图使用直接DLL COM及重定向隔离注册测试，不改用户实时关联或现有安装，不自动发布或打包，保留三项未推送README提交。


IV-P1-N10 Review：Worker `0dddb7437a42f64210c47d9cde2d9d8a2e969964`；62项Rust及12项安装器检查、直接COM12张预览/像素校验、Inno隔离72值与标准GUID、双主题双尺寸80张实际窗口截图通过。顶部按钮点击、缩放平移跟随、纯色恢复及关键截图已检查。未改变用户安装与实时关联；开始本地主线重建复验，不push/发布。


IV-P1-N10 DONE：集成 `9aea77d3596a8e3dc962573c03aa81f38c42ec87`；主线62+12测试/check/双构建，21状态GUI及12个COM预览、Inno72值与标准GUID回归通过。工作分支四组80张实机截图通过，边界含透明留白且不影响像素；B与按钮可用、保存状态、纯色选项保留。用户实际安装/关联和公开版本未更新，无push/打包。详见透明画布与图片边界验收.md。NEXT恢复IV-P1-01。


## IV-P1-N11 执行合同

Base `0af9790e037da570f2faf3ad9c552fe3f116359a`；分支 `codex/subfolder-browse`，工作树 `Temp/worktrees/subfolder-browse`。新增包含子文件夹开关，默认每次运行关闭；开启/显式打开时固定当前图片父目录为搜索根，普通导航及删除不重设根，关闭后回到当前图片同目录。后台单线程扫描，不解码列表中全部图片；新请求覆盖旧请求且协作取消，进度限频。列表设路径数量/估算内存/目录条目/深度/时间保护，触发保护明确显示部分结果；符号链接、junction等重解析点不递归，规范化检查不逃逸根。保持原预读和192MiB缓存边界，不把缓存预算说成进程内存硬上限。顶栏提供开关、范围及进度/部分结果提示。修正删除后的同步目录重扫，使递归范围稳定。用户要求以后提交日志中文：写入AGENTS并从本次所有普通/合并/收尾提交开始执行，不改写历史。仅本地提交与构建，不push/发布/安装/注册。


IV-P1-N11 Review：Worker `f2fd19ba54ac1dc828f55661127d07158b8e4cab`；69项Rust、5,004路径样本和各资源预算/取消测试，真实junction回环、四组明暗/两尺寸87状态及原有菜单25状态通过；子目录回收站删除hash与固定范围已确认。仅约定范围文件改变，提交日志中文，允许本地主线集成与重建，不发布。


IV-P1-N11 DONE：集成 `1780e663d3b424064eb85536557eef7970aa488d`；69+12检查/check/双构建，工作分支四组87状态及菜单25状态、主线38状态GUI通过。后台有界只向下索引，真实junction回环跳过、取消/各保护上限、5,004路径、关闭收窄与删除保持根已验证。AGENTS记录后续中文提交；此前提交与安装/远端不变，未push或打包。NEXT恢复IV-P1-01。


## IV-P1-N12 执行合同

Base `56a8923109aac35868a618f5938086763ad15588`；分支 `codex/relative-path-label`，工作树 `Temp/worktrees/relative-path-label`。顶部标签相对于固定浏览根显示，根目录仍仅文件名，子目录显示层级；标签沿用宽度及中间省略，悬停显示完整相对路径和实际文件路径。仅格式化已有路径，不扫描或重置范围；不改系统、安装、解码、发布。使用新建测试图片、隔离偏好完成双主题两尺寸回归，本地中文提交合入。

IV-P1-N12 Review：工作分支`10b0a245f83bb5fb6e6d6655911c6b0c65feaa39`，仅4个约定文件；73项测试/check/双构建及四组44张实机截图通过。路径转换不访问文件系统，不改变浏览根；允许本地合入并复验，不发布。

IV-P1-N12 DONE：集成 `bf37c8cc83268808bcddf7b20af956d537841194`；73项测试/check/双构建、分支44张和主线22张实机截图通过。根目录文件名、子目录相对路径、关闭范围恢复和完整悬停均验证；只改标签与验证资料，不改变导航或增加文件扫描。本地中文提交，安装/远端未更新。


## IV-FMT-01 执行合同

Base `50c592f56966d1f7f97f7e02ad55d1abc5a4b43c`；用户要求新增AVIF格式。独立分支 `codex/avif-support`，工作树 `Temp/worktrees/avif-support`。只扩展格式识别/解码和现有打开、浏览、缩略图入口，保留RGBA检查与原图；AVIF高位深按既有LDR管线显示8位，动画范围以实测和文档为准，不冒充HDR色彩管理。依赖须许可兼容、可构建；异常数据返回错误，设置尺寸/内存防护。不全仓格式化，不升级无关依赖，不改现有安装/实时关联，不push、打包或发布；仅使用合成图和公开测试样例，保持用户窗口与偏好。完成测试、构建、实机后中文提交、串行本地集成复验。

IV-FMT-01 REVIEW：实现分支 b36e1e7；87项Rust/13项安装契约检查通过，双构建通过；26张真实窗口截图及12个直接COM缩略图输出通过，现由协调者串行合入本地main并复验。

IV-FMT-01 DONE（2026-09-16）：本地主线32775e2集成后复验通过，87项Rust、13项安装契约、check与双构建；主线26张实机截图、12个直接COM缩略图输出通过。详细依据见 docs/formats/AVIF支持说明.md。原有26个本地提交完整保留，不改用户安装/关联，不push或发布。下一待办恢复IV-P1-01。


## IV-FMT-02 执行合同

Base `e329c87ed35217dfd50ff4e9143c91b57dbd3ef1`；branch `codex/animated-avif`，worktree `Temp/worktrees/animated-avif`。复用现有libavif依赖和AnimatedFrame/播放器，不引入新依赖、不重写渲染。Viewer解码全部序列，preview仅解码首帧；逐帧保留Alpha和容器变换，时间基精确换算为毫秒，不套用GIF的100ms短帧修正。动画沿用查看器循环预览习惯，帧数和总RGBA内存设上限，超限/坏后续帧明确报错而非伪装成首帧成功。按需要修复现有播放时钟累计延迟，保持原快捷键/布局。核对全部旧工作树干净并冻结，保留已有33个本地提交；只使用新建样例和隔离QA配置，不修改系统关联、安装或远端。主线复验通过才DONE。

IV-FMT-02 Review：Worker `955d61a77c33d9aeead4f639be3fe79879e70531`，101项Rust/13项安装契约/check/Debug通过，Release已链接且备用路径79张实机及16个COM输出通过。默认Release受用户旧进程占用，没有覆盖。范围核对通过，允许本地集成后复验，不发布或push。

IV-FMT-02 DONE：集成 `54f06ebec0ad39efde59afd74de89099ae43d1f0`；101项Rust、13项安装契约、check/Debug通过；Release编译链接完成，默认EXE复制受用户进程占用，交付备用 `target/animated-avif/release/imageview.exe` 并完成79张主线实机和16个COM输出复验。完整帧/变量时长/透明通道/播放暂停逐帧/宽窗口进度条通过，缩略图保持首帧；只在本地中文提交，不push、不更新安装/关联，默认旧Release不变。NEXT恢复IV-P1-01。


## IV-REL-040 发布合同

Base `0b3cb3cc1ce8209715aa03a89d278e633262993b`；分支 `codex/release-v0.4.0`，工作树 `Temp/worktrees/release-v0.4.0`。用户本轮明确要求打包发布：允许将现有41个本地提交及本轮版本变更推送main，创建新v0.4.0标签与正式Latest Release，保留旧版标签及附件。不强推、不重写历史、不自动升级当前安装或改变实时文件关联，不关闭已有窗口。版本包含静态/动态AVIF、子目录及相对路径、透明画布/图片边界和WebP/PSD缩略图修复。补齐便携包原生许可和验证工具过期计数，正式发布使用独立目标目录静态CRT构建并核验PE导入，避免默认EXE占用与新增C++运行库依赖。验证只使用新建测试文件、隔离偏好、隔离HKCU测试命名空间；先草稿上传核验，再公开发布并下载比对hash。P1长期门禁不自动标通过。

IV-REL-040 分支审查：Worker `42d69b8acb1c5e30cee0ae3db5ee7442abf20305`；101项Rust及18项安装/打包契约检查通过，变更仅版本、文档与构建打包工具，未改变业务解码/UI。新依赖许可纳入便携ZIP白名单；正式版使用静态CRT和PE导入守卫，主线构建及最终包验证尚待执行。


IV-REL-040 DONE：v0.4.0于2026-09-16T14:18:52Z正式发布并设Latest，ID 389994098，源码标签 `1649569a903e3620b60f3699ea0f89c176900014`；main及新标签已推送，三附件公开下载SHA256一致。101项Rust/18项契约/check/静态CRT构建、146张合格实机截图、28个COM输出和76值隔离注册及载荷安装卸载通过。当前安装和实时关联保持，不强推或覆盖旧附件；详见v0.4.0-validation.md，NEXT恢复IV-P1-01。


IV-P1-N13 Review：顶部置顶/背景、偏好与窗口对话框适配已提交；105项Rust/18项配置检查通过，主体Release四组88状态及修复后Debug70状态通过。保留原生父窗口及Esc取消回归；只合入本地主线，重新构建实机复验，不发布或安装。


IV-P1-N13 DONE：集成 `9fc85ba48c264ae7ccb24bc96117a0666816c5b5`；105项Rust/18项配置/check/双构建及当前Release92张实机截图通过。置顶/原生文件对话框/取消Esc、背景预设与RGB、偏好兼容、可见窄宽布局及AVIF回归完成。当前入口 `target/topbar-controls/x86_64-pc-windows-msvc/release/imageview.exe`，仅本地中文提交，不推送/打包/安装。NEXT恢复IV-P1-01。

IV-P1-N14 REVIEW：分支 `bd8964d48ad834f2bc8b0a3676396e3b19cca67e`；110项Rust/18项配置/格式/check及深色Debug完整交互与像素回归通过，接下来主线Release和双主题复验。

IV-P1-N14 DONE：集成 `98abd01e4abbdf34b1a70c0fc4c66d55d137153d`，110项Rust/18项配置/check/Release及74张主线实机通过。调色板/色相/HEX/快捷色与旧预设、置顶、持久化兼容。中文本地提交，不push或更新安装。

## IV-REL-050 执行合同

Base `dce3e4ab520fddff5f45459161ad23fd42fc64bd`；branch `codex/release-v0.5.0`，worktree `Temp/worktrees/release-v0.5.0`。本轮允许打包发布、推送现有本地功能及中文新提交、创建v0.5.0正式Latest并保留旧版本。README面向用户，每项主要功能配当前版本实机图；RGBA各通道、动画播放/暂停/逐帧展示，公开素材仅项目自有和合成测试图片，截图不能暴露本机路径。发布使用独立静态CRT构建、固定源码hash、110项现有Rust和配置测试、隔离安装与便携运行验证，公开下载复核hash。不更改系统关联或当前安装、不关闭用户窗口、不强推。

IV-REL-050 REVIEW：分支 `50360dc91986ecf1a1b497ac1b3cd84a78d72bc2`，业务源码与已截图0.5.0版本一致；110项Rust及24项配置/README检查通过。11组功能图解含10张静态图和1段实际播放暂停逐帧GIF，公开图片不含本机路径。准备串行合入后重新构建正式包。

IV-REL-050 DONE：v0.5.0 Latest正式发布，ID 392340200；源码 `0fb8b97049bb1f4ee974a8aae857e1ea897e6209`，README11组图文和公开图片hash核验通过，110项Rust/24项检查/正式包与便携复验通过。保留旧版，不更新当前安装；NEXT IV-P1-01。
