# LcL ImageViewer — 当前状态

> 更新时间：2026-09-14。以磁盘、Git和可重复实机结果为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-N08 -->

## 当前实现

P0 VALIDATED；P1 UI Stabilization ACTIVE；P2仍受P1门禁约束。
N01新拟态、N02的1秒隐藏、N03两侧玻璃导航、N04左键移图/右键移窗、N05打开所在文件夹、N06精简菜单/图片快捷键均已独立提交、合入main且复验。最新功能集成 `85bb27e077d62356f2e6957f6fd2e0b0834d0e6f`；最终文档HEAD通过Git读取。

右键菜单保留复制像素/文件路径、打开所在文件夹、删除图片、图像属性、像素检查器、打开文件及设置。移除重复的切图/视图/通道/主题/格式工具菜单，功能仍由顶部工具栏或快捷键提供。
新增A/D切图，保留原方向键；输入焦点、弹层、组合键受到保护。Delete和菜单共用确认弹层，后台只移入回收站，不提供永久删除回退。取消/失败保留文件，删除成功后下一张、末尾上一张、最终空态；同步目录和缓存。
保留打开目录修复、鼠标手势、侧边玻璃、1秒隐藏和原图解码/Shader。

## 最新验证与运行入口

- main35tests/0失败，workspace check、Debug和Release构建通过。
- 分支4组双主题/双尺寸实机100张最终截图；main Release再验浅色1280×860与深色880×560两组共50张，全部DPI96/100%。
- 每组用新建3张PNG验证A/D/边界、修饰键与弹层隔离、取消/Delete/菜单删除、下一张/末尾/空态，并在回收站核对3张原始文件SHA256。未操作用户原图或清空回收站；测试图片留在回收站可恢复。
- Release（常规路径已更新）：`target/release/imageview.exe`，SHA256 `2f8bc620af2890aabc47f3c229a64acf6b53ad9b3eb27a622f4acccc270353dc`。
- Debug（已同步）：`target/debug/imageview.exe`，SHA256 `05df3be3dde7598aea77b713b2e6c73249c9c9bf5b207f5bfe2aa91b5b2cdfbc`。
- 之前的 `target/folder-fix/release/imageview.exe` 是旧副本，未覆盖正在使用的窗口；查看本轮功能请重启上述常规Release。
- 实机测试前后正常用户配置SHA256一致。不改系统关联、快捷方式或安装目录，不push/发布。
- 证据：`docs/ui-qa/菜单快捷键验收.md`、`ui-verify-shots/menu-controls-final-*`、`menu-controls-main-*`。其中分支深色880以 `-retry` 目录为最终证据；早期失败轮次不计入成功数量。

## NEXT

NOW: IV-P1-N08 IN_PROGRESS — 设置页精简和标题拖窗；N07已经集成，随本轮串行复验收尾。

COMPLETED: IV-P1-N01 / IV-P1-N02 / IV-P1-N03 / IV-P1-N04 / IV-P1-N05 / IV-P1-N06 DONE。

## 限制

网络共享/无回收站设备/超大文件/全部权限情形未实机遍历；禁止非回收操作有回调守卫及单元测试，不扩大为全部设备已验证。其他DPI、跨屏、完整性能/辅助功能仍未覆盖。原历史格式差异未全仓清理，P1保持ACTIVE。用户旧运行窗口不会热更新，其他安装副本不自动更新。
