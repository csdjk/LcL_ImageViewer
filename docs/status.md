# LcL ImageViewer — 当前状态

> 更新时间：2026-09-15。以Git、当前构建与GitHub实际回读为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-FMT-01 -->

## 最新正式版本

**v0.3.1 已按本轮用户明确要求发布，Latest、非草稿/预发布。**

发布页：https://github.com/csdjk/LcL_ImageViewer/releases/tag/v0.3.1；ID `389181525`，发布时间 `2026-09-15T13:44:36Z`。
源码标签：`97fcf8e4309d8fe3d2e17c5c47cd216729735e34`；最终文档HEAD见Git。原v0.3.0文件保留，不覆盖旧附件。

## 已纳入安装包的改进

- 图片加载加速：首图解码与窗口/GPU初始化并行；目录后台扫描、导航复用；最新请求优先、有限去重预读；减少像素拷贝。静态WebP分支错误已修复。性能仅承诺已记录测试范围，不降低分辨率或Alpha。
- 打开方式安装修复：明确写入71个字符串值，通知Explorer刷新；对应安装任务需勾选。不抢占系统默认应用。
- 默认D盘：全新安装建议D:\Program Files\LcL ImageViewer，无D盘回退localappdata\Programs；升级沿用原位置且目录页可更改，不自动迁移。
- 保留新拟态主题、两侧磨砂导航、1秒隐藏、精简菜单/设置、A/D、Delete确认回收站、左键移图、画布右键拖主窗口及设置弹窗独立拖动。

## 发布与验证

主线54项Rust测试+11项安装器检查、check/Debug及Release workspace构建通过。真实Inno隔离71值写入、旧失效路径升级、其他应用哨兵保留、EXE/DLL载荷hash和卸载通过；默认目录解析通过。深色880×560设置16状态、浅色1280×860菜单10状态实机通过；ZIP解压启动通过。所有GUI验证DPI96/100%。

安装包5983996字节，ZIP5427566字节，另附SHA256SUMS.txt；已上传并公开下载逐项核对hash。完整记录见 `docs/releases/v0.3.1-validation.md`。本地统一入口 `dist/v0.3.1/LcL-ImageViewer-Setup-v0.3.1-win64.exe`，不再需要旧openwith-fix/default-d分散安装包。

Release程序 `0b6ce0d06bac1927a7a5475eafef3fe8bb22e63d9ac9eb7e56a1877157fc5bf1`；Debug程序 `ea28d50cc4ff22f4b0de21e98b88992aa82cfaf0507d20859359d61fd2a8353d`。两者均0.3.1，位于target/release和target/debug。未自动升级用户现有安装。

## NEXT与限制

NOW: IV-P1-01 READY — 完善通用验收入口。
COMPLETED: N01–N09、INSTALL-01/02、PERF-01、REL-030、REL-031 DONE。

P1保持ACTIVE，不将此次授权发布当作所有长期门禁完成。未配置代码签名；原iv-shell LNK4104警告保留。多DPI/混合缩放/跨屏、冷盘及全部格式/权限、生产环境完整安装卸载矩阵未全覆盖。注册及载荷测试在隔离命名空间执行，不等于修改当前用户的真实打开方式菜单；没有更改现有安装、UserChoice或其他应用的关联。

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


IV-FMT-01 IN_PROGRESS：按用户新需求增加AVIF解码、查看/切图及缩略图集成；使用独立工作树，不改现有安装与远端。
