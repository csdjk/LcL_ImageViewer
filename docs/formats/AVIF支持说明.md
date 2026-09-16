# AVIF 支持与验收

## 使用范围

- `.avif` 文件可直接打开、拖入窗口、在文件对话框中选择，并参与同目录/向下子目录切图；扩展名不区分大小写。实际解码依内容识别，不盲信扩展名。
- 静态 AVIF 输出直通 Alpha 的 RGBA8，复用 RGBA 通道、像素取样、棋盘格与图片边界功能。透明 RGB 不进行预乘存储。
- 8/10/12 位输入按现有查看器 LDR 管线转换为 8 位显示。不提供 HDR 色彩映射和 ICC 色彩管理；相关文件在附加信息中标注。此限制不改变其他格式原来的处理方式。
- 支持容器中的 clean aperture、逆时针旋转和镜像。非方形像素比例暂按原始像素网格显示，并提示。
- 动态 AVIF 只解码、显示首帧，不播放；附加信息明确显示帧数与“仅显示首帧”。缩略图使用相同首帧数据，不预解码整个动画。
- Windows 缩略图 DLL、安装脚本及手动注册脚本已加入 AVIF；本轮不执行系统注册，不修改当前安装，不构建安装包或发布。已有安装和公开版本不会随源码修改自动更新。

## 解码依赖与构建

`libavif-sys 0.17.0+libavif.1.0.4` + `libaom-sys 0.17.2+libaom.3.11.0`，源码静态编译链接，不依赖 Windows 扩展编解码器或额外运行时 AVIF DLL。Cargo.lock 只新增这两项和 cmake，没有升级既有依赖。

Windows 源码构建需要 Rust MSVC、Visual Studio C++ Build Tools、CMake 和 NASM。构建工具须在该次构建进程的 PATH 中可见，不应把本机路径写进仓库或改变全局 PATH。NASM 使用官方分发；本轮临时工具位于忽略目录 Temp。

原生解析器在解析/解码前限制单边 32768、总像素 67108864、序列 4096 帧、线程 4；本格式输入字节限制 128 MiB。RGBA 输出最多 256 MiB，但原生 YUV、读取缓冲、变换副本和应用缓存仍占用额外内存，因此这不是进程内存硬上限。超限/截断/异常文件返回明确错误，不改写原文件。

## 可重复验证

```powershell
cargo test --workspace
cargo check --workspace
cargo build --workspace
cargo build --release --workspace
python -m unittest discover -s tools/tests -p "test_*.py" -v
python tools/ui-qa/avif_smoke.py --binary target/release/imageview.exe --output ui-verify-shots/avif-new-run --commit <当前提交> --theme dark --width 1280 --height 860
cargo run -p iv-shell --example test_thumbnail -- target/release/iv_shell.dll Temp/avif-thumbs-new crates/iv-core/tests/fixtures/avif/alpha.avif crates/iv-core/tests/fixtures/avif/opaque.avif crates/iv-core/tests/fixtures/avif/animated.avif
```

GUI 与缩略图输出目录必须是新目录。GUI 只控制本次启动的程序，使用隔离 QA 偏好并恢复鼠标与前台，不关闭用户已有窗口；COM 探针直接加载本次 DLL，不依赖或写入真实注册表。

测试覆盖内容识别、兼容品牌/扩展尺寸 ftyp、透明度/颜色参考、大小写、混合目录、动态首帧、截断与畸形文件、尺寸防护；生成的高位深和容器变换测试补充实际编码容器回归。公开/生产图片与多 DPI 未全覆盖，不将单组样例当作所有 AVIF 变体兼容保证。

## 本轮证据

实现分支、构建、截图和主线复验结果在完成后记录于此。
