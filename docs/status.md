# LcL ImageViewer — 当前状态

> 更新时间：2026-10-10。以Git、当前构建与GitHub实际回读为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-01 -->

## 当前最新正式版：v0.7.1

IV-REL-071 DONE。2026-10-10T04:31:56Z发布，Release ID 408644251；GitHub Latest、非草稿/预发布。发布页：https://github.com/csdjk/LcL_ImageViewer/releases/tag/v0.7.1；源码标签`b132da4f4683d4f3742685a28184879f12385f6a`。

修复大动画展开超过256 MiB就被误报格式不支持的问题，完整帧自动使用私有临时文件映射，保留分辨率/透明度/帧时长并自动清理。165项Rust、1项大动画专项、50项Python、5项Mac边界检查通过；正式静态CRT、76项隔离注册/载荷安装卸载、46张正式与免安装实机截图通过。三个公开附件及16个README/媒体/许可资源hash一致。详见`docs/releases/v0.7.1-validation.md`。

本地安装包：`dist/v0.7.1/LcL-ImageViewer-Setup-v0.7.1-win64.exe`；免安装ZIP同目录。没有自动安装或更改默认打开方式，macOS包本次未更新。

## 大动画自动临时缓存（已纳入v0.7.1）

IV-ANIM-01 DONE：修复GIF/APNG/WebP/动态AVIF全部帧展开超过256 MiB就被误报为不支持格式的问题。小动画使用内存，大动画超过64 MiB帧保留阈值自动写入私有临时文件并只读映射；保留原始分辨率、全部帧、透明度与时长。缓存最后引用释放后清理；新增明确的资源/临时目录错误提示。原图、现有编辑/OCR及用户安装不变。

165项常规Rust测试、1项覆盖三种288 MiB动画全部216帧的专项、50项Python及5项Mac边界检查通过；当前Release四组深浅主题/窗口尺寸共37张实机截图验证播放、暂停、逐帧、单通道与当前帧编辑预览，一次原生透明PNG导出通过。main合入b16215e后再次测试/check/Python和真实窗口复验通过，运行源码与已验收Release完全一致。运行 `target/large-animation-cache/release/imageview.exe`；证据与限制见 `docs/ui-qa/大动画缓存验收.md`。

64 MiB仅是帧堆数据转存阈值，不是进程总内存上限；系统映射驻留页、GPU与解码工作区另计。需要系统临时目录有空间；首次打开仍需完整解码，单动画展开数据最多8 GiB、4096帧。修复现已纳入v0.7.1并推送；用户安装不会自动更新。

## 上一正式版：v0.7.0

IV-REL-070 DONE。2026-10-08T08:56:40Z 正式发布，Release ID 406624457，Latest、非草稿/预发布。发布页：https://github.com/csdjk/LcL_ImageViewer/releases/tag/v0.7.0；标签源码 `9b0eba0f5c150b14092c53f72b0fc7d9014341ab`。

包含主画布裁剪/旋转翻转/分辨率与统一编辑控件、像素格式和单通道读数、顶部按钮精简、Windows本机OCR和复制。160项Rust、50项Python、5项Mac边界、静态CRT、76项隔离注册与82张实机截图通过；三个公开附件及README引用资源hash核验通过。详见 `docs/releases/v0.7.0-validation.md`。

本地安装包 `dist/v0.7.0/LcL-ImageViewer-Setup-v0.7.0-win64.exe`，便携ZIP同目录。用户当前安装/默认程序未自动修改；本次未发布新的macOS DMG。下方“未发布”是历史任务阶段记录，上述功能现均已纳入v0.7.0。


## OCR收尾完成（已纳入v0.7.0）

IV-OCR-01 DONE：Windows本地识别、顶部结果面板、右键及Ctrl+Shift+C识别复制、语言选择、校对与换行复制已完成。160项Rust、50项Python、主线check及同源码二进制运行复验通过，真实OCR四种主题/尺寸及剪贴板读回/恢复通过，README新增实机图。英文相似字符仍可能误识别，不承诺识别率；macOS暂无OCR。本次用户已授权后续v0.7.0打包发布，实际公开结果见后续发布记录。


## 本地优化：编辑器控件统一（未发布）

IV-EDIT-03 DONE：顶部工具、操作按钮、数值输入、下拉框、比例/网格开关和底部确认统一新拟态表面，32点高度、8点间距，参数标签垂直居中。主要/次要/丢弃动作分层；提示行固定40点，待应用提示不挤压画布。编辑算法、保存和原图数据不变。

151项Rust、44项Python、check和Release构建通过；深浅主题与880×560/1280×860共64张实机截图及4次原生PNG导出逐字节验证通过，主线再次测试和原生窗口复验通过。运行 `target/editor-controls-style/release/imageview.exe`；旧inline-image-editor构建不包含本次样式。README功能配图与验收记录已同步，详情 `docs/ui-qa/编辑器统一控件验收.md`。未push、打包发布、替换当前用户安装或修改配置。

## 本地新增：裁剪与修改分辨率（未发布）

IV-EDIT-01 DONE：顶部编辑按钮、E 或右键菜单进入独立编辑窗口。支持拖框/移动/四角调整与坐标宽高精确裁剪，居中比例裁剪、分辨率比例锁定、常用倍率、双线性/最近邻，以及撤销/重做/重置。后台另存为新的8位RGBA PNG，不覆盖原图或已有文件。单通道查看不会改变导出原始RGBA，透明通道保留；动画仅当前帧，DDS取Mip0，HDR浮点暂不编辑。

运行 `target/basic-image-editor/release/imageview.exe`；包含之前顶部按钮精简和底栏像素格式。输入/输出单边最多16384、总像素最多16777216。141项Rust、36项Python、5项Mac边界与Release通过，双主题双尺寸50张最终版本应用截图、17个原生PNG导出均核验；合入main后再次测试/check和真实窗口/保存取消复验通过。运行源码与已测Release一致，复用同一二进制。详情 `docs/ui-qa/裁剪与分辨率编辑验收.md`。本次未push、打包发布、改用户安装或文件关联。

## 本地优化：原窗口主画布编辑（未发布）

IV-EDIT-02 DONE：取消独立编辑弹窗，顶部上下文工具栏配合原窗口主画布直接操作。支持框选裁剪与8手柄/比例、左右90°及任意角度旋转、水平/垂直翻转、鼠标中心缩放和平移、真正修改像素分辨率与最近邻/平滑采样；顺序编辑可撤销/重做，草稿切换与退出保护保留。

代码集成 `a9464e1`；148项Rust、40项Python和5项Mac边界检查通过。最终Release双主题双尺寸115张截图、42个实际PNG导出通过，旋转预览和应用画布像素差四组均为0；主线重新测试及16张原窗口复验通过，正常用户配置和源图未改变。运行 `target/inline-image-editor/release/imageview.exe`，包含此前顶部精简和像素格式功能；旧basic-image-editor构建是旧弹窗实现，不代表本次更新。详情 `docs/ui-qa/主画布编辑验收.md`。

本轮未push、打包、发布或更新已安装版本。8位RGBA PNG另存、动画当前帧/DDS Mip0/HDR边界及其他DPI限制见验收记录。

## 本地优化：底栏像素格式与单通道读数（未发布）

IV-P1-N17 DONE：底栏坐标用 `(x, y)` 括起；设置增加归一化RGBA、0–255整数RGBA、HEX三种格式并持久化，旧偏好默认HEX。R/G/B/A模式只显示对应分量，色块同步灰度；原始像素、完整检查器/HDR浮点与右键复制HEX不变。底栏槽宽按格式及Mip固定，动画取当前帧。

131项Rust、31项Python检查、cargo check和Release构建通过。双主题双尺寸176张格式/通道截图与12次同配置重启通过；动画/APNG/AVIF逐帧、DDS多Mip及主线复验记录见 `docs/ui-qa/底栏像素显示格式验收.md`。当前可运行 `target/pixel-readout-formats/release/imageview.exe`，包括上次顶部实际大小按钮精简。运行代码集成51adb52；已发布v0.6.0与当前用户安装未更改，未push。

## 本地优化：顶部保留实际大小按钮（未发布）

IV-P1-N16 DONE：删除顶部“适配窗口”按钮及无用图标，仅保留“实际大小”；工具栏自动收紧，无空白占位。F 适配窗口、0 实际大小和打开/切图自动适配继续可用，不改 v0.6.0 自动刷新或视图锁定逻辑。

124 项 Rust、27 项契约、check 与 Release 构建通过；双主题、880×560/1280×860 实机及主线复验通过，使用实际图像像素边界确认按钮/0 恢复 100%，F 与初始适配一致。运行 `target/topbar-actual-only/release/imageview.exe` 查看；已发布 v0.6.0 和用户安装未自动更新。详情 `docs/ui-qa/顶部缩放入口精简验收.md`。

## 上一正式版：v0.6.0

IV-REL-060 DONE。2026-09-22T23:47:02Z 正式发布，Release ID 394194384；GitHub Latest、非草稿、非预发布。发布页：https://github.com/csdjk/LcL_ImageViewer/releases/tag/v0.6.0；源码标签 `4089599b6361103b686c2a4ceda12cf0ba63d82a`。

`IV-TA-01` DONE，集成 `916de47`：修正 DDS 格式与标准 Mipmap/Volume 偏移，统一动画保留帧预算，增加当前图片自动刷新、F5 与视图锁定。124 项 Rust、24 项 Windows 契约、5 项 macOS 边界检查通过。正式 Windows 静态 CRT 构建、隔离安装/卸载、正式程序及 ZIP 解压程序共 41 张实机截图、三个公开附件重新下载校验均通过。详见 `docs/releases/v0.6.0-validation.md` 与 `docs/ui-qa/轻量贴图检查首轮验收.md`。当前用户安装及文件关联未自动更改。P1 保持 ACTIVE，NEXT 恢复 IV-P1-01。

## 上一正式版：v0.5.1

IV-REL-051 DONE。2026-09-20T11:26:21Z 正式发布，Release ID 392408919，Latest、非草稿/预发布。发布页：https://github.com/csdjk/LcL_ImageViewer/releases/tag/v0.5.1；源码标签 `7bce74beada33ab19ad38ce18f722bcc4236d327`。

本版将左右切图按钮由 48×64 圆角矩形改为 56×56 真圆，玻璃模糊遮罩、描边、阴影和焦点轮廓同步圆形化；点击切图、禁用态及 1 秒自动隐藏/恢复保持。110项Rust、24项Windows契约、5项macOS边界检查、静态CRT正式构建、76项隔离注册及15张正式/便携实机截图通过，三个公开附件重新下载SHA256一致。详见 `docs/releases/v0.5.1-validation.md`。

本地安装包：`dist/v0.5.1/LcL-ImageViewer-Setup-v0.5.1-win64.exe`；便携包同目录。当前用户安装未自动更新。

## macOS 预览构建

IV-MAC-01 DONE；Apple Silicon / Intel DMG 构建流程继续保留，无 Developer ID / 公证凭据。本次 v0.5.1 正式 Release 的附件为 Windows 版本，没有把旧 macOS 预览 DMG 改名发布。

## 上一正式版：v0.5.0

v0.5.0 于 2026-09-20T06:49:57Z 发布，Release ID 392340200；包含顶部置顶、背景调色板和 README 功能图解等更新。当前 Latest 已由 v0.5.1 取代，旧标签与附件继续保留。

## 历史开发记录

下方“未发布/未push”等是各任务当时状态；顶部置顶和调色板现已统一纳入v0.5.0并推送。


## 本地新增：自定义背景调色板（未发布）

IV-P1-N14 DONE：自定义RGB滑条替换为二维色板、色相条、颜色预览、快捷色块和HEX输入，实时预览、非法输入保护及保存恢复通过。集成 `98abd01e4abbdf34b1a70c0fc4c66d55d137153d`；110项Rust、18项配置、check/Release与74张主线实机验证通过。

最新本地程序仍为 `target/topbar-controls/x86_64-pc-windows-msvc/release/imageview.exe`。详见 `docs/ui-qa/背景调色板验收.md`。当前安装和公开v0.4.0包不自动更新；未推送或发布。

## 本地新增：顶部置顶与背景颜色（未发布）

IV-P1-N13 DONE：图钉开关及背景菜单已完成。支持棋盘格/跟随主题/黑白灰/自定义RGB；置顶及背景选择保存，原设置入口已移至顶部。原生文件对话框归属于查看器，取消时Esc不再穿透退出。

集成 `9fc85ba48c264ae7ccb24bc96117a0666816c5b5`；105项Rust、18项配置检查、check/Debug/Release及当前Release92张实机截图通过，覆盖两主题/两尺寸、7档可见布局、原生置顶、保存恢复、颜色/通道及AVIF动画。验收见 `docs/ui-qa/顶部置顶与背景菜单验收.md`。

最新本地程序：`target/topbar-controls/x86_64-pc-windows-msvc/release/imageview.exe`。本轮未推送、打包或更新用户安装，下面的v0.4.0公开发布包仍不包含本轮增量。

## v0.4.0 发布时记录

v0.4.0 为此前正式版本，当前最新正式版是上方的v0.5.0。

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

## 当前任务：v0.5.0发布与功能配图

IV-REL-050 DONE：v0.5.0及README功能图文已正式发布，详情以上方最新发布与验收文档为准。

## macOS预览安装包已完成

IV-MAC-01 DONE。两架构DMG来自GitHub运行35497058297，实际构建源e39c1b22460d16bc99eb56238cc84c9d06d05782，通过原生测试、挂载启动、动效像素检查及下载hash验证。未公证、仅ad-hoc签名；不包含Windows磨砂/Explorer扩展，Mac预览从应用内打开图片。安装包位于dist/macos-v0.5.0，详情docs/releases/macos.md。main必要复验通过，长期阶段不自动升级。
