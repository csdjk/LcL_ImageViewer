//! Isolated editing surface: immutable source + undoable crop/resize parameters.
use crate::image_edit::{self, Crop, Filter, Pixels, Plan, Source, MAX_SIDE};
use crate::ui::Palette;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Crop,
    Resize,
}
#[derive(Clone, Copy)]
enum Drag {
    New([f32; 2]),
    Move { start: [f32; 2], crop: Crop },
    Corner([f32; 2]),
}
pub enum Action {
    None,
    Save,
    Close,
    Open(PathBuf),
}

pub struct Editor {
    pub original: PathBuf,
    source: Source,
    plan: Plan,
    initial: Plan,
    saved: Option<Plan>,
    mode: Mode,
    lock_ratio: bool,
    undo: Vec<Plan>,
    redo: Vec<Plan>,
    drag: Option<Drag>,
    drag_before: Option<Plan>,
    source_texture: Option<egui::TextureHandle>,
    source_job: Option<Receiver<Result<Pixels, String>>>,
    preview_texture: Option<egui::TextureHandle>,
    preview_job: Option<(Plan, Receiver<Result<Pixels, String>>)>,
    preview_plan: Option<Plan>,
    changed_at: Instant,
    save_job: Option<(Plan, Receiver<Result<PathBuf, String>>)>,
    saved_path: Option<PathBuf>,
    error: Option<String>,
    confirm_close: bool,
    close_requested: bool,
}

fn pixels_job(
    ctx: &egui::Context,
    source: Source,
    plan: Option<Plan>,
) -> Result<Receiver<Result<Pixels, String>>, String> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name("image-edit-preview".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match plan {
                Some(mut p) => {
                    let s = (1024.0 / p.width.max(p.height) as f64).min(1.0);
                    p.width = (p.width as f64 * s).round().max(1.0) as u32;
                    p.height = (p.height as f64 * s).round().max(1.0) as u32;
                    source.render(p)
                }
                None => source.thumbnail(),
            }))
            .unwrap_or_else(|_| Err("编辑预览失败，请重试或缩小图片".into()));
            let _ = tx.send(result);
            ctx.request_repaint();
        })
        .map_err(|e| format!("无法启动预览：{e}"))?;
    Ok(rx)
}
fn texture(ctx: &egui::Context, name: &str, pixels: Pixels) -> egui::TextureHandle {
    ctx.load_texture(
        name,
        egui::ColorImage::from_rgba_unmultiplied(
            [pixels.width as usize, pixels.height as usize],
            &pixels.data,
        ),
        egui::TextureOptions::LINEAR,
    )
}

impl Editor {
    pub fn new(ctx: &egui::Context, original: PathBuf, source: Source) -> Result<Self, String> {
        let plan = Plan::full(source.size);
        let source_job = Some(pixels_job(ctx, source.clone(), None)?);
        Ok(Self {
            original,
            source,
            plan,
            initial: plan,
            saved: None,
            mode: Mode::Crop,
            lock_ratio: true,
            undo: vec![],
            redo: vec![],
            drag: None,
            drag_before: None,
            source_texture: None,
            source_job,
            preview_texture: None,
            preview_job: None,
            preview_plan: None,
            changed_at: Instant::now(),
            save_job: None,
            saved_path: None,
            error: None,
            confirm_close: false,
            close_requested: false,
        })
    }
    pub fn suggested_name(&self) -> String {
        format!(
            "{}_edited.png",
            self.original
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        )
    }
    pub fn request_close(&mut self) {
        if self.save_job.is_some() {
            self.error = Some("正在保存，请等待完成后再关闭".into());
        } else if self.plan != self.saved.unwrap_or(self.initial) {
            self.confirm_close = true;
        } else {
            self.close_requested = true;
        }
    }
    fn remember(&mut self, previous: Plan) {
        if self.plan != previous {
            if self.undo.len() >= 32 {
                self.undo.remove(0);
            }
            self.undo.push(previous);
            self.redo.clear();
            self.changed_at = Instant::now();
            self.error = None;
        }
    }
    fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(self.plan);
            self.plan = p;
            self.changed_at = Instant::now();
        }
    }
    fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(self.plan);
            self.plan = p;
            self.changed_at = Instant::now();
        }
    }
    pub fn save_to(&mut self, ctx: &egui::Context, path: PathBuf) {
        if self.save_job.is_some() {
            return;
        }
        if let Err(e) = self.plan.validate(self.source.size) {
            self.error = Some(e);
            return;
        }
        let (tx, rx) = mpsc::channel();
        let source = self.source.clone();
        let plan = self.plan;
        let original = self.original.clone();
        let ctx = ctx.clone();
        match std::thread::Builder::new()
            .name("image-edit-save".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    image_edit::save_new_png(&source, plan, &original, &path)
                }))
                .unwrap_or_else(|_| Err("保存任务异常，原图未修改".into()));
                let _ = tx.send(result);
                ctx.request_repaint();
            }) {
            Ok(_) => {
                self.save_job = Some((plan, rx));
                self.error = None;
            }
            Err(e) => self.error = Some(format!("无法启动保存：{e}")),
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.source_job.take() {
            match rx.try_recv() {
                Ok(Ok(p)) => self.source_texture = Some(texture(ctx, "iv-edit-source", p)),
                Ok(Err(e)) => self.error = Some(e),
                Err(TryRecvError::Empty) => self.source_job = Some(rx),
                Err(_) => self.error = Some("预览任务中断".into()),
            }
        }
        if let Some((plan, rx)) = self.preview_job.take() {
            match rx.try_recv() {
                Ok(Ok(p)) => {
                    if plan == self.plan {
                        self.preview_texture = Some(texture(ctx, "iv-edit-result", p));
                        self.preview_plan = Some(plan);
                    }
                }
                Ok(Err(e)) => self.error = Some(e),
                Err(TryRecvError::Empty) => self.preview_job = Some((plan, rx)),
                Err(_) => self.error = Some("预览任务中断".into()),
            }
        }
        if self.mode == Mode::Resize
            && self.preview_plan != Some(self.plan)
            && self.preview_job.is_none()
            && self.plan.validate(self.source.size).is_ok()
            && self.changed_at.elapsed() >= Duration::from_millis(180)
        {
            match pixels_job(ctx, self.source.clone(), Some(self.plan)) {
                Ok(rx) => self.preview_job = Some((self.plan, rx)),
                Err(e) => self.error = Some(e),
            }
        }
        if let Some((plan, rx)) = self.save_job.take() {
            match rx.try_recv() {
                Ok(Ok(path)) => {
                    self.saved = Some(plan);
                    self.saved_path = Some(path);
                    self.error = None;
                }
                Ok(Err(e)) => self.error = Some(e),
                Err(TryRecvError::Empty) => self.save_job = Some((plan, rx)),
                Err(_) => self.error = Some("保存任务中断，原图未修改".into()),
            }
        }
        if self.source_job.is_some()
            || self.preview_job.is_some()
            || self.save_job.is_some()
            || (self.mode == Mode::Resize && self.preview_plan != Some(self.plan))
        {
            ctx.request_repaint_after(Duration::from_millis(40));
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, pal: &Palette) -> Action {
        self.poll(ctx);
        let mut action = Action::None;
        let busy = self.save_job.is_some();
        let screen = ctx.screen_rect();
        let width = (screen.width() - 80.0).clamp(300.0, 820.0);
        let preview_height = (screen.height() - 322.0).clamp(100.0, 430.0);
        if !busy && !ctx.wants_keyboard_input() {
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Z)) {
                self.undo();
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Y)) {
                self.redo();
            }
        }
        egui::Window::new("裁剪与分辨率")
            .id(egui::Id::new("iv-image-editor"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .constrain_to(screen.shrink(8.0))
            .frame(
                egui::Frame::none()
                    .fill(pal.overlay)
                    .rounding(16.0)
                    .shadow(pal.shadow)
                    .inner_margin(16.0),
            )
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("编辑图片")
                            .size(19.0)
                            .strong()
                            .color(pal.text),
                    );
                    ui.label(format!("{} × {}", self.source.size.0, self.source.size.1));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add_enabled(!busy, egui::Button::new("关闭")).clicked() {
                            self.request_close();
                        }
                        if ui
                            .add_enabled(!busy && !self.redo.is_empty(), egui::Button::new("重做"))
                            .clicked()
                        {
                            self.redo();
                        }
                        if ui
                            .add_enabled(!busy && !self.undo.is_empty(), egui::Button::new("撤销"))
                            .clicked()
                        {
                            self.undo();
                        }
                    });
                });
                ui.label(
                    egui::RichText::new(&self.source.note)
                        .size(11.5)
                        .color(pal.dim),
                );
                ui.add_enabled_ui(!busy, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.mode, Mode::Crop, "裁剪");
                        ui.selectable_value(&mut self.mode, Mode::Resize, "修改分辨率");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("重置全部").clicked() {
                                let prev = self.plan;
                                self.plan = self.initial;
                                self.remember(prev);
                            }
                        });
                    });
                    let previous = self.plan;
                    if self.mode == Mode::Crop {
                        let mut crop = self.plan.crop;
                        ui.horizontal(|ui| {
                            ui.label("X");
                            ui.add(
                                egui::DragValue::new(&mut crop.x)
                                    .clamp_range(0..=self.source.size.0 - 1)
                                    .speed(1.0),
                            );
                            ui.label("Y");
                            ui.add(
                                egui::DragValue::new(&mut crop.y)
                                    .clamp_range(0..=self.source.size.1 - 1)
                                    .speed(1.0),
                            );
                            crop.w = crop.w.min(self.source.size.0 - crop.x);
                            crop.h = crop.h.min(self.source.size.1 - crop.y);
                            ui.label("宽");
                            ui.add(
                                egui::DragValue::new(&mut crop.w)
                                    .clamp_range(1..=self.source.size.0 - crop.x)
                                    .speed(1.0),
                            );
                            ui.label("高");
                            ui.add(
                                egui::DragValue::new(&mut crop.h)
                                    .clamp_range(1..=self.source.size.1 - crop.y)
                                    .speed(1.0),
                            );
                            ui.label("px");
                        });
                        ui.horizontal(|ui| {
                            if ui.button("全选").clicked() {
                                crop = Crop::full(self.source.size.0, self.source.size.1);
                            }
                            for (label, ratio) in
                                [("居中 1:1", (1, 1)), ("4:3", (4, 3)), ("16:9", (16, 9))]
                            {
                                if ui.button(label).clicked() {
                                    crop = Crop::centered_ratio(self.source.size, ratio);
                                }
                            }
                            ui.label(
                                egui::RichText::new("框选 / 拖角调整 / 拖动选区")
                                    .size(11.0)
                                    .color(pal.dim),
                            );
                        });
                        if crop != self.plan.crop {
                            self.plan.set_crop(crop);
                        }
                    } else {
                        ui.horizontal(|ui| {
                            ui.label("宽");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut self.plan.width)
                                        .clamp_range(1..=MAX_SIDE)
                                        .speed(1.0),
                                )
                                .changed()
                                && self.lock_ratio
                            {
                                self.plan.height = Plan::paired_dimension(
                                    self.plan.width,
                                    self.plan.crop.h,
                                    self.plan.crop.w,
                                );
                            }
                            ui.label("× 高");
                            if ui
                                .add(
                                    egui::DragValue::new(&mut self.plan.height)
                                        .clamp_range(1..=MAX_SIDE)
                                        .speed(1.0),
                                )
                                .changed()
                                && self.lock_ratio
                            {
                                self.plan.width = Plan::paired_dimension(
                                    self.plan.height,
                                    self.plan.crop.w,
                                    self.plan.crop.h,
                                );
                            }
                            ui.label("px");
                            if ui.checkbox(&mut self.lock_ratio, "锁定比例").changed()
                                && self.lock_ratio
                            {
                                self.plan.height = Plan::paired_dimension(
                                    self.plan.width,
                                    self.plan.crop.h,
                                    self.plan.crop.w,
                                );
                            }
                        });
                        ui.horizontal(|ui| {
                            for (label, n, d) in [("50%", 1, 2), ("100%", 1, 1), ("200%", 2, 1)] {
                                if ui.button(label).clicked() {
                                    self.plan.width = (self.plan.crop.w * n / d).max(1);
                                    self.plan.height = (self.plan.crop.h * n / d).max(1);
                                }
                            }
                            egui::ComboBox::from_id_source("iv-edit-filter")
                                .width(160.0)
                                .selected_text(if self.plan.filter == Filter::Smooth {
                                    "平滑（双线性）"
                                } else {
                                    "最近邻（像素图）"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.plan.filter,
                                        Filter::Smooth,
                                        "平滑（双线性）",
                                    );
                                    ui.selectable_value(
                                        &mut self.plan.filter,
                                        Filter::Nearest,
                                        "最近邻（像素图）",
                                    );
                                });
                            ui.label(
                                egui::RichText::new("先裁剪，再调整输出尺寸")
                                    .size(11.0)
                                    .color(pal.dim),
                            );
                        });
                    }
                    self.remember(previous);
                    self.draw_preview(ui, pal, preview_height);
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "裁剪 {} × {}  →  输出 {} × {} px",
                            self.plan.crop.w, self.plan.crop.h, self.plan.width, self.plan.height
                        ))
                        .color(pal.text),
                    );
                    ui.label(
                        egui::RichText::new("预览按窗口缩放")
                            .size(11.0)
                            .color(pal.dim),
                    );
                });
                let invalid = self.plan.validate(self.source.size).err();
                if let Some(error) = invalid.as_ref().or(self.error.as_ref()) {
                    ui.label(egui::RichText::new(error).size(12.0).color(pal.accent));
                } else if busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("正在保存… 原图保持不变");
                    });
                } else if let Some(path) = &self.saved_path {
                    ui.label(
                        egui::RichText::new(format!(
                            "已保存：{}",
                            path.file_name().unwrap_or_default().to_string_lossy()
                        ))
                        .size(12.0),
                    )
                    .on_hover_text(path.display().to_string());
                } else {
                    ui.label(
                        egui::RichText::new("另存为新的 PNG，不覆盖原图或已有文件")
                            .size(12.0)
                            .color(pal.dim),
                    );
                }
                ui.horizontal(|ui| {
                    if self.confirm_close {
                        ui.label("放弃未保存的编辑？");
                        if ui.button("放弃编辑").clicked() {
                            self.close_requested = true;
                        }
                        if ui.button("返回编辑").clicked() {
                            self.confirm_close = false;
                        }
                    } else {
                        if ui
                            .add_enabled(
                                !busy && invalid.is_none(),
                                egui::Button::new("另存为 PNG…")
                                    .min_size(Vec2::new(144.0, 32.0))
                                    .fill(pal.btn_pressed),
                            )
                            .clicked()
                        {
                            action = Action::Save;
                        }
                        if let Some(path) = &self.saved_path {
                            if ui
                                .add_enabled(
                                    !busy && self.saved == Some(self.plan),
                                    egui::Button::new("打开已保存图片"),
                                )
                                .clicked()
                            {
                                action = Action::Open(path.clone());
                            }
                        }
                    }
                });
            });
        if self.close_requested {
            Action::Close
        } else {
            action
        }
    }
    fn draw_preview(&mut self, ui: &mut egui::Ui, pal: &Palette, height: f32) {
        let (viewport, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::drag());
        let p = ui.painter().with_clip_rect(viewport);
        p.rect_filled(viewport, 8.0, pal.canvas);
        let size = if self.mode == Mode::Crop {
            self.source.size
        } else {
            (self.plan.width.max(1), self.plan.height.max(1))
        };
        let scale = ((viewport.width() - 16.0) / size.0 as f32)
            .min((height - 16.0) / size.1 as f32)
            .max(0.001);
        let rect = Rect::from_center_size(
            viewport.center(),
            Vec2::new(size.0 as f32, size.1 as f32) * scale,
        );
        // Checkerboard is preview-only, never blended into exported pixels.
        let tile = 12.0;
        for y in 0..(rect.height() / tile).ceil() as usize {
            for x in 0..(rect.width() / tile).ceil() as usize {
                let color = if (x + y) % 2 == 0 {
                    pal.canvas
                } else {
                    pal.btn_hover
                };
                p.rect_filled(
                    Rect::from_min_size(
                        rect.min + Vec2::new(x as f32 * tile, y as f32 * tile),
                        Vec2::splat(tile),
                    )
                    .intersect(rect),
                    0.0,
                    color,
                );
            }
        }
        if self.mode == Mode::Crop {
            if let Some(tex) = &self.source_texture {
                p.image(
                    tex.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            let to_point = |pos: Pos2| -> [f32; 2] {
                [(pos.x - rect.left()) / scale, (pos.y - rect.top()) / scale]
            };
            let c = self.plan.crop;
            let selected = Rect::from_min_size(
                rect.min + Vec2::new(c.x as f32, c.y as f32) * scale,
                Vec2::new(c.w as f32, c.h as f32) * scale,
            );
            if response.drag_started() {
                if let Some(pos) = ui
                    .input(|i| i.pointer.press_origin())
                    .filter(|p| rect.contains(*p))
                {
                    let corners = [
                        (
                            selected.left_top(),
                            [(c.x + c.w) as f32, (c.y + c.h) as f32],
                        ),
                        (selected.right_top(), [c.x as f32, (c.y + c.h) as f32]),
                        (selected.left_bottom(), [(c.x + c.w) as f32, c.y as f32]),
                        (selected.right_bottom(), [c.x as f32, c.y as f32]),
                    ];
                    self.drag = Some(
                        if let Some((_, fixed)) =
                            corners.iter().find(|(point, _)| point.distance(pos) <= 9.0)
                        {
                            Drag::Corner(*fixed)
                        } else if selected.contains(pos)
                            && c != Crop::full(self.source.size.0, self.source.size.1)
                        {
                            Drag::Move {
                                start: to_point(pos),
                                crop: c,
                            }
                        } else {
                            Drag::New(to_point(pos))
                        },
                    );
                    self.drag_before = Some(self.plan);
                }
            }
            if response.dragged() {
                if let (Some(drag), Some(pos)) = (self.drag, response.interact_pointer_pos()) {
                    let b = to_point(pos);
                    let crop = match drag {
                        Drag::New(a) | Drag::Corner(a) => Crop::from_points(a, b, self.source.size),
                        Drag::Move { start, crop } => crop.translated(
                            (b[0] - start[0]).round() as i64,
                            (b[1] - start[1]).round() as i64,
                            self.source.size,
                        ),
                    };
                    self.plan.set_crop(crop);
                    self.changed_at = Instant::now();
                }
            }
            if response.drag_stopped() {
                self.drag = None;
                if let Some(before) = self.drag_before.take() {
                    self.remember(before);
                }
            }
            let c = self.plan.crop;
            let selected = Rect::from_min_size(
                rect.min + Vec2::new(c.x as f32, c.y as f32) * scale,
                Vec2::new(c.w as f32, c.h as f32) * scale,
            );
            for mask in [
                Rect::from_min_max(rect.min, Pos2::new(rect.right(), selected.top())),
                Rect::from_min_max(Pos2::new(rect.left(), selected.bottom()), rect.max),
                Rect::from_min_max(
                    Pos2::new(rect.left(), selected.top()),
                    selected.left_bottom(),
                ),
                Rect::from_min_max(
                    selected.right_top(),
                    Pos2::new(rect.right(), selected.bottom()),
                ),
            ] {
                p.rect_filled(mask, 0.0, Color32::from_black_alpha(140));
            }
            p.rect_stroke(selected, 0.0, Stroke::new(1.5_f32, pal.accent));
            for n in [1.0, 2.0] {
                let x = selected.left() + selected.width() * n / 3.0;
                let y = selected.top() + selected.height() * n / 3.0;
                p.line_segment(
                    [
                        Pos2::new(x, selected.top()),
                        Pos2::new(x, selected.bottom()),
                    ],
                    Stroke::new(0.7_f32, Color32::from_white_alpha(130)),
                );
                p.line_segment(
                    [
                        Pos2::new(selected.left(), y),
                        Pos2::new(selected.right(), y),
                    ],
                    Stroke::new(0.7_f32, Color32::from_white_alpha(130)),
                );
            }
            for corner in [
                selected.left_top(),
                selected.right_top(),
                selected.left_bottom(),
                selected.right_bottom(),
            ] {
                p.rect_filled(
                    Rect::from_center_size(corner, Vec2::splat(7.0)),
                    1.0,
                    pal.accent,
                );
            }
            response.on_hover_cursor(egui::CursorIcon::Crosshair);
        } else if self.preview_plan == Some(self.plan) {
            if let Some(tex) = &self.preview_texture {
                p.image(
                    tex.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        } else {
            p.text(
                viewport.center(),
                egui::Align2::CENTER_CENTER,
                "正在生成预览…",
                egui::FontId::proportional(14.0),
                pal.dim,
            );
        }
    }
}
