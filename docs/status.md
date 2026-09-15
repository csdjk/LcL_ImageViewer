# LcL ImageViewer — 当前状态

> 更新时间：2026-09-15。以磁盘、Git、实际构建和GitHub回读为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-REL-031 -->

## 最新发布

**v0.3.0 已按用户明确授权发布到原GitHub仓库，标记为Latest，非草稿、非预发布。**

- Release：https://github.com/csdjk/LcL_ImageViewer/releases/tag/v0.3.0
- 标签源码提交：`c8ceef0e6bc4555020e4ea7b4fdbdce8a7c951c4`；发布于 `2026-09-15T02:01:35Z`。
- README已更新：5张v0.3.0实际配图（深/浅主题、Alpha、设置与右键菜单），完整鼠标/快捷键、下载升级与构建说明。
- 安装包、免安装ZIP、SHA256SUMS.txt均已上传；重新通过公开地址下载并逐个核对SHA256通过。
- 本地产物：`dist/v0.3.0/`。当前主程序为`target/release/imageview.exe`，版本0.3.0，SHA256 `802d3ab8c6a18621b01056d156fe311b6f232d6db2b207da7df145f98bd0f1ec`；Debug亦已更新到0.3.0。

## 当前功能

N01–N09已合入：新拟态深浅主题、两侧磨砂导航、1秒自动隐藏、精简无分类菜单和设置、A/D切图、Delete确认后回收站删除、打开图片父目录。设置标题左/右键**只拖动设置弹窗**，主窗口不动；画布右键仍拖动主窗口。

## 发布验证

40项workspace测试、check及Debug/Release workspace构建通过；当前版本设置弹窗16状态与菜单/回收站25状态实机回归通过，100%缩放。正式ZIP解压后程序启动通过；exe与安装器资源版本均为0.3.0，ZIP CRC/包内hash/公开下载hash通过。DLL可加载且入口导出可用。

只使用新建测试图片和隔离QA配置，不在用户当前安装上执行安装/卸载或注册。发布包不包含私密桌面、用户偏好、Temp、源码工作树或Debug符号。现有main历史保留，未强推或改写旧版本。

## NEXT及限制

NOW: IV-REL-031 IN_PROGRESS — 更新安装包并发布v0.3.1。
COMPLETED: IV-P1-N01至IV-P1-N09、IV-REL-030 DONE。

此次明确授权的版本发布不代表所有长期门禁完成。P1仍ACTIVE；其他DPI、混合缩放/跨屏、全部HDR/动画/回收站设备、桌面捕获性能、完整安装卸载系统集成未逐项覆盖。安装包和主程序未配置代码签名；iv-shell保留LNK4104导出可见性警告。详细记录见`docs/releases/v0.3.0-validation.md`。

## 安装器打开方式修复（仅本地，尚未发布）

发现已发布v0.3.0安装器遗漏ValueType，创建空注册键而未更新旧命令。用户已选assoc，非漏勾选问题。本地修复显式REG_SZ、ChangesAssociations与自身命名值卸载，集成`6ce3ea621bf81de8105af033c8f1accd489c44f1`；40项Rust测试+6项新测试与隔离真实安装回归PASS。修正版`dist/openwith-fix/LcL-ImageViewer-Setup-v0.3.0-openwith-fix-win64.exe`已生成，SHA256 `d3e989db872a13cab0f38409130df1bffa1daff87eaef9161f23a300f973a69f`；EXE仍为已发布0.3.0。

未更新GitHub附件、未push、未修改用户实时关联和当前安装，不能宣称用户当前打开方式菜单已恢复。临时办法：运行实际安装目录的imageview.exe，在设置→打开方式→注册；不从工程副本注册。详见安装器修复记录。

## D-drive default installer (local only)

IV-INSTALL-02 DONE. Integration `d084227f9f1b44b22586689c17e8e0bb504cefa4`. Fresh installs default to `D:\Program Files\LcL ImageViewer`; absent D drive falls back to localappdata. Existing installs keep their chosen directory; directory page remains editable. Includes INSTALL-01 Open With fixes. 11 installer tests PASS; original inert Inno initialization selected D and passed both branch assertions, but wizard capture was not validated. Production installer compiled, version0.3.0 verified, SHA256 `36e6e3e90f5c7961d6b4e57bf3df33f4b147d3edfba08ea74c61ceff0a73cbc2`. New package: `dist/default-d/LcL-ImageViewer-Setup-v0.3.0-d-drive-win64.exe`. No live install, registry, relocation, GitHub update or push. Details: `docs/releases/default-install-directory.md`.

## 图片加载与启动优化（仅本地开发版）

IV-PERF-01 DONE；最新功能集成 `9eaae72e950831f524d582183aafdb3425b677fc`。首图解码与GPU/窗口初始化并行、目录后台扫描/导航复用、最新请求优先且去重、有限预读、PNG解码器复用、RGBA/动画缓冲所有权转移及上传借用。保留原图尺寸、Alpha、Shader和DDS/HDR/Mip路径；静态WebP旧分支错误一并修正。当前一张解码不强制抢占；没有常驻程序或跨进程缓存。

同一组测试图，前后各20次新进程（4场景每种5次，暖文件缓存）测到CPU首次绘制提交：1024PNG444→388ms，4096PNG491→403ms，4096JPEG552→417ms。JPEG纯解码未变快，主要为阶段重叠。40次已预读导航32.433→0.744ms，不代表显示器呈现时间或所有新图耗时。详细边界及P95见`docs/performance/image-loading.md`。

54项Rust测试+11项安装器检查、check/Debug/Release workspace通过；10组真实通道ROI逐像素一致，分支41状态及主线16状态实际交互通过。主线8次新进程再测与输入hash检查通过。

当前Release SHA256 `89f86af7424c5bb9e1cfc732911549381aa89866fc9e8058fe20d6e5838026b5`；Debug `bd80b9ce4cf337d02e5f78081b5a6093f2bc5b1750a5262479fabd9b1ca7ca26`，路径仍为`target/release/imageview.exe`和`target/debug/imageview.exe`。该记录覆盖上方历史发布中提到的工程二进制哈希；旧公开v0.3.0附件哈希不变。现有D盘默认和Open With修复保留。本轮未push/发布/打包、未修改用户安装或关联。
