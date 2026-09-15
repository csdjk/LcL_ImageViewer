<div align="center">

<img src="docs/icon.png" width="96" alt="LcL ImageViewer 图标">

# LcL ImageViewer

**快速切换 R / G / B / Alpha，检查游戏贴图、透明边缘与像素数据。**

面向游戏美术、技术美术与独立开发者的轻量级 Windows 图片查看器。

[![Release](https://img.shields.io/github/v/release/csdjk/LcL_ImageViewer?style=flat-square)](https://github.com/csdjk/LcL_ImageViewer/releases/latest)
[![Windows x64](https://img.shields.io/badge/Windows-10%20%2F%2011%20x64-blue?style=flat-square)](https://github.com/csdjk/LcL_ImageViewer/releases/latest)
[![MIT](https://img.shields.io/badge/license-MIT-green?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/built%20with-Rust-orange?style=flat-square)](https://www.rust-lang.org/)

[下载使用](https://github.com/csdjk/LcL_ImageViewer/releases/latest) · [RGBA 通道检查](#rgba-通道检查) · [快捷键](#快捷键) · [反馈问题](https://github.com/csdjk/LcL_ImageViewer/issues)

</div>

## RGBA 通道检查

不仅查看一张图的最终颜色，也能单独检查它的每个通道。支持 **RGB 完整显示、R / G / B / Alpha 单通道灰度显示，以及忽略 Alpha 的彩色显示**，适合 UI、特效、遮罩和通道打包贴图的日常检查。

通过顶部工具栏的 **RGB / R / G / B / A** 按钮切换；窗口较窄时使用通道下拉菜单，也可以直接按数字键。

| 显示模式 | 快捷键 | 显示内容与用途 |
| --- | --- | --- |
| **R 红通道** | `1` | 将 R 通道显示为灰度，检查该通道的数据分布 |
| **G 绿通道** | `2` | 将 G 通道显示为灰度，检查该通道的数据分布 |
| **B 蓝通道** | `3` | 将 B 通道显示为灰度，检查该通道的数据分布 |
| **Alpha 透明度** | `4` | 将 Alpha 显示为灰度：黑色为全透明，白色为不透明，灰色为半透明 |
| **RGB 完整显示** | `5` 或 `C` | 显示彩色图像，并保留原图透明度；工具栏中的名称为 RGB |
| **RGB 忽略 Alpha** | `O` | 忽略透明度显示彩色内容，检查透明区域下仍然保存的 RGB 数据 |

**`O` 是进入“忽略 Alpha”模式，不是开关：恢复透明显示请按 `5` 或 `C`。** 工具栏的 **A 按钮**代表 Alpha，键盘上的 **`A`** 用于上一张；查看 Alpha 的快捷键是 **`4`**。

各模式一次显示一种通道视图，并非任意组合开关多个通道。切换只改变预览，不修改文件，也不重新编码图片。

### Alpha 实机示例

![Alpha 灰度视图：黑色为透明区域，白色为不透明主体](docs/screenshots/alpha-channel.jpg)

### 贴图检查怎么用？

| 检查对象 | 推荐操作 |
| --- | --- |
| **UI / 特效透明边缘** | `5` 看完整效果，`4` 看 Alpha 轮廓，结合棋盘格或纯色背景观察边缘与半透明过渡 |
| **透明区域里的颜色** | 按 `O` 查看被 Alpha 隐藏的 RGB，再用像素检查器核对透明像素中的颜色值 |
| **通道打包材质 / Mask** | 按 `1 → 2 → 3 → 4` 逐通道查看，检查各通道是否放入了预期的遮罩或材质数据 |
| **像素风 / 细小杂点** | 按 `N` 使用最近邻采样，放大后配合单通道视图定位异常像素；按 `0` 回到实际大小 |

例如，你的材质约定 R 存金属度、G 存粗糙度、B 存环境遮蔽、A 存遮罩，就可以逐个通道核对。**这只是通道用途的示例，并不是统一标准；查看器不会自动判断每个通道的材质含义。**

## 像素读取与透明度排查

把鼠标移到图片上，底部状态栏会显示**像素坐标与 `#RRGGBBAA`**。打开右键菜单中的 **“像素检查器…”**，可查看完整 RGBA 十进制和浮点读数；**“复制像素值”**可复制当前取到的颜色。动画读取当前帧，多 Mip 图片读取当前层级。

例如，`(255, 255, 255, 0)` 和 `(0, 0, 0, 0)` 都是全透明像素，但隐藏的 RGB 数据不同。结合 **Alpha 灰度视图、忽略 Alpha 模式和像素数值**，可以分别检查透明度与颜色，而不是只凭最终合成画面判断。

读数来自**解码后的图片像素**，不是屏幕截图，因此不会把界面的阴影、棋盘格或背景色混入结果。普通图片的内部数据为 RGBA8，浮点读数由其除以 255 得到，不额外做 sRGB 到线性的转换；HDR 保留浮点数据供读取，8 位读数会截到可表示范围。

## 界面预览

<table>
  <tr>
    <td width="50%"><img src="docs/screenshot-dark.jpg" alt="深色主题：悬浮工具栏与两侧切图按钮"></td>
    <td width="50%"><img src="docs/screenshot-light.jpg" alt="浅色主题：雾蓝灰新拟态控件"></td>
  </tr>
  <tr>
    <td align="center">深色主题</td>
    <td align="center">浅色主题</td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/settings.png" alt="紧凑设置弹窗，标题可独立拖动"></td>
    <td><img src="docs/screenshots/context-menu.png" alt="精简右键菜单"></td>
  </tr>
  <tr>
    <td align="center">设置弹窗 · 独立拖动</td>
    <td align="center">右键菜单 · 常用文件操作</td>
  </tr>
</table>

配图为程序实机示例，截图中的版本号仅对应截图时的构建。

## 浏览与检查功能

| 功能 | 说明 |
| --- | --- |
| **同目录浏览** | 左右悬浮按钮或快捷键切换图片；支持跳到第一张 / 最后一张，到达首尾不循环 |
| **缩放与平移** | 滚轮围绕光标缩放，左键 / 中键拖图，适配窗口与实际大小 100% 切换 |
| **采样方式** | 最近邻适合观察像素边界，双线性用于平滑缩放；可随时切换 |
| **Mipmap 检查** | 对包含多个 Mip 的图片切换层级，查看不同分辨率下的贴图与像素数据 |
| **HDR 曝光** | 保留 HDR 浮点数据，调节曝光辅助查看亮暗细节 |
| **动画检查** | GIF / APNG / 动态 WebP 播放暂停、逐帧步进和帧进度控制 |
| **图像属性** | 查看图像尺寸、格式及可用的压缩、Mip、动画等信息 |
| **文件操作** | 复制路径、直接打开图片所在文件夹、确认后将图片移入回收站 |
| **界面与背景** | 深浅主题、棋盘格 / 纯色透明衬底、可选桌面磨砂及其参数、减少动效；悬浮栏无操作 1 秒后开始淡出 |
| **紧凑设置** | 无分类列表，设置标题左键 / 右键拖动只移动内部弹窗，主窗口保持不动 |
| **加载与预读** | 首图解码与窗口初始化并行，目录后台扫描；有限相邻图片预读与缓存，优先处理最新的切图请求 |
| **Windows 集成** | 可选注册到“打开方式”、成为默认应用候选，以及资源管理器缩略图扩展；不自动抢占默认看图软件 |

## 支持的图片格式

| 类型 | 格式与说明 |
| --- | --- |
| **常规图片** | PNG、JPG / JPEG、BMP、TIFF、ICO、静态 WebP |
| **游戏贴图** | DDS、TGA、PSD、QOI |
| **动画图片** | GIF、APNG、动态 WebP |
| **高动态范围 / 数据图片** | Radiance HDR、PPM / PGM / PBM |

DDS 支持 BC1–BC7、部分未压缩 / 浮点格式和文件自带的 Mip 链；Cubemap / Texture Array 当前只读取第一个面或元素，并非完整的立方体或数组浏览器。PSD 读取已保存的**合成图**，支持 8 位 RGB / 灰度及 RAW / RLE，不解析图层、不支持 PSB，也不是图层编辑器。

## 下载与安装

在 [GitHub Releases](https://github.com/csdjk/LcL_ImageViewer/releases/latest) 下载最新版本：

| 文件 | 用途 |
| --- | --- |
| `LcL-ImageViewer-Setup-v*-win64.exe` | **安装版**：安装向导、开始菜单入口、卸载器，可选“打开方式”和缩略图注册 |
| `LcL-ImageViewer-v*-win64.zip` | **免安装版**：解压后直接运行 `imageview.exe`，普通看图无需注册 DLL |
| `SHA256SUMS.txt` | 安装包与 ZIP 的 SHA-256 校验值 |

全新安装默认 **`D:\Program Files\LcL ImageViewer`**；没有 D 盘时回退到 `%LOCALAPPDATA%\Programs\LcL ImageViewer`。升级优先沿用原目录，安装向导始终允许手动选择位置，不会自动把已有安装迁移到 D 盘。

安装或替换程序前关闭旧查看器。界面偏好保存在 `%APPDATA%\LcL ImageViewer`，免安装版也使用此位置。发布包尚未配置代码签名，可用 `Get-FileHash` 与 `SHA256SUMS.txt` 核对完整性。

**快速开始：** 把图片拖进窗口或按 `Ctrl+O` 打开，按 `1 / 2 / 3 / 4` 查看通道，按 `5` 恢复完整显示，再用 `A / D` 浏览同目录图片。

## 鼠标操作

| 操作 | 效果 |
| --- | --- |
| 画布左键 / 中键拖拽 | 移动图片，主窗口不动 |
| 画布右键拖拽 | 移动整个查看器窗口 |
| 画布右键单击 | 打开精简菜单；拖动结束不会误弹菜单 |
| 滚轮 | 以光标位置为中心缩放图片 |
| 窗口边缘左键拖拽 | 调整主窗口大小 |
| 设置标题或标题内空白处，左键 / 右键拖拽 | 移动设置弹窗，主窗口不动；当前会话保留位置，弹窗限制在查看器内部 |

开关、滑条、按钮和关闭按钮不属于设置弹窗的标题拖动区。

## 快捷键

| 按键 | 功能 |
| --- | --- |
| **`1` / `2` / `3` / `4`** | **R / G / B / Alpha 单通道灰度显示** |
| **`5` 或 `C`** | **恢复完整彩色显示，保留 Alpha** |
| **`O`** | **进入忽略 Alpha 的彩色显示模式** |
| `Ctrl+O` | 打开图片 |
| `A` / `D`、`←` / `→`、`PgUp` / `PgDn` | 同目录上一张 / 下一张 |
| `Home` / `End` | 同目录第一张 / 最后一张 |
| `F` | 适配窗口 |
| `0` | 实际大小 100% |
| `N` | 最近邻 / 双线性采样 |
| `↑` / `↓` | Mip 索引增加 / 减少；图片须包含多个 Mip |
| `Space` | 动画播放 / 暂停 |
| `,` / `.` | 动画上一帧 / 下一帧，并暂停播放 |
| `T` | 深色 / 浅色主题 |
| `Delete` | 确认后将当前图片移入回收站 |
| `Esc` | 优先关闭菜单或弹层；没有弹层时退出 |

输入或操作相关弹层时，不会误触发切图与删除。Delete 不响应组合键或长按连发，取消不改变文件；回收站不可用时不回退为永久删除。删除成功后优先显示下一张，末尾回退到上一张，全部删完回到空白状态。

## Windows 打开方式与缩略图

**打开方式：** 安装时勾选对应选项，或从**实际安装目录**运行程序，进入 **设置 → 打开方式 → 注册**。需要设为默认看图软件时，在 **设置 → 默认看图软件 → 系统设置** 中自行确认。不要从工程或临时解压副本注册后又移动 EXE，以免打开命令失效。

**资源管理器缩略图：** 可选的 `iv_shell.dll` 为 `.dds .tga .psd .qoi .hdr .ppm .pgm .pbm` 提供预览。安装版可勾选注册；免安装版在固定目录中保留 DLL 和脚本后，按需执行：

```powershell
# 注册缩略图扩展
powershell -NoProfile -ExecutionPolicy Bypass -File .\register_thumbnail.ps1

# 移动或删除免安装目录前，先注销本扩展
powershell -NoProfile -ExecutionPolicy Bypass -File .\unregister_thumbnail.ps1
```

注册使用当前用户的 HKCU，无需管理员权限；文件夹本身仍需有写入权限。启用缩略图前留意其他图片软件可能已占用同一格式的处理器。普通看图与 RGBA 切换不依赖这些注册操作。

<details>
<summary><strong>从源码构建</strong></summary>

需要 Windows x64 Rust 工具链；MSVC 构建需 C++ Build Tools / Windows SDK。制作安装包另需 Python 3.11+ 和 Inno Setup 7。

```powershell
cargo test --workspace --locked
cargo build --release --workspace --locked

# 主程序：target\release\imageview.exe
# 缩略图扩展：target\release\iv_shell.dll

python tools/package_release.py --iscc "C:\Path\To\Inno Setup 7\ISCC.exe"
# 安装包、ZIP、SHA256SUMS.txt 输出到 dist 下的对应版本目录
```

</details>

## 使用边界

这是查看与检查工具，不是图片编辑器：通道切换、缩放与曝光不会写回原图，**显式确认删除**则会把文件移入回收站。普通图片统一为 RGBA8，不适合用来核验 16 位源文件的全部原始精度；HDR 显示经过曝光与色调映射，不能把显示亮度等同于原始浮点值。

超大图片、复杂贴图和长动画首次打开仍可能需要解码时间；进程内缓存不等于跨进程秒开。同目录导航使用快照，外部新增或重命名图片后重新打开文件可刷新列表。桌面磨砂依赖系统捕获能力，其他 DPI、多显示器和全部格式变体仍需更多验证。

开发与验证资料：[协作入口](AGENTS.md) · [加载性能实测](docs/performance/image-loading.md) · [历史更新日志](CHANGELOG.md)。

## License

[MIT](LICENSE)
