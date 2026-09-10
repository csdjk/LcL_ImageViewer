//! 应用状态与交互：打开/导航/缩放/平移/通道切换/mip 切换/像素检查器。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use eframe::egui::{
    Color32, ComboBox, Frame, Key, Layout, Pos2, RichText, Slider, Stroke, Vec2,
    ViewportCommand,
};
use iv_core::decode::{DecodedImage, MipLevel, PixelData};
use iv_core::format::has_supported_ext;

use crate::loader::{Loader, Msg};
use crate::render::{ChannelMode, Renderer, Uniforms};
use crate::ui::{self, C};

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
}

impl App {
    pub fn new(cc: &eframe::CreationContext, initial_path: Option<PathBuf>) -> Self {
        ui::apply(&cc.egui_ctx);
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
            last_canvas_size: Vec2::ZERO,
            zoom_flash: None,
            pending_fit: false,
            pending_actual: false,
            playing: false,
            frame_index: 0,
            next_frame_at: Instant::now(),
            last_title: String::new(),
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
        if key(Key::Num1) {
            self.actual_size(canvas);
        }
        if key(Key::C) {
            self.channel = ChannelMode::Rgb;
        }
        if key(Key::R) {
            self.channel = ChannelMode::R;
        }
        if key(Key::G) {
            self.channel = ChannelMode::G;
        }
        if key(Key::B) {
            self.channel = ChannelMode::B;
        }
        if key(Key::A) {
            self.channel = ChannelMode::A;
        }
        if key(Key::O) {
            self.channel = ChannelMode::RgbOpaque;
        }
        if key(Key::N) {
            self.nearest = !self.nearest;
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
                format!("{name} — LcL ImageView")
            }
            None => "LcL ImageView".to_string(),
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }

    /// 顶部工具栏：文件信息 / 目录导航 / 通道 / mip / 视图控制 / 曝光。
    fn draw_toolbar(&mut self, ctx: &egui::Context) {
        let frame = Frame::default()
            .fill(C::BAR)
            .stroke(Stroke::new(1.0f32, C::BORDER))
            .inner_margin(egui::Margin::symmetric(10.0, 4.0));
        egui::TopBottomPanel::top("iv-toolbar")
            .exact_height(40.0)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // —— 左：打开 + 文件信息 ——
                    if ui
                        .button(RichText::new("打开").size(13.0))
                        .on_hover_text("打开文件 (Ctrl+O)")
                        .clicked()
                    {
                        self.open_dialog();
                    }
                    ui.separator();

                    if let Some(cur) = &self.current {
                        let img = &cur.img;
                        let raw =
                            cur.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                        // 超长文件名截断，避免把右侧控件挤变形
                        let name = if raw.chars().count() > 36 {
                            format!("{}…", raw.chars().take(35).collect::<String>())
                        } else {
                            raw.to_string()
                        };
                        let file_size = std::fs::metadata(&cur.path)
                            .map(|m| fmt_size(m.len()))
                            .unwrap_or_default();
                        ui.label(RichText::new(name).strong().color(C::TEXT));
                        ui::badge(ui, img.kind.label())
                            .on_hover_text(format!("文件大小 {file_size}"));
                        if let Some(c) = &img.compression {
                            ui::badge(ui, c);
                        }
                        ui.label(
                            RichText::new(format!("{}×{}", img.width, img.height))
                                .monospace()
                                .size(12.0)
                                .color(C::DIM),
                        );
                        if let Some(e) = &img.extra_meta {
                            ui.label(RichText::new(e).small().color(C::DIM));
                        }
                    } else if self.pending.is_some() {
                        ui.label(RichText::new("加载中…").color(C::DIM));
                    }

                    // —— 右：视图控制（right_to_left，从右端往左添加）——
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        // HDR 曝光滑条
                        let is_hdr = self
                            .current
                            .as_ref()
                            .map(|c| c.img.is_hdr)
                            .unwrap_or(false);
                        if is_hdr {
                            ui.add(
                                Slider::new(&mut self.exposure, 0.01..=64.0)
                                    .logarithmic(true)
                                    .text("曝光"),
                            )
                            .on_hover_text("HDR 曝光倍数");
                        }
                        // 采样切换
                        if ui
                            .selectable_label(self.nearest, RichText::new("近邻").size(13.0))
                            .on_hover_text("最近邻采样 (N) —— 放大看像素")
                            .clicked()
                        {
                            self.nearest = !self.nearest;
                        }
                        // 1:1 / 适配
                        if ui
                            .button(RichText::new("1:1").size(13.0))
                            .on_hover_text("实际大小 (1)")
                            .clicked()
                        {
                            self.pending_actual = true;
                        }
                        if ui
                            .button(RichText::new("适配").size(13.0))
                            .on_hover_text("适配窗口 (F)")
                            .clicked()
                        {
                            self.pending_fit = true;
                        }
                        // 动画播放控件（多帧时显示）
                        let anim_frames = self
                            .current
                            .as_ref()
                            .map(|c| c.img.frames.len())
                            .unwrap_or(0);
                        if anim_frames > 1 {
                            if ui
                                .button(
                                    RichText::new(if self.playing { "暂停" } else { "播放" })
                                        .size(12.5),
                                )
                                .on_hover_text("播放/暂停 (Space)，, . 逐帧")
                                .clicked()
                            {
                                self.toggle_play();
                            }
                            ui.label(
                                RichText::new(format!("{}/{}", self.frame_index + 1, anim_frames))
                                    .small()
                                    .color(C::DIM),
                            );
                            let before = self.frame_index;
                            if ui
                                .add_sized(
                                    [80.0, 18.0],
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
                            ui.separator();
                        }
                        // mip 选择（多 mip 时显示）
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
                                    .size(12.5),
                                )
                                .width(100.0)
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
                        // 通道按钮组（right_to_left：先添加的在右边，所以从 A 排到 RGB，标签最左）
                        ui.separator();
                        for (target, label, tip) in [
                            (ChannelMode::A, "A", "Alpha 通道 (A)"),
                            (ChannelMode::B, "B", "蓝通道 (B)"),
                            (ChannelMode::G, "G", "绿通道 (G)"),
                            (ChannelMode::R, "R", "红通道 (R)"),
                            (ChannelMode::Rgb, "RGB", "完整 RGBA（C），O 键忽略 Alpha"),
                        ] {
                            ui.selectable_value(
                                &mut self.channel,
                                target,
                                RichText::new(label).size(13.0).strong(),
                            )
                            .on_hover_text(tip);
                        }
                        ui.label(RichText::new("通道").small().color(C::DIM));
                        // 目录导航
                        let nav = self
                            .directory
                            .as_ref()
                            .filter(|d| d.files.len() > 1)
                            .map(|d| (d.index, d.files.len()));
                        if let Some((i, n)) = nav {
                            ui.separator();
                            ui.label(
                                RichText::new(format!("{} / {}", i + 1, n))
                                    .small()
                                    .color(C::DIM),
                            );
                            if ui
                                .button(RichText::new("›").size(15.0))
                                .on_hover_text("下一张 (→)")
                                .clicked()
                            {
                                self.step(1);
                            }
                            if ui
                                .button(RichText::new("‹").size(15.0))
                                .on_hover_text("上一张 (←)")
                                .clicked()
                            {
                                self.step(-1);
                            }
                        }
                    });
                });
            });
    }

    /// 错误横幅（工具栏下方，仅出错时显示）。
    fn draw_error_banner(&mut self, ctx: &egui::Context) {
        let Some(err) = self.error_msg.clone() else {
            return;
        };
        let frame = Frame::default()
            .fill(C::ERR_BG)
            .stroke(Stroke::new(1.0f32, C::ERR_BORDER))
            .inner_margin(egui::Margin::symmetric(10.0, 3.0));
        egui::TopBottomPanel::top("iv-error")
            .exact_height(28.0)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new(format!("无法打开：{err}")).size(13.0).color(C::ERR_TEXT));
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("×").size(15.0).color(C::ERR_TEXT))
                            .on_hover_text("关闭")
                            .clicked()
                        {
                            self.error_msg = None;
                        }
                    });
                });
            });
    }

    /// 底部状态栏：左侧像素检查器，右侧缩放/mip/导航指标。
    fn draw_statusbar(&mut self, ctx: &egui::Context) {
        let frame = Frame::default()
            .fill(C::BAR)
            .stroke(Stroke::new(1.0f32, C::BORDER))
            .inner_margin(egui::Margin::symmetric(10.0, 2.0));
        egui::TopBottomPanel::bottom("iv-status")
            .exact_height(24.0)
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // 左：像素色块 + 检查器（等宽字体）
                    if let Some([r, g, b, a]) = self.probe_color {
                        let (rc, _) =
                            ui.allocate_exact_size(egui::vec2(13.0, 13.0), egui::Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(
                            rc,
                            egui::Rounding::same(2.0),
                            Color32::from_rgb(0x5a, 0x5a, 0x5a),
                        );
                        p.rect_filled(
                            rc,
                            egui::Rounding::same(2.0),
                            Color32::from_rgba_unmultiplied(r, g, b, a),
                        );
                        p.rect_stroke(
                            rc,
                            egui::Rounding::same(2.0),
                            Stroke::new(1.0f32, C::BORDER),
                        );
                        ui.add_space(2.0);
                    }
                    let probe = self.probe_text.clone();
                    if !probe.is_empty() {
                        ui.label(RichText::new(probe).monospace().size(12.5).color(C::DIM));
                    } else if self.current.is_some() {
                        ui.label(
                            RichText::new("光标移到图像上查看像素")
                                .small()
                                .color(Color32::from_rgb(0x55, 0x5a, 0x64)),
                        );
                    }

                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        // 目录位置
                        let nav = self
                            .directory
                            .as_ref()
                            .filter(|d| d.files.len() > 1)
                            .map(|d| (d.index, d.files.len()));
                        if let Some((i, n)) = nav {
                            ui.label(
                                RichText::new(format!("{} / {}", i + 1, n))
                                    .small()
                                    .color(C::DIM),
                            )
                            .on_hover_text("目录中的位置（←→ 切换）");
                            ui.separator();
                        }
                        // 动画帧位置
                        let anim_frames = self
                            .current
                            .as_ref()
                            .map(|c| c.img.frames.len())
                            .unwrap_or(0);
                        if anim_frames > 1 {
                            ui.label(
                                RichText::new(format!(
                                    "帧 {}/{}{}",
                                    self.frame_index + 1,
                                    anim_frames,
                                    if self.playing { "" } else { " · 已暂停" }
                                ))
                                .small()
                                .color(C::DIM),
                            )
                            .on_hover_text("Space 播放/暂停 · , . 逐帧");
                            ui.separator();
                        }
                        // mip
                        let mip_count = self
                            .current
                            .as_ref()
                            .map(|c| c.img.mips.len())
                            .unwrap_or(1);
                        if mip_count > 1 {
                            ui.label(
                                RichText::new(format!("Mip {}/{}", self.mip_index, mip_count - 1))
                                    .small()
                                    .color(C::DIM),
                            )
                            .on_hover_text("↑↓ 切换 mip");
                            ui.separator();
                        }
                        // 缩放
                        ui.label(
                            RichText::new(format!("{:.1}%", self.view.scale * 100.0))
                                .monospace()
                                .size(12.5)
                                .color(C::TEXT),
                        )
                        .on_hover_text("缩放比例（滚轮 · F 适配 · 1 实际大小）");
                        // 采样/通道标记
                        if self.nearest {
                            ui.label(RichText::new("近邻").small().color(C::ACCENT));
                            ui.separator();
                        }
                        if self.channel != ChannelMode::Rgb {
                            ui.label(RichText::new(self.channel.label()).small().color(C::ACCENT));
                            ui.separator();
                        }
                    });
                });
            });
    }

    /// 画布右键菜单：像素 / 文件 / 导航 / 视图 / 属性。
    fn draw_context_menu(&mut self, ui: &mut egui::Ui, canvas: egui::Rect) {
        ui.set_min_width(210.0);
        let has_image = self.current.is_some();

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
                p.rect_stroke(rc, egui::Rounding::same(3.0), Stroke::new(1.0f32, C::BORDER));
                if ui
                    .button(RichText::new(format!("复制像素值  {hex}")).monospace())
                    .clicked()
                {
                    ui.ctx().output_mut(|o| o.copied_text = hex.clone());
                    ui.close_menu();
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
                ui.close_menu();
            }
            if ui.button("在资源管理器中显示").clicked() {
                let _ = std::process::Command::new("explorer")
                    .arg(format!("/select,{}", path.display()))
                    .spawn();
                ui.close_menu();
            }
            ui.separator();
        }

        if ui.button("打开文件…").on_hover_text("Ctrl+O").clicked() {
            self.open_dialog();
            ui.close_menu();
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
                    ui.close_menu();
                }
                if ui.button("下一张").on_hover_text("→").clicked() {
                    self.step(1);
                    ui.close_menu();
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
                ui.close_menu();
            }
            ui.horizontal(|ui| {
                if ui.button("上一帧").on_hover_text(",").clicked() {
                    self.step_frame(-1);
                    ui.close_menu();
                }
                if ui.button("下一帧").on_hover_text(".").clicked() {
                    self.step_frame(1);
                    ui.close_menu();
                }
            });
        }

        // 视图操作
        ui.separator();
        if ui.button("适配窗口").on_hover_text("F").clicked() {
            self.fit(canvas.size());
            ui.close_menu();
        }
        if ui.button("实际大小 100%").on_hover_text("1").clicked() {
            self.actual_size(canvas.size());
            ui.close_menu();
        }
        if ui.button("放大").on_hover_text("滚轮 ↑").clicked() {
            self.zoom_at(canvas.center(), 1.25);
            ui.close_menu();
        }
        if ui.button("缩小").on_hover_text("滚轮 ↓").clicked() {
            self.zoom_at(canvas.center(), 0.8);
            ui.close_menu();
        }
        if ui
            .selectable_label(self.nearest, "最近邻采样")
            .on_hover_text("N —— 放大看像素")
            .clicked()
        {
            self.nearest = !self.nearest;
            ui.close_menu();
        }

        // 通道 / mip 子菜单
        ui.separator();
        ui.menu_button(format!("通道：{}", self.channel.label()), |ui| {
            for (target, label, tip) in [
                (ChannelMode::Rgb, "RGB 完整", "C"),
                (ChannelMode::RgbOpaque, "RGB（忽略 Alpha）", "O"),
                (ChannelMode::R, "仅红通道", "R"),
                (ChannelMode::G, "仅绿通道", "G"),
                (ChannelMode::B, "仅蓝通道", "B"),
                (ChannelMode::A, "仅 Alpha", "A"),
            ] {
                ui.selectable_value(&mut self.channel, target, label)
                    .on_hover_text(tip);
            }
        });
        let mip_sizes: Vec<(u32, u32)> = self
            .current
            .as_ref()
            .map(|c| c.img.mips.iter().map(|m| (m.width, m.height)).collect())
            .unwrap_or_default();
        if mip_sizes.len() > 1 {
            let before = self.mip_index;
            ui.menu_button(
                format!("Mip 级别：{}/{}", self.mip_index, mip_sizes.len() - 1),
                |ui| {
                    for (i, (w, h)) in mip_sizes.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.mip_index,
                            i,
                            format!("Mip {i} · {w}×{h}"),
                        );
                    }
                },
            );
            if self.mip_index != before {
                self.upload_current_mip();
                self.auto_fit = true;
            }
        }

        ui.separator();
        if ui.button("图像属性…").clicked() {
            self.show_props = true;
            ui.close_menu();
        }
    }

    /// 图像属性窗口（右键菜单打开）。
    fn draw_props_window(&mut self, ctx: &egui::Context) {
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
                                ui.label(RichText::new(k).color(C::DIM));
                                ui.label(RichText::new(v).color(C::TEXT));
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
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_loader_messages();
        self.update_window_title(ctx);

        // 顶部工具栏 / 错误横幅 / 底部状态栏（先于中央面板声明）
        self.draw_toolbar(ctx);
        self.draw_error_banner(ctx);
        self.draw_statusbar(ctx);
        self.draw_props_window(ctx);

        // 中央画布
        let (canvas_rect, canvas_hover) = egui::CentralPanel::default()
            .frame(Frame::default().fill(C::CANVAS))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                // 画布交互：拖拽平移 + 滚轮缩放
                let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
                // 右键菜单（像素/文件/导航/视图/属性）
                resp.context_menu(|ui| self.draw_context_menu(ui, rect));
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
                    ui::draw_placeholder(ui.painter(), rect, loading);
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
                    self.probe_text = format!(
                        "{x}, {y}  #{r:02X}{g:02X}{b:02X}{a:02X}  ({r}, {g}, {b}, {a})  [{:.3} {:.3} {:.3} {:.3}]",
                        f[0], f[1], f[2], f[3]
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
                    if has_alpha {
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

        // 缩放百分比浮层（半透明胶囊）
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
                let pos = Pos2::new(canvas_rect.right() - 16.0, canvas_rect.top() + 16.0);
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
