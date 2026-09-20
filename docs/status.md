# LcL ImageViewer — 当前状态

> 更新时间：2026-09-18。以Git、当前构建与GitHub实际回读为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-N14 -->

## 当前任务

IV-P1-N14 IN_PROGRESS：自定义背景调色板，独立工作树开发，本轮不发布或更新安装。

## 本地新增：顶部置顶与背景颜色（未发布）

IV-P1-N13 DONE：图钉开关及背景菜单已完成。支持棋盘格/跟随主题/黑白灰/自定义RGB；置顶及背景选择保存，原设置入口已移至顶部。原生文件对话框归属于查看器，取消时Esc不再穿透退出。

集成 `9fc85ba48c264ae7ccb24bc96117a0666816c5b5`；105项Rust、18项配置检查、check/Debug/Release及当前Release92张实机截图通过，覆盖两主题/两尺寸、7档可见布局、原生置顶、保存恢复、颜色/通道及AVIF动画。验收见 `docs/ui-qa/顶部置顶与背景菜单验收.md`。

最新本地程序：`target/topbar-controls/x86_64-pc-windows-msvc/release/imageview.exe`。本轮未推送、打包或更新用户安装，下面的v0.4.0公开发布包仍不包含本轮增量。

## 最新正式版本

**v0.4.0 已正式发布并设为Latest，非草稿/预发布。**

发布页：https://github.com/csdjk/LcL_ImageViewer/releases/tag/v0.4.0；ID `389994098`；发布时间 `2026-09-16T14:18:52Z`。源码标签 `1649569a903e3620b60f3699ea0f89c176900014`。main及v0.4.0已推送，旧版保留，公开附件下载SHA256一致。

包含静态/动态AVIF与透明通道、子目录和相对路径浏览、全画布棋盘格与图片边界、WebP/PSD/AVIF缩略图及动画计时修复。之前本地未发布的增量已统一纳入本版，详见 `docs/releases/v0.4.0.md`。

## 本轮验证与产物

101项Rust、18项打包/安装契约、workspace check、正式静态CRT构建通过；146张合格实机截图、28个COM缩略图输出、76项隔离注册及EXE/DLL安装卸载hash验证完成，ZIP解压启动通过。完整范围和一次未确认原因的初始暂停采样见 `docs/releases/v0.4.0-validation.md`。

安装版：`dist/v0.4.0/LcL-ImageViewer-Setup-v0.4.0-win64.exe`。
免安装版：`dist/v0.4.0/LcL-ImageViewer-v0.4.0-win64.zip`。
校验：`dist/v0.4.0/SHA256SUMS.txt`。

正式构建位于 `target/distribution/x86_64-pc-windows-msvc/release`；不能把旧 `target/release`、`target/animated-avif` 或现有安装目录视为已升级。本轮只发布，不自动替换用户安装、关联或旧窗口。

## NEXT与限制

IV-REL-040 DONE。NOW: IV-P1-01 READY — 完善通用验收入口。

P1保持ACTIVE。未做代码签名；保留既有iv-shell LNK4104提示；完整多DPI/跨屏及生产安装矩阵未全覆盖。AVIF 10/12位转RGBA8，HDR/ICC及按源文件有限循环次数自动停止未实现。发布文件使用静态CRT，导入表未包含外部VC运行库或AVIF解码DLL。

## 历史增量记录

以下“未push/未发布”等描述为各任务当时状态；上述增量现在均已纳入v0.4.0并推送，当前状态以上方正式发布信息为准。

## README 当前功能说明（仅本地提交）

IV-DOC-01 DONE：README不再按版本介绍更新；开头突出RGBA单通道灰度、Alpha/忽略透明度和像素读取，保留当前全部功能、格式/安装/快捷键及使用边界。复用既有实机示例，不冒充重新截图。README提交 `09a19d9`、集成 `f873c64`；主线文档检查及54项workspace测试通过。仅README与协调文档改变，程序和已发布v0.3.1不变；本次未push，GitHub首页尚未同步。


## README 用户首页精简

IV-DOC-02 DONE：用户版README已提交 `8e458797f6460f9f6d0883f4cbc481f95fc19207` 并合入本地main。正文从208行缩至107行；只保留功能概览、界面预览、格式、下载和操作，RGBA不再展开教程。文档结构/引用/图片与快捷键核对通过。上一轮文档已推送，本轮新改动尚未push；现有程序、安装包、标签与Release不变。


## 本地新增：缩略图、透明画布与图片边界

IV-P1-N10 DONE，集成 `9aea77d3596a8e3dc962573c03aa81f38c42ec87`。WebP缩略图注册补齐；修复PSD等槽GUID多余右花括号及PSD额外通道RLE；缩略图只取动画首帧并按Alpha正确缩小。透明图棋盘格铺满画布，保留纯色；顶部图片边界开关/B快捷键，包含透明留白、跟随缩放平移、默认关且持久化。

62项Rust+12项安装器检查/check/双构建、分支80张及main21状态真实GUI、12个COM缩略图与像素检查、72值隔离注册验证通过。常规target/release与target/debug均更新；Release SHA256 `2fb2f900abaa036541afb68fbbe7ae847695eb84a9a194af2df17d8c5f7ad739`。无push/打包/实际安装与关联修改；公开v0.3.1包仍是旧包，Explorer实际启用需要后续更新安装和注册，不是运行新EXE就完成。PSD ZIP/CMYK/PSB及其他DPI等边界见验收文档。


## 当前任务

IV-P1-N11 DONE，集成 `1780e663d3b424064eb85536557eef7970aa488d`：顶部包含子文件夹/S开关，默认每次运行关闭，固定根向下索引；原左右按钮及快捷键跨子目录浏览，删除不丢根。后台扫描进度/取消与5万张、32MiB路径预算等保护；超限显示部分，目录链接不跟随。69+12检查和双构建、分支112及主线38状态实机回归通过，100%缩放。AGENTS已规定今后中文提交，本轮所有提交中文；常规Debug/Release更新，不改用户安装/关联，不push或发布。


IV-P1-N12 DONE：顶部以浏览根显示相对路径，根目录仍为文件名，保留中间省略，悬停完整相对路径与实际路径；关闭子目录模式即恢复文件名。集成 `bf37c8cc83268808bcddf7b20af956d537841194`，73项测试/check/双构建、分支44张及主线22张实机截图通过。Debug/Release已更新，当前安装及远端未变，提交均中文。


IV-FMT-01 DONE：AVIF解码、透明通道、查看/切图及缩略图集成已合入本地main并完成主线复验；未修改现有安装或远端。


## AVIF 静态支持阶段（2026-09-16，FMT-01）

IV-FMT-01 已完成：静态/透明AVIF，8/10/12位转RGBA8，容器裁剪/旋转/镜像、动态首帧，文件对话框/目录导航/缩略图/关联脚本同步。动态播放和HDR/ICC色彩管理不在本轮支持范围。

主线集成32775e2：87项Rust回归、13项安装契约检查、workspace check及Debug/Release构建通过；两主题实际窗口26张截图与12个直接COM缩略图输出复验通过，用户偏好和样例原文件未变。详情 docs/formats/AVIF支持说明.md。可运行 target/release/imageview.exe；当前安装、已发布版本和远端未更新。


IV-FMT-02 DONE：动态AVIF完整帧播放已合入本地main并完成复验，静态支持与首帧缩略图保持。


## FMT-02 动态 AVIF 开发阶段记录（发布前）

动态AVIF自动循环播放、Space暂停/恢复、逗号/句号前后逐帧、宽窗口帧进度条已接通；每帧透明度、时间基和容器变换保留。改进动画时间轴，避免累积重绘延迟。有4096帧及整段RGBA含首帧副本256MiB防护，缩略图仅解码首帧；HDR/ICC和按文件有限循环次数自动停止未实现。

集成 `54f06ebec0ad39efde59afd74de89099ae43d1f0`；主线101项Rust/13项安装契约/check/Debug通过，Release编译链接完成。默认Release EXE被用户旧进程占用，Cargo复制阶段返回101，原路径未更新；本轮新程序请运行 **`target/animated-avif/release/imageview.exe`**。备用Release实机双主题79张截图及16个直接COM缩略图输出通过，详见 `docs/formats/AVIF支持说明.md`。中文提交已本地保存，没有push、发布安装包或修改安装/系统关联。
