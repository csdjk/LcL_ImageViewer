# Windows 新拟态 UI 验收

入口：`tools/ui-qa/neumorphic_smoke.py`。需要交互式 Windows 桌面、Python 3.10+、Pillow 和当前构建的 imageview.exe。

```powershell
python -B tools/ui-qa/neumorphic_smoke.py --binary <imageview.exe绝对路径> --output <不存在的忽略目录> --input <图片路径> --commit <已构建源码commit> --theme light --width 1280 --height 860
```

省略 input 检查空状态。切换 dark 和 880×560 覆盖基础矩阵。`--actions <JSON文件>` 支持 move/click/right-click/down/up/key/wait/shot/wheel；坐标为客户区逻辑点，须与当前参考截图核对后使用。示例：`[{"kind":"key","code":"1"},{"kind":"shot","name":"selected-r"}]`。

脚本只启动/关闭自己的 imageview 进程，通过 PID、完整可执行文件路径和唯一可见客户区三重校验窗口。64 位 Win32 句柄使用明确类型；PrintWindow 捕获真实窗口并裁剪客户区，空白或尺寸不符会失败。截图 JSON 记录 commit、可执行文件/输入 SHA-256、主题、动作、逻辑/物理尺寸、DPI、裁剪和时间。脚本不宣称仅截图成功就通过视觉审查。

运行前必须关闭其他 imageview。脚本备份此应用唯一的 `%APPDATA%/LcL ImageViewer/data/app.ron` 到输出目录；临时设置测试主题及关闭桌面磨砂，退出时恢复原始字节并核对。原文件不存在时只清理该次创建的 app.ron，不删除其他数据。若脚本被强制中断，应关闭其子进程后用 `app.ron.original` 恢复配置。不要在运行时另开同一应用。脚本不修改系统 DPI、注册表或文件关联。

首次无法切到前台时，仅临时连接线程输入队列激活已验证窗口并立即解除；仍失败就停止，不把鼠标/键盘事件发给别的应用。验收过程中请不要操作鼠标键盘。进入 Windows 集成区域只截图，不触发注册/解除注册/默认应用操作。

## 正在使用查看器时验收

当前版本增加可选 `--isolated-profile`，每次生成随机QA标识，通过子进程环境变量 `LCL_IV_QA_PROFILE` 指向 `LcL ImageViewer QA-<标识>` 独立应用ID。不修改正在使用的profile；只关闭本次启动的PID。默认模式保留原有多进程保护。仅当前支持隔离标识的二进制可使用，旧版禁止绕过保护。两侧导航动作矩阵为 `side-navigation-1280.json` / `side-navigation-880.json`，用03/02/01命名的3张测试图验证边界及切换；原始图片与截图均放忽略目录。
