//! 应用状态与交互：打开/导航/缩放/平移/通道切换/mip 切换/像素检查器。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::{
    Color32, ComboBox, CursorIcon, Frame, Key, PointerButton, Pos2, ResizeDirection, RichText,
    Sense, Slider, Stroke, Vec2, ViewportCommand,
};
use iv_core::decode::{DecodedImage, MipLevel, PixelData};

use crate::loader::{Loader, Msg};
use crate::render::{ChannelMode, Renderer, Uniforms, MAX_GLASS};
use crate::ui::{self, Icon, Palette, ThemeMode};
use crate::winassoc;

/// 预读缓存内存预算（解码后像素总量）
const CACHE_BUDGET_BYTES: usize = 192 * 1024 * 1024;

/// 指针静止后等待 1 秒再淡出；判定和定时重绘共用此值。
const OVERLAY_HIDE_DELAY: Duration = Duration::from_secs(1);

/// 当前显示的图像。
struct CurrentImage {
    path: PathBuf,
    img: Arc<DecodedImage>,
}

/// 视图变换：图像左上角在画布中的位置（逻辑像素）+ 缩放（逻辑像素/图像像素）。
#[derive(Clone, Copy)]
struct ViewTransform {
    offset: Vec2,
    scale: f32,
}

/// 同目录图像列表。
struct Directory {
    files: Vec<PathBuf>,
    index: usize,
}

/// 按字节预算的 LRU 预读缓存。
struct ImageCache {
    map: HashMap<PathBuf, Arc<DecodedImage>>,
    order: Vec<PathBuf>,
    bytes: usize,
    stamps: HashMap<PathBuf, crate::file_watch::FileState>,
}

impl ImageCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
            bytes: 0,
            stamps: HashMap::new(),
        }
    }

    fn image_bytes(img: &DecodedImage) -> usize {
        let px_bytes = |d: &PixelData| match d {
            PixelData::Rgba8(v) => v.len(),
            PixelData::RgbaF32(v) => v.len() * 4,
        };
        let mip: usize = img.mips.iter().map(|m| px_bytes(&m.data)).sum();
        // 动画帧也计入预算（动画图内存大头在这里）
        let frm: usize = img.frames.iter().map(|f| px_bytes(&f.data)).sum();
        mip + frm
    }

    fn stamp(path: &Path) -> Option<crate::file_watch::FileState> {
        let state = crate::file_watch::FileState::read(path);
        matches!(state, crate::file_watch::FileState::Present { .. }).then_some(state)
    }

    fn get(&mut self, path: &Path) -> Option<(Arc<DecodedImage>, crate::file_watch::FileState)> {
        if !self.map.contains_key(path) { return None; }
        if Self::stamp(path).as_ref() != self.stamps.get(path) {
            self.remove(path);
            return None;
        }
        self.order.retain(|p| p != path);
        self.order.push(path.to_path_buf());
        Some((self.map.get(path)?.clone(), self.stamps.get(path)?.clone()))
    }

    fn remove(&mut self, path: &Path) {
        self.stamps.remove(path);
        if let Some(img) = self.map.remove(path) {
            self.bytes = self.bytes.saturating_sub(Self::image_bytes(&img));
        }
        self.order.retain(|p| p != path);
    }

    fn put(&mut self, path: PathBuf, img: Arc<DecodedImage>, keep: &Path, stamp: crate::file_watch::FileState) {
        if self.map.contains_key(&path) {
            return;
        }
        // Stamp pixels with their decoded source, never a later on-disk version.
        if !matches!(stamp, crate::file_watch::FileState::Present { .. }) { return; }
        // Oversized speculative entries must not evict all useful neighbours.
        if Self::image_bytes(&img) > CACHE_BUDGET_BYTES && path != keep { return; }
        self.stamps.insert(path.clone(), stamp);
        self.bytes += Self::image_bytes(&img);
        self.map.insert(path.clone(), img);
        self.order.push(path);
        // 超预算时从最旧开始清（保留当前显示）
        while self.bytes > CACHE_BUDGET_BYTES && self.order.len() > 1 {
            let victim = self.order.iter().position(|p| p != keep).unwrap_or(0);
            let Some(p) = self.order.get(victim).cloned() else {
                break;
            };
            if let Some(img) = self.map.remove(&p) {
                self.bytes -= Self::image_bytes(&img);
            }
            self.order.remove(victim);
            self.stamps.remove(&p);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoadIntent { Open, PreserveView, Reload }

pub struct App {
    loader: Loader,
    file_watcher: crate::file_watch::FileWatcher,
    opened_path: Option<PathBuf>,
    auto_refresh: bool,
    lock_view: bool,
    load_intent: LoadIntent,
    scanner: crate::directory::Scanner,
    directory_generation: u64,
    /// 当前浏览会话的固定搜索根；普通切图不修改它。
    directory_scope: Option<crate::directory::Scope>,
    include_subfolders: bool,
    directory_scanning: bool,
    directory_stats: crate::directory::ScanStats,
    directory_recover_index: Option<usize>,
    cache: ImageCache,
    current: Option<CurrentImage>,
    /// 正在异步加载的路径（显示"加载中"）
    pending: Option<PathBuf>,
    perf_paint_pending: bool,
    directory: Option<Directory>,
    view: ViewTransform,
    auto_fit: bool,
    channel: ChannelMode,
    nearest: bool,
    mip_index: usize,
    exposure: f32,
    renderer: Option<Renderer>,
    error_msg: Option<String>,
    delete_prompt: Option<crate::recycle::Target>,
    delete_job: Option<(PathBuf, Receiver<Result<(), String>>)>,
    /// 底部状态栏文本（像素检查器，每帧更新）
    probe_text: String,
    /// 像素检查器的完整 RGBA / 浮点读数。
    probe_detail: String,
    /// 光标下的像素颜色（状态栏色块 / 右键菜单）
    probe_color: Option<[u8; 4]>,
    /// 图像属性窗口是否打开
    show_props: bool,
    /// 可固定的完整像素检查器窗口是否打开。
    show_probe: bool,
    /// 设置窗口是否打开
    show_settings: bool,
    /// 半透明图像背景：true=棋盘格，false=纯画布背景色
    checkerboard: bool,
    /// 画布纯色与界面主题分离；None 沿用主题/桌面磨砂。
    background_color: Option<[u8; 3]>,
    background_menu_pos: Option<Pos2>,
    background_button_rect: egui::Rect,
    background_custom_open: bool,
    background_palette: crate::color_palette::ColorPalette,
    always_on_top: bool,
    applied_topmost: Option<bool>,
    dialog_escape_until: Option<Instant>,
    /// Show the full image rectangle, including transparent margins.
    show_image_bounds: bool,
    /// "打开方式"注册状态缓存（打开设置窗口时刷新）
    assoc_registered: bool,
    /// 画布上一帧尺寸（检测 resize）
    last_canvas_size: Vec2,
    /// 缩放百分比浮层剩余显示时间（秒）与值
    zoom_flash: Option<(f32, f32)>,
    /// 工具栏触发的延迟动作（画布尺寸确定后执行）
    pending_fit: bool,
    pending_actual: bool,
    /// 动画播放状态（仅当当前图帧数 >1 时有意义）
    playing: bool,
    frame_index: usize,
    /// 下一帧的切换时刻
    next_frame_at: Instant,
    /// 当前窗口标题缓存（变化时才发送 ViewportCommand）
    last_title: String,
    /// 当前主题（退出时持久化）
    theme: ThemeMode,
    /// 悬浮工具栏 / 状态栏不透明度（0..1，自动淡入淡出）
    top_alpha: f32,
    bot_alpha: f32,
    /// 指针最后一次移动时刻（悬浮层自动显隐依据）
    last_move: Instant,
    /// 减少界面淡入淡出动效。
    reduce_motion: bool,
    /// 自绘右键菜单的打开位置（None = 关闭）
    ctx_menu_pos: Option<Pos2>,
    /// 右键拖动窗口的屏幕坐标锚点；与右键单击菜单互斥。
    right_window_drag: Option<crate::backdrop::WindowDrag>,
    /// 设置弹窗在客户区中的位置；首次居中，此后在当前会话保留拖动位置。
    settings_popup_pos: Option<Pos2>,
    right_gesture_dragged: bool,
    /// 本帧玻璃区域（逻辑坐标矩形 + 模糊混合系数 + 圆角半径），绘制悬浮层时收集，
    /// 供图像 shader 在这些区域内做背景模糊（磨砂玻璃）
    glass_regions: Vec<(egui::Rect, f32, f32)>,
    /// 窗口磨砂背景开关（持久化）；关闭时画布回退为不透明渐变
    backdrop: bool,
    /// 磨砂不透明度 0..1（主题 tint 强度；越小越透亮）
    backdrop_opacity: f32,
    /// 磨砂模糊强度 0..1（捕获时降采样深度 + 盒式模糊遍数）
    backdrop_blur: f32,
    /// 磨砂背景亮度 0.5..1.5（捕获画面明暗微调）
    backdrop_brightness: f32,
    /// 本进程主窗口句柄（首次 update 时枚举获取）
    hwnd: Option<isize>,
    /// 伪磨砂背景：窗口背后画面的低分辨率模糊快照纹理
    backdrop_tex: Option<egui::TextureHandle>,
    /// 桌面捕获不可用时的可诊断原因；成功捕获后清空。
    backdrop_capture_error: Option<String>,
    /// 截图排除恢复失败是独立故障，不能被后续成功捕获覆盖。
    backdrop_restore_error: Option<String>,
    /// 桌面捕获与 CPU 高斯在单一后台任务中执行，避免阻塞 UI 线程。
    backdrop_capture_tx: Sender<Result<crate::backdrop::Capture, String>>,
    backdrop_capture_rx: Receiver<Result<crate::backdrop::Capture, String>>,
    backdrop_job_running: bool,
    /// 后台任务执行期间又发生位移/周期刷新时，仅保留一次最新请求。
    backdrop_capture_requested: bool,
    /// 截图排除标志（WDA_EXCLUDEFROMCAPTURE）当前是否已施加到窗口
    capture_excluded: bool,
    /// 截图排除施加时刻（DWM 生效需等待约 80ms 才能 BitBlt）
    exclude_since: Option<Instant>,
    /// 最近一次背景快照时刻（捕获节流 + 空闲周期刷新依据）
    last_capture: Instant,
    /// 停止移动/调整后释放截图排除的时刻
    exclude_release_at: Option<Instant>,
    /// 截图排除不被系统支持（老系统）→ 永久回退渐变画布
    exclude_unsupported: bool,
    /// 上次视口外框（检测窗口移动/调整大小以触发重捕）
    last_outer_rect: Option<egui::Rect>,
    /// 防止玻璃区域超限时每帧重复输出同一诊断。
    glass_overflow_reported: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, initial_path: Option<PathBuf>, loader: Loader) -> Self {
        crate::perf::mark("app_new", None, 0.0);
        // 恢复已有显式主题；新用户默认使用浅色新拟态。
        // SetTheme 经 egui-winit → winit → DWM 沉浸式暗色模式同步系统标题栏
        let theme = match cc.storage.and_then(|s| s.get_string("iv-theme")).as_deref() {
            Some("dark") => ThemeMode::Dark,
            _ => ThemeMode::Light,
        };
        ThemeMode::apply_to(&cc.egui_ctx, theme);
        let renderer = cc
            .wgpu_render_state
            .as_ref()
            .map(crate::render::Renderer::new);
        crate::perf::mark("renderer_ready", None, 0.0);
        let (backdrop_capture_tx, backdrop_capture_rx) = mpsc::channel();
        loader.set_waker(cc.egui_ctx.clone());
        let scanner = crate::directory::Scanner::spawn(cc.egui_ctx.clone());
        let mut app = Self {
            loader,
            file_watcher: crate::file_watch::FileWatcher::spawn(cc.egui_ctx.clone()),
            opened_path: None,
            auto_refresh: cc.storage.and_then(|s| s.get_string("iv-auto-refresh")).as_deref() != Some("off"),
            lock_view: cc.storage.and_then(|s| s.get_string("iv-lock-view")).as_deref() == Some("on"),
            load_intent: LoadIntent::Open,
            scanner,
            directory_generation: 0,
            directory_scope: None,
            // 避免新进程在用户不知情时递归扫描很大的目录。
            include_subfolders: false,
            directory_scanning: false,
            directory_stats: crate::directory::ScanStats::default(),
            directory_recover_index: None,
            cache: ImageCache::new(),
            current: None,
            pending: None,
            perf_paint_pending: false,
            directory: None,
            view: ViewTransform {
                offset: Vec2::ZERO,
                scale: 1.0,
            },
            auto_fit: true,
            channel: ChannelMode::Rgb,
            nearest: false,
            mip_index: 0,
            exposure: 1.0,
            renderer,
            error_msg: None,
            delete_prompt: None,
            delete_job: None,
            probe_text: String::new(),
            probe_detail: String::new(),
            probe_color: None,
            show_props: false,
            show_probe: false,
            show_settings: false,
            show_image_bounds: cc.storage.and_then(|s| s.get_string("iv-image-bounds")).as_deref() == Some("on"),
            background_color: ui::parse_background_color(cc.storage.and_then(|s| s.get_string("iv-background-color")).as_deref()),
            background_menu_pos: None,
            background_button_rect: egui::Rect::NOTHING,
            background_custom_open: false,
            background_palette: crate::color_palette::ColorPalette::default(),
            always_on_top: cc.storage.and_then(|s| s.get_string("iv-always-on-top")).as_deref() == Some("on"),
            applied_topmost: None,
            dialog_escape_until: None,
            checkerboard: cc
                .storage
                .and_then(|s| s.get_string("iv-checkerboard"))
                .as_deref()
                != Some("off"),
            // 新安装使用安静的新拟态画布；已有用户显式开启的桌面磨砂继续保留。
            backdrop: cc
                .storage
                .and_then(|s| s.get_string("iv-backdrop"))
                .as_deref()
                == Some("on"),
            backdrop_opacity: load_f32(cc.storage, "iv-bd-opacity", 0.42),
            backdrop_blur: load_f32(cc.storage, "iv-bd-blur", 0.5),
            backdrop_brightness: load_f32(cc.storage, "iv-bd-bright", 1.0),
            hwnd: None,
            backdrop_tex: None,
            backdrop_capture_error: None,
            backdrop_restore_error: None,
            backdrop_capture_tx,
            backdrop_capture_rx,
            backdrop_job_running: false,
            backdrop_capture_requested: false,
            capture_excluded: false,
            exclude_since: None,
            // 拨回过去：首帧立即可触发第一次背景捕获
            last_capture: Instant::now() - Duration::from_secs(10),
            exclude_release_at: None,
            exclude_unsupported: false,
            last_outer_rect: None,
            glass_overflow_reported: false,
            assoc_registered: false,
            last_canvas_size: Vec2::ZERO,
            zoom_flash: None,
            pending_fit: false,
            pending_actual: false,
            playing: false,
            frame_index: 0,
            next_frame_at: Instant::now(),
            last_title: String::new(),
            theme,
            top_alpha: 1.0,
            bot_alpha: 1.0,
            last_move: Instant::now(),
            reduce_motion: cc
                .storage
                .and_then(|s| s.get_string("iv-reduce-motion"))
                .as_deref()
                == Some("on"),
            ctx_menu_pos: None,
            right_window_drag: None,
            settings_popup_pos: None,
            right_gesture_dragged: false,
            glass_regions: Vec::new(),
        };
        if let Some(p) = initial_path {
            app.open(p);
        }
        app
    }

    /// Explicit opens refresh the directory in the background; navigation reuses its snapshot.
    fn open(&mut self, path: PathBuf) { self.open_impl(path, true); }

    fn open_impl(&mut self, path: PathBuf, rescan: bool) {
        let path = std::path::absolute(&path).unwrap_or(path);
        self.load_intent = if self.lock_view
            && (self.current.is_some() || self.load_intent != LoadIntent::Open) {
            LoadIntent::PreserveView
        } else { LoadIntent::Open };
        self.opened_path = Some(path.clone());
        self.file_watcher.set_path(self.auto_refresh.then(|| path.clone()));
        crate::perf::mark("load_request", Some(&path), 0.0);
        self.error_msg = None;
        self.background_menu_pos = None;
        if self.load_intent == LoadIntent::Open {
            self.mip_index = 0;
            self.auto_fit = true;
        }
        let cached = self.cache.get(&path);
        if cached.is_none() {
            self.pending = Some(path.clone());
            self.current = None;
            // Enqueue before directory work; submitting cannot block behind the decoder.
            self.loader.load(path.clone());
        } else {
            self.loader.cancel_queued();
        }
        if rescan {
            self.directory_scope = crate::directory::Scope::for_image(&path, self.include_subfolders);
            self.directory = Some(Directory { files: vec![path.clone()], index: 0 });
            self.directory_recover_index = None;
            self.request_directory_scan(Some(path.clone()));
        } else if let Some(index) = self.directory.as_ref().and_then(|d| d.files.iter().position(|p| p == &path)) {
            self.directory.as_mut().unwrap().index = index;
        }
        if let Some((img, source)) = cached { self.set_current(path, img, source); }
    }

    fn request_directory_scan(&mut self, anchor: Option<PathBuf>) {
        self.directory_generation = self.directory_generation.wrapping_add(1);
        self.directory_stats = crate::directory::ScanStats::default();
        if let Some(scope) = self.directory_scope.clone() {
            self.directory_scanning = true;
            self.scanner.request(self.directory_generation, scope, anchor);
        } else {
            self.directory_scanning = false;
            self.scanner.cancel(self.directory_generation);
        }
    }

    fn toggle_subfolders(&mut self) {
        if self.delete_active() { return; }
        let path = self.pending.clone().or_else(|| self.current.as_ref().map(|c| c.path.clone()));
        let Some(path) = path else { return; };
        self.include_subfolders = !self.include_subfolders;
        self.directory_scope = crate::directory::Scope::for_image(&path, self.include_subfolders);
        self.directory = Some(Directory { files: vec![path.clone()], index: 0 });
        self.directory_recover_index = None;
        // 删除过期预读，不取消用户真正等待的图片。
        if self.pending.is_some() { self.loader.load(path.clone()); }
        else { self.loader.cancel_queued(); }
        self.request_directory_scan(Some(path));
        self.last_move = Instant::now();
    }

    fn browse_description(&self) -> String {
        let Some(scope) = &self.directory_scope else { return "先打开一张图片，再选择浏览范围。".into(); };
        let mode = if scope.recursive { "当前目录及所有子文件夹（只向下）" } else { "仅当前图片所在目录" };
        let current = self.pending.as_ref().or_else(|| self.current.as_ref().map(|c| &c.path));
        let relative = current.and_then(|p| p.strip_prefix(&scope.root).ok())
            .map(|p| p.display().to_string()).unwrap_or_default();
        format!("范围：{mode}\n根目录：{}\n当前：{relative}\n{}{}", scope.root.display(),
            if self.directory_scanning { "正在后台扫描；切换开关或打开其他图片可取消。\n" } else { "" },
            self.directory_stats.description())
    }

    fn poll_directory(&mut self) {
        while let Some(result) = self.scanner.poll() {
            if result.id != self.directory_generation { continue; }
            self.directory_stats = result.stats;
            let Some(mut files) = result.files else { continue; };
            self.directory_scanning = false;
            let path = self.pending.clone().or_else(|| self.current.as_ref().map(|c| c.path.clone()));
            let recovery = self.directory_recover_index.take();
            // 扫描期间切换或外部修改文件，都不应使当前图片突然跳到列表首项。
            if let Some(path) = &path {
                if !files.contains(path) && self.directory_scope.as_ref().map_or(false, |scope| scope.contains(path)) {
                    files.push(path.clone());
                }
            }
            let index = path.as_ref().and_then(|p| files.iter().position(|f| f == p))
                .unwrap_or_else(|| recovery.unwrap_or(0).min(files.len().saturating_sub(1)));
            self.directory = Some(Directory { files, index });
            if path.is_none() && recovery.is_some() {
                let next = self.directory.as_ref().and_then(|d| d.files.get(d.index).cloned());
                if let Some(next) = next { self.open_impl(next, false); }
            }
            if self.pending.is_none() { self.schedule_prefetch(); }
        }
    }

    fn set_current(&mut self, path: PathBuf, img: Arc<DecodedImage>, source: crate::file_watch::FileState) {
        let reloading = self.load_intent == LoadIntent::Reload;
        let preserve_view = self.load_intent != LoadIntent::Open;
        self.mip_index = self.mip_index.min(img.mips.len().saturating_sub(1));
        if img.is_hdr && !preserve_view {
            self.exposure = 1.0;
        }
        let preserve_animation = reloading && self.current.as_ref().is_some_and(|c| c.img.frames.len() > 1);
        self.frame_index = if preserve_animation {
            self.frame_index.min(img.frames.len().saturating_sub(1))
        } else { 0 };
        self.playing = img.frames.len() > 1 && (!preserve_animation || self.playing);
        self.next_frame_at = Instant::now()
            + Duration::from_millis(img.frames.get(self.frame_index).map(|f| f.delay_ms as u64).unwrap_or(100));
        self.cache.put(path.clone(), img.clone(), &path, source);
        self.current = Some(CurrentImage { path, img });
        self.pending = None;
        self.error_msg = None;
        let upload_started = Instant::now();
        self.upload_current_mip();
        if reloading && self.frame_index > 0 { self.upload_current_frame(); }
        crate::perf::mark("image_ready", self.current.as_ref().map(|c| c.path.as_path()), upload_started.elapsed().as_secs_f64()*1000.0);
        self.perf_paint_pending = true;
        if !preserve_view { self.auto_fit = true; }
        // 主动安排一次适配：auto_fit 只在窗口尺寸变化时触发 fit，
        // 切图时窗口尺寸通常没变，必须走 pending_fit 才能立即居中适配
        self.pending_fit = !preserve_view;
        self.pending_actual = false;
        // 切图时短暂亮出悬浮层，提示目录位置与文件信息
        self.last_move = Instant::now();
        if !reloading { self.schedule_prefetch(); }
        self.load_intent = LoadIntent::Open;
    }

    fn reload_current(&mut self) {
        if self.delete_active() { return; }
        let Some(path) = self.opened_path.clone() else { return; };
        self.cache.remove(&path);
        self.load_intent = if self.current.as_ref().is_some_and(|c| c.path == path) {
            LoadIntent::Reload
        } else if self.lock_view && self.load_intent == LoadIntent::PreserveView {
            LoadIntent::PreserveView
        } else { LoadIntent::Open };
        self.pending = Some(path.clone());
        self.error_msg = None;
        self.loader.reload(path);
    }

    fn poll_file_changes(&mut self) {
        if self.delete_active() { return; }
        let Some(change) = self.file_watcher.poll() else { return; };
        if !self.auto_refresh || self.opened_path.as_ref() != Some(&change.path) { return; }
        match change.state {
            crate::file_watch::FileState::Present { .. } => self.reload_current(),
            crate::file_watch::FileState::Unavailable(error) => {
                self.loader.cancel_queued();
                self.cache.remove(&change.path);
                self.pending = None;
                self.error_msg = Some(format!("文件暂不可读，等待后续保存：{error}"));
            }
        }
    }

    /// 把当前 mip 像素上传 GPU。
    fn upload_current_mip(&mut self) {
        if let (Some(cur), Some(r)) = (&self.current, &mut self.renderer) {
            if let Some(mip) = cur.img.mips.get(self.mip_index) {
                r.upload_texture(mip.width, mip.height, &mip.data);
            }
        }
    }

    /// 动画：上传当前帧（走纹理复用热路径）。
    fn upload_current_frame(&mut self) {
        if let (Some(cur), Some(r)) = (&self.current, &mut self.renderer) {
            if let Some(f) = cur.img.frames.get(self.frame_index) {
                r.upload_texture(cur.img.width, cur.img.height, &f.data);
            }
        }
    }

    /// 当前帧应显示的时长。
    fn current_frame_delay(&self) -> Duration {
        self.current
            .as_ref()
            .and_then(|c| c.img.frames.get(self.frame_index))
            .map(|f| Duration::from_millis(f.delay_ms as u64))
            .unwrap_or(Duration::from_millis(100))
    }

    /// 动画播放/暂停切换。
    fn toggle_play(&mut self) {
        let anim = self
            .current
            .as_ref()
            .map(|c| c.img.frames.len())
            .unwrap_or(0);
        if anim < 2 {
            return;
        }
        self.playing = !self.playing;
        if self.playing {
            self.next_frame_at = Instant::now() + self.current_frame_delay();
        }
    }

    /// 动画逐帧步进（自动暂停）。
    fn step_frame(&mut self, n: isize) {
        let anim = self
            .current
            .as_ref()
            .map(|c| c.img.frames.len())
            .unwrap_or(0);
        if anim < 2 {
            return;
        }
        self.frame_index = (self.frame_index as isize + n).rem_euclid(anim as isize) as usize;
        self.playing = false;
        self.upload_current_frame();
    }

    /// Prefer immediate next/previous images, without copying the whole directory.
    fn schedule_prefetch(&mut self) {
        let Some(d) = &self.directory else { return; };
        let candidates: Vec<_> = [1isize, -1, 2, -2].into_iter()
            .filter_map(|offset| {
                let i = d.index as isize + offset;
                if i < 0 { None } else { d.files.get(i as usize).cloned() }
            }).collect();
        let paths = candidates.into_iter().filter(|p| self.cache.get(p).is_none()).collect::<Vec<_>>();
        self.loader.prefetch(paths);
    }

    /// 相对当前文件移动 n 步。
    fn step(&mut self, n: isize) {
        let target = self.directory.as_ref().and_then(|d| {
            let ni = d.index as isize + n;
            if ni >= 0 && (ni as usize) < d.files.len() {
                Some(d.files[ni as usize].clone())
            } else {
                None
            }
        });
        if let Some(p) = target {
            self.open_impl(p, false);
        }
    }

    fn jump_to(&mut self, index: usize) {
        if let Some(p) = self
            .directory
            .as_ref()
            .and_then(|d| d.files.get(index).cloned())
        {
            self.open_impl(p, false);
        }
    }

    /// 当前显示的 mip。
    fn current_mip(&self) -> Option<&MipLevel> {
        self.current.as_ref()?.img.mips.get(self.mip_index)
    }

    /// 适配窗口（contain fit）。
    fn fit(&mut self, canvas: Vec2) {
        if let Some(mip) = self.current_mip() {
            let iw = mip.width as f32;
            let ih = mip.height as f32;
            let s = (canvas.x / iw).min(canvas.y / ih).min(64.0);
            self.view.scale = s;
            self.view.offset = (canvas - Vec2::new(iw, ih) * s) / 2.0;
            self.auto_fit = true;
        }
    }

    /// 实际大小（100%）。
    fn actual_size(&mut self, canvas: Vec2) {
        if let Some(mip) = self.current_mip() {
            let iw = mip.width as f32;
            let ih = mip.height as f32;
            self.view.scale = 1.0;
            self.view.offset = (canvas - Vec2::new(iw, ih)) / 2.0;
            self.auto_fit = false;
        }
    }

    /// 以点 anchor（画布逻辑坐标）为中心缩放。
    fn zoom_at(&mut self, anchor: Pos2, factor: f32) {
        let old = self.view.scale;
        let new = (old * factor).clamp(0.005, 512.0);
        if (new - old).abs() < f32::EPSILON {
            return;
        }
        let img_pt = (anchor.to_vec2() - self.view.offset) / old;
        self.view.offset = anchor.to_vec2() - img_pt * new;
        self.view.scale = new;
        self.auto_fit = false;
        self.zoom_flash = Some((0.8, new * 100.0));
    }

    /// 画布逻辑坐标 → 图像像素坐标。
    fn pos_to_image(&self, canvas_pos: Pos2) -> Option<(u32, u32)> {
        let mip = self.current_mip()?;
        let p = (canvas_pos.to_vec2() - self.view.offset) / self.view.scale;
        if p.x < 0.0 || p.y < 0.0 || p.x >= mip.width as f32 || p.y >= mip.height as f32 {
            return None;
        }
        Some((p.x as u32, p.y as u32))
    }

    fn handle_loader_messages(&mut self) {
        while let Some(msg) = self.loader.poll() {
            match msg {
                Msg::Outdated(path) => {
                    self.cache.remove(&path);
                    if self.pending.as_ref() == Some(&path) { self.reload_current(); }
                }
                Msg::Ready(Ok((path, img, source))) => {
                    if !path.is_file() {
                        self.cache.remove(&path);
                        if self.pending.as_ref() == Some(&path) {
                            self.pending = None;
                            self.error_msg = Some("图片在加载期间已被移动或删除".into());
                        }
                        continue;
                    }
                    if self.pending.as_ref() == Some(&path) {
                        if let Some(d) = &mut self.directory {
                            if let Some(i) = d.files.iter().position(|p| *p == path) {
                                d.index = i;
                            }
                        }
                        self.set_current(path, img, source);
                    } else {
                        if !self.directory_scope.as_ref().map_or(false, |scope| scope.contains(&path)) { continue; }
                        // 预读结果：只进缓存
                        let keep = self
                            .current
                            .as_ref()
                            .map(|c| c.path.clone())
                            .unwrap_or_default();
                        self.cache.put(path, img, &keep, source);
                    }
                }
                Msg::Ready(Err((path, err))) => {
                    if self.pending.as_ref() == Some(&path) {
                        self.pending = None;
                        if self.load_intent == LoadIntent::Reload && self.current.is_some() {
                            self.error_msg = Some(format!("刷新失败，保留上一版本；保存后重试或按 F5：{err}"));
                        } else {
                            self.current = None;
                            self.error_msg = Some(err);
                        }
                    }
                }
            }
        }
    }

    fn handle_global_input(&mut self, ctx: &egui::Context, canvas: Vec2) {
        // A synchronous native picker can leave its dismissing Escape in winit's
        // pending input queue. Consume only that key until it is released/settled.
        if let Some(until) = self.dialog_escape_until {
            if Instant::now() >= until && !ctx.input(|i| i.key_down(Key::Escape)) {
                self.dialog_escape_until = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(16));
                if ctx.input(|i| i.key_pressed(Key::Escape)) { return; }
            }
        }
        if self.delete_active() {
            if self.delete_job.is_none() && ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.delete_prompt = None;
            }
            return; // dialog owns all keys, including held Delete and navigation
        }
        // 背景菜单优先关闭，不让 Esc 穿透为退出。
        if self.background_menu_pos.is_some() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.background_menu_pos = None;
            self.last_move = Instant::now();
            return;
        }
        // Esc 逐层关闭：右键菜单 → 下拉弹窗 → 属性/设置窗口 → 退出程序
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.right_window_drag.take().is_some() {
                self.right_gesture_dragged = true;
                return;
            }
            if self.ctx_menu_pos.is_some() {
                self.ctx_menu_pos = None;
                return;
            }
            if ctx.memory(|m| m.any_popup_open()) {
                ctx.memory_mut(|m| m.close_popup());
                return;
            }
            if self.show_props {
                self.show_props = false;
                return;
            }
            if self.show_probe {
                self.show_probe = false;
                return;
            }
            if self.show_settings {
                self.show_settings = false;
                return;
            }
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        // 拖拽文件进窗口
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .find(|p| p.is_file())
        });
        if let Some(p) = dropped {
            self.open(p);
            return;
        }

        // Ctrl+O 打开文件
        if ctx.input(|i| (i.modifiers.command || i.modifiers.ctrl) && !i.modifiers.shift && i.key_pressed(Key::O)) {
            self.open_dialog();
            return;
        }

        let blocked = self.show_settings || self.show_props || self.show_probe
            || self.ctx_menu_pos.is_some() || self.background_menu_pos.is_some() || ctx.memory(|m| m.any_popup_open());
        if !viewer_shortcuts_allowed(ctx.wants_keyboard_input(), blocked) {
            return;
        }
        if ctx.input(|i| fresh_delete_press(&i.events) || (cfg!(target_os = "macos")
            && i.modifiers == egui::Modifiers::NONE && i.key_pressed(Key::Backspace))) {
            self.request_delete();
            return;
        }
        let key = |k: Key| ctx.input(|i| i.modifiers == egui::Modifiers::NONE && i.key_pressed(k));
        if key(Key::F5) {
            self.reload_current();
        }
        if key(Key::L) {
            self.lock_view = !self.lock_view;
            self.last_move = Instant::now();
        }
        if key(Key::ArrowLeft) || key(Key::PageUp) || key(Key::A) {
            self.step(-1);
        }
        if key(Key::ArrowRight) || key(Key::PageDown) || key(Key::D) {
            self.step(1);
        }
        if key(Key::Home) {
            self.jump_to(0);
        }
        if key(Key::End) {
            if let Some(d) = &self.directory {
                self.jump_to(d.files.len().saturating_sub(1));
            }
        }
        if key(Key::F) {
            self.fit(canvas);
        }
        if key(Key::Num0) {
            self.actual_size(canvas);
        }
        if key(Key::C) {
            self.channel = ChannelMode::Rgb;
        }
        // 通道切换：1/2/3/4 = R/G/B/A，5 = 完整 RGBA（数字键单手可达）
        if key(Key::Num1) {
            self.channel = ChannelMode::R;
        }
        if key(Key::Num2) {
            self.channel = ChannelMode::G;
        }
        if key(Key::Num3) {
            self.channel = ChannelMode::B;
        }
        if key(Key::Num4) {
            self.channel = ChannelMode::A;
        }
        if key(Key::Num5) {
            self.channel = ChannelMode::Rgb;
        }
        if key(Key::O) {
            self.channel = ChannelMode::RgbOpaque;
        }
        if key(Key::B) {
            self.show_image_bounds = !self.show_image_bounds;
        }
        if key(Key::S) {
            self.toggle_subfolders();
        }
        if key(Key::N) {
            self.nearest = !self.nearest;
        }
        // T 切换深/浅主题
        if key(Key::T) {
            self.toggle_theme(ctx);
        }
        // 动画：空格播放/暂停，, . 逐帧
        if key(Key::Space) {
            self.toggle_play();
        }
        if key(Key::Comma) {
            self.step_frame(-1);
        }
        if key(Key::Period) {
            self.step_frame(1);
        }
        // ↑↓ 切换 DDS mip
        let mip_count = self.current.as_ref().map(|c| c.img.mips.len()).unwrap_or(1);
        if key(Key::ArrowUp) && self.mip_index + 1 < mip_count {
            self.mip_index += 1;
            self.upload_current_mip();
            self.auto_fit = true;
        }
        if key(Key::ArrowDown) && self.mip_index > 0 {
            self.mip_index -= 1;
            self.upload_current_mip();
            self.auto_fit = true;
        }
    }

    fn delete_active(&self) -> bool {
        self.delete_prompt.is_some() || self.delete_job.is_some()
    }

    fn request_delete(&mut self) {
        if self.delete_active() || self.pending.is_some() { return; }
        let Some(current) = &self.current else { return; };
        match crate::recycle::Target::new(&current.path) {
            Ok(target) => {
                self.delete_prompt = Some(target);
                self.ctx_menu_pos = None;
                self.right_window_drag = None;
            }
            Err(err) => { self.error_msg = Some(err); self.ctx_menu_pos = None; }
        }
    }

    fn draw_delete_dialog(&mut self, ctx: &egui::Context, pal: &Palette) {
        if !self.delete_active() { return; }
        let screen = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("iv-delete-shield")))
            .rect_filled(screen, 0.0, Color32::from_black_alpha(80));
        let mut cancel = false;
        let mut confirm = false;
        let busy = self.delete_job.is_some();
        let name = self.delete_prompt.as_ref().map(|t| t.path.clone())
            .or_else(|| self.delete_job.as_ref().map(|j| j.0.clone())).unwrap_or_default();
        egui::Area::new(egui::Id::new("iv-delete-confirm"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).movable(false)
            .show(ctx, |ui| { ui::menu_frame(pal).show(ui, |ui| {
                ui.set_width(400.0);
                ui.heading("删除图片");
                ui.add_space(8.0);
                ui.label(if busy { "正在移入回收站…" } else { "将当前图片移入回收站？" });
                ui.add_space(8.0);
                ui::filename_label(ui, name.file_name().unwrap_or_default().to_string_lossy().as_ref(),
                    370.0, pal.text_bright).on_hover_text(name.display().to_string());
                ui.label(RichText::new("可从回收站恢复；无法回收时取消，不永久删除。").small().color(pal.dim));
                ui.add_space(16.0);
                if busy {
                    ui.spinner();
                } else {
                    ui.horizontal(|ui| {
                        cancel = ui.add_sized([175.0, 34.0], egui::Button::new("取消 (Esc)")).clicked();
                        ui.add_space(10.0);
                        confirm = ui.add_sized([175.0, 34.0], egui::Button::new("移入回收站")).clicked();
                    });
                }
                ui.add_space(8.0);
            }); });
        if cancel { self.delete_prompt = None; }
        if confirm {
            if let Some(target) = self.delete_prompt.take() {
                let path = target.path.clone();
                let (tx, rx) = mpsc::channel();
                let wake = ctx.clone();
                match std::thread::Builder::new().name("iv-recycle".into()).spawn(move || {
                    let _ = tx.send(crate::recycle::recycle(&target));
                    wake.request_repaint();
                }) {
                    Ok(_) => self.delete_job = Some((path, rx)),
                    Err(err) => self.error_msg = Some(format!("无法启动回收操作：{err}")),
                }
            }
        }
    }

    fn poll_delete(&mut self, ctx: &egui::Context) {
        let Some((_, receiver)) = &self.delete_job else { return; };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(50));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => Err("回收任务异常结束，请检查文件状态".into()),
        };
        let (path, _) = self.delete_job.take().unwrap();
        match result {
            Err(err) => self.error_msg = Some(err),
            Ok(()) => {
                // A stale decoder/prefetch result must not re-add the deleted image.
                let old_index = self.directory.as_ref().map_or(0, |d| d.index);
                self.cache.remove(&path);
                self.current = None;
                self.pending = None;
                self.opened_path = None;
                self.file_watcher.set_path(None);
                self.loader.cancel_queued();
                self.playing = false;
                self.probe_color = None;
                self.probe_text.clear();
                self.probe_detail.clear();
                self.show_props = false;
                self.show_probe = false;
                self.error_msg = None;
                // 从已有索引移除目标；不在 UI 上递归重扫，也不把范围缩成子目录。
                if let Some(d) = &mut self.directory {
                    d.files.retain(|p| p != &path);
                    d.index = old_index.min(d.files.len().saturating_sub(1));
                }
                let next = self.directory.as_ref().and_then(|d| {
                    next_index_after_delete(old_index, d.files.len()).and_then(|i| d.files.get(i).cloned())
                });
                if let Some(next) = next {
                    self.directory_recover_index = None;
                    self.open_impl(next, false);
                }
                else {
                    self.directory_recover_index = Some(old_index);
                    self.pending_fit = false;
                    self.pending_actual = false;
                    self.zoom_flash = None;
                    self.view = ViewTransform { offset: Vec2::ZERO, scale: 1.0 };
                }
                let anchor = self.pending.clone().or_else(|| self.current.as_ref().map(|c| c.path.clone()));
                self.request_directory_scan(anchor);
            }
        }
        self.last_move = Instant::now();
    }

    /// 弹出打开文件对话框。
    fn open_dialog(&mut self) {
        // Make the modal dialog owned by this actual viewer: Windows keeps it above
        // a pinned parent and returns activation to the viewer when it closes.
        self.background_menu_pos = None;
        let Some(hwnd) = self.hwnd.or_else(crate::backdrop::find_own_window) else {
            self.error_msg = Some("窗口尚未准备好，请稍后再打开图片".into());
            return;
        };
        let owner = DialogOwner(hwnd);
        let mut dialog = rfd::FileDialog::new().set_parent(&owner);
        if let Some(folder) = self.current.as_ref().and_then(|c| c.path.parent()) {
            dialog = dialog.set_directory(folder);
        }
        let selected = dialog
            .add_filter(
                "所有支持的图像",
                &[
                    "png", "jpg", "jpeg", "bmp", "gif", "webp", "avif", "ico", "tif", "tiff", "hdr", "dds",
                    "psd", "qoi", "tga", "ppm", "pgm", "pbm",
                ],
            )
            .pick_file();
        self.dialog_escape_until = Some(Instant::now() + Duration::from_millis(250));
        if let Some(p) = selected { self.open(p); }
    }

    /// 窗口标题跟随当前文件。
    fn update_window_title(&mut self, ctx: &egui::Context) {
        let title = match &self.current {
            Some(c) => {
                let name = c.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                format!("{name} — LcL ImageViewer")
            }
            None => "LcL ImageViewer".to_string(),
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    fn apply_topmost(&mut self, ctx: &egui::Context) {
        if self.applied_topmost != Some(self.always_on_top) {
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(ui::window_level(self.always_on_top)));
            self.applied_topmost = Some(self.always_on_top);
        }
    }

    fn use_desktop_background(&self) -> bool {
        cfg!(windows) && self.backdrop && self.background_color.is_none()
    }

    fn draw_background_menu(&mut self, ctx: &egui::Context, pal: &Palette) {
        let Some(pos) = self.background_menu_pos else { return };
        if self.show_settings || self.delete_active() {
            self.background_menu_pos = None;
            return;
        }
        let screen = ctx.screen_rect().shrink(8.0);
        let pos = Pos2::new(pos.x.clamp(screen.left(), (screen.right() - 256.0).max(screen.left())), pos.y);
        let mut close = false;
        let shown = egui::Area::new(egui::Id::new("iv-background-menu"))
            .order(egui::Order::Foreground).fixed_pos(pos).movable(false).constrain_to(screen)
            .show(ctx, |ui| {
                ui::menu_frame(pal).show(ui, |ui| {
                    ui.set_width(228.0);
                    ui.spacing_mut().item_spacing.y = 4.0;
                    if !self.background_custom_open {
                        ui.label(RichText::new("画布背景").strong().color(pal.text));
                        let choices = [
                            ("棋盘格", true, None),
                            ("跟随主题", false, None),
                            ("白色", false, Some([255, 255, 255])),
                            ("灰色", false, Some([128, 128, 128])),
                            ("黑色", false, Some([0, 0, 0])),
                        ];
                        for (label, checker, color) in choices {
                            let selected = self.checkerboard == checker && self.background_color == color;
                            let response = ui.add_sized([228.0, 30.0], egui::SelectableLabel::new(selected, label));
                            if let Some(rgb) = color {
                                let rect = egui::Rect::from_center_size(response.rect.right_center() - Vec2::new(18.0, 0.0), Vec2::splat(16.0));
                                ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                                ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0_f32, pal.dim));
                            }
                            if response.clicked() {
                                self.checkerboard = checker;
                                self.background_color = color;
                                close = true;
                            }
                        }
                        if ui.add_sized([228.0, 30.0], egui::SelectableLabel::new(false, "自定义颜色…")).clicked() {
                            self.background_palette.set_rgb(self.background_color.unwrap_or([128, 128, 128]));
                            self.background_custom_open = true;
                        }
                    } else {
                        ui.horizontal(|ui| {
                            if ui::icon_btn(ui, Icon::Prev, false, pal).on_hover_text("返回背景选项").clicked() {
                                self.background_custom_open = false;
                            }
                            ui.label(RichText::new("自定义颜色").strong().color(pal.text));
                        });
                        ui.add_space(4.0);
                        if self.background_palette.show(ui, pal) {
                            self.background_color = Some(self.background_palette.rgb());
                            self.checkerboard = false;
                        }
                        ui.add_space(4.0);
                        let valid = self.background_palette.valid_hex();
                        if ui.add_enabled(valid, egui::Button::new("应用并关闭")
                            .fill(pal.btn_pressed).min_size(Vec2::new(228.0, 30.0))).clicked()
                            || (valid && ui.input(|i| i.key_pressed(Key::Enter))) {
                            self.background_color = Some(self.background_palette.rgb());
                            self.checkerboard = false;
                            close = true;
                        }
                    }
                    ui.label(RichText::new("仅改变画布，不修改图片").size(11.5).color(pal.dim));
                });
            });
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p|
            !shown.response.rect.contains(p) && !self.background_button_rect.contains(p)));
        if close || outside { self.background_menu_pos = None; }
        self.last_move = Instant::now();
    }

    /// 切换深/浅主题（立即生效，退出时经 eframe persistence 持久化）。
    fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.theme = ThemeMode::toggle(ctx);
    }

    /// 悬浮层自动显隐：指针移动、点击或按住鼠标操作时显示，
    /// 指针静止 1s 后淡出。控件的持久焦点和静止悬停不能阻止淡出。
    fn update_overlay_visibility(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| {
            i.events
                .iter()
                .any(overlay_pointer_activity)
        }) {
            self.last_move = Instant::now();
        }
        // 菜单展开时保持入口可见，关闭后再开始一秒计时。
        if self.background_menu_pos.is_some() { self.last_move = Instant::now(); }
        let has_image = self.current.is_some();
        let idle = self.last_move.elapsed();
        let recent = idle < OVERLAY_HIDE_DELAY;
        let pointer_down = ctx.input(|i| i.pointer.any_down());
        let (top_show, bot_show) = overlay_targets(has_image, idle, pointer_down);
        let dt = ctx.input(|i| i.unstable_dt).min(0.1);
        fn fade(a: &mut f32, show: bool, dt: f32, reduce_motion: bool) -> bool {
            let t = if show { 1.0 } else { 0.0 };
            if reduce_motion {
                *a = t;
            } else if show {
                *a = (*a + dt / 0.12).min(1.0);
            } else {
                *a = (*a - dt / 0.18).max(0.0);
            }
            *a != t
        }
        let animating = fade(&mut self.top_alpha, top_show, dt, self.reduce_motion)
            | fade(&mut self.bot_alpha, bot_show, dt, self.reduce_motion);
        if animating {
            ctx.request_repaint();
        } else if recent && !pointer_down {
            // 静止计时到点后再评估一次，触发淡出
            let remain = OVERLAY_HIDE_DELAY.saturating_sub(self.last_move.elapsed());
            ctx.request_repaint_after(remain.max(Duration::from_millis(16)));
        }
    }

    /// 只允许从当前响应拥有的右键拖拽启动，按钮/菜单不会穿透到画布。
    fn begin_right_window_drag(&mut self, ctx: &egui::Context, response: &egui::Response) {
        if !response.drag_started_by(PointerButton::Secondary) {
            return;
        }
        self.right_gesture_dragged = true;
        self.ctx_menu_pos = None;
        let fixed_window = ctx.input(|i| {
            i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
        });
        if fixed_window {
            return;
        }
        #[cfg(target_os = "macos")]
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        #[cfg(windows)]
        if let (Some(hwnd), Some(start)) = (self.hwnd, ctx.input(|i| i.pointer.press_origin())) {
            self.right_window_drag = crate::backdrop::WindowDrag::begin(
                hwnd, [start.x, start.y], ctx.pixels_per_point(),
            );
        }
    }

    /// 顶部悬浮工具栏：打开 / 目录导航 / 文件信息 / 通道 / mip / 视图控制 / 动画 / 曝光 / 主题。
    fn draw_top_overlay(&mut self, ctx: &egui::Context, pal: &Palette) {
        if self.show_settings || self.top_alpha <= 0.01 {
            return;
        }
        let alpha = self.top_alpha;
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        let area = egui::Area::new(egui::Id::new("iv-top"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 10.0))
            // 无边框：胶囊本体兼作标题栏——空白处可拖拽移动窗口、双击切换最大化。
            //（内部按钮在更上层优先响应，不会误触发拖动/双击）
            .sense(Sense::click_and_drag())
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                let _frame_resp = ui::capsule(pal).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;

                        // —— 打开 ——
                        if ui::icon_btn(ui, Icon::Open, false, pal)
                            .on_hover_text(if cfg!(target_os = "macos") { "打开文件 (⌘O)" } else { "打开文件 (Ctrl+O)" })
                            .clicked()
                        {
                            self.open_dialog();
                        }
                        if self.current.is_none() && self.pending.is_none() {
                            ui::bar_label(ui, "打开或拖入图片", 12.5, pal.dim);
                        }

                        // 文件夹树开关与原来的侧边箭头/快捷键共用同一导航索引。
                        if self.current.is_some() || self.pending.is_some() {
                            if ui::icon_btn(ui, Icon::Subfolders, self.include_subfolders, pal)
                                .on_hover_text(format!("包含子文件夹 (S)\n{}", self.browse_description())).clicked() {
                                self.toggle_subfolders();
                            }
                        }
                        if let Some(d) = self.directory.as_ref() {
                            ui::sep(ui, pal);
                            let text = if self.directory_scanning {
                                format!("扫描…{}", self.directory_stats.files)
                            } else {
                                format!("{}/{}{}", if d.files.is_empty() { 0 } else { d.index + 1 }, d.files.len(),
                                    if self.directory_stats.partial() { "·部分" } else { "" })
                            };
                            // 固定槽宽：计数、扫描状态和“部分”提示不让左侧开关来回移动。
                            let galley = ui.fonts(|f| f.layout_no_wrap(text,
                                egui::FontId::new(11.0, egui::FontFamily::Monospace), pal.dim));
                            let (slot, response) = ui.allocate_exact_size(Vec2::new(88.0, 32.0), Sense::hover());
                            ui.painter().galley(slot.center() - galley.size() / 2.0, galley, pal.dim);
                            response.on_hover_text(self.browse_description());
                        }

                        // —— 文件信息 ——
                        if let Some(cur) = &self.current {
                            ui::sep(ui, pal);
                            let img = &cur.img;
                            let label_path = self.directory_scope.as_ref()
                                .map(|scope| scope.display_path(&cur.path))
                                .unwrap_or_else(|| cur.path.file_name().map(Path::new).unwrap_or(&cur.path));
                            let raw = label_path.to_string_lossy();
                            let file_size = std::fs::metadata(&cur.path)
                                .map(|m| fmt_size(m.len()))
                                .unwrap_or_default();
                            let window_width = ctx.screen_rect().width();
                            let name_width = ui::topbar_name_width(window_width);
                            ui::filename_label(ui, raw.as_ref(), name_width, pal.text_bright)
                                .on_hover_ui(|ui| {
                                    ui.label(raw.as_ref());
                                    ui.weak(cur.path.to_string_lossy().as_ref());
                                });
                            ui::badge(ui, img.kind.label(), pal)
                                .on_hover_text(format!("文件大小 {file_size}"));
                            if window_width >= 1200.0 {
                                if let Some(c) = &img.compression {
                                    ui::badge(ui, c, pal);
                                }
                            }
                        } else if self.pending.is_some() {
                            ui::sep(ui, pal);
                            ui::bar_label(ui, "加载中…", 12.5, pal.dim);
                        }

                        if self.current.is_some() {
                            // —— 通道 ——
                            ui::sep(ui, pal);
                            let compact = ctx.screen_rect().width() < 1120.0;
                            let channels = [
                                (
                                    ChannelMode::Rgb,
                                    "RGB",
                                    "完整 RGBA（5 或 C），O 键忽略 Alpha",
                                ),
                                (ChannelMode::R, "R", "红通道 (1)"),
                                (ChannelMode::G, "G", "绿通道 (2)"),
                                (ChannelMode::B, "B", "蓝通道 (3)"),
                                (ChannelMode::A, "A", "Alpha 通道 (4)"),
                            ];
                            if compact {
                                ui.scope(|ui| {
                                    ui.spacing_mut().interact_size.y = 32.0;
                                    ComboBox::from_id_source("iv-channel-compact")
                                        .selected_text(self.channel.label())
                                        .width(52.0)
                                        .show_ui(ui, |ui| {
                                            for (target, label, tip) in channels {
                                                ui.selectable_value(
                                                    &mut self.channel,
                                                    target,
                                                    label,
                                                )
                                                .on_hover_text(tip);
                                            }
                                        });
                                });
                            } else {
                                ui::bar_label(ui, "通道", 12.0, pal.dim);
                                for (target, label, tip) in channels {
                                    let selected = self.channel == target;
                                    if ui::segment_button(ui, label, selected, pal)
                                        .on_hover_text(tip)
                                        .clicked()
                                    {
                                        self.channel = target;
                                    }
                                }
                            }

                            // —— mip 选择（多 mip 时显示）——
                            let mip_sizes: Vec<(u32, u32)> = self
                                .current
                                .as_ref()
                                .map(|c| c.img.mips.iter().map(|m| (m.width, m.height)).collect())
                                .unwrap_or_default();
                            if mip_sizes.len() > 1 && ctx.screen_rect().width() >= 1200.0 {
                                let before = self.mip_index;
                                ComboBox::from_id_source("iv-mip")
                                    .selected_text(
                                        RichText::new(format!(
                                            "Mip {}/{}",
                                            self.mip_index,
                                            mip_sizes.len() - 1
                                        ))
                                        .size(12.0),
                                    )
                                    .width(92.0)
                                    .show_ui(ui, |ui| {
                                        for (i, (w, h)) in mip_sizes.iter().enumerate() {
                                            ui.selectable_value(
                                                &mut self.mip_index,
                                                i,
                                                format!("Mip {i} · {w}×{h}"),
                                            );
                                        }
                                    });
                                if self.mip_index != before {
                                    self.upload_current_mip();
                                    self.auto_fit = true;
                                }
                            }

                            // —— 视图控制 ——
                            ui::sep(ui, pal);
                            if ui::icon_btn(ui, Icon::Grid, self.nearest, pal)
                                .on_hover_text("最近邻采样 (N) —— 放大看像素")
                                .clicked()
                            {
                                self.nearest = !self.nearest;
                            }
                            if ui::icon_btn(ui, Icon::Fit, false, pal)
                                .on_hover_text("适配窗口 (F)")
                                .clicked()
                            {
                                self.pending_fit = true;
                            }
                            if ui::icon_btn(ui, Icon::Actual, false, pal)
                                .on_hover_text("实际大小 (0)")
                                .clicked()
                            {
                                self.pending_actual = true;
                            }

                            if ui::icon_btn(ui, Icon::Bounds, self.show_image_bounds, pal)
                                .on_hover_text("显示图片边界 (B) — 包含透明区域的完整矩形")
                                .clicked()
                            {
                                self.show_image_bounds = !self.show_image_bounds;
                            }

                            // —— 动画播放控件（多帧时显示）——
                            let anim_frames = self
                                .current
                                .as_ref()
                                .map(|c| c.img.frames.len())
                                .unwrap_or(0);
                            if anim_frames > 1 && ctx.screen_rect().width() >= 1200.0 {
                                ui::sep(ui, pal);
                                let play_icon = if self.playing {
                                    Icon::Pause
                                } else {
                                    Icon::Play
                                };
                                if ui::icon_btn(ui, play_icon, false, pal)
                                    .on_hover_text("播放/暂停 (Space) · 逗号/句号逐帧")
                                    .clicked()
                                {
                                    self.toggle_play();
                                }
                                let before = self.frame_index;
                                if ui
                                    .add_sized(
                                        [70.0, 18.0],
                                        Slider::new(&mut self.frame_index, 0..=anim_frames - 1)
                                            .show_value(false),
                                    )
                                    .changed()
                                    && self.frame_index != before
                                {
                                    // 拖帧条时暂停，松手后保持静止（视频播放器惯例）
                                    self.playing = false;
                                    self.upload_current_frame();
                                }
                                // 帧计数固定槽位：槽宽按最大文本预留，右对齐绘制，
                                // 位数变化不再引起胶囊宽度抖动
                                let mono = egui::FontId::new(11.0, egui::FontFamily::Monospace);
                                let slot_w = ui
                                    .fonts(|f| {
                                        f.layout_no_wrap(
                                            format!("{anim_frames}/{anim_frames}"),
                                            mono.clone(),
                                            pal.dim,
                                        )
                                    })
                                    .size()
                                    .x;
                                let (slot, _) = ui.allocate_exact_size(
                                    Vec2::new(slot_w, 18.0),
                                    egui::Sense::hover(),
                                );
                                let galley = ui.fonts(|f| {
                                    f.layout_no_wrap(
                                        format!("{}/{}", self.frame_index + 1, anim_frames),
                                        mono,
                                        pal.dim,
                                    )
                                });
                                let gp = Pos2::new(
                                    slot.right() - galley.size().x,
                                    slot.center().y - galley.size().y / 2.0,
                                );
                                ui.painter().galley(gp, galley, pal.dim);
                            }

                            // —— HDR 曝光 ——
                            let is_hdr =
                                self.current.as_ref().map(|c| c.img.is_hdr).unwrap_or(false);
                            if is_hdr && ctx.screen_rect().width() >= 1200.0 {
                                ui::sep(ui, pal);
                                ui.add_sized(
                                    [100.0, 18.0],
                                    Slider::new(&mut self.exposure, 0.01..=64.0)
                                        .logarithmic(true)
                                        .text("曝光"),
                                )
                                .on_hover_text("HDR 曝光倍数");
                            }
                        }

                        // —— 背景入口：无图、窄窗口也始终提供 ——
                        ui::sep(ui, pal);
                        let background = ui::icon_btn(ui, Icon::Background, self.background_menu_pos.is_some(), pal)
                            .on_hover_text("背景颜色 · 棋盘格 / 主题 / 纯色");
                        self.background_button_rect = background.rect;
                        if background.clicked() {
                            if self.background_menu_pos.is_some() {
                                self.background_menu_pos = None;
                            } else {
                                self.background_menu_pos = Some(background.rect.left_bottom() + Vec2::new(0.0, 8.0));
                                self.background_custom_open = false;
                                self.ctx_menu_pos = None;
                                ctx.memory_mut(|m| m.close_popup());
                            }
                            self.last_move = Instant::now();
                        }
                        // —— 主题切换 + 设置 ——
                        if ctx.screen_rect().width() >= 1000.0 {
                            let (icon, tip) = match self.theme {
                                ThemeMode::Dark => (Icon::Sun, "切换到浅色主题 (T)"),
                                ThemeMode::Light => (Icon::Moon, "切换到深色主题 (T)"),
                            };
                            if ui::icon_btn(ui, icon, false, pal)
                                .on_hover_text(tip)
                                .clicked()
                            {
                                self.toggle_theme(ctx);
                            }
                        }
                        if ui::icon_btn(ui, Icon::Settings, false, pal)
                            .on_hover_text("设置")
                            .clicked()
                        {
                            self.background_menu_pos = None;
                            self.show_settings = true;
                            self.assoc_registered = winassoc::is_registered();
                        }

                        // —— 窗口控制：置顶 / 最小化 / 最大化·还原 / 关闭 ——
                        ui::sep(ui, pal);
                        if ui::icon_btn(ui, Icon::Pin, self.always_on_top, pal)
                            .on_hover_text(if self.always_on_top { "取消置顶" } else { "窗口置顶 · 保持在其他普通窗口上方" })
                            .clicked()
                        {
                            self.always_on_top = !self.always_on_top;
                            self.apply_topmost(ctx);
                        }
                        if ui::icon_btn(ui, Icon::Min, false, pal)
                            .on_hover_text("最小化")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
                        }
                        let (micon, mtip) = if maximized {
                            (Icon::Restore, "还原")
                        } else {
                            (Icon::Max, "最大化")
                        };
                        if ui::icon_btn(ui, micon, false, pal)
                            .on_hover_text(mtip)
                            .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                        }
                        if ui::icon_btn_danger(ui, Icon::Close, pal)
                            .on_hover_text("关闭")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(ViewportCommand::Close);
                        }
                    });
                });
            });
        // 顶栏胶囊（无边框标题栏）：空白处拖拽移动窗口，双击切换最大化/还原。
        // 按钮在更上层优先捕获指针，故仅空白处会落到胶囊本体的 drag/click。
        let resp = area.response;
        self.begin_right_window_drag(ctx, &resp);
        if resp.drag_started_by(PointerButton::Primary) && !maximized {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if resp.double_clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
        let rect = resp.rect;
        if pal.overlay.a() < 255 {
            self.glass_regions.push((rect, alpha, ui::PANEL_RADIUS));
        }
    }

    /// 窗口两侧的目录导航。与顶栏共用1秒显隐，但不参与标题栏的拖拽。
    fn draw_side_navigation(&mut self, ctx: &egui::Context, pal: &Palette) {
        if self.top_alpha <= 0.01 || self.show_settings || self.ctx_menu_pos.is_some()
            || self.current.is_none()
        {
            return;
        }
        let Some((index, count)) = self.directory.as_ref()
            .filter(|d| d.files.len() > 1).map(|d| (d.index, d.files.len())) else {
            return;
        };
        let enabled = navigation_enabled(index, count);
        let rects = ui::side_navigation_rects(ctx.screen_rect());
        for (side, (icon, step, label)) in [
            (Icon::Prev, -1, "上一张 (A / ←)"),
            (Icon::Next, 1, "下一张 (D / →)"),
        ].into_iter().enumerate() {
            let response = egui::Area::new(egui::Id::new(("iv-side-navigation", side)))
                .order(egui::Order::Foreground)
                .fixed_pos(rects[side].min)
                .movable(false)
                .sense(Sense::hover())
                .show(ctx, |ui| {
                    ui.set_opacity(self.top_alpha);
                    ui::glass_navigation_button(ui, icon, enabled[side], pal)
                        .on_hover_text(if enabled[side] {
                            label
                        } else if side == 0 {
                            "已经是第一张"
                        } else {
                            "已经是最后一张"
                        })
                }).inner;
            // 模糊遮罩和按钮使用同一个实际矩形、圆角及显隐系数。
            self.glass_regions.push((response.rect, self.top_alpha, ui::SIDE_NAV_RADIUS));
            if enabled[side] && response.clicked() {
                self.step(step);
            }
        }
    }

    fn has_context_tools(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|cur| cur.img.mips.len() > 1 || cur.img.frames.len() > 1 || cur.img.is_hdr)
    }

    /// 窄于 1200pt 时将格式专属工具放到第二行，避免挤压主标题栏。
    fn draw_context_tools_overlay(&mut self, ctx: &egui::Context, pal: &Palette) {
        if self.top_alpha <= 0.01
            || ctx.screen_rect().width() >= 1200.0
            || !self.has_context_tools()
        {
            return;
        }
        let alpha = self.top_alpha;
        let mip_sizes: Vec<(u32, u32)> = self
            .current
            .as_ref()
            .map(|c| c.img.mips.iter().map(|m| (m.width, m.height)).collect())
            .unwrap_or_default();
        let anim_frames = self
            .current
            .as_ref()
            .map(|c| c.img.frames.len())
            .unwrap_or_default();
        let is_hdr = self.current.as_ref().is_some_and(|c| c.img.is_hdr);
        let area = egui::Area::new(egui::Id::new("iv-context-tools"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 70.0))
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                let _frame_resp = ui::capsule(pal).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let mut has_group = false;
                        if mip_sizes.len() > 1 {
                            let before = self.mip_index;
                            ui.scope(|ui| {
                                ui.spacing_mut().interact_size.y = 32.0;
                                ComboBox::from_id_source("iv-mip-context")
                                    .selected_text(format!(
                                        "Mip {}/{}",
                                        self.mip_index,
                                        mip_sizes.len() - 1
                                    ))
                                    .width(94.0)
                                    .show_ui(ui, |ui| {
                                        for (index, (width, height)) in mip_sizes.iter().enumerate()
                                        {
                                            ui.selectable_value(
                                                &mut self.mip_index,
                                                index,
                                                format!("Mip {index} · {width}×{height}"),
                                            );
                                        }
                                    });
                            });
                            if self.mip_index != before {
                                self.upload_current_mip();
                                self.auto_fit = true;
                            }
                            has_group = true;
                        }
                        if anim_frames > 1 {
                            if has_group {
                                ui::sep(ui, pal);
                            }
                            let icon = if self.playing {
                                Icon::Pause
                            } else {
                                Icon::Play
                            };
                            if ui::icon_btn(ui, icon, false, pal)
                                .on_hover_text("播放/暂停 (Space) · 逗号/句号逐帧")
                                .clicked()
                            {
                                self.toggle_play();
                            }
                            ui::bar_label_mono(
                                ui,
                                format!("{}/{}", self.frame_index + 1, anim_frames),
                                12.0,
                                pal.dim,
                            );
                            has_group = true;
                        }
                        if is_hdr {
                            if has_group {
                                ui::sep(ui, pal);
                            }
                            ui::bar_label(ui, "曝光", 12.0, pal.dim);
                            ui.add_sized(
                                [120.0, 22.0],
                                Slider::new(&mut self.exposure, 0.01..=64.0)
                                    .logarithmic(true)
                                    .show_value(true),
                            );
                        }
                    });
                });
            });
        let rect = area.response.rect;
        if pal.overlay.a() < 255 {
            self.glass_regions.push((rect, alpha, ui::PANEL_RADIUS));
        }
    }

    /// 错误胶囊（顶栏下方，出错时常显）。
    fn draw_error_overlay(&mut self, ctx: &egui::Context, pal: &Palette) {
        let Some(err) = self.error_msg.clone() else {
            return;
        };
        let frame = Frame::default()
            .fill(pal.err_bg)
            .stroke(Stroke::new(1.0f32, pal.err_border))
            .rounding(egui::Rounding::same(ui::PANEL_RADIUS))
            .inner_margin(egui::Margin::symmetric(12.0, 5.0))
            .shadow(pal.shadow);
        let area = egui::Area::new(egui::Id::new("iv-error"))
            .order(egui::Order::Foreground)
            .anchor(
                egui::Align2::CENTER_TOP,
                Vec2::new(
                    0.0,
                    if self.has_context_tools() && ctx.screen_rect().width() < 1200.0 {
                        130.0
                    } else {
                        70.0
                    },
                ),
            )
            .show(ctx, |ui| {
                let _frame_resp = frame.show(ui, |ui| {
                    let content_width = (ctx.screen_rect().width() - 72.0).clamp(120.0, 680.0);
                    ui.set_max_width(content_width);
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [content_width - 36.0, 0.0],
                            egui::Label::new(
                                RichText::new(format!("无法打开：{err}"))
                                    .size(12.5)
                                    .color(pal.err_text),
                            )
                            .wrap(true),
                        );
                        if ui::icon_btn_danger(ui, Icon::Close, pal)
                            .on_hover_text("关闭")
                            .clicked()
                        {
                            self.error_msg = None;
                        }
                    });
                });
            });
        let rect = area.response.rect;
        if pal.overlay.a() < 255 {
            self.glass_regions.push((rect, 1.0, ui::PANEL_RADIUS));
        }
    }

    /// 底部悬浮状态栏：像素检查器 + 状态标记 + 缩放。
    fn draw_bottom_overlay(&mut self, ctx: &egui::Context, pal: &Palette) {
        if self.show_settings || self.bot_alpha <= 0.01 {
            return;
        }
        let alpha = self.bot_alpha;
        let area = egui::Area::new(egui::Id::new("iv-bottom"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -10.0))
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                let _frame_resp = ui::capsule(pal).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // —— 像素检查器：色块固定位 + 定宽槽位文本 ——
                        // 色块始终占位（无像素时不填色），避免布局伸缩
                        let mono = egui::FontId::new(11.5, egui::FontFamily::Monospace);
                        let (rc, _) =
                            ui.allocate_exact_size(egui::vec2(13.0, 13.0), egui::Sense::hover());
                        {
                            let p = ui.painter();
                            p.rect_filled(
                                rc,
                                egui::Rounding::same(2.0),
                                Color32::from_rgb(0x5a, 0x5a, 0x5a),
                            );
                            if let Some([r, g, b, a]) = self.probe_color {
                                p.rect_filled(
                                    rc,
                                    egui::Rounding::same(2.0),
                                    Color32::from_rgba_unmultiplied(r, g, b, a),
                                );
                            }
                            p.rect_stroke(
                                rc,
                                egui::Rounding::same(2.0),
                                Stroke::new(1.0f32, pal.border),
                            );
                        }
                        // 默认只保留坐标与 HEX；完整 RGBA/浮点值点击后在检查器中查看。
                        let (iw, ih) = self
                            .current
                            .as_ref()
                            .map(|c| (c.img.width, c.img.height))
                            .unwrap_or((9999, 9999));
                        let sample = format!("{iw}, {ih}   #RRGGBBAA");
                        let slot_w = ui
                            .fonts(|f| f.layout_no_wrap(sample, mono.clone(), pal.dim))
                            .size()
                            .x;
                        let (slot, slot_resp) =
                            ui.allocate_exact_size(Vec2::new(slot_w, 20.0), egui::Sense::click());
                        let probe = self.probe_text.clone();
                        let (txt, color) = if probe.is_empty() {
                            ("光标移到图像上查看像素".to_string(), pal.faint)
                        } else {
                            (probe, pal.dim)
                        };
                        let galley = ui.fonts(|f| f.layout_no_wrap(txt, mono.clone(), color));
                        let gp = Pos2::new(slot.left(), slot.center().y - galley.size().y / 2.0);
                        ui.painter().galley(gp, galley, color);
                        let slot_resp = slot_resp
                            .on_hover_text("坐标与 HEX；点击打开可固定、可复制的完整像素检查器");
                        if slot_resp.clicked() {
                            self.show_probe = true;
                        }

                        ui::sep(ui, pal);

                        // —— 状态标记（非常态才显示）——
                        if self.nearest {
                            ui.label(RichText::new("近邻").size(11.0).color(pal.accent));
                        }
                        if self.channel != ChannelMode::Rgb {
                            ui.label(
                                RichText::new(self.channel.label())
                                    .size(11.0)
                                    .color(pal.accent),
                            );
                        }
                        let mip_count =
                            self.current.as_ref().map(|c| c.img.mips.len()).unwrap_or(1);
                        if mip_count > 1 {
                            ui.label(
                                RichText::new(format!("Mip {}/{}", self.mip_index, mip_count - 1))
                                    .size(11.0)
                                    .color(pal.dim),
                            )
                            .on_hover_text("↑↓ 切换 mip");
                        }
                        let anim_frames = self
                            .current
                            .as_ref()
                            .map(|c| c.img.frames.len())
                            .unwrap_or(0);
                        if anim_frames > 1 {
                            // 帧状态槽位：按“帧 n/n · 已暂停”最大宽度预留，
                            // 帧号推进 / 播放暂停切换都不再改变胶囊宽度
                            let mono = egui::FontId::new(11.0, egui::FontFamily::Monospace);
                            let slot_w = ui
                                .fonts(|f| {
                                    f.layout_no_wrap(
                                        format!("帧 {anim_frames}/{anim_frames} · 已暂停"),
                                        mono.clone(),
                                        pal.dim,
                                    )
                                })
                                .size()
                                .x;
                            let (slot, slot_resp) = ui
                                .allocate_exact_size(Vec2::new(slot_w, 18.0), egui::Sense::hover());
                            let fd = anim_frames.to_string().len();
                            let txt = format!(
                                "帧 {:>fd$}/{}{}",
                                self.frame_index + 1,
                                anim_frames,
                                if self.playing { "" } else { " · 已暂停" },
                                fd = fd
                            );
                            let galley = ui.fonts(|f| f.layout_no_wrap(txt, mono, pal.dim));
                            let gp =
                                Pos2::new(slot.left(), slot.center().y - galley.size().y / 2.0);
                            ui.painter().galley(gp, galley, pal.dim);
                            slot_resp.on_hover_text("Space 播放/暂停 · , . 逐帧");
                        }

                        // —— 缩放（固定槽位，右对齐；mono 复用上方定义）——
                        let slot_w = ui
                            .fonts(|f| {
                                f.layout_no_wrap("6400%".to_string(), mono.clone(), pal.text)
                            })
                            .size()
                            .x;
                        let (slot, slot_resp) =
                            ui.allocate_exact_size(Vec2::new(slot_w, 18.0), egui::Sense::hover());
                        let galley = ui.fonts(|f| {
                            f.layout_no_wrap(
                                format!("{:.0}%", self.view.scale * 100.0),
                                mono,
                                pal.text,
                            )
                        });
                        let gp = Pos2::new(
                            slot.right() - galley.size().x,
                            slot.center().y - galley.size().y / 2.0,
                        );
                        ui.painter().galley(gp, galley, pal.text);
                        slot_resp.on_hover_text("缩放比例（滚轮 · F 适配 · 0 实际大小）");
                    });
                });
            });
        let rect = area.response.rect;
        if pal.overlay.a() < 255 {
            self.glass_regions.push((rect, alpha, ui::PANEL_RADIUS));
        }
    }

    /// 自绘右键菜单壳：定位于右键点击处，点击菜单外 / Esc 关闭。
    ///（egui 0.27 内置右键菜单不响应 Esc 且状态为 pub(crate) 不可控，故菜单壳自绘）
    fn draw_ctx_menu_overlay(&mut self, ctx: &egui::Context, pal: &Palette, canvas: egui::Rect) {
        let Some(pos) = self.ctx_menu_pos else {
            return;
        };
        let safe_rect = ctx.screen_rect().shrink(8.0);
        let menu_height = (safe_rect.height() - 16.0).max(240.0);
        let area = egui::Area::new(egui::Id::new("iv-ctxmenu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .constrain_to(safe_rect)
            .show(ctx, |ui| {
                // 限制最大宽度：否则按钮/分隔线会把菜单撑满可用宽度
                ui.set_max_width(232.0);
                let _frame_resp = ui::menu_frame(pal).show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_source("iv-context-scroll")
                        .max_height(menu_height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| self.draw_context_menu(ui, canvas));
                });
            });
        // 左键点击菜单外关闭（右键点别处由画布重新定位菜单）
        let rect = area.response.rect;
        if pal.overlay.a() < 255 {
            self.glass_regions.push((rect, 1.0, 16.0));
        }
        let outside_click = ctx.input(|i| {
            i.pointer.primary_pressed()
                && i.pointer
                    .latest_pos()
                    .map(|p| !rect.contains(p))
                    .unwrap_or(false)
        });
        if outside_click {
            self.ctx_menu_pos = None;
        }
    }

    /// 无分类单列菜单，所有操作统一行高；视图/通道/动画由工具栏和快捷键提供。
    fn draw_context_menu(&mut self, ui: &mut egui::Ui, _canvas: egui::Rect) {
        ui.set_min_width(232.0);
        let has_image = self.current.is_some();
        let ctx = ui.ctx().clone();
        let pal = ui::palette(&ctx);

        // 光标下的像素（菜单弹出前的 hover 值已冻结）
        if let Some([r, g, b, a]) = self.probe_color {
            let hex = format!("#{r:02X}{g:02X}{b:02X}{a:02X}");
            let (header, resp) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(
                    header.shrink2(Vec2::new(2.0, 1.0)),
                    egui::Rounding::same(8.0),
                    pal.btn_hover,
                );
            }
            let rc = egui::Rect::from_center_size(
                Pos2::new(header.left() + 17.0, header.center().y),
                Vec2::splat(16.0),
            );
            {
                let p = ui.painter();
                p.rect_filled(
                    rc,
                    egui::Rounding::same(3.0),
                    Color32::from_rgb(0x5a, 0x5a, 0x5a),
                );
                p.rect_filled(
                    rc,
                    egui::Rounding::same(3.0),
                    Color32::from_rgba_unmultiplied(r, g, b, a),
                );
                p.rect_stroke(
                    rc,
                    egui::Rounding::same(3.0),
                    Stroke::new(1.0f32, pal.border),
                );
                p.text(
                    Pos2::new(header.left() + 32.0, header.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("复制像素值  {hex}"),
                    egui::FontId::new(12.5, egui::FontFamily::Monospace),
                    pal.text,
                );
            }
            if resp.clicked() {
                ui.ctx().output_mut(|o| o.copied_text = hex.clone());
                self.ctx_menu_pos = None;
            }
        }

        // 文件操作
        if let Some(cur) = &self.current {
            let path = cur.path.clone();
            if ui::menu_item(ui, "复制文件路径", "", false, &pal).clicked() {
                ui.ctx()
                    .output_mut(|o| o.copied_text = path.display().to_string());
                self.ctx_menu_pos = None;
            }
            if ui::menu_item(ui, "打开所在文件夹", "", false, &pal).clicked() {
                // 打开当前图片的父目录，而不是用 /select 拼接文件路径。
                if let Err(err) = open_containing_directory(&path) {
                    self.error_msg = Some(format!("打开所在文件夹失败：{err}"));
                }
                self.ctx_menu_pos = None;
            }
            if ui::menu_item(ui, "删除图片…", "Del", false, &pal).clicked() {
                self.request_delete();
            }
        }

        if self.opened_path.is_some() {
            if ui::menu_item(ui, "刷新图片", "F5", false, &pal).clicked() {
                self.reload_current();
                self.ctx_menu_pos = None;
            }
            if ui::menu_item(ui, "锁定切图视图", "L", self.lock_view, &pal).clicked() {
                self.lock_view = !self.lock_view;
                self.ctx_menu_pos = None;
            }
        }
        if has_image {
            if ui::menu_item(ui, "图像属性…", "", false, &pal).clicked() {
                self.show_props = true;
                self.ctx_menu_pos = None;
            }
            if ui::menu_item(ui, "像素检查器…", "", false, &pal).clicked() {
                self.show_probe = true;
                self.ctx_menu_pos = None;
            }
        }
        if ui::menu_item(ui, "打开文件…", if cfg!(target_os = "macos") { "⌘O" } else { "Ctrl+O" }, false, &pal).clicked() {
            self.open_dialog();
            self.ctx_menu_pos = None;
        }
        if ui::menu_item(ui, "设置…", "", false, &pal).clicked() {
            self.show_settings = true;
            self.assoc_registered = winassoc::is_registered();
            self.ctx_menu_pos = None;
        }
    }

    /// 图像属性窗口（右键菜单打开）。
    fn draw_props_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        if !self.show_props {
            return;
        }
        let mut open = self.show_props;
        let mut win_rect = None;
        if let Some(cur) = &self.current {
            let img = &cur.img;
            let name = cur
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            let path = cur.path.display().to_string();
            let file_size = std::fs::metadata(&cur.path)
                .map(|m| fmt_size(m.len()))
                .unwrap_or_default();
            win_rect = egui::Window::new("图像属性")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(360.0)
                .show(ctx, |ui| {
                    egui::Grid::new("iv-props-grid")
                        .num_columns(2)
                        .spacing([18.0, 7.0])
                        .striped(true)
                        .show(ui, |ui| {
                            let row = |k: &str, v: String, ui: &mut egui::Ui| {
                                ui.label(RichText::new(k).color(pal.dim));
                                ui.label(RichText::new(v).color(pal.text));
                                ui.end_row();
                            };
                            row("文件名", name.clone(), ui);
                            row("路径", path.clone(), ui);
                            row("格式", img.kind.label().to_string(), ui);
                            row(
                                "压缩",
                                img.compression.clone().unwrap_or_else(|| "无".into()),
                                ui,
                            );
                            row("尺寸", format!("{} × {}", img.width, img.height), ui);
                            row("Mip 级数", format!("{}", img.mips.len()), ui);
                            if img.frames.len() > 1 {
                                row("动画", format!("{} 帧", img.frames.len()), ui);
                            }
                            row("Alpha", if img.has_alpha { "有" } else { "无" }.into(), ui);
                            row("HDR", if img.is_hdr { "是" } else { "否" }.into(), ui);
                            if let Some(e) = &img.extra_meta {
                                row("备注", e.clone(), ui);
                            }
                            row("文件大小", file_size.clone(), ui);
                        });
                })
                .map(|r| r.response.rect);
        } else {
            open = false;
        }
        self.show_props = open;
        if let Some(r) = win_rect {
            if pal.overlay.a() < 255 {
                self.glass_regions.push((r, 1.0, 16.0));
            }
        }
    }

    /// 完整像素检查器：底栏保持简洁，专业读数在这里可固定和复制。
    fn draw_probe_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        if !self.show_probe {
            return;
        }
        let mut open = self.show_probe;
        let win_rect = egui::Window::new("像素检查器")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(400.0)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                if let Some([r, g, b, a]) = self.probe_color {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        ui.painter().rect_filled(
                            rect,
                            egui::Rounding::same(7.0),
                            Color32::from_rgba_unmultiplied(r, g, b, a),
                        );
                        ui.painter().rect_stroke(
                            rect,
                            egui::Rounding::same(7.0),
                            Stroke::new(1.0_f32, pal.border),
                        );
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&self.probe_text).monospace().color(pal.text));
                            ui.label(
                                RichText::new(&self.probe_detail)
                                    .monospace()
                                    .size(12.0)
                                    .color(pal.dim),
                            );
                        });
                    });
                    if ui::text_button(ui, "复制完整读数", 0.0, pal).clicked() {
                        ui.output_mut(|o| {
                            o.copied_text = format!("{}  {}", self.probe_text, self.probe_detail)
                        });
                    }
                } else {
                    ui.label(RichText::new("将光标移到图像上以读取原始像素值").color(pal.dim));
                }
            })
            .map(|r| r.response.rect);
        self.show_probe = open;
        if let Some(rect) = win_rect {
            if pal.overlay.a() < 255 {
                self.glass_regions.push((rect, 1.0, 16.0));
            }
        }
    }

    /// 设置使用单层紧凑列表；标题只拖动客户区内的弹窗，不移动原生窗口。
    fn draw_settings_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        if !self.show_settings {
            return;
        }
        let screen = ctx.screen_rect();
        let content_width = ui::SETTINGS_CONTENT_WIDTH.min((screen.width() - 64.0).max(280.0));
        let scroll_height = (screen.height() - 144.0).max(180.0);
        let mut close = false;
        let mut popup_delta = Vec2::ZERO;
        let window = egui::Window::new("设置")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .constrain_to(screen.shrink(8.0))
            .frame(egui::Frame::none()
                .fill(pal.overlay)
                .rounding(ui::PANEL_RADIUS)
                .shadow(pal.shadow)
                .inner_margin(16.0))
            .default_width(content_width);
        let window = if let Some(pos) = self.settings_popup_pos {
            window.fixed_pos(pos)
        } else {
            window.anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        };
        let shown = window.show(ctx, |ui| {
                ui.set_width(content_width);
                ui.spacing_mut().item_spacing.y = 0.0;
                let (header, closed) = ui::settings_header(ui, pal);
                close = closed;
                if header.dragged_by(PointerButton::Primary)
                    || header.dragged_by(PointerButton::Secondary)
                {
                    // 首次起拖补齐阈值内位移，后续按逻辑坐标增量移动内部面板。
                    let starting = header.drag_started_by(PointerButton::Primary)
                        || header.drag_started_by(PointerButton::Secondary);
                    popup_delta = if starting {
                        ctx.input(|i| i.pointer.interact_pos().zip(i.pointer.press_origin()))
                            .map_or(header.drag_delta(), |(now, start)| now - start)
                    } else {
                        header.drag_delta()
                    };
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .id_source("iv-settings-scroll")
                    .max_height(scroll_height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(content_width);
                        ui.spacing_mut().interact_size.y = 32.0;
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui::setting_row(ui, pal, "主题", "切换查看器外观", |ui| {
                            if ui::segment_button(ui, "深色", self.theme == ThemeMode::Dark, pal).clicked() {
                                ThemeMode::apply_to(ctx, ThemeMode::Dark);
                                self.theme = ThemeMode::Dark;
                            }
                            if ui::segment_button(ui, "浅色", self.theme == ThemeMode::Light, pal).clicked() {
                                ThemeMode::apply_to(ctx, ThemeMode::Light);
                                self.theme = ThemeMode::Light;
                            }
                        });
                        ui::setting_row(ui, pal, "减少动效", "关闭工具栏淡入淡出动画", |ui| {
                            ui::toggle(ui, &mut self.reduce_motion, pal);
                        });
                        ui::setting_row(ui, pal, "自动刷新", "保存当前图片后更新，保留检查位置", |ui| {
                            if ui::toggle(ui, &mut self.auto_refresh, pal).changed() {
                                self.file_watcher.set_path(
                                    self.opened_path.clone().filter(|_| self.auto_refresh),
                                );
                                if self.auto_refresh { self.reload_current(); }
                            }
                        });
                        ui::setting_row(ui, pal, "锁定切图视图", "切图保留缩放、位置、Mip 和曝光 · L", |ui| {
                            ui::toggle(ui, &mut self.lock_view, pal);
                        });
                        #[cfg(windows)]
                        ui::setting_row(ui, pal, "桌面磨砂", "柔化窗口后方内容；不可用时回退为主题渐变", |ui| {
                            let mut on = self.backdrop;
                            if ui::toggle(ui, &mut on, pal).changed() && on != self.backdrop {
                                self.backdrop = on;
                                if on {
                                    self.last_capture = Instant::now() - Duration::from_secs(10);
                                } else {
                                    self.backdrop_tex = None;
                                    if let Some(renderer) = &mut self.renderer {
                                        renderer.clear_backdrop();
                                    }
                                }
                            }
                        });
                        if let Some(error) = &self.backdrop_restore_error {
                            ui.label(RichText::new(format!("截图状态异常：{error}")).small().color(pal.err_text));
                        }
                        if cfg!(windows) && self.backdrop {
                            if self.backdrop_restore_error.is_none() {
                                if let Some(error) = &self.backdrop_capture_error {
                                    ui.label(RichText::new(format!("桌面磨砂不可用：{error}")).small().color(pal.err_text));
                                }
                            }
                            // 仅开启时展开参数，保留原有范围、持久化键和即时更新语义。
                            ui::compact_slider_row(ui, pal, "不透明度", &mut self.backdrop_opacity, 0.05..=0.95);
                            if ui::compact_slider_row(ui, pal, "模糊强度", &mut self.backdrop_blur, 0.0..=1.0) {
                                self.last_capture = Instant::now() - Duration::from_secs(10);
                            }
                            ui::compact_slider_row(ui, pal, "背景亮度", &mut self.backdrop_brightness, 0.6..=1.4);
                        }
                        #[cfg(windows)]
                        ui::setting_row(ui, pal, "打开方式", "将查看器添加到或移出 Windows 的打开方式列表", |ui| {
                            let label = if self.assoc_registered { "解除注册" } else { "注册" };
                            if ui::text_button(ui, label, 112.0, pal)
                                .on_hover_text("仅在点击后修改当前用户的打开方式注册")
                                .clicked()
                            {
                                let result = if self.assoc_registered { winassoc::unregister() } else { winassoc::register() };
                                if let Err(e) = result { self.error_msg = Some(e); }
                                self.assoc_registered = winassoc::is_registered();
                            }
                        });
                        #[cfg(windows)]
                        ui::setting_row(ui, pal, "默认看图软件", "在 Windows 默认应用设置中确认，不会自动改为默认应用", |ui| {
                            if ui::text_button(ui, "系统设置…", 112.0, pal).clicked() {
                                if !self.assoc_registered {
                                    if let Err(e) = winassoc::register() { self.error_msg = Some(e); }
                                    self.assoc_registered = winassoc::is_registered();
                                }
                                winassoc::open_default_apps_settings();
                            }
                        });
                    });
                ui.add_space(8.0);
                ui.label(RichText::new(format!("{} · v{}", winassoc::APP_NAME, env!("CARGO_PKG_VERSION")))
                    .size(11.5).color(pal.faint));
            });
        if let Some(shown) = shown {
            if popup_delta != Vec2::ZERO || self.settings_popup_pos.is_some() {
                // 收缩窗口或展开磨砂参数后也重新约束，保证标题与关闭按钮可见。
                let pos = ui::clamp_settings_popup(shown.response.rect.translate(popup_delta), screen);
                if self.settings_popup_pos != Some(pos) {
                    self.settings_popup_pos = Some(pos);
                    ctx.request_repaint();
                }
            }
        }
        if close {
            self.show_settings = false;
        }
    }

}

impl eframe::App for App {
    /// 退出时持久化主题与外观设置（eframe persistence）。
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string("iv-auto-refresh", if self.auto_refresh { "on" } else { "off" }.into());
        storage.set_string("iv-lock-view", if self.lock_view { "on" } else { "off" }.into());
        storage.set_string("iv-always-on-top", if self.always_on_top { "on" } else { "off" }.into());
        storage.set_string("iv-background-color", ui::background_color_key(self.background_color));
        storage.set_string("iv-image-bounds", if self.show_image_bounds { "on" } else { "off" }.into());
        storage.set_string(
            "iv-theme",
            match self.theme {
                ThemeMode::Dark => "dark".into(),
                ThemeMode::Light => "light".into(),
            },
        );
        storage.set_string(
            "iv-checkerboard",
            if self.checkerboard { "on" } else { "off" }.into(),
        );
        storage.set_string(
            "iv-backdrop",
            if self.backdrop { "on" } else { "off" }.into(),
        );
        storage.set_string("iv-bd-opacity", self.backdrop_opacity.to_string());
        storage.set_string("iv-bd-blur", self.backdrop_blur.to_string());
        storage.set_string("iv-bd-bright", self.backdrop_brightness.to_string());
        storage.set_string(
            "iv-reduce-motion",
            if self.reduce_motion { "on" } else { "off" }.into(),
        );
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_delete(ctx);
        self.poll_file_changes();
        self.handle_loader_messages();
        self.poll_directory();
        self.update_window_title(ctx);
        self.apply_topmost(ctx);
        let pal = ui::palette(ctx);

        // 首帧：获取本进程窗口句柄（伪磨砂的截图排除/抓图都基于它）
        if self.hwnd.is_none() {
            self.hwnd = crate::backdrop::find_own_window();
            if let Some(hwnd) = self.hwnd {
                // 无边框窗口启用 Win11 圆角，与内部玻璃圆角语言一致
                crate::backdrop::set_rounded_corners(hwnd);
                // 无边框无系统标题栏：上次退出时若窗口被拖出屏幕，启动时夹回工作区
                crate::backdrop::clamp_window_onscreen(hwnd);
            }
        }

        // ---- 伪磨砂背景：捕获窗口背后画面 → 低分辨率模糊快照作画布背景 ----
        // wgpu flip-model swapchain 窗口上 DWM 磨砂材质（Acrylic/Mica）不生效，
        // 故自实现：捕获时临时把窗口设为"截图排除"（不影响屏幕显示），
        // BitBlt 抓到的即是窗口背后的画面。
        let now = Instant::now();
        let mut capture_completed = false;
        if self.backdrop_job_running {
            match self.backdrop_capture_rx.try_recv() {
                Ok(result) => {
                    self.backdrop_job_running = false;
                    capture_completed = true;
                    if self.use_desktop_background() {
                        match result {
                            Ok(cap) => {
                                if let Some(renderer) = &mut self.renderer {
                                    renderer.upload_backdrop(cap.width, cap.height, &cap.rgba);
                                }
                                let img = egui::ColorImage::from_rgba_unmultiplied(
                                    [cap.width as usize, cap.height as usize],
                                    &cap.rgba,
                                );
                                self.backdrop_tex = Some(ctx.load_texture(
                                    "iv-backdrop",
                                    img,
                                    egui::TextureOptions::LINEAR,
                                ));
                                self.backdrop_capture_error = None;
                            }
                            Err(error) => self.backdrop_capture_error = Some(error),
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.backdrop_job_running = false;
                    capture_completed = true;
                    self.backdrop_capture_error = Some("桌面磨砂后台任务已断开".into());
                }
            }
        }

        if self.use_desktop_background() && !self.exclude_unsupported {
            if let Some(hwnd) = self.hwnd {
                let outer = ctx.input(|i| i.viewport().outer_rect);
                let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
                let moved = outer.is_some() && outer != self.last_outer_rect;
                if outer.is_some() {
                    self.last_outer_rect = outer;
                }
                let due_periodic =
                    now.duration_since(self.last_capture) > Duration::from_millis(1500);
                if !minimized && (moved || due_periodic) {
                    self.backdrop_capture_requested = true;
                }

                if self.backdrop_capture_requested && !self.capture_excluded {
                    let ok = crate::backdrop::set_exclude_from_capture(hwnd, true);
                    if ok {
                        self.capture_excluded = true;
                        self.exclude_since = Some(now);
                    } else {
                        self.exclude_unsupported = true; // 老系统：回退渐变画布
                        self.backdrop_capture_error =
                            Some("Windows 未接受截图排除，已回退为主题渐变".into());
                    }
                }
                if self.capture_excluded {
                    // 排除生效要等 DWM 合成几帧；同时只运行一个后台捕获任务。
                    let settled = self
                        .exclude_since
                        .is_some_and(|t| now.duration_since(t) >= Duration::from_millis(80));
                    let throttled =
                        now.duration_since(self.last_capture) >= Duration::from_millis(150);
                    if self.backdrop_capture_requested
                        && !self.backdrop_job_running
                        && settled
                        && throttled
                    {
                        let tx = self.backdrop_capture_tx.clone();
                        let blur = self.backdrop_blur;
                        match std::thread::Builder::new()
                            .name("iv-backdrop-capture".into())
                            .spawn(move || {
                                let result = std::panic::catch_unwind(|| {
                                    crate::backdrop::capture_behind(hwnd, blur)
                                })
                                .unwrap_or_else(|_| Err("桌面磨砂后台任务异常终止".into()));
                                let _ = tx.send(result);
                            }) {
                            Ok(_) => {
                                self.backdrop_job_running = true;
                                self.backdrop_capture_requested = false;
                                self.last_capture = now;
                            }
                            Err(error) => {
                                self.backdrop_capture_error =
                                    Some(format!("无法启动桌面磨砂后台任务：{error}"));
                                self.backdrop_capture_requested = false;
                            }
                        }
                    }
                    if moved {
                        // 拖动/调整中持续持有排除标志，停止 250ms 后释放
                        self.exclude_release_at = Some(now + Duration::from_millis(250));
                    } else if capture_completed && !self.backdrop_capture_requested {
                        // 空闲周期快照完成：立即释放，尽量缩短对系统截图的影响
                        self.exclude_release_at = Some(now);
                    } else if !self.backdrop_job_running
                        && self
                            .exclude_since
                            .is_some_and(|t| now.duration_since(t) > Duration::from_secs(2))
                    {
                        // 兜底：捕获持续失败（如中途最小化）也必须释放
                        self.exclude_release_at = Some(now);
                    }
                    if !self.backdrop_job_running && !self.backdrop_capture_requested {
                        if let Some(at) = self.exclude_release_at {
                            if now < at {
                                ctx.request_repaint_after(at - now);
                            } else {
                                if crate::backdrop::set_exclude_from_capture(hwnd, false) {
                                    self.backdrop_restore_error = None;
                                } else {
                                    self.backdrop_restore_error =
                                        Some("截图排除状态恢复失败，请重启应用".into());
                                }
                                self.capture_excluded = false;
                                self.exclude_since = None;
                                self.exclude_release_at = None;
                            }
                        }
                    }
                    if self.capture_excluded || self.backdrop_job_running {
                        ctx.request_repaint_after(Duration::from_millis(40));
                    }
                } else if !minimized {
                    // 空闲心跳：驱动周期刷新
                    ctx.request_repaint_after(Duration::from_millis(1600));
                }
            }
        } else if self.backdrop_job_running {
            // 设置被关闭时先等待已在读取屏幕的后台任务完成，再恢复截图状态。
            ctx.request_repaint_after(Duration::from_millis(40));
        } else if self.capture_excluded {
            // 设置被关闭：释放截图排除并丢弃快照
            if let Some(hwnd) = self.hwnd {
                if crate::backdrop::set_exclude_from_capture(hwnd, false) {
                    self.backdrop_restore_error = None;
                } else {
                    self.backdrop_restore_error = Some("截图排除状态恢复失败，请重启应用".into());
                }
            }
            self.capture_excluded = false;
            self.exclude_since = None;
            self.exclude_release_at = None;
            self.backdrop_capture_requested = false;
            self.backdrop_tex = None;
            if let Some(renderer) = &mut self.renderer {
                renderer.clear_backdrop();
            }
        }

        // 全窗口画布（图像铺满，UI 以悬浮层叠加其上）。
        // 磨砂开启且有背景快照：画模糊快照 + 主题 tint；
        // 否则（关闭/老系统/首张快照未到）回退不透明对角渐变。
        let backdrop_tex_id = if self.use_desktop_background() {
            self.backdrop_tex.as_ref().map(|t| t.id())
        } else {
            None
        };
        let (canvas_rect, canvas_hover) = egui::CentralPanel::default()
            .frame(Frame::default().fill(pal.canvas))
            .show(ctx, |ui| {
                ui.set_enabled(!self.delete_active());
                let rect = ui.available_rect_before_wrap();
                if let Some(tex_id) = backdrop_tex_id {
                    // 伪磨砂：窗口背后画面的模糊快照铺满画布
                    // 背景亮度：顶点色乘法调暗（≤1.0），>1.0 的部分再用白色叠加提亮
                    let bright = self.backdrop_brightness.clamp(0.5, 1.5);
                    let img_tint = if bright <= 1.0 {
                        let v = (255.0 * bright) as u8;
                        Color32::from_rgb(v, v, v)
                    } else {
                        Color32::WHITE
                    };
                    ui.painter().image(
                        tex_id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        img_tint,
                    );
                    if bright > 1.0 {
                        let add = ((bright - 1.0) / 0.5).clamp(0.0, 1.0);
                        ui.painter().rect_filled(
                            rect,
                            0.0,
                            Color32::from_white_alpha((add * 110.0) as u8),
                        );
                    }
                    // 主题 tint（磨砂不透明度）：压暗/提亮以保证悬浮层可读性
                    let a = (self.backdrop_opacity.clamp(0.0, 1.0) * 220.0) as u8;
                    let tint = if pal.is_dark {
                        Color32::from_black_alpha(a)
                    } else {
                        Color32::from_white_alpha(a)
                    };
                    ui.painter().rect_filled(rect, 0.0, tint);
                } else if let Some(rgb) = self.background_color {
                    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                } else {
                    // 玻璃拟态底衬：画布对角渐变（回退路径）
                    ui::paint_canvas_bg(ui.painter(), rect, &pal);
                }
                // 画布左键/中键平移图像；右键拖窗；右键单击仍打开菜单。
                let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                if ctx.input(|i| i.pointer.button_pressed(PointerButton::Secondary)) {
                    self.right_gesture_dragged = false;
                }
                self.begin_right_window_drag(ctx, &resp);
                if resp.secondary_clicked() && !self.right_gesture_dragged {
                    self.ctx_menu_pos = resp.interact_pointer_pos();
                }
                // 从窗口边缘（6px 热区）发起的拖拽交给系统缩放（BeginResize）。
                let drag_from_edge = ctx
                    .input(|i| i.pointer.press_origin())
                    .map(|p| edge_resize_dir(ctx.screen_rect(), p, 6.0).is_some())
                    .unwrap_or(false);
                let secondary_down = ctx.input(|i| i.pointer.button_down(PointerButton::Secondary));
                let pan = [PointerButton::Primary, PointerButton::Middle].into_iter().any(|button| {
                    canvas_pan_allowed(button, drag_from_edge, secondary_down) && resp.dragged_by(button)
                });
                if pan && self.current.is_some() {
                    // 起拖时补上越过拖拽阈值前的位移，避免手感滞后。
                    let starting = resp.drag_started_by(PointerButton::Primary)
                        || resp.drag_started_by(PointerButton::Middle);
                    let delta = if starting {
                        ctx.input(|i| i.pointer.interact_pos().zip(i.pointer.press_origin()))
                            .map_or(resp.drag_delta(), |(now, start)| now - start)
                    } else {
                        resp.drag_delta()
                    };
                    self.view.offset += delta;
                    self.auto_fit = false;
                    ctx.set_cursor_icon(CursorIcon::Grabbing);
                }
                let hover_pos = resp.hover_pos();
                if let Some(p) = hover_pos {
                    let scroll = ctx.input(|i| i.raw_scroll_delta.y);
                    let zoom = ctx.input(|i| i.zoom_delta());
                    if scroll.abs() > 0.5 {
                        self.zoom_at(p, (scroll / 400.0).exp());
                    }
                    if (zoom - 1.0).abs() > 0.001 {
                        self.zoom_at(p, zoom);
                    }
                }
                // 空状态 / 加载中占位（画布内居中）
                if self.current.is_none() {
                    let loading = self
                        .pending
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str());
                    ui::draw_placeholder(ui.painter(), rect, loading, &pal);
                }
                (rect, hover_pos)
            })
            .inner;

        let canvas_size = canvas_rect.size();
        if canvas_size != self.last_canvas_size {
            if self.auto_fit {
                // 适配模式：窗口 resize 时重新适配
                self.fit(canvas_size);
            } else if self.last_canvas_size != Vec2::ZERO {
                // 手动视图模式：随窗口尺寸差值平移视野，保持图像中心不跑飞
                self.view.offset += (canvas_size - self.last_canvas_size) / 2.0;
            }
            self.last_canvas_size = canvas_size;
        }
        // 工具栏按钮触发的动作（此时画布尺寸已知）
        if self.pending_fit {
            self.fit(canvas_size);
            self.pending_fit = false;
        }
        if self.pending_actual {
            self.actual_size(canvas_size);
            self.pending_actual = false;
        }

        self.handle_global_input(ctx, canvas_size);

        // 动画播放调度：到点切帧，并按剩余时间请求重绘
        if self.playing {
            let anim = self
                .current
                .as_ref()
                .map(|c| c.img.frames.len())
                .unwrap_or(0);
            if anim > 1 {
                let now = Instant::now();
                if let Some((index, deadline)) = self.current.as_ref().and_then(|cur| {
                    advance_animation(&cur.img.frames, self.frame_index, self.next_frame_at, now)
                }) {
                    self.next_frame_at = deadline;
                    if index != self.frame_index {
                        self.frame_index = index;
                        self.upload_current_frame();
                    }
                }
                let wait = self
                    .next_frame_at
                    .saturating_duration_since(Instant::now())
                    .max(Duration::from_millis(1));
                ctx.request_repaint_after(wait);
            } else {
                self.playing = false;
            }
        }

        // 悬浮层自动显隐 + 绘制（Foreground 层，叠加在画布之上）
        // 玻璃区域每帧重建：各悬浮层绘制时收集其矩形与淡入系数
        self.glass_regions.clear();
        self.update_overlay_visibility(ctx);
        if !self.delete_active() {
            self.draw_side_navigation(ctx, &pal);
            self.draw_top_overlay(ctx, &pal);
            self.draw_background_menu(ctx, &pal);
            self.draw_context_tools_overlay(ctx, &pal);
            self.draw_error_overlay(ctx, &pal);
            self.draw_bottom_overlay(ctx, &pal);
            self.draw_ctx_menu_overlay(ctx, &pal, canvas_rect);
            self.draw_props_window(ctx, &pal);
            self.draw_probe_window(ctx, &pal);
            self.draw_settings_window(ctx, &pal);
            // 无边框：边缘八向缩放（光标提示 + BeginResize），置于所有悬浮层之后
            if let Some(drag) = &self.right_window_drag {
                if ctx.input(|i| i.pointer.button_down(PointerButton::Secondary)) && drag.advance() {
                    ctx.set_cursor_icon(CursorIcon::Move);
                    ctx.request_repaint_after(Duration::from_millis(16));
                } else {
                    self.right_window_drag = None;
                }
            } else {
                borderless_chrome(ctx);
            }
        }
        self.draw_delete_dialog(ctx, &pal);

        // 像素检查器（光标 → 图像坐标 → 像素值）
        // 右键菜单弹出 / 拖拽时 hover 消失 → 冻结上一帧值；指针离开窗口才清空
        if ctx.input(|i| i.pointer.latest_pos()).is_none() {
            self.probe_text.clear();
            self.probe_detail.clear();
            self.probe_color = None;
        } else if let Some(p) = canvas_hover {
            self.probe_text.clear();
            self.probe_detail.clear();
            self.probe_color = None;
            if let Some((x, y)) = self.pos_to_image(p) {
                // 动画图取当前帧，静态图取当前 mip（均先在局部取完再写回）
                let probe = self.current.as_ref().and_then(|cur| {
                    if cur.img.frames.len() > 1 {
                        let f = cur.img.frames.get(self.frame_index)?;
                        let (w, h) = (cur.img.width, cur.img.height);
                        Some((f.data.rgba8_at(w, h, x, y), f.data.rgba_f32_at(w, h, x, y)))
                    } else {
                        let mip = cur.img.mips.get(self.mip_index)?;
                        Some((mip.rgba8_at(x, y), mip.rgba_f32_at(x, y)))
                    }
                });
                if let Some((Some([r, g, b, a]), f32px)) = probe {
                    self.probe_color = Some([r, g, b, a]);
                    let f = f32px.unwrap_or([0.0; 4]);
                    // 位数填充：坐标 / RGB / 浮点均定宽，底栏槽位宽度不随数值抖动
                    let (iw, ih) = self
                        .current
                        .as_ref()
                        .map(|c| (c.img.width, c.img.height))
                        .unwrap_or((9999, 9999));
                    self.probe_text = format!(
                        "{x:>xw$}, {y:>yw$}   #{r:02X}{g:02X}{b:02X}{a:02X}",
                        xw = iw.to_string().len(),
                        yw = ih.to_string().len(),
                    );
                    self.probe_detail = format!(
                        "RGBA ({r:>3}, {g:>3}, {b:>3}, {a:>3})   Float [{:>7.3} {:>7.3} {:>7.3} {:>7.3}]",
                        f[0], f[1], f[2], f[3],
                    );
                }
            }
        }

        // 图像绘制（wgpu paint callback 覆盖画布）
        if self.glass_regions.len() > MAX_GLASS {
            if !self.glass_overflow_reported {
                eprintln!(
                    "glass region overflow: {} regions, capacity {MAX_GLASS}",
                    self.glass_regions.len()
                );
                self.glass_overflow_reported = true;
            }
        } else {
            self.glass_overflow_reported = false;
        }
        if let Some(r) = &self.renderer {
            if r.has_image() {
                if let Some(mip) = self.current_mip() {
                    let ppp = ctx.pixels_per_point();
                    let csize = canvas_rect.size() * ppp;
                    let offset = self.view.offset * ppp;
                    let scale_phys = self.view.scale * ppp;
                    let mut flags = 0u32;
                    if self.nearest {
                        flags |= 1;
                    }
                    let has_alpha = self
                        .current
                        .as_ref()
                        .map(|c| c.img.has_alpha)
                        .unwrap_or(false);
                    // 半透明背景：棋盘格或纯画布背景色（ALPHA_BLENDING 透出底色）
                    if has_alpha && self.checkerboard && self.background_color.is_none() {
                        flags |= 2;
                    }
                    let is_hdr = self.current.as_ref().map(|c| c.img.is_hdr).unwrap_or(false);
                    if is_hdr {
                        flags |= 4;
                    }
                    if self.use_desktop_background() && self.backdrop_tex.is_some() {
                        flags |= 8;
                    }
                    // 玻璃区域（逻辑坐标 → 画布内物理像素）
                    let mut glass_rects = [[0.0f32; 4]; MAX_GLASS];
                    let mut glass_alpha = [[0.0f32; 4]; 2];
                    let mut glass_corner = [[0.0f32; 4]; 2];
                    // 仅注册半透明表面：两侧导航有真实磨砂，其余新拟态面板保持原样。
                    let glass_count = self.glass_regions.len().min(MAX_GLASS);
                    for (i, (rect, a, corner)) in
                        self.glass_regions.iter().take(glass_count).enumerate()
                    {
                        let r = rect.translate(-canvas_rect.min.to_vec2());
                        glass_rects[i] =
                            [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp];
                        glass_alpha[i / 4][i % 4] = *a;
                        glass_corner[i / 4][i % 4] = corner * ppp;
                    }
                    let (canvas_top, canvas_bottom) = if let Some(rgb) = self.background_color {
                        // The image shader writes gamma-encoded values to an UNORM target.
                        // Do not linearize a user-selected color a second time.
                        let color = ui::background_uniform(rgb);
                        (color, color)
                    } else {
                        let (top, bottom) = ui::canvas_gradient(&pal);
                        (egui::Rgba::from(top).to_array(), egui::Rgba::from(bottom).to_array())
                    };
                    let uniforms = Uniforms {
                        canvas_size: [csize.x, csize.y],
                        image_size: [mip.width as f32, mip.height as f32],
                        screen_offset: [offset.x, offset.y],
                        scale: scale_phys,
                        channel_mode: self.channel as u32,
                        exposure: self.exposure,
                        flags,
                        glass_count: glass_count as u32,
                        checker_cell: 8.0 * ppp,
                        canvas_top,
                        canvas_bottom,
                        backdrop_params: [
                            self.backdrop_brightness,
                            self.backdrop_opacity,
                            if pal.is_dark { 1.0 } else { 0.0 },
                            20.0 * ppp,
                        ],
                        glass_rects,
                        glass_alpha,
                        glass_corner,
                    };
                    r.write_uniforms(&uniforms);
                    let canvas_painter = egui::Painter::new(
                        ctx.clone(),
                        egui::LayerId::new(egui::Order::Background, egui::Id::new("iv-canvas")),
                        canvas_rect,
                    );
                    canvas_painter.add(crate::render::new_paint_callback(canvas_rect));
                    if self.show_image_bounds {
                        if let Some(bounds) = ui::image_bounds_rect(canvas_rect.min, self.view.offset,
                            self.view.scale, Vec2::new(mip.width as f32, mip.height as f32)) {
                            ui::paint_image_bounds(&canvas_painter, bounds, &pal);
                        }
                    }
                    if self.perf_paint_pending {
                        crate::perf::mark("image_paint_queued", self.current.as_ref().map(|c| c.path.as_path()), 0.0);
                        self.perf_paint_pending = false;
                    }
                }
            }
        }

        // 缩放百分比浮层（右下角半透明胶囊，避开底部状态栏）
        if let Some((t, pct)) = &mut self.zoom_flash {
            *t -= ctx.input(|i| i.unstable_dt);
            if *t <= 0.0 {
                self.zoom_flash = None;
            } else {
                let alpha = (*t / 0.8).clamp(0.0, 1.0);
                let painter = egui::Painter::new(
                    ctx.clone(),
                    egui::LayerId::new(egui::Order::Foreground, egui::Id::new("iv-zoom")),
                    canvas_rect,
                );
                let galley = ctx.fonts(|f| {
                    f.layout_no_wrap(
                        format!("{pct:.0}%"),
                        egui::FontId::new(24.0, egui::FontFamily::Proportional),
                        Color32::WHITE,
                    )
                });
                let pos = Pos2::new(canvas_rect.right() - 16.0, canvas_rect.bottom() - 64.0);
                let rect = galley.rect.translate(pos.to_vec2()).expand(10.0);
                painter.rect_filled(
                    rect,
                    egui::Rounding::same(10.0),
                    Color32::from_black_alpha((170.0 * alpha) as u8),
                );
                painter.galley(
                    pos,
                    galley,
                    Color32::from_white_alpha((230.0 * alpha) as u8),
                );
            }
        }

        // Worker completion wakes egui immediately; a slow load only needs a low-rate fallback.
        if self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

/// 相对图片路径先转为绝对路径，再取父目录；不依赖显示字符串或图片仍存在。
fn containing_directory(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    absolute.parent().map(Path::to_path_buf).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "图片路径没有所在目录")
    })
}

/// 参数保持为原生路径，交由 Command 处理空格/引号；不经过 cmd 或 PowerShell。
fn containing_directory_command(path: &Path) -> std::io::Result<std::process::Command> {
    let directory = containing_directory(path)?;
    if !std::fs::metadata(&directory)?.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "图片所在路径不是文件夹",
        ));
    }
    let mut command = std::process::Command::new(if cfg!(target_os = "macos") { "/usr/bin/open" } else { "explorer.exe" });
    command.arg(directory);
    Ok(command)
}

fn open_containing_directory(path: &Path) -> std::io::Result<()> {
    containing_directory_command(path)?.spawn()?;
    Ok(())
}

/// 从 eframe storage 读取 f32 设置项，缺失/解析失败时用默认值。
fn load_f32(storage: Option<&dyn eframe::Storage>, key: &str, default: f32) -> f32 {
    storage
        .and_then(|s| s.get_string(key))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn fmt_size(b: u64) -> String {
    if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
    }
}

/// 无边框窗口的边缘八向缩放：指针贴近窗口边缘（6px 热区）显示对应缩放光标，
/// 按下即交给系统缩放（BeginResize）。最大化时禁用（窗口不可再缩放）。
/// 拖动移动窗口由整个画布和顶栏胶囊处理；双击最大化由顶栏胶囊处理。
fn borderless_chrome(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.latest_pos()) else {
        return;
    };
    let Some(dir) = edge_resize_dir(ctx.screen_rect(), pos, 6.0) else {
        return;
    };
    // 本函数在每帧末尾调用，set_cursor_icon 后于各控件执行，边缘缩放光标优先
    ctx.set_cursor_icon(edge_cursor(dir));
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
    }
}

/// 画布左键和中键都可平移；边缘留给缩放，右键组合不同时移动图片。
fn canvas_pan_allowed(button: PointerButton, from_edge: bool, secondary_down: bool) -> bool {
    matches!(button, PointerButton::Primary | PointerButton::Middle) && !from_edge && !secondary_down
}

/// 点击也重置1秒计时，避免指针不移动时连续切图触发中途隐藏。
fn overlay_pointer_activity(event: &egui::Event) -> bool {
    matches!(event, egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. })
}

/// 不循环目录，不在第一张/最后一张提供无效切换；单图目录隐藏两侧导航。
fn navigation_enabled(index: usize, count: usize) -> [bool; 2] {
    [count > 1 && index > 0 && index < count, count > 1 && index < count - 1]
}

/// 顶栏始终跟随活动状态；底栏仅在已经打开图像时显示。
fn overlay_targets(
    has_image: bool,
    idle: Duration,
    pointer_down: bool,
) -> (bool, bool) {
    let active = idle < OVERLAY_HIDE_DELAY || pointer_down;
    (active, has_image && active)
}

/// 指针是否落在窗口边缘缩放热区（border 宽，逻辑像素），命中则返回八向之一。
/// 四个角优先于单条边；非热区（含多条边同时为 true 的极小窗回退）返回 None。
fn edge_resize_dir(win: egui::Rect, pos: Pos2, border: f32) -> Option<ResizeDirection> {
    if !win.contains(pos) {
        return None;
    }
    let l = pos.x - win.left() < border;
    let r = win.right() - pos.x < border;
    let t = pos.y - win.top() < border;
    let b = win.bottom() - pos.y < border;
    Some(match (t, b, l, r) {
        (true, false, true, false) => ResizeDirection::NorthWest,
        (true, false, false, true) => ResizeDirection::NorthEast,
        (false, true, true, false) => ResizeDirection::SouthWest,
        (false, true, false, true) => ResizeDirection::SouthEast,
        (true, false, false, false) => ResizeDirection::North,
        (false, true, false, false) => ResizeDirection::South,
        (false, false, true, false) => ResizeDirection::West,
        (false, false, false, true) => ResizeDirection::East,
        _ => return None,
    })
}

/// 缩放方向 → 对应的系统鼠标光标。
fn edge_cursor(d: ResizeDirection) -> CursorIcon {
    match d {
        ResizeDirection::North => CursorIcon::ResizeNorth,
        ResizeDirection::South => CursorIcon::ResizeSouth,
        ResizeDirection::East => CursorIcon::ResizeEast,
        ResizeDirection::West => CursorIcon::ResizeWest,
        ResizeDirection::NorthEast => CursorIcon::ResizeNorthEast,
        ResizeDirection::NorthWest => CursorIcon::ResizeNorthWest,
        ResizeDirection::SouthEast => CursorIcon::ResizeSouthEast,
        ResizeDirection::SouthWest => CursorIcon::ResizeSouthWest,
    }
}

#[cfg(test)]
mod tests {
    use super::{navigation_enabled, overlay_pointer_activity, overlay_targets, OVERLAY_HIDE_DELAY};
    use std::time::Duration;

    #[test]
    fn canvas_left_and_middle_pan_but_right_and_resize_do_not() {
        use eframe::egui::PointerButton::{Primary, Secondary, Middle};
        for button in [Primary, Middle] {
            assert!(super::canvas_pan_allowed(button, false, false));
            assert!(!super::canvas_pan_allowed(button, true, false));
            assert!(!super::canvas_pan_allowed(button, false, true));
        }
        assert!(!super::canvas_pan_allowed(Secondary, false, false));
    }

    #[test]
    fn stationary_pointer_clicks_restart_hide_timer() {
        for pressed in [true, false] {
            let event = eframe::egui::Event::PointerButton {
                pos: eframe::egui::Pos2::ZERO,
                button: eframe::egui::PointerButton::Primary,
                pressed,
                modifiers: eframe::egui::Modifiers::NONE,
            };
            assert!(overlay_pointer_activity(&event));
        }
        assert!(!overlay_pointer_activity(&eframe::egui::Event::PointerGone));
    }

    #[test]
    fn navigation_respects_directory_boundaries() {
        assert_eq!(navigation_enabled(0, 0), [false, false]);
        assert_eq!(navigation_enabled(0, 1), [false, false]);
        assert_eq!(navigation_enabled(0, 3), [false, true]);
        assert_eq!(navigation_enabled(1, 3), [true, true]);
        assert_eq!(navigation_enabled(2, 3), [true, false]);
        assert_eq!(navigation_enabled(3, 3), [false, false]);
    }

    #[test]
    fn overlays_hide_at_the_one_second_boundary() {
        assert_eq!(OVERLAY_HIDE_DELAY, Duration::from_secs(1));
        assert_eq!(
            overlay_targets(true, Duration::from_millis(999), false),
            (true, true)
        );
        assert_eq!(
            overlay_targets(true, Duration::from_millis(1000), false),
            (false, false)
        );
        assert_eq!(
            overlay_targets(true, Duration::from_millis(1001), false),
            (false, false)
        );
    }

    #[test]
    fn overlays_hide_when_pointer_is_stationary() {
        assert_eq!(overlay_targets(true, OVERLAY_HIDE_DELAY, false), (false, false));
        assert_eq!(overlay_targets(false, OVERLAY_HIDE_DELAY, false), (false, false));
    }

    #[test]
    fn pointer_activity_shows_only_available_bars() {
        assert_eq!(overlay_targets(true, Duration::ZERO, false), (true, true));
        assert_eq!(overlay_targets(false, Duration::ZERO, false), (true, false));
    }

    #[test]
    fn active_interaction_keeps_bars_visible_temporarily() {
        assert_eq!(overlay_targets(true, OVERLAY_HIDE_DELAY, true), (true, true));
    }
}

#[cfg(test)]
mod folder_open_tests {
    use super::{containing_directory, containing_directory_command};
    use std::path::Path;

    #[test]
    fn bare_filename_opens_current_directory() {
        assert_eq!(
            containing_directory(Path::new("图片 副本.png")).unwrap(),
            std::env::current_dir().unwrap()
        );
    }

    #[test]
    fn relative_nested_file_uses_its_own_directory() {
        let expected = std::env::current_dir().unwrap().join("素材 空格");
        assert_eq!(
            containing_directory(Path::new("素材 空格").join("贴图.png").as_path()).unwrap(),
            expected
        );
    }

    #[test]
    fn explorer_gets_one_directory_argument_not_a_file_or_select_switch() {
        let directory = std::env::current_dir().unwrap();
        // 图片即使已删除，只要父目录存在仍应能打开；这里不创建或启动任何文件。
        let command = containing_directory_command(&directory.join("未保存文件, & (副本).png")).unwrap();
        assert_eq!(command.get_program(), if cfg!(target_os = "macos") { "/usr/bin/open" } else { "explorer.exe" });
        assert_eq!(command.get_args().collect::<Vec<_>>(), vec![directory.as_os_str()]);
    }

    #[test]
    fn missing_directory_reports_an_error_instead_of_opening_explorer_home() {
        let path = std::env::current_dir().unwrap()
            .join("__iv_nonexistent_parent_for_folder_test__").join("图片.png");
        assert!(containing_directory_command(&path).is_err());
    }

    #[test]
    #[cfg(windows)]
    fn windows_roots_unicode_spaces_and_punctuation_are_preserved() {
        for (file, directory) in [
            (r"C:\image.png", r"C:\"),
            (r"E:\中文 素材, & (1)\图片 - 副本.png", r"E:\中文 素材, & (1)"),
            (r"\\server\share\中文 空格\image.png", r"\\server\share\中文 空格"),
        ] {
            assert_eq!(containing_directory(Path::new(file)).unwrap(), Path::new(directory));
        }
    }
}

/// New shortcuts only belong to the viewer, not text fields or other open UI.
fn viewer_shortcuts_allowed(wants_keyboard: bool, overlay_open: bool) -> bool {
    !wants_keyboard && !overlay_open
}
fn fresh_delete_press(events: &[egui::Event]) -> bool {
    events.iter().any(|e| matches!(e, egui::Event::Key {
        key: Key::Delete, pressed: true, repeat: false, modifiers, ..
    } if *modifiers == egui::Modifiers::NONE))
}
fn next_index_after_delete(old_index: usize, remaining: usize) -> Option<usize> {
    if remaining == 0 { None } else { Some(old_index.min(remaining - 1)) }
}

#[cfg(test)]
mod menu_key_tests {
    use super::*;
    #[test]
    fn text_fields_and_overlays_do_not_trigger_viewer_shortcuts() {
        assert!(viewer_shortcuts_allowed(false, false));
        assert!(!viewer_shortcuts_allowed(true, false));
        assert!(!viewer_shortcuts_allowed(false, true));
    }
    #[test]
    fn delete_is_single_press_and_never_shift_delete() {
        let event = |modifiers, repeat| egui::Event::Key {
            key: Key::Delete, physical_key: None, pressed: true, repeat, modifiers,
        };
        assert!(fresh_delete_press(&[event(egui::Modifiers::NONE, false)]));
        assert!(!fresh_delete_press(&[event(egui::Modifiers::SHIFT, false)]));
        assert!(!fresh_delete_press(&[event(egui::Modifiers::CTRL, false)]));
        assert!(!fresh_delete_press(&[event(egui::Modifiers::NONE, true)]));
    }
    #[test]
    fn delete_advances_then_falls_back_then_clears() {
        assert_eq!(next_index_after_delete(1, 2), Some(1));
        assert_eq!(next_index_after_delete(2, 2), Some(1));
        assert_eq!(next_index_after_delete(0, 0), None);
    }
}

#[cfg(test)]
mod cache_validity_tests {
    use super::*;
    fn fixture() -> (PathBuf, Arc<DecodedImage>) {
        let path = std::env::temp_dir().join(format!("iv-cache-{}-{}.png",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::write(&path, [1]).unwrap();
        let image = Arc::new(DecodedImage { width:1,height:1,
            mips:vec![MipLevel {width:1,height:1,data:PixelData::Rgba8(vec![9,8,7,6])}],
            kind:iv_core::decode::ImageKind::Png,compression:None,has_alpha:true,is_hdr:false,
            extra_meta:None,frames:Vec::new() });
        (path,image)
    }
    #[test]
    fn edited_or_removed_files_do_not_hit_stale_cache() {
        let (path,image)=fixture(); let mut cache=ImageCache::new();
        cache.put(path.clone(),image.clone(),&path,ImageCache::stamp(&path).unwrap()); assert!(cache.get(&path).is_some());
        std::fs::write(&path,[1,2]).unwrap(); assert!(cache.get(&path).is_none()); assert_eq!(cache.bytes,0);
        cache.put(path.clone(),image,&path,ImageCache::stamp(&path).unwrap()); std::fs::remove_file(&path).unwrap();
        assert!(cache.get(&path).is_none()); assert_eq!(cache.bytes,0);
    }
    #[test]
    fn cache_hits_promote_real_lru_without_copying_pixels() {
        let (a,image)=fixture(); let (b,second)=fixture(); let mut cache=ImageCache::new();
        cache.put(a.clone(),image.clone(),&a,ImageCache::stamp(&a).unwrap());
        cache.put(b.clone(),second,&b,ImageCache::stamp(&b).unwrap());
        assert!(Arc::ptr_eq(&cache.get(&a).unwrap().0,&image));
        assert_eq!(cache.order,vec![b.clone(),a.clone()]); assert_eq!(cache.bytes,8);
        std::fs::remove_file(a).unwrap(); std::fs::remove_file(b).unwrap();
    }

    #[test]
    fn saving_between_decode_delivery_and_cache_insert_does_not_relabel_old_pixels() {
        let (path, image) = fixture();
        let decoded_version = ImageCache::stamp(&path).unwrap();
        std::fs::write(&path, [1, 2]).unwrap();
        let mut cache = ImageCache::new();
        cache.put(path.clone(), image, &path, decoded_version);
        assert!(cache.get(&path).is_none());
        assert_eq!(cache.bytes, 0);
        std::fs::remove_file(path).unwrap();
    }
}


/// Advance on the original timeline, not `now + delay`, to avoid cumulative slow-down.
/// Skip whole missed cycles arithmetically, then inspect at most one cycle. A suspended
/// window therefore cannot cause an unbounded loop or upload every missed frame.
fn advance_animation(
    frames: &[iv_core::AnimatedFrame],
    index: usize,
    deadline: Instant,
    now: Instant,
) -> Option<(usize, Instant)> {
    if frames.len() < 2 || index >= frames.len() || now < deadline {
        return None;
    }
    let delay = |i: usize| Duration::from_millis(u64::from(frames[i].delay_ms.max(1)));
    let cycle: Duration = (0..frames.len()).map(delay).sum();
    let remainder = now.duration_since(deadline).as_nanos() % cycle.as_nanos();
    let remainder = Duration::new(
        (remainder / 1_000_000_000) as u64,
        (remainder % 1_000_000_000) as u32,
    );
    let mut next = now - remainder;
    let mut frame = index;
    for _ in 0..frames.len() {
        frame = (frame + 1) % frames.len();
        next += delay(frame);
        if next > now {
            return Some((frame, next));
        }
    }
    None
}

#[cfg(test)]
mod animation_timing_tests {
    use super::*;

    fn frames(delays: &[u32]) -> Vec<iv_core::AnimatedFrame> {
        delays
            .iter()
            .map(|&delay_ms| iv_core::AnimatedFrame {
                data: PixelData::Rgba8(vec![0, 0, 0, 255]),
                delay_ms,
            })
            .collect()
    }

    #[test]
    fn variable_frame_delays_do_not_accumulate_render_latency() {
        let start = Instant::now();
        let result = advance_animation(
            &frames(&[10, 30, 60]),
            0,
            start + Duration::from_millis(10),
            start + Duration::from_millis(75),
        );
        assert_eq!(result, Some((2, start + Duration::from_millis(100))));
    }

    #[test]
    fn long_suspend_skips_cycles_and_only_returns_the_current_frame() {
        let start = Instant::now();
        let result = advance_animation(
            &frames(&[10, 30, 60]),
            0,
            start + Duration::from_millis(10),
            start + Duration::from_millis(86_400_250),
        );
        assert_eq!(result, Some((2, start + Duration::from_millis(86_400_300))));
    }

    #[test]
    fn frame_boundary_wraps_without_resetting_the_timeline() {
        let start = Instant::now();
        let data = frames(&[10, 30, 60]);
        assert_eq!(
            advance_animation(&data, 2, start, start),
            Some((0, start + Duration::from_millis(10)))
        );
        assert_eq!(
            advance_animation(&data, 1, start, start + Duration::from_millis(60)),
            Some((0, start + Duration::from_millis(70)))
        );
    }

    #[test]
    fn invalid_static_and_not_yet_due_inputs_do_not_advance() {
        let start = Instant::now();
        assert!(advance_animation(&[], 0, start, start).is_none());
        assert!(advance_animation(&frames(&[0]), 0, start, start).is_none());
        assert!(advance_animation(&frames(&[1, 1]), 2, start, start).is_none());
        assert!(
            advance_animation(&frames(&[1, 1]), 0, start + Duration::from_millis(1), start)
                .is_none()
        );
        assert_eq!(
            advance_animation(&frames(&[0, 0]), 0, start, start),
            Some((1, start + Duration::from_millis(1)))
        );
    }
}

/// rfd uses raw-window-handle 0.5; bridge the already-owned native window without
/// transferring ownership or extending its lifetime beyond the synchronous dialog.
struct DialogOwner(isize);
// SAFETY: constructed only from this App's live HWND, kept alive throughout pick_file.
unsafe impl raw_window_handle::HasRawWindowHandle for DialogOwner {
    fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
        #[cfg(windows)] {
            let mut handle = raw_window_handle::Win32WindowHandle::empty();
            handle.hwnd = self.0 as *mut std::ffi::c_void;
            raw_window_handle::RawWindowHandle::Win32(handle)
        }
        #[cfg(target_os = "macos")] {
            crate::backdrop::dialog_handle(self.0)
        }
    }
}
