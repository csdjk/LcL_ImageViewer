# LcL ImageViewer — 当前状态

> 更新时间：2026-09-14。以磁盘、Git和可重复实机结果为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-N09 -->

## 当前实现

P0 VALIDATED；P1 UI Stabilization ACTIVE；P2仍受P1门禁约束。
N01新拟态、N02的1秒隐藏、N03两侧玻璃导航、N04鼠标手势、N05打开所在文件夹、N06菜单快捷键与回收站删除、N07无分类右键菜单、N08紧凑设置及标题拖窗均已独立提交、合入main并复验。最新功能集成 `d3299f57c1c0680beb2cfc4eec884a4239d62445`；最终文档HEAD通过Git读取，不自引用。

设置页为无分类单层列表，主题、透明背景、减少动效、桌面磨砂、打开方式、默认看图软件六项保留；说明为悬停提示。磨砂开启时展开不透明度/模糊强度/背景亮度三条参数，原有范围、偏好键和错误诊断保留。移除分类大卡片、材质说明和冗长页脚。

设置标题及顶部空白处按住左键或右键移动整个原生窗口，使用屏幕物理坐标差保持跟手；内部设置面板不会独立移动。标题与关闭按钮相隔8点；正文、开关、滑条和关闭操作不会误拖窗口。保留画布左键移图/右键移窗、A/D、Delete确认、打开所在文件夹、两侧磨砂导航及1秒隐藏。

## 最新验证与运行入口

- main `cargo test --workspace --offline`：37 passed / 0 failed；workspace check、Debug及Release构建通过。
- N08最终源码四组104张真实Windows截图：深浅主题、1280×860/880×560。main重建Release后两组52张设置回归，另有N07无分类菜单10张专门复验。全部DPI96/100%。
- 标题左键、标题右键、顶部空白左键实际窗口位移分别72×36、-48×24、-36×-24；正文/开关/滑条窗口位移均为0。控制项切换、实际保存参数、关闭、A/D、Delete确认取消、菜单、隐藏恢复通过，测试图片SHA256不变。
- Release（常规路径）：`target/release/imageview.exe`，SHA256 `e34ec44c7c3a67adc09fd16e75bb0bd9468dfec9b9520d99cbf9fc8636a08ef7`。
- Debug（已同步更新）：`target/debug/imageview.exe`，SHA256 `28b410f41e32053c515dba1aedda9fa27fe672f91fa9af165c1c56726d04591c`。
- 主线测试前后正常用户配置SHA256一致。不关闭用户已有窗口，不执行注册/注销/默认应用，不改快捷方式、关联或安装目录；没有push或发布。
- 当前验收：`docs/ui-qa/紧凑设置页验收.md`、`docs/ui-qa/无分类菜单验收.md`。最终证据在 `ui-verify-shots/settings-stable-*`、`settings-main-*` 与 `flat-menu-main-finish-dark-880`。早期失败或试验目录不作为成功证据。

## NEXT

NOW: IV-P1-N09 IN_PROGRESS — 纠正为设置弹窗内部拖动，主窗口固定。

COMPLETED: IV-P1-N01 / IV-P1-N02 / IV-P1-N03 / IV-P1-N04 / IV-P1-N05 / IV-P1-N06 / IV-P1-N07 / IV-P1-N08 DONE。

## 限制

未覆盖其他DPI、跨屏、全部最大化/全屏手势、桌面捕获性能及恢复全矩阵、完整辅助功能。设置页系统集成按钮只检查布局，不执行系统写入。回收站多设备/网络路径限制仍见原验收记录，本轮不重复删除测试。全仓历史格式差异保留，P1不标为VALIDATED。正常运行路径已更新；旧窗口不会热更新，其他安装目录与folder-fix副本不自动更新。
