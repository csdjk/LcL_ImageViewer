# LcL ImageViewer — 当前状态

> 更新时间：2026-09-15。以磁盘、Git、实际构建和GitHub回读为准。

<!-- project-stage: P1 -->
<!-- project-next: IV-INSTALL-01 -->

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

NOW: IV-INSTALL-01 IN_PROGRESS — 修复安装器打开方式注册；公开v0.3.0附件尚未更新。
COMPLETED: IV-P1-N01至IV-P1-N09、IV-REL-030 DONE。

此次明确授权的版本发布不代表所有长期门禁完成。P1仍ACTIVE；其他DPI、混合缩放/跨屏、全部HDR/动画/回收站设备、桌面捕获性能、完整安装卸载系统集成未逐项覆盖。安装包和主程序未配置代码签名；iv-shell保留LNK4104导出可见性警告。详细记录见`docs/releases/v0.3.0-validation.md`。
