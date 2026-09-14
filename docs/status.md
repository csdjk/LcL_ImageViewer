# LcL ImageViewer — 当前状态

> 更新时间：2026-09-14。以磁盘、Git和可重复实机结果为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-N06 -->

## 当前实现

P0 VALIDATED；P1 UI Stabilization ACTIVE；P2仍受P1门禁约束。

N01新拟态主题、N02的1秒隐藏、N03两侧玻璃导航、N04鼠标手势、N05打开图片所在文件夹均已在独立工作树完成、串行合入main并复验。最新功能集成 `9ba63ec28ae226af8722a2844e0998b4f3b0251c`；最终状态文档HEAD通过Git读取。

右键项“打开所在文件夹”直接打开当前图片的父目录，不再用/select选择文件；相对路径先转绝对路径，参数保留原生路径，不手工拼接引号。目录不可用或程序启动失败会显示提示。

画布左键/中键移图，右键拖窗，右键单击菜单；保留侧边玻璃导航、窗口边缘缩放、原图解码/Shader/取样、1秒隐藏及已有用户偏好。

## 最新验证与运行入口

- main29tests/0失败；workspace check、Debug和Release构建通过。
- 分支双主题双尺寸4组真实菜单操作全部PASS；main Release再验浅色1280×860与深色880×560两组PASS。全部DPI96。
- 通过Shell.Application的实际目录和Explorer截图确认进入当前图片父目录，测试目录含中文、空格、逗号、&、括号；不是只检查spawn返回。
- 修复版Release：`target/folder-fix/release/imageview.exe`；SHA256 `5700c72092f1d26cd8f69353f0a85c986c1f3d8f743b5bcc4bfe53b395634e2d`。
- Debug：`target/debug/imageview.exe`；SHA256 `222c9abddbeba7620effebe3ddfdc226e65f9a89f6de2ca4fb66180505f85fee`，已同步更新。
- 原 `target/release/imageview.exe` 仍被用户运行占用，未覆盖或终止；本轮修复版位于上述备用路径。
- 记录：`docs/ui-qa/打开所在目录验收.md`；截图/元数据：`ui-verify-shots/folder-final-*`、`folder-main-*`。旧鼠标/导航验收记录保留。

## NEXT

NOW: IV-P1-N06 IN_PROGRESS — 精简菜单、A/D切图、Delete回收站删除；之后恢复IV-P1-01。

COMPLETED: IV-P1-N01 / IV-P1-N02 / IV-P1-N03 / IV-P1-N04 / IV-P1-N05 DONE。

## 保留限制

既有历史提交保留，无push/发布/文件关联或快捷方式修改。用户运行中的旧窗口不会热更新。UNC仅测试路径构造，未实测网络共享/超长路径/全部权限情形。其他DPI、跨屏、完整性能与辅助功能矩阵仍未覆盖；全仓历史rustfmt差异未清理，P1不标为VALIDATED。
