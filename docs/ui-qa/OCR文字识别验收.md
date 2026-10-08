# 本机 OCR 与图片文字复制（IV-OCR-01）

## 实现范围

Windows 使用 `Windows.Media.Ocr.OcrEngine`，由独立 `image-ocr` 工作线程按需初始化 WinRT；复用锁定版本的 windows-rs 0.58，只启用额外系统 API feature，不引入在线服务、模型文件、Python 运行时或新的第三方 OCR 依赖。文字识别不会向服务器发送图片，不保存用户 OCR 文本、临时识别图片或日志。

顶部 `Icon::Ocr` 打开结果面板；右键“识别并复制图片文字”和 Ctrl+Shift+C 启动识别并复制非空结果。结果可选择或校对；复制全文支持保留换行或合并为空格。识别语言列表仅显示系统已安装的 OCR 语言，默认“自动（系统语言）”不是图片语言自动检测。语言和换行选择保存，图片与识别文本不作为偏好保存。

输入使用不可变的原始 RGBA 数据。动画在进入识别时暂停，取当前帧，关闭后恢复先前播放状态；静态多 Mip 使用 Mip 0，不使用界面缩放、单通道、背景网格或编辑预览像素。HDR 浮点图明确拒绝。透明字自动选用黑/白底仅供识别，Alpha=0 隐藏颜色不参与估计。单次预处理长边最多 4096（且不超过系统限制），总像素最多 8 Mi，保持比例，过大图片会明确显示实际识别尺寸。

同一面板仅允许一个后台任务，取消标志和 45 秒等待超时会尝试取消系统异步操作。切图、文件刷新或帧失效清除旧快照并取消任务，过期结果不写剪贴板。识别期间剪贴板序列变化时不自动覆盖；空结果、异常和取消也不清空原剪贴板。对同一不可变快照重新打开结果可使用已有结果，不自动重复识别。缺语言/组件或不支持平台均明确提示，不安装系统组件或下载模型。

## 参考资料与支持边界

- Microsoft 官方 Rust OCR 示例：<https://github.com/microsoft/windows-rs/tree/master/crates/samples/windows/ocr>
- API 与语言枚举：<https://microsoft.github.io/windows-docs-rs/doc/windows/Media/Ocr/struct.OcrEngine.html>
- 微软命名空间文档注明桌面应用支持可能要求 package identity：<https://github.com/MicrosoftDocs/winrt-api/blob/docs/windows.media.ocr/windows_media_ocr.md>。因此只以本次实际 unpackaged EXE 验证代表本机环境，不承诺每种 Windows 安装都可用；失败提示检查系统组件/应用标识，不误报为识别成功。

当前机器语言枚举结果为 `en-US`、`zh-Hans-CN`；系统 MaxImageDimension 回读为 10000，应用仍使用更低的有界内存配置。macOS 此次未接入 OCR，原有 macOS 查看器代码与工作流保持。

## 验证方式

算法与状态回归由 Rust 单元测试覆盖：原图/当前帧/Mip 0、透明像素合成、预处理尺寸限制、取消、空文本复制、换行与过期任务防止写剪贴板。`tools/tests/test_ocr.py` 检查入口、平台 feature、离线边界和共享控件。

真实验证入口 `tools/ui-qa/ocr_smoke.py` 只生成文字示例图，经当前 Windows EXE 的快捷键调用一次真实 OCR 并检查 OS 剪贴板；后续界面矩阵复用结果，避免重复模型调用。只在能完整备份内存型剪贴板格式时继续测试；如果用户中途复制了新内容，不覆盖其新值。截图、源码版本和二进制哈希保存在根工作区忽略的 `ui-verify-shots/`，正常用户配置和原图哈希另行检查。不能将预处理单元测试或语言枚举称作真实识别通过。

最终构建、真实识别结果和主线复验记录在完成后追加。


## 2026-10-08 收尾验收

原命令超时阻塞已解除；工作树新增中文相邻汉字分词空格整理。构建 015f5b56d2f4495e33f995e0a416ca2806d1c0bf，160项Rust与check通过。实际系统OCR一次，后续复用结果检查深浅主题/880×560/1280×860、语言下拉、换行/单行复制、校对复制和顶部重新打开。共14张实机截图，系统剪贴板实际读回一致，原剪贴板格式完整恢复、原图及正常用户偏好未改变。中文及英文数字已识别，英文相似字母仍有误差，不将识别率算为100%。证据：`ui-verify-shots/ocr-release070-worker/report.json`；复验命令使用tools/ui-qa/ocr_smoke.py与该目录replay.json。
