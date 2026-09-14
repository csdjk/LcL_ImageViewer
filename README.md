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

悬浮式沉浸布局：工具栏与状态栏使用雾蓝灰新拟态表面，12–16 点圆角、多层柔影与凸起/凹陷操作反馈。指针静止时自动隐藏，让图像占据全部视野。新用户默认浅色，深/浅双主题一键切换（`T`），已有主题偏好继续保留。设计规范见 [新拟态 UI 规范](docs/新拟态UI规范.md)。上方预览图为历史版本；本轮实际截图与验收记录见 [新拟态验收](docs/ui-qa/新拟态验收.md)。

## 功能特性

- **广泛格式支持**：DDS (BC1–BC7) / PSD / TGA / QOI / HDR / PNM / GIF / WebP / APNG，以及 PNG / JPG / BMP / TIFF 等常规格式
- **动画播放**：GIF / WebP / APNG 播放暂停、逐帧步进、帧进度条
- **贴图检查**：R / G / B / A 单通道查看、Mipmap 层级切换、HDR 曝光调节、最近邻/双线性采样切换
- **像素检查器**：光标处像素坐标 + RGBA8 十六进制/十进制 + 浮点值实时读数
- **图像导航**：同目录图片上一张/下一张快速切换
- **视图操作**：画布左键/中键拖拽平移图像、右键拖拽移动窗口（单击右键打开菜单）、滚轮以光标为中心缩放、适配窗口 / 实际大小、全屏
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
| `A` / `D` 或 `←` / `→` | 同目录上一张 / 下一张 | `1` `2` `3` `4` | 查看 R / G / B / A 单通道 |
| 滚轮 | 缩放（以光标为中心） | `5` 或 `C` | 恢复完整 RGBA |
| 画布左键拖拽 | 平移图像 | `O` | 忽略 Alpha |
| 中键拖拽 | 平移图像 | `F` | 适配窗口 |
| `0` | 实际大小 100% | `N` | 最近邻 / 双线性采样 |
| `PgUp` / `PgDn` | 切换 Mipmap 层级 | `Space` | 动画播放 / 暂停 |
| `,` `.` | 逐帧步进 | `T` | 深色 / 浅色主题切换 |
| `F11` | 全屏 | `Esc` | 关闭弹层 / 退出程序 |
| 右键单击 / 拖拽 | 精简文件菜单 / 移动窗口 | `Delete` | 确认后将当前图片移入回收站 |

右键菜单保留复制像素/路径、打开所在文件夹、删除图片、图像属性、像素检查器、打开文件与设置；移除重复的导航、缩放、通道、主题和格式工具，相关功能仍在顶部工具栏及快捷键中。A/D在输入框或弹层中不切图；Delete不会响应组合键或长按连发，取消不改变文件，无法回收时不永久删除。删除成功后显示下一张，末尾回退到上一张，最后一张删除后回到空态。

设置页为紧凑无分类列表。按住“设置”标题或标题内空白处，用左键或右键拖动设置弹窗，主窗口保持不动；控件和关闭按钮不会误拖。弹窗位置在当前会话保留并限制在查看器客户区内，画布右键仍移动主窗口。

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

## 开发协作

- [当前状态](docs/status.md)：当前阶段、已验证事实和唯一 NEXT。
- [路线图](docs/roadmap.md)：阶段顺序与 Stage Gate。
- [任务板](docs/task-board.md)：Task、Owner、依赖、修改范围和验收。
- [当前开发计划](docs/development-plan.md)：复杂 Task 的完整执行合同。
- [待决策事项](docs/needs-decision.md)：必须由用户决定的问题与恢复条件。
- [玻璃磨砂 UI 优化方案](docs/玻璃磨砂UI优化方案.md)：视觉、渲染与验收规范。

所有开发 Agent 先阅读根目录 [`AGENTS.md`](AGENTS.md)。定时开发只在人工跑通一个完整 Task 闭环后启用，Prompt 见 [`docs/scheduled-developer-prompt.md`](docs/scheduled-developer-prompt.md)。

## 技术栈

- **Rust** workspace：`iv-viewer`（主程序）/ `iv-core`（格式解码）/ `iv-shell`（Explorer 缩略图扩展）
- **egui 0.27 + eframe**：即时模式 GUI，悬浮层、自绘右键菜单、新拟态主题均基于 egui 绘制
- **wgpu**：GPU 渲染（WGSL shader 实现通道分离 / Mip 采样 / HDR 曝光）
- **Windows 集成**：DWM 沉浸式标题栏、HKCU 文件关联、IThumbnailProvider 缩略图

## License

[MIT](LICENSE)
