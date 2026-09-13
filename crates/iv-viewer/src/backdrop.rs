//! 窗口背景伪磨砂（fake acrylic）。
//!
//! 背景：wgpu 的 flip-model swapchain 会绕过 DWM 的重定向表面，系统磨砂材质
//! （DWMWA_SYSTEMBACKDROP_TYPE / SetWindowCompositionAttribute / ExtendFrame）
//! 在这类窗口上全部"返回成功但视觉无效"（已实测验证）。因此改为自实现：
//!
//! 1. 捕获前临时给窗口设置 WDA_EXCLUDEFROMCAPTURE（捕获 API 取不到本窗口，
//!    屏幕上的正常显示完全不受影响）；
//! 2. BitBlt 抓取窗口背后的屏幕区域，直接 StretchBlt 降采样到约 1/8 尺寸
//!    （大幅降采样 + 盒式模糊 ≈ 大半径高斯模糊，即"磨砂"）；
//! 3. 由应用作为画布背景纹理绘制，再叠一层主题 tint 保证可读性。
//!
//! 只用 user32/gdi32/kernel32（GNU 工具链直接链接，无需运行时解析）。

#![allow(non_snake_case)]

use std::sync::atomic::{AtomicIsize, Ordering};

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(cb: Option<unsafe extern "system" fn(isize, isize) -> i32>, lp: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    fn GetWindowTextLengthW(hwnd: isize) -> i32;
    fn GetWindowTextW(hwnd: isize, buf: *mut u16, max: i32) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn GetClientRect(hwnd: isize, rect: *mut WRect) -> i32;
    fn ClientToScreen(hwnd: isize, pt: *mut WPoint) -> i32;
    fn GetDC(hwnd: isize) -> isize;
    fn ReleaseDC(hwnd: isize, dc: isize) -> i32;
    fn SetWindowDisplayAffinity(hwnd: isize, affinity: u32) -> i32;
    fn GetWindowRect(hwnd: isize, rect: *mut WRect) -> i32;
    fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32)
        -> i32;
    fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    fn GetMonitorInfoW(mon: isize, info: *mut MonitorInfo) -> i32;
    fn IsZoomed(hwnd: isize) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn CreateCompatibleBitmap(hdc: isize, w: i32, h: i32) -> isize;
    fn SelectObject(hdc: isize, obj: isize) -> isize;
    fn DeleteObject(obj: isize) -> i32;
    fn DeleteDC(hdc: isize) -> i32;
    fn SetStretchBltMode(hdc: isize, mode: i32) -> i32;
    fn SetBrushOrgEx(hdc: isize, x: i32, y: i32, ppt: isize) -> i32;
    fn StretchBlt(
        ddc: isize,
        xd: i32,
        yd: i32,
        wd: i32,
        hd: i32,
        sdc: isize,
        xs: i32,
        ys: i32,
        ws: i32,
        hs: i32,
        rop: u32,
    ) -> i32;
    fn GetDIBits(
        hdc: isize,
        hbm: isize,
        start: u32,
        lines: u32,
        bits: *mut u8,
        bmi: *mut BitmapInfo,
        usage: u32,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcessId() -> u32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(hwnd: isize, attr: u32, value: *const i32, size: u32) -> i32;
}

/// SetWindowDisplayAffinity 取值
const WDA_NONE: u32 = 0x00;
/// 从所有捕获（BitBlt/截图工具/录屏）中排除本窗口，屏幕显示不受影响
const WDA_EXCLUDEFROMCAPTURE: u32 = 0x11;
const SRCCOPY: u32 = 0x00CC_0020;
/// HALFTONE 拉伸模式：降采样质量最好（盒式平均），等效预模糊
const HALFTONE: i32 = 4;
/// DWMWA_WINDOW_CORNER_PREFERENCE：Win11+ 窗口圆角策略
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
/// DWMWCP_ROUND：圆角
const DWMWCP_ROUND: i32 = 2;
/// MonitorFromWindow：取与窗口交集最大的显示器
const MONITOR_DEFAULTTONEAREST: u32 = 2;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

#[repr(C)]
struct WRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct WPoint {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: WRect,
    work: WRect,
    flags: u32,
}

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    clr_used: u32,
    clr_important: u32,
}

#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [u32; 3],
}

static FOUND_HWND: AtomicIsize = AtomicIsize::new(0);
static FOUND_AREA: AtomicIsize = AtomicIsize::new(0);

unsafe extern "system" fn enum_cb(hwnd: isize, _lp: isize) -> i32 {
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid != GetCurrentProcessId() || IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    // 进程里还有多个顶层窗口：调试构建的控制台窗口（ConsoleWindowClass，
    // 标题为 exe 路径）、winit 事件目标窗口、IME 辅助窗口等。
    // 主窗口标题始终以 "ImageViewer" 结尾（"<文件> — LcL ImageViewer"），
    // 控制台标题是 exe 路径（"imageview.exe"），不会误匹配。
    let n = GetWindowTextLengthW(hwnd);
    if n <= 0 {
        return 1;
    }
    let mut buf = vec![0u16; (n + 1) as usize];
    GetWindowTextW(hwnd, buf.as_mut_ptr(), n + 1);
    let title = String::from_utf16_lossy(&buf[..n as usize]);
    if !title.ends_with("ImageViewer") {
        return 1;
    }
    let mut r = WRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetClientRect(hwnd, &mut r) != 0 {
        let area = (r.right - r.left).max(0) as isize * (r.bottom - r.top).max(0) as isize;
        if area > FOUND_AREA.load(Ordering::Relaxed) {
            FOUND_AREA.store(area, Ordering::Relaxed);
            FOUND_HWND.store(hwnd, Ordering::Relaxed);
        }
    }
    1
}

/// 枚举当前进程的顶层可见窗口，按标题（以 "ImageViewer" 结尾）识别主窗口，
/// 多个匹配时取客户区面积最大者。
pub fn find_own_window() -> Option<isize> {
    FOUND_HWND.store(0, Ordering::Relaxed);
    FOUND_AREA.store(0, Ordering::Relaxed);
    unsafe {
        EnumWindows(Some(enum_cb), 0);
    }
    match FOUND_HWND.load(Ordering::Relaxed) {
        0 => None,
        h => Some(h),
    }
}

/// 截图排除开关：开启后所有捕获 API 都取不到本窗口内容
/// （屏幕上用户看到的显示完全不变）。返回系统是否接受（老系统可能不支持）。
pub fn set_exclude_from_capture(hwnd: isize, exclude: bool) -> bool {
    let affinity = if exclude {
        WDA_EXCLUDEFROMCAPTURE
    } else {
        WDA_NONE
    };
    unsafe { SetWindowDisplayAffinity(hwnd, affinity) != 0 }
}

/// 无边框窗口启用 Win11 圆角（DWMWA_WINDOW_CORNER_PREFERENCE = ROUND）。
/// 去掉系统标题栏/边框后，默认是直角方窗；设为圆角更现代，也与内部的
/// 玻璃圆角胶囊语言一致。最大化时 DWM 自动不应用圆角。返回是否被接受。
pub fn set_rounded_corners(hwnd: isize) -> bool {
    let v = DWMWCP_ROUND;
    unsafe { DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &v, 4) == 0 }
}

/// 启动时把窗口夹回最近显示器的工作区：无边框窗口没有系统标题栏，
/// 若上次退出时窗口被拖到屏幕外，恢复后将无从抓取（仅剩任务栏右键）。
/// 仅在首帧调用一次（不影响用户之后正常拖动到任意位置）。
pub fn clamp_window_onscreen(hwnd: isize) {
    unsafe {
        if IsIconic(hwnd) != 0 || IsZoomed(hwnd) != 0 {
            return; // 最小化/最大化时不调整
        }
        let mut r = WRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd, &mut r) == 0 {
            return;
        }
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MonitorInfo {
            size: std::mem::size_of::<MonitorInfo>() as u32,
            monitor: WRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            work: WRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
            flags: 0,
        };
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return;
        }
        let wa = mi.work;
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        // 某轴向比工作区还宽/高时贴起点；否则整窗夹进工作区
        let nx = if w <= wa.right - wa.left {
            r.left.clamp(wa.left, wa.right - w)
        } else {
            wa.left
        };
        let ny = if h <= wa.bottom - wa.top {
            r.top.clamp(wa.top, wa.bottom - h)
        } else {
            wa.top
        };
        if (nx, ny) != (r.left, r.top) {
            SetWindowPos(
                hwnd,
                0,
                nx,
                ny,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

/// 窗口背后画面的低分辨率模糊快照（RGBA8）。
pub struct Capture {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// 抓取本窗口背后的屏幕区域，直接降采样到约 1/8 尺寸并柔化。
/// 调用前须先 set_exclude_from_capture(hwnd, true) 并等待约 80ms
/// （让 DWM 合成器应用排除标志），否则会把本窗口内容一起抓进去。
pub fn capture_behind(hwnd: isize, blur: f32) -> Result<Capture, String> {
    unsafe {
        if IsIconic(hwnd) != 0 {
            return Err("窗口已最小化".into());
        }
        // 抓客户区（而非含标题栏的外框）：快照与画布 1:1 对齐，
        // 窗口边缘处的模糊背景才能与窗外实景无缝衔接
        let mut r = WRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetClientRect(hwnd, &mut r) == 0 {
            return Err(format!(
                "GetClientRect 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        if w <= 0 || h <= 0 {
            return Err(format!("客户区尺寸无效：{w}×{h}"));
        }
        let mut pt = WPoint { x: 0, y: 0 };
        if ClientToScreen(hwnd, &mut pt) == 0 {
            return Err(format!(
                "ClientToScreen 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        // 模糊强度 0..1 → 降采样因子 5..14（采样越少越糊）；采样边长限制 48..480
        let blur = blur.clamp(0.0, 1.0);
        let divisor = (5.0 + blur * 9.0).round() as i32;
        let tw = ((w + divisor / 2) / divisor).clamp(48, 480);
        let th = ((h + divisor / 2) / divisor).clamp(48, 480);

        let screen = GetDC(0); // 整个虚拟屏幕（多显示器/负坐标均有效）
        if screen == 0 {
            return Err(format!("GetDC 失败：{}", std::io::Error::last_os_error()));
        }
        let mem = CreateCompatibleDC(screen);
        if mem == 0 {
            ReleaseDC(0, screen);
            return Err(format!(
                "CreateCompatibleDC 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        let hbm = CreateCompatibleBitmap(screen, tw, th);
        if hbm == 0 {
            DeleteDC(mem);
            ReleaseDC(0, screen);
            return Err(format!(
                "CreateCompatibleBitmap 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        let old = SelectObject(mem, hbm);
        if old == 0 {
            DeleteObject(hbm);
            DeleteDC(mem);
            ReleaseDC(0, screen);
            return Err(format!(
                "SelectObject 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        SetStretchBltMode(mem, HALFTONE);
        SetBrushOrgEx(mem, 0, 0, 0); // HALFTONE 模式要求设置画刷原点
        let ok = StretchBlt(mem, 0, 0, tw, th, screen, pt.x, pt.y, w, h, SRCCOPY);

        let mut bi = BitmapInfo {
            header: BitmapInfoHeader {
                size: 40,
                width: tw,
                height: -th, // 负值 = 自上而下
                planes: 1,
                bit_count: 32,
                compression: 0, // BI_RGB
                size_image: 0,
                x_ppm: 0,
                y_ppm: 0,
                clr_used: 0,
                clr_important: 0,
            },
            colors: [0; 3],
        };
        let mut buf = vec![0u8; (tw * th * 4) as usize];
        let lines = GetDIBits(mem, hbm, 0, th as u32, buf.as_mut_ptr(), &mut bi, 0);
        SelectObject(mem, old);
        DeleteObject(hbm);
        DeleteDC(mem);
        ReleaseDC(0, screen);
        if ok == 0 || lines == 0 {
            return Err(format!(
                "StretchBlt/GetDIBits 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        // GDI 输出 BGRA → RGBA
        for px in buf.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        let mut cap = Capture {
            rgba: buf,
            width: tw as u32,
            height: th as u32,
        };
        // 高斯模糊让降采样结果更细腻，半径随模糊强度
        soften(&mut cap, blur);
        Ok(cap)
    }
}

/// 可分离高斯模糊（水平 + 垂直两遍）：低分辨率快照上开销可忽略，
/// 相比盒式模糊更接近真实大半径高斯，磨砂无方块感、更细腻。
/// `strength` 0..1 → 高斯半径 1..=4（作用于降采样后的快照像素）。
fn soften(cap: &mut Capture, strength: f32) {
    let (w, h) = (cap.width as usize, cap.height as usize);
    if w == 0 || h == 0 {
        return;
    }
    let radius = 1 + (strength.clamp(0.0, 1.0) * 3.0).round() as i32;
    let kernel = gaussian_kernel(radius);
    let mut tmp = vec![0u8; cap.rgba.len()];
    blur_pass(&cap.rgba, &mut tmp, w, h, &kernel, true); // 水平
    blur_pass(&tmp, &mut cap.rgba, w, h, &kernel, false); // 垂直
}

/// 生成归一化的一维高斯核（长度 2*radius+1）。
fn gaussian_kernel(radius: i32) -> Vec<f32> {
    let sigma = (radius as f32 * 0.6).max(0.5);
    let mut k = Vec::with_capacity((radius * 2 + 1) as usize);
    let mut sum = 0.0;
    for i in -radius..=radius {
        let w = (-(i as f32).powi(2) / (2.0 * sigma * sigma)).exp();
        k.push(w);
        sum += w;
    }
    for w in &mut k {
        *w /= sum;
    }
    k
}

/// 单遍一维高斯模糊（horizontal=水平/垂直），边缘像素箝位复用。
fn blur_pass(src: &[u8], dst: &mut [u8], w: usize, h: usize, kernel: &[f32], horizontal: bool) {
    let radius = (kernel.len() / 2) as i32;
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0f32; 3];
            for (ki, &kw) in kernel.iter().enumerate() {
                let off = ki as i32 - radius;
                let (xx, yy) = if horizontal {
                    (x as i32 + off, y as i32)
                } else {
                    (x as i32, y as i32 + off)
                };
                let xx = xx.clamp(0, w as i32 - 1) as usize;
                let yy = yy.clamp(0, h as i32 - 1) as usize;
                let i = (yy * w + xx) * 4;
                acc[0] += src[i] as f32 * kw;
                acc[1] += src[i + 1] as f32 * kw;
                acc[2] += src[i + 2] as f32 * kw;
            }
            let o = (y * w + x) * 4;
            dst[o] = acc[0].round() as u8;
            dst[o + 1] = acc[1].round() as u8;
            dst[o + 2] = acc[2].round() as u8;
            dst[o + 3] = 255;
        }
    }
}
