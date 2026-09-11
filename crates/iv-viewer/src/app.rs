//! 应用状态与交互：打开/导航/缩放/平移/通道切换/mip 切换/像素检查器。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::{
    Color32, ComboBox, Frame, Key, Pos2, RichText, Slider, Stroke, Vec2, ViewportCommand,
};
use iv_core::decode::{DecodedImage, MipLevel, PixelData};
use iv_core::format::has_supported_ext;

use crate::loader::{Loader, Msg};
use crate::render::{ChannelMode, Renderer, Uniforms};
use crate::ui::{self, Icon, Palette, ThemeMode};
use crate::winassoc;

/// 预读缓存内存预算（解码后像素总量）
const CACHE_BUDGET_BYTES: usize = 192 * 1024 * 1024;

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
}

impl ImageCache {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
            bytes: 0,
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

    fn get(&self, path: &Path) -> Option<Arc<DecodedImage>> {
        self.map.get(path).cloned()
    }

    fn contains(&self, path: &Path) -> bool {
        self.map.contains_key(path)
    }

    fn put(&mut self, path: PathBuf, img: Arc<DecodedImage>, keep: &Path) {
        if self.map.contains_key(&path) {
            return;
        }
        self.bytes += Self::image_bytes(&img);
        self.map.insert(path.clone(), img);
        self.order.push(path);
        // 超预算时从最旧开始清（保留当前显示）
        while self.bytes > CACHE_BUDGET_BYTES && self.order.len() > 1 {
            let victim = self.order.iter().position(|p| p != keep).unwrap_or(0);
            let Some(p) = self.order.get(victim).cloned() else { break };
            if let Some(img) = self.map.remove(&p) {
                self.bytes -= Self::image_bytes(&img);
            }
            self.order.remove(victim);
        }
    }
}

pub struct App {
    loader: Loader,
    cache: ImageCache,
    current: Option<CurrentImage>,
    /// 正在异步加载的路径（显示"加载中"）
    pending: Option<PathBuf>,
    directory: Option<Directory>,
    view: ViewTransform,
    auto_fit: bool,
    channel: ChannelMode,
    nearest: bool,
    mip_index: usize,
    exposure: f32,
    renderer: Option<Renderer>,
    error_msg: Option<String>,
    /// 底部状态栏文本（像素检查器，每帧更新）
    probe_text: String,
    /// 光标下的像素颜色（状态栏色块 / 右键菜单）
    probe_color: Option<[u8; 4]>,
    /// 图像属性窗口是否打开
    show_props: bool,
    /// 设置窗口是否打开
    show_settings: bool,
    /// 半透明图像背景：true=棋盘格，false=纯画布背景色
    checkerboard: bool,
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
    /// 上一帧指针是否悬停在任一悬浮层上
    over_overlay: bool,
    /// 自绘右键菜单的打开位置（None = 关闭）
    ctx_menu_pos: Option<Pos2>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, initial_path: Option<PathBuf>) -> Self {
        // 恢复上次主题（eframe persistence 存储），默认深色；
        // SetTheme 经 egui-winit → winit → DWM 沉浸式暗色模式同步系统标题栏
        let theme = match cc.storage.and_then(|s| s.get_string("iv-theme")).as_deref() {
            Some("light") => ThemeMode::Light,
            _ => ThemeMode::Dark,
        };
        ThemeMode::apply_to(&cc.egui_ctx, theme);
        let renderer = cc.wgpu_render_state.as_ref().map(crate::render::Renderer::new);
        let mut app = Self {
            loader: Loader::spawn(),
            cache: ImageCache::new(),
            current: None,
            pending: None,
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
            probe_text: String::new(),
            probe_color: None,
            show_props: false,
            show_settings: false,
            checkerboard: cc
                .storage
                .and_then(|s| s.get_string("iv-checkerboard"))
                .as_deref()
                != Some("off"),
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
            over_overlay: false,
            ctx_menu_pos: None,
        };
        if let Some(p) = initial_path {
            app.open(p);
        }
        app
    }

    /// 打开一个文件：扫目录 → 缓存命中直接显示，否则异步加载。
    fn open(&mut self, path: PathBuf) {
        self.error_msg = None;
        self.refresh_directory(&path);
        self.mip_index = 0;
        self.auto_fit = true;
        if let Some(img) = self.cache.get(&path) {
            self.set_current(path, img);
        } else {
            self.pending = Some(path.clone());
            self.current = None;
            self.loader.load(path);
        }
    }

    /// 扫描同目录文件列表并定位 index。
    fn refresh_directory(&mut self, path: &Path) {
        if let Some(dir) = path.parent() {
            let mut files: Vec<PathBuf> = match std::fs::read_dir(dir) {
                Ok(rd) => rd
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_file() && has_supported_ext(p))
                    .collect(),
                Err(_) => Vec::new(),
            };
            files.sort_by(|a, b| {
                let ka = a
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                let kb = b
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                ka.cmp(&kb)
            });
            let index = files.iter().position(|p| p == path).unwrap_or(0);
            self.directory = Some(Directory { files, index });
        }
    }

    fn set_current(&mut self, path: PathBuf, img: Arc<DecodedImage>) {
        self.mip_index = self.mip_index.min(img.mips.len().saturating_sub(1));
        if img.is_hdr {
            self.exposure = 1.0;
        }
        // 动画状态重置：多帧图自动从头播放
        self.frame_index = 0;
        self.playing = img.frames.len() > 1;
        self.next_frame_at = Instant::now()
            + Duration::from_millis(img.frames.first().map(|f| f.delay_ms as u64).unwrap_or(100));
        self.cache.put(path.clone(), img.clone(), &path);
        self.current = Some(CurrentImage { path, img });
        self.pending = None;
        self.upload_current_mip();
        self.auto_fit = true;
        // 主动安排一次适配：auto_fit 只在窗口尺寸变化时触发 fit，
        // 切图时窗口尺寸通常没变，必须走 pending_fit 才能立即居中适配
        self.pending_fit = true;
        // 切图时短暂亮出悬浮层，提示目录位置与文件信息
        self.last_move = Instant::now();
        self.schedule_prefetch();
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

    /// 预读目录中相邻文件（前后各 2）。
    fn schedule_prefetch(&mut self) {
        let (files, index) = match &self.directory {
            Some(d) => (d.files.clone(), d.index),
            None => return,
        };
        for di in [index as isize - 2, index as isize - 1, index as isize + 1, index as isize + 2] {
            if di >= 0 && (di as usize) < files.len() {
                let p = &files[di as usize];
                if !self.cache.contains(p) {
                    self.loader.prefetch(p.clone());
                }
            }
        }
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
            self.open(p);
        }
    }

    fn jump_to(&mut self, index: usize) {
        if let Some(p) = self
            .directory
            .as_ref()
            .and_then(|d| d.files.get(index).cloned())
        {
            self.open(p);
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
                Msg::Ready(Ok((path, img))) => {
                    if self.pending.as_ref() == Some(&path) {
                        if let Some(d) = &mut self.directory {
                            if let Some(i) = d.files.iter().position(|p| *p == path) {
                                d.index = i;
                            }
                        }
                        self.set_current(path, img);
                    } else {
                        // 预读结果：只进缓存
                        let keep = self
                            .current
                            .as_ref()
                            .map(|c| c.path.clone())
                            .unwrap_or_default();
                        self.cache.put(path, img, &keep);
                    }
                }
                Msg::Ready(Err((path, err))) => {
                    if self.pending.as_ref() == Some(&path) {
                        self.pending = None;
                        self.current = None;
                        self.error_msg = Some(err);
                    }
                }
            }
        }
    }

    fn handle_global_input(&mut self, ctx: &egui::Context, canvas: Vec2) {
        // Esc 逐层关闭：右键菜单 → 下拉弹窗 → 属性/设置窗口 → 退出程序
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
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
        if ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(Key::O)) {
            self.open_dialog();
            return;
        }

        let key = |k: Key| ctx.input(|i| i.key_pressed(k));
        if key(Key::ArrowLeft) || key(Key::PageUp) {
            self.step(-1);
        }
        if key(Key::ArrowRight) || key(Key::PageDown) {
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
        let mip_count = self
            .current
            .as_ref()
            .map(|c| c.img.mips.len())
            .unwrap_or(1);
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

    /// 弹出打开文件对话框。
    fn open_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter(
                "所有支持的图像",
                &[
                    "png", "jpg", "jpeg", "bmp", "gif", "webp", "ico", "tif", "tiff", "hdr",
                    "dds", "psd", "qoi", "tga", "ppm", "pgm", "pbm",
                ],
            )
            .pick_file()
        {
            self.open(p);
        }
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

    /// 切换深/浅主题（立即生效，退出时经 eframe persistence 持久化）。
    fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.theme = ThemeMode::toggle(ctx);
    }

    /// 指针是否悬停在指定矩形上（悬浮层显隐判断用，与层叠命中无关）。
    fn rect_hovered(ctx: &egui::Context, rect: egui::Rect) -> bool {
        ctx.input(|i| {
            i.pointer
                .latest_pos()
                .map(|p| rect.contains(p))
                .unwrap_or(false)
        })
    }

    /// 悬浮层自动显隐：指针移动 / 悬停悬浮层 / 弹窗打开 / 出错时显示，
    /// 指针静止约 1.6s 后淡出；无图像时顶栏常显（保留“打开”入口）。
    fn update_overlay_visibility(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.pointer.delta().length_sq() > 0.0) {
            self.last_move = Instant::now();
        }
        let has_image = self.current.is_some();
        let recent = self.last_move.elapsed() < Duration::from_millis(1600);
        let busy = self.over_overlay
            || ctx.input(|i| i.pointer.any_down())
            || ctx.memory(|m| m.any_popup_open())
            || self.ctx_menu_pos.is_some()
            || self.show_props
            || self.show_settings
            || self.error_msg.is_some();
        let top_show = !has_image || recent || busy;
        let bot_show = has_image && (recent || busy);
        let dt = ctx.input(|i| i.unstable_dt).min(0.1);
        fn fade(a: &mut f32, show: bool, dt: f32) -> bool {
            let t = if show { 1.0 } else { 0.0 };
            *a += (t - *a) * (dt * 9.0).min(1.0);
            if (*a - t).abs() < 0.02 {
                *a = t;
            }
            *a != t // true = 仍在动画中
        }
        let animating =
            fade(&mut self.top_alpha, top_show, dt) | fade(&mut self.bot_alpha, bot_show, dt);
        if animating {
            ctx.request_repaint();
        } else if recent && !busy {
            // 静止计时到点后再评估一次，触发淡出
            let remain = Duration::from_millis(1600).saturating_sub(self.last_move.elapsed());
            ctx.request_repaint_after(remain.max(Duration::from_millis(16)));
        }
    }

    /// 顶部悬浮工具栏：打开 / 目录导航 / 文件信息 / 通道 / mip / 视图控制 / 动画 / 曝光 / 主题。
    /// 返回指针是否悬停在胶囊上。
    fn draw_top_overlay(&mut self, ctx: &egui::Context, pal: &Palette) -> bool {
        if self.top_alpha <= 0.01 {
            return false;
        }
        let alpha = self.top_alpha;
        let area = egui::Area::new(egui::Id::new("iv-top"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 10.0))
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                ui::capsule(pal).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;

                        // —— 打开 ——
                        if ui::icon_btn(ui, Icon::Open, false, pal)
                            .on_hover_text("打开文件 (Ctrl+O)")
                            .clicked()
                        {
                            self.open_dialog();
                        }
                        if self.current.is_none() && self.pending.is_none() {
                            ui::bar_label(ui, "打开或拖入图片", 12.5, pal.dim);
                        }

                        // —— 目录导航 ——
                        let nav = self
                            .directory
                            .as_ref()
                            .filter(|d| d.files.len() > 1)
                            .map(|d| (d.index, d.files.len()));
                        if let Some((i, n)) = nav {
                            ui::sep(ui, pal);
                            if ui::icon_btn(ui, Icon::Prev, false, pal)
                                .on_hover_text("上一张 (←)")
                                .clicked()
                            {
                                self.step(-1);
                            }
                            ui::bar_label_mono(ui, format!("{}/{}", i + 1, n), 11.5, pal.dim);
                            if ui::icon_btn(ui, Icon::Next, false, pal)
                                .on_hover_text("下一张 (→)")
                                .clicked()
                            {
                                self.step(1);
                            }
                        }

                        // —— 文件信息 ——
                        if let Some(cur) = &self.current {
                            ui::sep(ui, pal);
                            let img = &cur.img;
                            let raw =
                                cur.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                            // 超长文件名截断，避免把胶囊撑得过宽
                            let name = if raw.chars().count() > 28 {
                                format!("{}…", raw.chars().take(27).collect::<String>())
                            } else {
                                raw.to_string()
                            };
                            let file_size = std::fs::metadata(&cur.path)
                                .map(|m| fmt_size(m.len()))
                                .unwrap_or_default();
                            ui::bar_label(ui, name, 13.0, pal.text_bright);
                            ui::badge(ui, img.kind.label(), pal)
                                .on_hover_text(format!("文件大小 {file_size}"));
                            if let Some(c) = &img.compression {
                                ui::badge(ui, c, pal);
                            }
                            ui::bar_label_mono(
                                ui,
                                format!("{}×{}", img.width, img.height),
                                11.5,
                                pal.dim,
                            );
                            if let Some(e) = &img.extra_meta {
                                ui::bar_label(ui, e, 11.0, pal.faint);
                            }
                        } else if self.pending.is_some() {
                            ui::sep(ui, pal);
                            ui::bar_label(ui, "加载中…", 12.5, pal.dim);
                        }

                        if self.current.is_some() {
                            // —— 通道 ——
                            ui::sep(ui, pal);
                            ui::bar_label(ui, "通道", 11.0, pal.faint);
                            for (target, label, tip) in [
                                (ChannelMode::Rgb, "RGB", "完整 RGBA（5 或 C），O 键忽略 Alpha"),
                                (ChannelMode::R, "R", "红通道 (1)"),
                                (ChannelMode::G, "G", "绿通道 (2)"),
                                (ChannelMode::B, "B", "蓝通道 (3)"),
                                (ChannelMode::A, "A", "Alpha 通道 (4)"),
                            ] {
                                ui.selectable_value(
                                    &mut self.channel,
                                    target,
                                    RichText::new(label).size(12.0),
                                )
                                .on_hover_text(tip);
                            }

                            // —— mip 选择（多 mip 时显示）——
                            let mip_sizes: Vec<(u32, u32)> = self
                                .current
                                .as_ref()
                                .map(|c| {
                                    c.img
                                        .mips
                                        .iter()
                                        .map(|m| (m.width, m.height))
                                        .collect()
                                })
                                .unwrap_or_default();
                            if mip_sizes.len() > 1 {
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

                            // —— 动画播放控件（多帧时显示）——
                            let anim_frames = self
                                .current
                                .as_ref()
                                .map(|c| c.img.frames.len())
                                .unwrap_or(0);
                            if anim_frames > 1 {
                                ui::sep(ui, pal);
                                let play_icon =
                                    if self.playing { Icon::Pause } else { Icon::Play };
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
                            let is_hdr = self
                                .current
                                .as_ref()
                                .map(|c| c.img.is_hdr)
                                .unwrap_or(false);
                            if is_hdr {
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

                        // —— 主题切换 + 设置 ——
                        ui::sep(ui, pal);
                        let (icon, tip) = match self.theme {
                            ThemeMode::Dark => (Icon::Sun, "切换到浅色主题 (T)"),
                            ThemeMode::Light => (Icon::Moon, "切换到深色主题 (T)"),
                        };
                        if ui::icon_btn(ui, icon, false, pal).on_hover_text(tip).clicked() {
                            self.toggle_theme(ctx);
                        }
                        if ui::icon_btn(ui, Icon::Settings, false, pal)
                            .on_hover_text("设置")
                            .clicked()
                        {
                            self.show_settings = true;
                            self.assoc_registered = winassoc::is_registered();
                        }
                    });
                });
            });
        Self::rect_hovered(ctx, area.response.rect)
    }

    /// 错误胶囊（顶栏下方，出错时常显）。返回指针是否悬停。
    fn draw_error_overlay(&mut self, ctx: &egui::Context, pal: &Palette) -> bool {
        let Some(err) = self.error_msg.clone() else {
            return false;
        };
        let frame = Frame::default()
            .fill(pal.err_bg)
            .stroke(Stroke::new(1.0f32, pal.err_border))
            .rounding(egui::Rounding::same(18.0))
            .inner_margin(egui::Margin::symmetric(12.0, 5.0))
            .shadow(pal.shadow);
        let area = egui::Area::new(egui::Id::new("iv-error"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 56.0))
            .show(ctx, |ui| {
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("无法打开：{err}"))
                                .size(12.5)
                                .color(pal.err_text),
                        );
                        if ui::icon_btn(ui, Icon::Close, false, pal)
                            .on_hover_text("关闭")
                            .clicked()
                        {
                            self.error_msg = None;
                        }
                    });
                });
            });
        Self::rect_hovered(ctx, area.response.rect)
    }

    /// 底部悬浮状态栏：像素检查器 + 状态标记 + 缩放。返回指针是否悬停。
    fn draw_bottom_overlay(&mut self, ctx: &egui::Context, pal: &Palette) -> bool {
        if self.bot_alpha <= 0.01 {
            return false;
        }
        let alpha = self.bot_alpha;
        let area = egui::Area::new(egui::Id::new("iv-bottom"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_BOTTOM, Vec2::new(0.0, -10.0))
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                ui::capsule(pal).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;

                        // —— 像素检查器：色块固定位 + 定宽槽位文本 ——
                        // 色块始终占位（无像素时不填色），避免布局伸缩
                        let mono = egui::FontId::new(11.5, egui::FontFamily::Monospace);
                        let (rc, _) = ui
                            .allocate_exact_size(egui::vec2(13.0, 13.0), egui::Sense::hover());
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
                        // 槽位宽按本图最大可能文本测量；probe_text 已做位数填充，内容长度恒定
                        let (iw, ih) = self
                            .current
                            .as_ref()
                            .map(|c| (c.img.width, c.img.height))
                            .unwrap_or((9999, 9999));
                        let sample = format!(
                            "{iw}, {ih}  #RRGGBBAA  (255, 255, 255, 255)  [ 64.000  64.000  64.000  64.000]"
                        );
                        let slot_w = ui
                            .fonts(|f| f.layout_no_wrap(sample, mono.clone(), pal.dim))
                            .size()
                            .x;
                        let (slot, slot_resp) = ui
                            .allocate_exact_size(Vec2::new(slot_w, 18.0), egui::Sense::hover());
                        let probe = self.probe_text.clone();
                        let (txt, color) = if probe.is_empty() {
                            ("光标移到图像上查看像素".to_string(), pal.faint)
                        } else {
                            (probe, pal.dim)
                        };
                        let galley = ui.fonts(|f| f.layout_no_wrap(txt, mono.clone(), color));
                        let gp =
                            Pos2::new(slot.left(), slot.center().y - galley.size().y / 2.0);
                        ui.painter().galley(gp, galley, color);
                        slot_resp
                            .on_hover_text("光标处像素：坐标 · HEX · RGBA · 线性浮点值");

                        ui::sep(ui, pal);

                        // —— 状态标记（非常态才显示）——
                        if self.nearest {
                            ui.label(RichText::new("近邻").size(11.0).color(pal.accent));
                        }
                        if self.channel != ChannelMode::Rgb {
                            ui.label(
                                RichText::new(self.channel.label()).size(11.0).color(pal.accent),
                            );
                        }
                        let mip_count = self
                            .current
                            .as_ref()
                            .map(|c| c.img.mips.len())
                            .unwrap_or(1);
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
                            let (slot, slot_resp) = ui.allocate_exact_size(
                                Vec2::new(slot_w, 18.0),
                                egui::Sense::hover(),
                            );
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
        Self::rect_hovered(ctx, area.response.rect)
    }

    /// 自绘右键菜单壳：定位于右键点击处，点击菜单外 / Esc 关闭。
    ///（egui 0.27 内置右键菜单不响应 Esc 且状态为 pub(crate) 不可控，故菜单壳自绘）
    fn draw_ctx_menu_overlay(&mut self, ctx: &egui::Context, pal: &Palette, canvas: egui::Rect) {
        let Some(pos) = self.ctx_menu_pos else {
            return;
        };
        let area = egui::Area::new(egui::Id::new("iv-ctxmenu"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .constrain(true)
            .show(ctx, |ui| {
                ui::menu_frame(pal).show(ui, |ui| {
                    self.draw_context_menu(ui, canvas);
                });
            });
        // 左键点击菜单外关闭（右键点别处由画布重新定位菜单）
        let rect = area.response.rect;
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

    /// 画布右键菜单：像素 / 文件 / 导航 / 视图 / 属性 / 主题。
    fn draw_context_menu(&mut self, ui: &mut egui::Ui, canvas: egui::Rect) {
        ui.set_min_width(210.0);
        let has_image = self.current.is_some();
        let ctx = ui.ctx().clone();
        let pal = ui::palette(&ctx);

        // 光标下的像素（菜单弹出前的 hover 值已冻结）
        if let Some([r, g, b, a]) = self.probe_color {
            let hex = format!("#{r:02X}{g:02X}{b:02X}{a:02X}");
            ui.horizontal(|ui| {
                let (rc, _) =
                    ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::hover());
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
                p.rect_stroke(rc, egui::Rounding::same(3.0), Stroke::new(1.0f32, pal.border));
                if ui
                    .button(RichText::new(format!("复制像素值  {hex}")).monospace())
                    .clicked()
                {
                    ui.ctx().output_mut(|o| o.copied_text = hex.clone());
                    self.ctx_menu_pos = None;
                }
            });
            ui.separator();
        }

        // 文件操作
        if let Some(cur) = &self.current {
            let path = cur.path.clone();
            if ui.button("复制文件路径").clicked() {
                ui.ctx()
                    .output_mut(|o| o.copied_text = path.display().to_string());
                self.ctx_menu_pos = None;
            }
            if ui.button("在资源管理器中显示").clicked() {
                let _ = std::process::Command::new("explorer")
                    .arg(format!("/select,{}", path.display()))
                    .spawn();
                self.ctx_menu_pos = None;
            }
            ui.separator();
        }

        if ui.button("打开文件…").on_hover_text("Ctrl+O").clicked() {
            self.open_dialog();
            self.ctx_menu_pos = None;
        }
        if ui.button("设置…").clicked() {
            self.show_settings = true;
            self.assoc_registered = winassoc::is_registered();
            self.ctx_menu_pos = None;
        }

        // 目录导航
        if self
            .directory
            .as_ref()
            .map(|d| d.files.len() > 1)
            .unwrap_or(false)
        {
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("上一张").on_hover_text("←").clicked() {
                    self.step(-1);
                    self.ctx_menu_pos = None;
                }
                if ui.button("下一张").on_hover_text("→").clicked() {
                    self.step(1);
                    self.ctx_menu_pos = None;
                }
            });
        }

        if !has_image {
            return;
        }

        // 动画播放控制
        let anim = self
            .current
            .as_ref()
            .map(|c| c.img.frames.len())
            .unwrap_or(0);
        if anim > 1 {
            ui.separator();
            if ui
                .button(if self.playing { "暂停动画" } else { "播放动画" })
                .on_hover_text("Space")
                .clicked()
            {
                self.toggle_play();
                self.ctx_menu_pos = None;
            }
            ui.horizontal(|ui| {
                if ui.button("上一帧").on_hover_text(",").clicked() {
                    self.step_frame(-1);
                    self.ctx_menu_pos = None;
                }
                if ui.button("下一帧").on_hover_text(".").clicked() {
                    self.step_frame(1);
                    self.ctx_menu_pos = None;
                }
            });
        }

        // 视图操作
        ui.separator();
        if ui.button("适配窗口").on_hover_text("F").clicked() {
            self.fit(canvas.size());
            self.ctx_menu_pos = None;
        }
        if ui.button("实际大小 100%").on_hover_text("0").clicked() {
            self.actual_size(canvas.size());
            self.ctx_menu_pos = None;
        }
        if ui.button("放大").on_hover_text("滚轮 ↑").clicked() {
            self.zoom_at(canvas.center(), 1.25);
            self.ctx_menu_pos = None;
        }
        if ui.button("缩小").on_hover_text("滚轮 ↓").clicked() {
            self.zoom_at(canvas.center(), 0.8);
            self.ctx_menu_pos = None;
        }
        if ui
            .selectable_label(self.nearest, "最近邻采样")
            .on_hover_text("N —— 放大看像素")
            .clicked()
        {
            self.nearest = !self.nearest;
            self.ctx_menu_pos = None;
        }

        // 通道（内联按钮行，替代 egui 子菜单）
        ui.separator();
        ui.label(RichText::new("通道").small().color(pal.faint));
        ui.horizontal(|ui| {
            for (target, label, tip) in [
                (ChannelMode::Rgb, "RGB", "完整 RGBA（5 或 C），O 键忽略 Alpha"),
                (ChannelMode::R, "R", "红通道 (1)"),
                (ChannelMode::G, "G", "绿通道 (2)"),
                (ChannelMode::B, "B", "蓝通道 (3)"),
                (ChannelMode::A, "A", "Alpha 通道 (4)"),
            ] {
                if ui
                    .selectable_value(&mut self.channel, target, RichText::new(label).size(12.0))
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.ctx_menu_pos = None;
                }
            }
        });
        // mip（多 mip 时内联列出）
        let mip_sizes: Vec<(u32, u32)> = self
            .current
            .as_ref()
            .map(|c| c.img.mips.iter().map(|m| (m.width, m.height)).collect())
            .unwrap_or_default();
        if mip_sizes.len() > 1 {
            ui.separator();
            ui.label(RichText::new("Mip 级别").small().color(pal.faint));
            let before = self.mip_index;
            for (i, (w, h)) in mip_sizes.iter().enumerate() {
                if ui
                    .selectable_value(&mut self.mip_index, i, format!("Mip {i} · {w}×{h}"))
                    .clicked()
                {
                    self.ctx_menu_pos = None;
                }
            }
            if self.mip_index != before {
                self.upload_current_mip();
                self.auto_fit = true;
            }
        }

        ui.separator();
        if ui.button("图像属性…").clicked() {
            self.show_props = true;
            self.ctx_menu_pos = None;
        }
        let theme_label = match self.theme {
            ThemeMode::Dark => "切换到浅色主题",
            ThemeMode::Light => "切换到深色主题",
        };
        if ui.button(theme_label).on_hover_text("T").clicked() {
            self.toggle_theme(&ctx);
            self.ctx_menu_pos = None;
        }
    }

    /// 图像属性窗口（右键菜单打开）。
    fn draw_props_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        if !self.show_props {
            return;
        }
        let mut open = self.show_props;
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
            egui::Window::new("图像属性")
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
                            row(
                                "Alpha",
                                if img.has_alpha { "有" } else { "无" }.into(),
                                ui,
                            );
                            row("HDR", if img.is_hdr { "是" } else { "否" }.into(), ui);
                            if let Some(e) = &img.extra_meta {
                                row("备注", e.clone(), ui);
                            }
                            row("文件大小", file_size.clone(), ui);
                        });
                });
        } else {
            open = false;
        }
        self.show_props = open;
    }

    /// 设置窗口（顶栏齿轮 / 右键菜单打开）：外观 + Windows 集成 + 关于。
    fn draw_settings_window(&mut self, ctx: &egui::Context, pal: &Palette) {
        if !self.show_settings {
            return;
        }
        let mut open = self.show_settings;
        egui::Window::new("设置")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(330.0)
            .show(ctx, |ui| {
                // —— 外观 ——
                ui.label(RichText::new("外观").color(pal.text).strong());
                ui.add_space(2.0);
                egui::Grid::new("iv-settings-look")
                    .num_columns(2)
                    .spacing([16.0, 9.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("主题").color(pal.dim));
                        ui.horizontal(|ui| {
                            let mut mode = self.theme;
                            let changed = ui
                                .selectable_value(&mut mode, ThemeMode::Dark, "深色")
                                .changed()
                                | ui.selectable_value(&mut mode, ThemeMode::Light, "浅色")
                                    .changed();
                            if changed && mode != self.theme {
                                ThemeMode::apply_to(ctx, mode);
                                self.theme = mode;
                            }
                        });
                        ui.end_row();

                        ui.label(RichText::new("透明背景").color(pal.dim))
                            .on_hover_text("含 Alpha 通道图片的背景");
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.checkerboard, true, "棋盘格");
                            ui.selectable_value(&mut self.checkerboard, false, "纯色");
                        });
                        ui.end_row();
                    });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // —— Windows 集成 ——
                ui.label(RichText::new("Windows 集成").color(pal.text).strong());
                ui.add_space(2.0);
                let (status, tip) = if self.assoc_registered {
                    ("已注册到「打开方式」", "列表中将显示图标与名称，可选「始终」")
                } else {
                    ("未注册", "注册后才能出现在打开方式列表并支持「始终」")
                };
                ui.horizontal(|ui| {
                    let status_color = if self.assoc_registered { pal.accent } else { pal.faint };
                    ui.label(RichText::new(status).color(status_color));
                    ui.label(RichText::new(tip).small().color(pal.faint));
                });
                ui.horizontal(|ui| {
                    if self.assoc_registered {
                        if ui
                            .button("解除注册")
                            .on_hover_text("从打开方式列表与默认应用候选中移除")
                            .clicked()
                        {
                            if let Err(e) = winassoc::unregister() {
                                self.error_msg = Some(e);
                            }
                            self.assoc_registered = winassoc::is_registered();
                        }
                    } else if ui
                        .button("注册到「打开方式」")
                        .on_hover_text("写入 HKCU，无需管理员权限")
                        .clicked()
                    {
                        if let Err(e) = winassoc::register() {
                            self.error_msg = Some(e);
                        }
                        self.assoc_registered = winassoc::is_registered();
                    }
                    if ui
                        .button("设为默认看图软件…")
                        .on_hover_text("打开系统「默认应用」设置页")
                        .clicked()
                    {
                        // 未注册时先补注册，否则系统默认应用页里找不到本应用
                        if !self.assoc_registered {
                            if let Err(e) = winassoc::register() {
                                self.error_msg = Some(e);
                            }
                            self.assoc_registered = winassoc::is_registered();
                        }
                        winassoc::open_default_apps_settings();
                    }
                });
                ui.label(
                    RichText::new("Win10/11 的默认关联需在系统设置页确认，程序无法代为设置")
                        .small()
                        .color(pal.faint),
                );

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                // —— 关于 ——
                ui.horizontal(|ui| {
                    ui.label(RichText::new(winassoc::APP_NAME).color(pal.text).strong());
                    ui.label(
                        RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                            .small()
                            .color(pal.faint),
                    );
                });
                ui.label(
                    RichText::new("轻量级游戏美术看图工具 · DDS/PSD/TGA/QOI/HDR/GIF/WebP/APNG")
                        .small()
                        .color(pal.faint),
                );
            });
        self.show_settings = open;
    }
}

impl eframe::App for App {
    /// 退出时持久化主题与外观设置（eframe persistence）。
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
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
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_loader_messages();
        self.update_window_title(ctx);
        let pal = ui::palette(ctx);

        // 全窗口画布（图像铺满，UI 以悬浮层叠加其上）
        let (canvas_rect, canvas_hover) = egui::CentralPanel::default()
            .frame(Frame::default().fill(pal.canvas))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                // 画布交互：拖拽平移 + 滚轮缩放
                let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                // 右键菜单（自绘悬浮菜单，支持 Esc 关闭）
                if resp.secondary_clicked() {
                    self.ctx_menu_pos = resp.interact_pointer_pos();
                }
                if resp.dragged() {
                    self.view.offset += resp.drag_delta();
                    self.auto_fit = false;
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
                if now >= self.next_frame_at {
                    self.frame_index = (self.frame_index + 1) % anim;
                    self.upload_current_frame();
                    self.next_frame_at = now + self.current_frame_delay();
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
        self.update_overlay_visibility(ctx);
        let over_top = self.draw_top_overlay(ctx, &pal);
        let over_err = self.draw_error_overlay(ctx, &pal);
        let over_bot = self.draw_bottom_overlay(ctx, &pal);
        self.over_overlay = over_top || over_err || over_bot;
        self.draw_ctx_menu_overlay(ctx, &pal, canvas_rect);
        self.draw_props_window(ctx, &pal);
        self.draw_settings_window(ctx, &pal);

        // 像素检查器（光标 → 图像坐标 → 像素值）
        // 右键菜单弹出 / 拖拽时 hover 消失 → 冻结上一帧值；指针离开窗口才清空
        if ctx.input(|i| i.pointer.latest_pos()).is_none() {
            self.probe_text.clear();
            self.probe_color = None;
        } else if let Some(p) = canvas_hover {
            self.probe_text.clear();
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
                        "{x:>xw$}, {y:>yw$}  #{r:02X}{g:02X}{b:02X}{a:02X}  ({r:>3}, {g:>3}, {b:>3}, {a:>3})  [{:>7.3} {:>7.3} {:>7.3} {:>7.3}]",
                        f[0], f[1], f[2], f[3],
                        xw = iw.to_string().len(),
                        yw = ih.to_string().len(),
                    );
                }
            }
        }

        // 图像绘制（wgpu paint callback 覆盖画布）
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
                    if has_alpha && self.checkerboard {
                        flags |= 2;
                    }
                    let is_hdr = self
                        .current
                        .as_ref()
                        .map(|c| c.img.is_hdr)
                        .unwrap_or(false);
                    if is_hdr {
                        flags |= 4;
                    }
                    let uniforms = Uniforms {
                        canvas_size: [csize.x, csize.y],
                        image_size: [mip.width as f32, mip.height as f32],
                        screen_offset: [offset.x, offset.y],
                        scale: scale_phys,
                        channel_mode: self.channel as u32,
                        exposure: self.exposure,
                        flags,
                        _pad: [0.0; 2],
                    };
                    r.write_uniforms(&uniforms);
                    egui::Painter::new(
                        ctx.clone(),
                        egui::LayerId::new(
                            egui::Order::Background,
                            egui::Id::new("iv-canvas"),
                        ),
                        canvas_rect,
                    )
                    .add(crate::render::new_paint_callback(canvas_rect));
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

        // 加载中持续重绘
        if self.pending.is_some() {
            ctx.request_repaint();
        }
    }
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
