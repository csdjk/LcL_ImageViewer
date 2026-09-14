<div align="center">

<img src="docs/icon.png" width="96" alt="LcL ImageViewer">

# LcL ImageViewer

**轻量级 Windows 看图工具，为游戏美术与贴图工作者设计**

[![Release](https://img.shields.io/github/v/release/csdjk/LcL_ImageViewer?style=flat-square)](https://github.com/csdjk/LcL_ImageViewer/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20x64-blue?style=flat-square)](https://github.com/csdjk/LcL_ImageViewer/releases)
[![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange?style=flat-square)](https://www.rust-lang.org/)

</div>

---

## 界面预览

<table>
  <tr>
    <td><img src="docs/screenshot-light.jpg" alt="浅色主题"></td>
    <td><img src="docs/screenshot-dark.jpg" alt="深色主题"></td>
  </tr>
  <tr>
    <td align="center">浅色 · 雾感苔绿</td>
    <td align="center">深色 · 深绿灰</td>
  </tr>
</table>

悬浮式沉浸布局：工具栏与状态栏以半透明胶囊悬浮于画布之上，指针静止时自动隐藏，让图像占据全部视野。新拟态（Neumorphism）柔和质感，深/浅双主题一键切换（`T`），退出自动记忆。

## 功能特性

- **广泛格式支持**：DDS (BC1–BC7) / PSD / TGA / QOI / HDR / PNM / GIF / WebP / APNG，以及 PNG / JPG / BMP / TIFF 等常规格式
- **动画播放**：GIF / WebP / APNG 播放暂停、逐帧步进、帧进度条
- **贴图检查**：R / G / B / A 单通道查看、Mipmap 层级切换、HDR 曝光调节、最近邻/双线性采样切换
- **像素检查器**：光标处像素坐标 + RGBA8 十六进制/十进制 + 浮点值实时读数
- **图像导航**：同目录图片上一张/下一张快速切换
- **视图操作**：左键拖拽移动窗口、中键拖拽平移图像、滚轮以光标为中心缩放、适配窗口 / 实际大小、全屏
- **资源管理器缩略图**：可选注册 `.dds .tga .psd .qoi .hdr .ppm .pgm .pbm` 缩略图预览（仅当前用户，不影响系统已有处理器）

## 下载

前往 [Releases](https://github.com/csdjk/LcL_ImageViewer/releases) 页面下载最新版本：

| 文件 | 说明 |
| --- | --- |
| `LcL-ImageViewer-Setup-vX.Y.Z-win64.exe` | 安装程序（推荐）：免管理员，含开始菜单快捷方式、文件关联、缩略图注册（可选）、卸载器 |
| `LcL-ImageViewer-vX.Y.Z-win64.zip` | 绿色便携包：解压即用，`imageview.exe` 单文件可运行 |

## 快捷键

| 按键 | 功能 | 按键 | 功能 |
| --- | --- | --- | --- |
| `←` / `→` | 同目录上一张 / 下一张 | `1` `2` `3` `4` | 查看 R / G / B / A 单通道 |
| 滚轮 | 缩放（以光标为中心） | `5` 或 `C` | 恢复完整 RGBA |
| 左键拖拽 | 移动窗口 | `O` | 忽略 Alpha |
| 中键拖拽 | 平移图像 | `F` | 适配窗口 |
| `0` | 实际大小 100% | `N` | 最近邻 / 双线性采样 |
| `PgUp` / `PgDn` | 切换 Mipmap 层级 | `Space` | 动画播放 / 暂停 |
| `,` `.` | 逐帧步进 | `T` | 深色 / 浅色主题切换 |
| `F11` | 全屏 | `Esc` | 关闭弹层 / 退出程序 |
| 右键 | 更多功能菜单 |  |  |

## 资源管理器缩略图

安装版可在安装时勾选自动注册；便携版手动注册：

```powershell
# 注册（右键 → 使用 PowerShell 运行）
register_thumbnail.ps1
# 卸载
unregister_thumbnail.ps1
```

仅写入当前用户注册表（HKCU），无需管理员权限，不覆盖系统已有图片处理器。

## 自行构建

环境要求：Rust 工具链（Windows x64）、Inno Setup 7（仅打安装包需要）。

```powershell
# 构建（release 启用 LTO 全量优化）
cargo build --release --workspace

# 产物
#   target\release\imageview.exe   主程序（单文件可运行）
#   target\release\iv_shell.dll    Explorer 缩略图扩展

# 打安装包（可选）
& "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe" tools\setup.iss
# 输出 dist\LcL-ImageViewer-Setup-vX.Y.Z-win64.exe
```

## 技术栈

- **Rust** workspace：`iv-viewer`（主程序）/ `iv-core`（格式解码）/ `iv-shell`（Explorer 缩略图扩展）
- **egui 0.27 + eframe**：即时模式 GUI，悬浮层、自绘右键菜单、新拟态主题均基于 egui 绘制
- **wgpu**：GPU 渲染（WGSL shader 实现通道分离 / Mip 采样 / HDR 曝光）
- **Windows 集成**：DWM 沉浸式标题栏、HKCU 文件关联、IThumbnailProvider 缩略图

## License

[MIT](LICENSE)
