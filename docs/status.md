# LcL ImageViewer — 当前状态

> 更新时间：2026-09-14。以磁盘、Git和可重复实机结果为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-N05 -->

## 当前实现

P0 VALIDATED；P1 UI Stabilization ACTIVE；P2仍受P1门禁约束。

N01新拟态主题、N02的1秒隐藏、N03两侧玻璃导航、N04鼠标手势均已在独立工作树完成、串行合入main并复验。最新功能集成提交 `dec4c0abf81a873277fbc5d22cb2485f5c5fbad5`；最终状态文档HEAD通过Git读取。

画布左键/中键拖拽平移图像，右键拖拽移动整个窗口；右键单击仍显示菜单，右键拖出再回原位不弹菜单。保留顶部工具栏空白左键拖窗、左键窗口边缘缩放、方向键与两侧玻璃按钮切图、滚轮缩放和1秒隐藏。原图解码、Shader、取样及用户主题偏好不变。

左右按钮保持窗口两侧居中、半透明真实场景模糊；其余UI为新拟态。首末无效方向禁用，顶栏无重复箭头。

## 最新验证

- main上24tests / 0失败；workspace check、Debug与Release构建通过。
- N04最终分支源码48张实际Windows截图：双主题、1280×860/880×560，DPI96。
- main重建Release后24张实机截图：浅色1280与深色880，各12状态。真实几何与像素检查通过：左键80×45平移不移动窗口，右键72×36只移动窗口，图像相对位置不变；右键拖出返回/单击菜单、Esc关菜单、中键兼容、侧导航、边缘缩放及隐藏恢复通过。
- 证据为 `docs/ui-qa/鼠标拖拽验收.md`、`ui-verify-shots/mouse-final-*`、`ui-verify-shots/mouse-main-*`；逐张JSON记录commit、程序/输入hash、窗口标题、DPI、尺寸、窗口外框、真实拖拽轨迹。
- Release：`target/release/imageview.exe`，SHA256 `f371791ff6e6583a1eedb764b07b8dde86d7cca91880461517d0b780bd3c74e3`。
- Debug：`target/debug/imageview.exe`，SHA256 `f96429a241c506ab2970d20ba2a9c7f9129c64f1836f7c7c173a5c3dc62895e2`。本次两种程序均更新，旧运行窗口不会自动热更新。
- 使用隔离QA profile，不操作用户已有窗口/文件关联/安装目录/快捷方式。没有push或发布。

## NEXT

NOW: IV-P1-N05 IN_PROGRESS — 修复为直接打开当前图片所在目录；之后恢复IV-P1-01。

COMPLETED: IV-P1-N01 / IV-P1-N02 / IV-P1-N03 / IV-P1-N04 DONE。

## 保留限制

全仓历史rustfmt差异未清理。其他DPI、跨屏、全部最大化/全屏手势、动画/HDR全矩阵、桌面捕获性能和完整辅助功能尚未覆盖；P1不标为VALIDATED。既有本地提交均保留；新主程序只在工程目录构建，不会更新其他安装副本或开始菜单目标。旧截图不作为新构建证明。
