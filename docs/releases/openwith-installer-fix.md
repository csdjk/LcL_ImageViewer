# v0.3.0 安装器“打开方式”修复

## 诊断（2026-09-15）

用户安装记录显示0.3.0且已选中`thumbs,assoc`，但应用名称、SupportedTypes、ProgId及Capabilities等键没有值；Applications的打开命令仍指向不存在的旧安装路径。因此不是未勾选安装任务。

根因：`tools/setup.iss`的71条Registry记录遗漏`ValueType`。Inno默认`none`仅创建键，不写入ValueData；空字符串也不会自动创建REG_SZ。应用内`winassoc::register()`使用显式字符串写入，不受安装脚本问题影响。

修复全部71条记录为`ValueType: string`；启用`ChangesAssociations=yes`通知Explorer刷新；OpenWithProgids和RegisteredApplications卸载时仅删除本应用的命名值。应用EXE、显示/交互逻辑、用户默认应用选择不变。

来源：
- https://jrsoftware.org/ishelp/topic_registrysection.htm
- https://jrsoftware.org/ishelp/topic_setup_changesassociations.htm
- https://learn.microsoft.com/en-us/windows/win32/shell/app-registration

## 验证

新增6项安装器契约测试：显式值类型、Explorer刷新、支持格式与程序一致、启动路径及参数引号、共享键卸载和不写UserChoice/默认扩展值。

使用真实Inno Setup 7.1.0编译并运行测试安装器，将源Registry记录全部重定向到独立的`HKCU\Software\LcL\InstallerQA\<随机ID>`，禁止写入真实Software\Classes、RegisteredApplications及默认关联。复现旧版空值与旧路径未更新；修正版升级后逐项核对全部71个REG_SZ值，含中文和空格安装路径。测试卸载后其他应用的哨兵值保留，自己的OpenWithProgids/RegisteredApplications条目移除。

分支运行：`Temp/installer-openwith-regression-worker-final/report.json`，结果PASS；真实关联/默认选择和安装记录在测试前后的快照相同。最初一次测试脚本的双BOM编译失败已修正，不计为成功证据。

```powershell
python -B tools/tests/test_installer_associations.py
python -B tools/tests/installer_registry_smoke.py --iscc <ISCC.exe> --baseline 8fc0d51921cbcedce4aaf6827970409ef92ba6a9 --output <新的忽略目录>
```

## 交付边界与立即修复方法

使用实际安装目录中的`imageview.exe`，进入“设置 → 打开方式 → 注册”。不要从target/debug或target/release启动后注册，否则会登记工程内程序路径。该操作添加候选项而不主动改动UserChoice；程序会通知Shell刷新。

本轮不修改用户当前安装/真实关联、不运行完整正式安装器、不更改默认看图程序、不推送或覆盖GitHub公开附件。已发布的v0.3.0安装包仍需后续公开更新。安装器修复先以独立命名的本地文件交付，软件主体继续使用已发布0.3.0版本；不可与原附件混称同一份文件。

真实测试仅覆盖命名空间隔离的注册记录、升级及卸载行为，不等于已实测用户机器的“打开方式”候选菜单，也不等于完整系统安装/卸载回归。

## 本地主线交付

本地集成 `6ce3ea621bf81de8105af033c8f1accd489c44f1`。main的40项Rust测试及6项安装器契约测试通过；主线真实隔离安装/升级/卸载回归PASS（`Temp/installer-openwith-regression-main/report.json`），71个值逐项核验。修正版编译成功并核对资源版本0.3.0，主体EXE保持公开版hash不变。

安装包：`dist/openwith-fix/LcL-ImageViewer-Setup-v0.3.0-openwith-fix-win64.exe`，5968283字节。SHA256：`d3e989db872a13cab0f38409130df1bffa1daff87eaef9161f23a300f973a69f`。未执行用户真实安装，也未推送或覆盖线上附件。
