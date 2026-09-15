# 安装默认目录：D 盘（IV-INSTALL-02）

> 发布状态更新：本记录中的改进已统一纳入 v0.3.1 正式安装包和便携包；下文“仅本地/未发布”描述的是当时验证阶段，原始测试证据保留。

## 当前策略

全新安装时，D 盘存在则建议 `D:\Program Files\LcL ImageViewer`；不存在则回退到 `{localappdata}\Programs\LcL ImageViewer`。目录选择页始终显示，用户可手动修改。保留同一 AppId 和 `UsePreviousAppDir=yes`，升级优先使用原安装目录，不自动迁移当前安装，不提高权限或修改目录权限。

只调整 `tools/setup.iss` 的默认目录策略。原来的 71 个显式字符串注册值、`ChangesAssociations=yes`、自有注册值卸载保护全部保留；GUI/解码和已发布的 0.3.0 主程序没有改变。

## 已有验证与范围

- 11 项只读安装器检查通过，包括此前 6 项打开方式检查和本次 5 项目录策略检查。
- 用独立 AppId、无负载/无注册/无快捷方式/无运行项的 Inno 探针复用生产目录解析代码。初始化输出 `selected=D:\Program Files\LcL ImageViewer`、`has_d=1`、`branch_checks=PASS`；初始化中包含有D/无D两个分支断言。原始记录在 `Temp/installer-default-d-worker/wizard.txt`，编译输入在同目录 `directory-preview.iss`。
- 上述探针的截图阶段未获得有效画面，不能把整轮向导验证标为通过；未执行任何安装。无D分支通过向纯选择函数传入False验证，未卸载或改动真实磁盘。
- 后续精简版 `tools/tests/installer_default_dir_smoke.py` 是可重复的无界面逻辑验证入口；自动执行被安全检查拦截，本轮没有重试或绕过，也不宣称该版本已运行。
- 升级沿用原路径及目录页可编辑由配置检查与 Inno 官方语义确认；没有在用户当前安装上做升级/迁移操作。

## 重复验证

```powershell
python -B -m unittest discover -s tools/tests -p "test_installer*.py" -v
```

Inno 官方说明：`topic_setup_defaultdirname.htm`、`topic_setup_usepreviousappdir.htm`、`topic_setup_disabledirpage.htm`（jrsoftware.org/ishelp）。

## 交付

主线合入后只生成新的本地安装包，复用原版0.3.0主程序并包含打开方式修复。旧安装包和GitHub附件不覆盖。本轮不push、不发布、不更改用户当前的安装位置或文件关联。最终安装包路径、源码提交及SHA256在 `dist/default-d/build-manifest.json` 与项目状态文档中记录。
