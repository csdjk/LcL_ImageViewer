//! 应用状态与交互：打开/导航/缩放/平移/通道切换/mip 切换/像素检查器。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use eframe::egui;
use eframe::egui::{Key, Pos2, Vec2};
use iv_core::decode::{DecodedImage, MipLevel, PixelData};
use iv_core::format::has_supported_ext;

use crate::loader::{Loader, Msg};
use crate::render::{ChannelMode, Renderer, Uniforms};

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
        img.mips
            .iter()
            .map(|m| match &m.data {
                PixelData::Rgba8(v) => v.len(),
                PixelData::RgbaF32(v) => v.len() * 4,
            })
            .sum()
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
    /// 画布上一帧尺寸（检测 resize）
    last_canvas_size: Vec2,
    /// 缩放百分比浮层剩余显示时间（秒）与值
    zoom_flash: Option<(f32, f32)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext, initial_path: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
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
            last_canvas_size: Vec2::ZERO,
            zoom_flash: None,
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
        self.cache.put(path.clone(), img.clone(), &path);
        self.current = Some(CurrentImage { path, img });
        self.pending = None;
        self.upload_current_mip();
        self.auto_fit = true;
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
                return;
            }
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

    /// 底部状态栏左侧文本。
    fn status_text(&self) -> String {
        if let Some(cur) = &self.current {
            let img = &cur.img;
            let name = cur
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            let mip_note = if img.mips.len() > 1 {
                format!("  mip {}/{}", self.mip_index, img.mips.len() - 1)
            } else {
                String::new()
            };
            let comp = img
                .compression
                .as_deref()
                .map(|c| format!("  {c}"))
                .unwrap_or_default();
            let extra = img
                .extra_meta
                .as_deref()
                .map(|e| format!("  {e}"))
                .unwrap_or_default();
            let file_size = std::fs::metadata(&cur.path)
                .map(|m| fmt_size(m.len()))
                .unwrap_or_default();
            format!(
                "{name}  {}  {}x{}{comp}{mip_note}{extra}  {file_size}  {:.1}%",
                img.kind.label(),
                img.width,
                img.height,
                self.view.scale * 100.0,
            )
        } else if self.pending.is_some() {
            "加载中…".to_string()
        } else {
            "ImageView".to_string()
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_loader_messages();

        // 状态栏（先于中央面板声明）
        egui::TopBottomPanel::bottom("iv-status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
                let left = self.status_text();
                ui.label(left);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !self.probe_text.is_empty() {
                        ui.label(&self.probe_text);
                    }
                });
            });
        });

        // 中央画布
        let canvas_rect = egui::CentralPanel::default()
            .frame(egui::Frame::default().fill(egui::Color32::from_gray(30)))
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();
                // 画布交互：拖拽平移 + 滚轮缩放
                let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
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
                rect
            })
            .inner;

        let canvas_size = canvas_rect.size();
        if canvas_size != self.last_canvas_size {
            if self.auto_fit {
                self.fit(canvas_size);
            }
            self.last_canvas_size = canvas_size;
        }

        self.handle_global_input(ctx, canvas_size);

        // 像素检查器（光标 → 图像坐标 → 像素值）
        self.probe_text.clear();
        if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
            if canvas_rect.contains(p) {
                if let Some((x, y)) = self.pos_to_image(p) {
                    if let Some(mip) = self.current_mip() {
                        if let Some([r, g, b, a]) = mip.rgba8_at(x, y) {
                            let f = mip.rgba_f32_at(x, y).unwrap_or([0.0; 4]);
                            self.probe_text = format!(
                                "{x}, {y}  #{r:02X}{g:02X}{b:02X}{a:02X}  ({r}, {g}, {b}, {a})  [{:.3} {:.3} {:.3} {:.3}]",
                                f[0], f[1], f[2], f[3]
                            );
                        }
                    }
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

        // 居中提示（无图/加载中/错误）
        if self.current.is_none() {
            let painter = egui::Painter::new(
                ctx.clone(),
                egui::LayerId::new(egui::Order::Background, egui::Id::new("iv-hint")),
                canvas_rect,
            );
            let text = if let Some(err) = &self.error_msg {
                format!("无法打开：{err}")
            } else if self.pending.is_some() {
                "加载中…".to_string()
            } else {
                "拖入图片 · Ctrl+O 打开\n\n←→ 切换 · 滚轮缩放 · C/R/G/B/A 通道 · ↑↓ mip · N 像素网格\n\nPNG · JPG · TGA · DDS (BC1-7) · PSD · HDR · WebP · GIF …".to_string()
            };
            painter.text(
                canvas_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::TextStyle::Body.resolve(&ctx.style()),
                egui::Color32::from_gray(150),
            );
        }

        // 缩放百分比浮层
        if let Some((t, pct)) = &mut self.zoom_flash {
            *t -= ctx.input(|i| i.unstable_dt);
            if *t <= 0.0 {
                self.zoom_flash = None;
            } else {
                let painter = egui::Painter::new(
                    ctx.clone(),
                    egui::LayerId::new(egui::Order::Foreground, egui::Id::new("iv-zoom")),
                    canvas_rect,
                );
                painter.text(
                    Pos2::new(canvas_rect.right() - 24.0, canvas_rect.top() + 32.0),
                    egui::Align2::RIGHT_TOP,
                    format!("{pct:.0}%"),
                    egui::TextStyle::Heading.resolve(&ctx.style()),
                    egui::Color32::from_white_alpha(200),
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
