# AVIF 支持与验收

## 使用范围

- `.avif` 文件可直接打开、拖入窗口、在文件对话框中选择，并参与同目录/向下子目录切图；扩展名不区分大小写。实际解码依内容识别，不盲信扩展名。
- 静态 AVIF 输出直通 Alpha 的 RGBA8，复用 RGBA 通道、像素取样、棋盘格与图片边界功能。透明 RGB 不进行预乘存储。
- 8/10/12 位输入按现有查看器 LDR 管线转换为 8 位显示。不提供 HDR 色彩映射和 ICC 色彩管理；相关文件在附加信息中标注。此限制不改变其他格式原来的处理方式。
- 支持容器中的 clean aperture、逆时针旋转和镜像。非方形像素比例暂按原始像素网格显示，并提示。
- 动态 AVIF 解码完整帧序列，自动循环播放，支持 Space 播放/暂停、逗号/句号逐帧与帧进度条；逐帧保留透明度和容器变换。沿用查看器其他动画格式的循环预览习惯，不按文件内的有限重复次数自动停止。资源管理器缩略图仍仅解码首帧，不预解码整个动画。
- Windows 缩略图 DLL、安装脚本及手动注册脚本已加入 AVIF；本轮不执行系统注册，不修改当前安装，不构建安装包或发布。已有安装和公开版本不会随源码修改自动更新。

## 解码依赖与构建

`libavif-sys 0.17.0+libavif.1.0.4` + `libaom-sys 0.17.2+libaom.3.11.0`，源码静态编译链接，不依赖 Windows 扩展编解码器或额外运行时 AVIF DLL。Cargo.lock 只新增这两项和 cmake，没有升级既有依赖。

Windows 源码构建需要 Rust MSVC、Visual Studio C++ Build Tools、CMake 和 NASM。构建工具须在该次构建进程的 PATH 中可见，不应把本机路径写进仓库或改变全局 PATH。NASM 使用官方分发；本轮临时工具位于忽略目录 Temp。

原生解析器在解析/解码前限制单边 32768、总像素 67108864、序列 4096 帧、线程 4；本格式输入字节限制 128 MiB。静态单帧 RGBA 输出最多 256 MiB；动态按未裁剪尺寸预估全序列与首帧 Mip 副本，总计最多 256 MiB，超限在帧解码前拒绝，不静默截断动画。帧时长用整数时间戳舍入到毫秒，零/不足 1ms 按 1ms，不套用 GIF 的 100ms 修正。原生 YUV、读取缓冲、变换副本和应用缓存仍占用额外内存，因此这不是进程内存硬上限。超限/截断/异常文件返回明确错误，不改写原文件。

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

## 静态支持阶段历史证据（FMT-01）

实现提交 `c1f9b17`；分支最终工作区测试 87 项通过；安装契约测试 13 项通过；workspace check、Debug/Release 构建通过。保留已有 iv-shell 的 LNK4104 PRIVATE 导出提示，本轮未扩大修改范围。

实际 Release：深色 1280×860、浅色 880×560，DPI 96，共 26 张真实窗口截图；隔离偏好均恢复，源文件哈希未变。混合 PNG/AVIF、大小写扩展名、子目录切换及返回通过；中心 RGBA/Alpha 图像区域与 PNG 参考的最大通道差均为 0。已抽查两主题 RGBA/Alpha 关键区域，蓝橙色块与五级透明度、灰度 Alpha 显示正常，完整截图存于 `ui-verify-shots/avif-worker-dark-1280` 和 `ui-verify-shots/avif-worker-light-880`。

真实缩略图 DLL 直接 COM：3 种 AVIF × 128/512 两尺寸，并与对应 PNG 输出对照，共 12 个输出；6 组透明度与可见 RGB 完全一致，alpha 类型标志正确。证据 `Temp/avif-worker-thumbs/verification.json`。没有注册 DLL 或重启 Explorer。

本轮 Release EXE SHA256 `944dfc1604b3c94e577a1f751f2661c065d77dc1584d040dbc10e54bdc0b7f2f`；DLL SHA256 `e7a2d480c61878900d319c1d5376c7c94f943dc17077ef6cbc6f2a3b30a00cae`。主线集成 `32775e2` 后重新完成 87 项 Rust 测试、13 项安装契约检查、workspace check、Debug/Release 构建。主线 Release 两主题/两尺寸再录 26 张截图，核心 RGBA/Alpha/R/边界/不透明/动画首帧区域与分支验证完全一致；偏好恢复与原文件哈希检查通过。主线 COM 缩略图再输出并验证 12 项，透明度、可见像素与 PNG 参考完全一致。

主线证据：`Temp/avif-main-{tests,check,debug,release,installer}.log`、`ui-verify-shots/avif-main-dark-1280`、`ui-verify-shots/avif-main-light-880`、`Temp/avif-main-thumbs/verification.json`。主线 EXE SHA256 `c060847f3f632cd98427601a66202f3b7619bc8f5a1fa0164dea2cc6faeee3be`，DLL SHA256 `bd96f29c6d2cf6327b2999f9e7af83c7b8a912609960794ebfdb041b782daf74`；PE 导入表未包含外部 AVIF/AOM/dav1d DLL。

截至本轮结束，源文件、当前用户偏好、已安装版本和系统文件关联未更改；未制作安装包、未 push 或发布。最后状态提交仅更新文档，业务代码与上述主线构建一致。


## 动态播放增量（FMT-02）

动态解码沿用固定libavif/libaom版本，无依赖升级。播放器按原时间轴推进并合并掉队帧，避免每次重绘延迟累计导致越播越慢；长时间挂起通过跳过整轮定位，不逐帧追赶。暂停/逐帧/拖条保持现有操作语义。

新增可复现入口：`python tools/ui-qa/animated_avif_smoke.py --binary target/release/imageview.exe --output ui-verify-shots/animated-avif-new --commit <当前提交> --theme dark --width 1280 --height 860`。本轮测试与实际窗口结果完成后填写，不将旧截图作为新验收。


FMT-02 分支验证：101项Rust回归、13项安装契约检查及workspace check通过，Debug构建通过。Release源码编译和链接完成，但Cargo向默认EXE复制时因用户运行中的旧程序（PID 32036）报Access Denied；没有关闭、移动或覆盖旧程序，已从本次 `target/release/deps/imageview.exe` 复制到 `target/animated-avif/release/imageview.exe`，随附当前缩略图DLL与原生依赖许可文件。默认Release路径不能宣称已更新。

分支Release双主题/两尺寸共79张实际窗口截图，验证自动播放/暂停稳定/前后逐帧/完整循环、宽窗口真实拖拽帧条和点击播放、RGBA/Alpha/R与独立PNG参考、菜单及隐藏恢复；DPI96，源文件哈希与用户偏好保持。窄窗口沿用第二行播放/帧数控制和快捷键，帧条仍只在宽窗口展示。首次原子点击滑块探针失败后改为持续按住拖拽验收，未绕过断言；修正后复验全部通过。已人工查看深色末帧和浅色动画控件/Alpha合成画面。

证据：`ui-verify-shots/animated-avif-worker-final-dark-1280`、`ui-verify-shots/animated-avif-worker-final-light-880`；COM直接调用16个输出（4类AVIF及其PNG参考，各128/512两尺寸）透明度和可见RGB完全一致，见 `Temp/animated-avif-worker-thumbs/verification.json`。未注册或修改任何系统关联。

分支Release EXE SHA256 `00eb1dc9f259aaffccbfcf1242b33009fe1a0e433dd214c52f21183624190080`。主线复验结果待填。
