# macOS 预览构建

本分支新增 macOS 平台适配，使用 GitHub Actions 的标准 macOS runner 原生构建，不在 Windows 上伪装或改后缀生成安装包。现有 Windows 正式版保持不变。

## 安装

在 [macOS 构建工作流](https://github.com/csdjk/LcL_ImageViewer/actions/workflows/macos-package.yml) 选择成功的运行，从 Artifacts 下载与机器匹配的安装包压缩文件，解压后打开 DMG，将 **LcL ImageViewer.app** 拖入 **Applications**。

| 包名后缀 | 对应电脑 |
| --- | --- |
| `arm64.dmg` | Apple Silicon（M 系列芯片） |
| `x86_64.dmg` | Intel Mac |

工作流产物保留 30 天，源码和打包脚本长期保留，可手动再次运行。目标最低系统为 macOS 12.0；实际验证的 runner 系统与二进制架构写在 `build-manifest.json`，不能将最低部署目标等同于已经测试所有旧系统。

## 签名状态

预览包使用 **ad-hoc 本地签名**，没有 Developer ID 身份签名，也没有 Apple 公证。包内校验文件只能验证下载完整性，不能替代开发者身份或公证。

首次打开可能遇到安全提示。确认包来自本项目且校验匹配后，可按 macOS“系统设置 → 隐私与安全性”的“仍要打开”提示确认本应用；不要关闭整个系统的 Gatekeeper。公开商业分发前需要配置自己的 Developer ID 与公证流程。

## 功能范围

共享代码保留常见图片/游戏贴图解码、RGBA 单通道、GIF/APNG/WebP/AVIF 动画、逐帧、缩放平移、子目录浏览、背景调色板和置顶。

Mac 适配包含：Command+O 打开面板、原生父窗口、通过 Finder 打开所在目录、系统废纸篓（失败时不永久删除）、系统中文字体。字体仅在 Mac 上读取，不随安装包分发。

Windows 专属的桌面捕获磨砂、注册表文件关联和 Explorer 缩略图扩展未移植。Mac 预览暂不提供 Finder Quick Look 扩展或文件“打开方式”关联；先打开应用，再用按钮、Command+O 或拖入图片。

README 的现有功能示意图来自 Windows 实机，不冒充 macOS 截图。macOS 的编译、测试、启动、截图和 DMG 校验证据由工作流单独输出。

## 可重复构建

工作流 `.github/workflows/macos-package.yml` 对 `aarch64-apple-darwin` 与 `x86_64-apple-darwin` 分别执行：

```sh
cargo test --locked -p iv-core -p iv-viewer --target <target>
cargo build --locked --release -p iv-viewer --target <target>
python3 tools/macos/package.py --target <target> --arch <arch>
python3 tools/macos/smoke.py --app 'dist/macos/LcL ImageViewer.app' --output evidence
```

`.app` 包含图标、许可、版本和源码提交信息；打包脚本核验 Mach-O 架构及动态库依赖，不允许依赖 runner 的 Homebrew dylib。DMG 包含 Applications 快捷入口及安装说明；工作流挂载 DMG 后重新核验并启动其中的应用。屏幕录制权限不足时必须明确记录，不将缺少截图视为视觉验收通过。

本次不自动发布新 Release 或替换 Windows Latest。
