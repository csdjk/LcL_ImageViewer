# LcL ImageViewer — 当前状态

> 更新时间：2026-09-14。本文是当前阶段和唯一 NEXT 的权威来源。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-N03 -->

## 当前阶段

```text
P0 Viewer Foundation      VALIDATED
P1 UI Stabilization       ACTIVE（用户授权改为新拟态）
P2 Release Hardening      GATED BY P1 VALIDATED
```

## 当前实现

用户于2026-09-14明确将视觉方向改为浅色粉彩、单色系明暗、12–16点圆角、多层柔影、凸起/凹陷操作反馈。`IV-P1-N01` 已在独立工作树完成、经审查合入main并复验，状态 **DONE**。主线功能集成提交 `635b7dcc195c5c220882cb9589b3762ebc124eff`；最终状态文档提交的HEAD通过Git读取，不在本文自引用。

当前规范是 `docs/新拟态UI规范.md`，验收记录是 `docs/ui-qa/新拟态验收.md`。旧玻璃方案保留作历史参考，桌面磨砂仍是可选画布功能。新用户默认浅色并关闭桌面磨砂；已有显式主题/背景偏好继续保留。

## 已完成验证

- main 集成提交上 `cargo test --workspace --offline`：**15 passed / 0 failed**。
- `cargo check --workspace --offline` 与 `cargo build --release -p iv-viewer --offline`：**PASS**。
- 工作分支最终源码50张实际Windows原始截图：双主题、1280×860/880×560、通道、悬停、按下、Tab焦点、菜单、设置/滚动、自动隐藏/恢复、空态、DDS与长中文文件名。
- main重建的Release程序又完成22张截图：浅色1280×860的13状态、深色880×560的8状态、浅色空态；全部DPI96/100%，元数据及配置恢复核对通过。
- 相同测试PNG内部ROI与旧版RGB差异为0；四个主题/尺寸组合工具栏恢复区域差异为0。该比较不扩展为全格式或设计稿相似度承诺。
- 上轮已验收主程序（历史构建），SHA-256 `ec37f483172dad922561aee51b097b6c5a432d200541ad908f95b8cb55483256`。没有发布、push、安装器分发或注册表写操作。

完整证据位于忽略目录 `ui-verify-shots/neumorphic-*`。原始PNG逐张配有commit、二进制/输入hash、动作、主题、尺寸、DPI和裁剪JSON。较早迭代截图不能替代最终证据。

## 当前 NEXT

```text
NOW: IV-P1-N03 REVIEW — 两侧玻璃切图按钮，串行复用并验证N02的1秒隐藏
AFTER: IV-P1-01 READY — 完善通用验收入口
AFTER: IV-P1-02 / IV-P1-03 blocked by IV-P1-01
COMPLETED: IV-P1-N01 DONE — 新拟态 UI 已集成且 main 复验
```

## 保留的历史与风险

原main上的功能与工作流提交（截至 `10686275ff3ec026086e9a244a72d8df143de833`）完整保留，已验证它仍是当前main的祖先。所有新增提交仅在本地；远端没有被修改。

全仓rustfmt原有差异尚未清理；本轮只格式化UI实现文件，未将大规模历史格式化混入功能修改。未验证其他DPI、HDR/动画/全格式、全键盘/辅助功能/全部禁用态、桌面捕获恢复、窗口状态矩阵和性能预算；不声明GPU≤2ms，也不将P1标为VALIDATED。

当前没有新增阻塞产品/架构决策。push、Release与安装包分发需要另外明确授权。

## 当前1秒延时追加任务

`IV-P1-N02 BLOCKED`：任务分支 `codex/iv-p1-n02-autohide-1s` 的源码 `a23f07dff8cd1f2a5e1f5c85830a667313e34cdb` 将等待改为1秒；16tests/check/Release构建通过。主工作区 `target/release/imageview.exe` 现为该分支的新构建，SHA-256 `17afe8948da9af871ee407cc8abbfd1cc1c8fcae56870850d58fe5a3f65a219b`；main源码尚未包含此功能变更。旧Debug程序仍被用户运行占用，未强制关闭。本轮实机复验、Debug更新和main合入待关闭旧窗口后继续。
