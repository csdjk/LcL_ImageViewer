# LcL ImageViewer — 当前状态

> 更新时间：2026-09-14。本文是当前阶段和唯一NEXT的权威来源。

<!-- project-stage: P1 -->
<!-- project-next: IV-P1-01 -->

## 当前阶段

```text
P0 Viewer Foundation      VALIDATED
P1 UI Stabilization       ACTIVE（新拟态 + 两侧玻璃导航）
P2 Release Hardening      GATED BY P1 VALIDATED
```

## 当前实现

`IV-P1-N01` 新拟态主题、`IV-P1-N02` 1秒自动隐藏、`IV-P1-N03` 两侧玻璃切图均已在独立任务工作树提交、串行审查、合入main并复验，状态 **DONE**。最新功能集成提交 `6de1d75f8b9ccb511246f61efc6efeaef3c96f9d`；最终状态文档提交的HEAD通过Git读取，不在本文自引用。

主界面保留浅色雾蓝灰/深色新拟态；上一张/下一张移到窗口两侧垂直居中（距边18点，48×64点，圆角16点），仅两侧按钮为半透明真实场景模糊。第一张/最后一张禁用无效方向，不循环；顶栏不再重复箭头。单图/空态和设置/右键菜单状态不显示两侧导航。

等待1秒后开始淡出，保留120ms淡入/180ms淡出；移动或点击重置等待，按住操作时保持可见。不改变图像解码、Shader、原始像素取样、文件关联。保留已有主题和桌面磨砂偏好。

## 已完成验证

- main集成提交上 `cargo test --workspace --offline`：**21 passed / 0 failed**。
- `cargo check --workspace --offline`、`cargo build --release -p iv-viewer --offline`：**PASS**。
- 最终任务源码64张实际Windows截图：双主题、1280×860/880×560、默认/悬停/按下、首末边界/无效点击、鼠标/键盘切图、按住、到期前/隐藏/恢复、菜单/设置；全部DPI96/100%。
- main重建Release后40张实机截图：浅色1280×860与深色880×560各16状态，另有6次Tab+默认及空态。主线导航标题、首末禁用、隐藏恢复、图像内部ROI及玻璃高频方差检查通过。
- 不被UI覆盖的相同输入图像内部ROI差异为0；隐藏后的工具栏与两侧按钮恢复区域一致。不是全格式/全DPI或设计稿98%相似度承诺。
- 使用显式QA隔离profile，不再因用户旧Debug运行而改写其配置。真实用户配置SHA未变、旧Debug未强制关闭。
- 主程序：`target/release/imageview.exe`；SHA256 `cab0a613ed6d41184436c58f04d38b66445465e5dc48bb050b7f71b64a13dd49`。

证据：`docs/ui-qa/两侧玻璃导航验收.md`，忽略目录 `ui-verify-shots/side-glass-final-*` 与 `side-glass-main-*`，逐张JSON记录commit/输入/二进制hash、窗口标题、状态、客户区、DPI和捕获方式。早期迭代截图不能替代最终证据。既有新拟态验收在 `docs/ui-qa/新拟态验收.md`。

## 当前 NEXT

```text
NOW: IV-P1-01 READY — 完善通用验收入口
AFTER: IV-P1-02 / IV-P1-03 blocked by IV-P1-01
COMPLETED: IV-P1-N01 / IV-P1-N02 / IV-P1-N03 DONE
```

## 保留的历史与限制

既有本地提交完整保留，原 `1068627` 已验证仍为main祖先。没有push、发布、安装器分发或注册表写操作。用户正在使用的旧Debug二进制未覆盖，运行新功能应打开上述Release；旧窗口不是新构建的验收对象。

全仓既有rustfmt差异未清理，不混入全仓格式化。未覆盖其他DPI、动画/HDR/全格式、全部辅助功能与键盘操作、桌面捕获恢复、窗口状态及性能预算；P1仍为ACTIVE。不声明GPU毫秒数或98%相似度。当前没有新增产品/架构待决策。
