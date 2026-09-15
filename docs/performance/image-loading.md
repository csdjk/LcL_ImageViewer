# 图片解码、启动与切图优化（IV-PERF-01）

## 范围与行为

本轮用户反馈打开新图片时短暂停顿并看到“正在解码”。解码将文件里的编码图像变为像素；当前查看器还要保留 CPU 像素用于 RGBA 取样，再将图像上传 GPU 显示。加载提示不是解码慢的可靠计时器，目录扫描、窗口/GPU 初始化、排队和拷贝也会延长等待。

保持原始尺寸及当前色彩/透明度转换规则，不添加有损预览、不改 Shader、DDS/HDR/Mip 解码或帧时序。文件不会被重新编码写回。

实现：

- 首图读取/解码在 eframe 窗口和 GPU 初始化之前启动，与初始化并行，而不是串行相加。
- 目录扫描独立后台执行；同目录 A/D 导航复用目录快照。先按扩展名过滤，再读取必要的文件类型；排序键只生成一次。
- 前台只保留最新请求，优先于预读；预读批次替换、去重且最多四个，结果队列从最多64张改为最多2张。正在执行的一张仍允许解码结束，不宣称支持任意解码中途抢占。
- 完成后直接唤醒界面，不依靠加载期间持续忙重绘。
- 静态 PNG 复用已解析的解码器；RGBA8 像素使用所有权转移，动画收帧不复制完整帧，GPU 上传借用已有 RGBA8 切片。HDR 转半精度路径不变。
- 缓存继续使用192 MiB预算，命中提升LRU，并检查文件长度/修改时间，避免普通外部编辑后复用旧图。预算不是进程总内存上限：当前超大图、正在解码的临时缓冲和结果队列仍占内存。
- 回归发现旧静态 WebP 被当成动画、可能报“动画无有效帧”；修正静态/动画分支并增加像素测试。

## 可重复基线

基线源码：`c58b721`（仅在原行为上加可选计时）。优化源码：`f316300`。

二者均为同一依赖锁定、同一Release配置，未切换解码库。原二进制保留在 `Temp/perf-load-baseline/imageview.exe`。测试输入统一生成到 `Temp/perf-load-fixtures`，前后报告中的输入SHA256清单完全一致。

本机处理器为 Intel Core i7-13700KF（16核24逻辑处理器），显示设备包含 NVIDIA GeForce RTX 4070 Ti；截图验证DPI96/100%。

每种启动场景5个新进程，操作系统文件缓存已热，不清空磁盘缓存；不是冷开机/冷盘测试。计时从 Rust `main` 入口到首次 `image_paint_queued`，表示CPU已提交图像绘制命令，不是屏幕真正完成显示的时间，也不包括Explorer发起进程前的时间。小样本P95仅作描述，不构成性能保证。

| 场景 | 优化前中位数 | 优化后中位数 | 耗时降低 |
|---|---:|---:|---:|
| 1024×1024 RGBA PNG，首次打开 | 443.581 ms | 387.952 ms | 12.54% |
| 4096×4096 RGBA PNG，首次打开 | 491.166 ms | 403.211 ms | 17.91% |
| 4096×4096 JPEG，首次打开 | 551.919 ms | 416.663 ms | 24.51% |
| 12张图+2000个其他文件目录，已预读的连续切图 | 32.433 ms | 0.744 ms | 97.71% |

最后一行只计算40次导航请求（每进程8次，排除首次打开），前后均给预读相同等待时间；P95为36.367 → 0.922 ms。这不是所有新图片都能0.744 ms显示，也不是显示器切图耗时。

单次目录扫描31.094 → 0.898 ms（中位数），并从每次导航重扫改为显式打开时后台扫描。4096 PNG解码含读取100.608 → 91.639 ms，CPU纹理上传15.877 → 5.005 ms。JPEG解码本身156.131 → 161.647 ms，并没有更快；其启动改善来自与初始化重叠及减少复制，不能将这解释为JPEG算法加速。

密集目录首次启动中位数418.154 → 384.836 ms，但五次样本P95从422.057增加至432.511 ms，不宣称所有尾延迟均改善。

原始基线：`Temp/perf-load-before/report.json`；优化结果：`Temp/perf-load-after/report.json`，同目录包含逐次TSV计时日志。

```powershell
# 输入只生成一次；下次测量使用同目录，不再加 --generate
python -B tools/perf/load_benchmark.py --generate --fixtures Temp/perf-load-fixtures

# 输出必须是新目录；before/after分别指向相应Release
python -B tools/perf/load_benchmark.py --fixtures Temp/perf-load-fixtures --binary Temp/perf-load-baseline/imageview.exe --output Temp/perf-before-new --commit c58b721 --samples 5
python -B tools/perf/load_benchmark.py --fixtures Temp/perf-load-fixtures --binary target/release/imageview.exe --output Temp/perf-after-new --commit <当前提交> --samples 5
```

`LCL_IV_PERF=1`仅将阶段计时输出到stderr，默认关闭，不写用户日志文件。当前文件名仅记录basename。QA使用独立profile，不覆盖普通界面偏好。

## 验证

54项Rust测试通过：包括原40项，最新请求去重/有界队列/真实worker调度、目录过滤、缓存外部编辑失效，以及PNG各种通道/16bit、JPEG、GIF、APNG、WebP像素与帧延时一致性。原DDS/TGA/PSD测试保留。

`tools/perf/compare_display.py`从两个真实Windows程序捕获深色880×560、浅色1280×860下RGB/R/G/B/Alpha，比较中央320×240像素：10组逐像素完全一致。原截图与元数据见 `ui-verify-shots/perf-display-comparison`，图片不是重绘稿。只覆盖该ROI，不把它扩大为所有文件都做过全屏比较。

任务分支实际回归：`perf-menu-light-1280` 25状态通过，使用本任务新建图片确认回收并核对回收站原文件哈希；`perf-popup-dark-880` 16状态通过，弹窗位置/主窗口固定/控件/边界/导航与画布右键拖窗保留。已查看当前菜单及RGB截图，未改用户原图或普通偏好。

主线集成 `9eaae72e950831f524d582183aafdb3425b677fc`：54项Rust测试、11项安装器只读检查、workspace check、Debug和Release workspace构建通过。主线Release每场景2次新进程复测（共8次），输入与原20次基线完全相同；两次仅做回归检查，不替换上表的五次基线。主线16状态实际弹窗/导航/画布拖动回归通过，DPI96。证据：`Temp/perf-load-main/report.json` 与 `ui-verify-shots/perf-main-popup-light-1280`。

本地主程序：`target/release/imageview.exe`，SHA256 `89f86af7424c5bb9e1cfc732911549381aa89866fc9e8058fe20d6e5838026b5`；Debug：`target/debug/imageview.exe`，SHA256 `bd80b9ce4cf337d02e5f78081b5a6093f2bc5b1750a5262479fabd9b1ca7ca26`。仍是0.3.0的本地优化开发构建，不是新的公开发布版本。已有安装包和用户安装目录未替换。


## 明确保留的限制

新进程仍需GPU/窗口/字体初始化，未实现常驻进程或跨进程缓存。单个大PSD/DDS/HDR或长动画仍可能需要明显解码时间；当前动画仍收集完整帧序列后显示，未增加渐进式首帧。文件系统/驱动/安全软件成本未单独全面测量。

同目录导航使用快照，没有新增文件夹监控器；外部新增/重命名后重新打开图片刷新列表，删除确认后的目录刷新仍保留立即扫描。长度/修改时间均保持不变的外部改写无法由此缓存戳识别。未在网络盘、多DPI、混合屏、全部权限/超大图组合下验证。

本轮不安装或覆盖用户已安装的程序，不修改关联，不push、不更新GitHub Release或安装包。已有D盘默认目录和打开方式安装器修复保留。
