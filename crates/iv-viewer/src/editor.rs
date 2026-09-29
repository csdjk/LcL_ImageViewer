//! In-place editor: fixed contextual bars around the SAME native window's main canvas.
//! View zoom is separate from pixel operations. All edits apply sequentially to snapshots.
use crate::edit_ops::{self, CanvasView, Document, Operation};
use crate::image_edit::{self, Crop, Filter, Pixels, Plan, Source};
use crate::ui::{self, Icon, Palette};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
struct DraftKey {
    id: u64,
    mode: u8,
    width: u32,
    height: u32,
    angle: u64,
    nearest: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pan,
    Crop,
    Rotate,
    Resize,
}
#[derive(Clone, Copy)]
enum Drag {
    New([f32; 2]),
    Move { start: [f32; 2], crop: Crop },
    Handle { index: usize, crop: Crop },
    Pan { start: Pos2, offset: [f32; 2] },
}
pub enum Action {
    None,
    Save,
    Close,
    Open(PathBuf),
}

pub struct Editor {
    pub original: PathBuf,
    document: Document,
    mode: Mode,
    crop: Crop,
    crop_ratio: usize,
    width: u32,
    height: u32,
    lock_ratio: bool,
    filter: Filter,
    angle: f64,
    view: CanvasView,
    fit_pending: bool,
    last_viewport: Vec2,
    drag: Option<Drag>,
    texture: Option<egui::TextureHandle>,
    texture_revision: Option<u64>,
    preview_job: Option<(u64, Receiver<Result<Pixels, String>>)>,
    edit_job: Option<Receiver<Result<Source, String>>>,
    draft_job: Option<(DraftKey, Receiver<Result<Pixels, String>>)>,
    draft_texture: Option<(DraftKey, egui::TextureHandle)>,
    last_draft: Option<DraftKey>,
    draft_changed: Instant,
    save_job: Option<(u64, Receiver<Result<PathBuf, String>>)>,
    saved_revision: u64,
    saved_path: Option<PathBuf>,
    error: Option<String>,
    confirm_close: bool,
    close_requested: bool,
    pending_mode: Option<Mode>,
    after_apply_mode: Option<Mode>,
    pub checkerboard: bool,
    pub background: Option<[u8; 3]>,
}
fn job<T: Send + 'static>(
    ctx: &egui::Context,
    name: &str,
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<Receiver<Result<T, String>>, String> {
    let (tx, rx) = mpsc::channel();
    let ctx = ctx.clone();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let value = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| Err("图像处理失败，原图未修改，请减小尺寸后重试".into()));
            let _ = tx.send(value);
            ctx.request_repaint();
        })
        .map_err(|e| format!("无法启动处理任务：{e}"))?;
    Ok(rx)
}
fn command(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(label).min_size(Vec2::new(58.0, 30.0)),
    )
}
fn dim(ui: &mut egui::Ui, label: &str, v: &mut u32) -> bool {
    ui.label(label);
    ui.add_sized(
        [66.0, 28.0],
        egui::DragValue::new(v).clamp_range(1..=u32::MAX).speed(1.0),
    )
    .changed()
}
impl Editor {
    pub fn new(ctx: &egui::Context, original: PathBuf, source: Source) -> Result<Self, String> {
        let (width, height) = source.size;
        let mut s = Self {
            original,
            document: Document::new(source),
            mode: Mode::Pan,
            crop: Crop::full(width, height),
            crop_ratio: 0,
            width,
            height,
            lock_ratio: true,
            filter: Filter::Smooth,
            angle: 0.0,
            view: CanvasView::default(),
            fit_pending: true,
            last_viewport: Vec2::ZERO,
            drag: None,
            texture: None,
            texture_revision: None,
            preview_job: None,
            edit_job: None,
            save_job: None,
            saved_revision: 0,
            saved_path: None,
            draft_job: None,
            draft_texture: None,
            last_draft: None,
            draft_changed: Instant::now(),
            error: None,
            confirm_close: false,
            close_requested: false,
            pending_mode: None,
            after_apply_mode: None,
            checkerboard: true,
            background: None,
        };
        s.request_preview(ctx)?;
        Ok(s)
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
    fn busy(&self) -> bool {
        self.edit_job.is_some() || self.save_job.is_some()
    }
    fn size(&self) -> (u32, u32) {
        self.document.current.source.size
    }
    fn pending(&self) -> bool {
        match self.mode {
            Mode::Crop => self.crop != Crop::full(self.size().0, self.size().1),
            Mode::Resize => (self.width, self.height) != self.size(),
            Mode::Rotate => self.angle.abs() > 0.0001,
            Mode::Pan => false,
        }
    }
    fn reset_draft(&mut self) {
        self.crop = Crop::full(self.size().0, self.size().1);
        (self.width, self.height) = self.size();
        self.angle = 0.0;
        self.drag = None;
        self.error = None;
        self.draft_texture = None;
    }
    fn request_mode(&mut self, m: Mode) {
        if m == self.mode {
            return;
        }
        if self.pending() {
            self.pending_mode = Some(m);
        } else {
            self.mode = m;
            self.reset_draft();
        }
    }
    pub fn request_close(&mut self) {
        if self.busy() {
            self.error = Some("正在处理，请完成后再退出编辑".into());
        } else if self.pending() || self.document.current.id != self.saved_revision {
            self.confirm_close = true;
            self.pending_mode = None;
        } else {
            self.close_requested = true;
        }
    }
    pub fn escape(&mut self) {
        if self.confirm_close {
            self.confirm_close = false;
        } else if self.pending_mode.is_some() {
            self.pending_mode = None;
        } else if self.pending() {
            self.reset_draft();
        } else {
            self.request_close();
        }
    }
    fn request_preview(&mut self, ctx: &egui::Context) -> Result<(), String> {
        if self.preview_job.is_some() {
            return Ok(());
        }
        let source = self.document.current.source.clone();
        let id = self.document.current.id;
        let limit = ctx.input(|i| i.max_texture_side).min(4096) as u32;
        self.preview_job = Some((
            id,
            job(ctx, "image-edit-preview", move || source.preview(limit))?,
        ));
        Ok(())
    }
    fn changed(&mut self, ctx: &egui::Context) {
        self.reset_draft();
        self.fit_pending = true;
        self.texture_revision = None;
        if let Err(e) = self.request_preview(ctx) {
            self.error = Some(e);
        }
    }
    fn start_op(&mut self, ctx: &egui::Context, op: Operation) {
        if self.busy() {
            return;
        }
        let source = self.document.current.source.clone();
        match job(ctx, "image-edit-operation", move || {
            edit_ops::apply(&source, op)
        }) {
            Ok(rx) => {
                self.edit_job = Some(rx);
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn apply_draft(&mut self, ctx: &egui::Context) {
        if !self.pending() {
            return;
        }
        match self.mode {
            Mode::Crop => self.start_op(ctx, Operation::Crop(self.crop)),
            Mode::Resize => {
                self.start_op(ctx, Operation::Resize(self.width, self.height, self.filter))
            }
            Mode::Rotate => self.start_op(ctx, Operation::Rotate(self.angle)),
            Mode::Pan => (),
        }
    }
    fn undo(&mut self, ctx: &egui::Context) {
        if self.pending() {
            self.reset_draft();
            return;
        }
        if self.document.undo() {
            self.changed(ctx);
        }
    }
    fn redo(&mut self, ctx: &egui::Context) {
        if !self.pending() && self.document.redo() {
            self.changed(ctx);
        }
    }
    pub fn save_to(&mut self, ctx: &egui::Context, path: PathBuf) {
        if self.save_job.is_some() || self.edit_job.is_some() {
            return;
        }
        if self.pending() {
            self.error = Some("请先应用或取消当前调整，再另存图片".into());
            return;
        }
        let source = self.document.current.source.clone();
        let plan = Plan::full(source.size);
        let original = self.original.clone();
        match job(ctx, "image-edit-save", move || {
            image_edit::save_new_png(&source, plan, &original, &path)
        }) {
            Ok(rx) => {
                self.save_job = Some((self.document.current.id, rx));
                self.error = None;
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn draft_key(&self) -> Option<DraftKey> {
        if !self.pending() || !matches!(self.mode, Mode::Rotate | Mode::Resize) {
            return None;
        }
        Some(DraftKey {
            id: self.document.current.id,
            mode: if self.mode == Mode::Rotate { 1 } else { 2 },
            width: self.width,
            height: self.height,
            angle: self.angle.to_bits(),
            nearest: self.filter == Filter::Nearest,
        })
    }
    fn poll_draft(&mut self, ctx: &egui::Context) {
        let key = self.draft_key();
        if key != self.last_draft {
            self.last_draft = key;
            self.draft_changed = Instant::now();
            self.error = None;
        }
        if let Some((k, rx)) = self.draft_job.take() {
            match rx.try_recv() {
                Ok(Ok(p)) => {
                    if Some(k) == key {
                        self.draft_texture = Some((
                            k,
                            ctx.load_texture(
                                "iv-inline-draft",
                                egui::ColorImage::from_rgba_unmultiplied(
                                    [p.width as usize, p.height as usize],
                                    &p.data,
                                ),
                                egui::TextureOptions::NEAREST,
                            ),
                        ));
                    }
                }
                Ok(Err(e)) => {
                    if Some(k) == key {
                        self.error = Some(e);
                    }
                }
                Err(TryRecvError::Empty) => self.draft_job = Some((k, rx)),
                Err(_) => self.error = Some("调整预览中断".into()),
            }
        }
        if let Some(k) = key {
            let valid = if self.mode == Mode::Rotate {
                edit_ops::rotated_size(self.size(), self.angle).is_ok()
            } else {
                image_edit::pixel_bytes(self.width, self.height).is_ok()
            };
            if valid
                && self.draft_texture.as_ref().map(|(k, _)| *k) != Some(k)
                && self.error.is_none()
            {
                if self.draft_job.is_none()
                    && self.draft_changed.elapsed() >= Duration::from_millis(180)
                {
                    let source = self.document.current.source.clone();
                    let op = if self.mode == Mode::Rotate {
                        Operation::Rotate(self.angle)
                    } else {
                        Operation::Resize(self.width, self.height, self.filter)
                    };
                    let limit = ctx.input(|i| i.max_texture_side).min(4096) as u32;
                    match job(ctx, "image-edit-draft", move || {
                        edit_ops::apply(&source, op)?.preview(limit)
                    }) {
                        Ok(rx) => self.draft_job = Some((k, rx)),
                        Err(e) => self.error = Some(e),
                    }
                }
                ctx.request_repaint_after(Duration::from_millis(35));
            }
        }
    }
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.edit_job.take() {
            match rx.try_recv() {
                Ok(Ok(source)) => {
                    self.document.commit(source);
                    if let Some(m) = self.after_apply_mode.take() {
                        self.mode = m;
                    }
                    self.changed(ctx);
                }
                Ok(Err(e)) => {
                    self.error = Some(e);
                    self.after_apply_mode = None;
                }
                Err(TryRecvError::Empty) => self.edit_job = Some(rx),
                Err(_) => self.error = Some("编辑任务已中断，已保留上一步图像".into()),
            }
        }
        if let Some((id, rx)) = self.preview_job.take() {
            match rx.try_recv() {
                Ok(Ok(p)) => {
                    if id == self.document.current.id {
                        self.texture = Some(ctx.load_texture(
                            "iv-inline-edit",
                            egui::ColorImage::from_rgba_unmultiplied(
                                [p.width as usize, p.height as usize],
                                &p.data,
                            ),
                            egui::TextureOptions::NEAREST,
                        ));
                        self.texture_revision = Some(id);
                    }
                }
                Ok(Err(e)) => self.error = Some(e),
                Err(TryRecvError::Empty) => self.preview_job = Some((id, rx)),
                Err(_) => self.error = Some("预览任务已中断".into()),
            }
        }
        if self.preview_job.is_none()
            && self.texture_revision != Some(self.document.current.id)
            && self.error.is_none()
        {
            if let Err(e) = self.request_preview(ctx) {
                self.error = Some(e);
            }
        }
        if let Some((id, rx)) = self.save_job.take() {
            match rx.try_recv() {
                Ok(Ok(path)) => {
                    self.saved_revision = id;
                    self.saved_path = Some(path);
                    self.error = None;
                }
                Ok(Err(e)) => self.error = Some(e),
                Err(TryRecvError::Empty) => self.save_job = Some((id, rx)),
                Err(_) => self.error = Some("保存任务中断，原图未修改".into()),
            }
        }
        if self.busy() || self.preview_job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }
    }
    fn ratio(&self) -> Option<(u32, u32)> {
        match self.crop_ratio {
            1 => Some(self.size()),
            2 => Some((1, 1)),
            3 => Some((4, 3)),
            4 => Some((3, 4)),
            5 => Some((16, 9)),
            6 => Some((9, 16)),
            _ => None,
        }
    }
    fn preview_size(&self) -> (u32, u32) {
        match self.mode {
            Mode::Resize if image_edit::pixel_bytes(self.width, self.height).is_ok() => {
                (self.width, self.height)
            }
            Mode::Rotate => edit_ops::rotated_size(self.size(), self.angle).unwrap_or(self.size()),
            _ => self.size(),
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, pal: &Palette) -> Action {
        self.poll(ctx);
        let mut action = Action::None;
        let busy = self.busy();
        if !busy
            && !self.confirm_close
            && self.pending_mode.is_none()
            && !ctx.wants_keyboard_input()
            && !ctx.memory(|m| m.any_popup_open())
        {
            let command = ctx.input(|i| i.modifiers.command || i.modifiers.ctrl);
            if command {
                if ctx.input(|i| i.key_pressed(egui::Key::Z)) {
                    if ctx.input(|i| i.modifiers.shift) {
                        self.redo(ctx);
                    } else {
                        self.undo(ctx);
                    }
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Y)) {
                    self.redo(ctx);
                }
                if ctx.input(|i| i.key_pressed(egui::Key::S)) && !self.pending() {
                    action = Action::Save;
                }
            } else {
                for (k, m) in [
                    (egui::Key::V, Mode::Pan),
                    (egui::Key::C, Mode::Crop),
                    (egui::Key::R, Mode::Rotate),
                    (egui::Key::I, Mode::Resize),
                ] {
                    if ctx.input(|i| i.key_pressed(k)) {
                        self.request_mode(m);
                    }
                }
                if ctx.input(|i| i.key_pressed(egui::Key::F)) {
                    self.fit_pending = true;
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Num0)) {
                    self.view.scale = 1.0;
                    self.view.center(
                        self.preview_size(),
                        [self.last_viewport.x, self.last_viewport.y],
                    );
                    self.fit_pending = false;
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.apply_draft(ctx);
                }
                if self.mode == Mode::Crop {
                    let step = if ctx.input(|i| i.modifiers.shift) {
                        10
                    } else {
                        1
                    };
                    let mut delta = [0, 0];
                    for (k, n, v) in [
                        (egui::Key::ArrowLeft, 0, -step),
                        (egui::Key::ArrowRight, 0, step),
                        (egui::Key::ArrowUp, 1, -step),
                        (egui::Key::ArrowDown, 1, step),
                    ] {
                        if ctx.input(|i| i.key_pressed(k)) {
                            delta[n] += v;
                        }
                    }
                    if delta != [0, 0] {
                        self.crop = self.crop.translated(delta[0], delta[1], self.size());
                    }
                }
            }
        }
        let frame = egui::Frame::none()
            .fill(pal.overlay)
            .inner_margin(egui::Margin::symmetric(14.0, 8.0));
        egui::TopBottomPanel::top("iv-inline-edit-tools")
            .frame(frame)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 7.0);
                ui.horizontal(|ui| {
                    if command(ui, "返回看图", !busy)
                        .on_hover_text("退出编辑；未保存的内容会先确认")
                        .clicked()
                    {
                        self.request_close();
                    }
                    ui.separator();
                    ui.add_enabled_ui(
                        !busy && !self.confirm_close && self.pending_mode.is_none(),
                        |ui| {
                            for (m, name) in [
                                (Mode::Pan, "移动"),
                                (Mode::Crop, "裁剪"),
                                (Mode::Rotate, "旋转 / 翻转"),
                                (Mode::Resize, "分辨率"),
                            ] {
                                if ui
                                    .add_sized(
                                        [if m == Mode::Rotate { 106.0 } else { 62.0 }, 30.0],
                                        egui::SelectableLabel::new(self.mode == m, name),
                                    )
                                    .clicked()
                                {
                                    self.request_mode(m);
                                }
                            }
                        },
                    );
                    if ui.available_width() > 360.0 {
                        let (_, drag) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width() - 340.0, 30.0),
                            Sense::click_and_drag(),
                        );
                        if drag.drag_started() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        if ui::icon_btn(ui, if max { Icon::Restore } else { Icon::Max }, false, pal)
                            .on_hover_text("最大化 / 还原窗口")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!max));
                        }
                        if ui::icon_btn(ui, Icon::Min, false, pal)
                            .on_hover_text("最小化")
                            .clicked()
                        {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        if command(
                            ui,
                            "重置全部",
                            !busy
                                && !self.confirm_close
                                && (self.document.current.id != 0 || self.pending()),
                        )
                        .clicked()
                        {
                            self.document.reset();
                            self.changed(ctx);
                        }
                        if command(
                            ui,
                            "重做",
                            !busy && self.document.can_redo() && !self.pending(),
                        )
                        .on_hover_text("Ctrl+Y / Ctrl+Shift+Z")
                        .clicked()
                        {
                            self.redo(ctx);
                        }
                        if command(
                            ui,
                            "撤销",
                            !busy && (self.document.can_undo() || self.pending()),
                        )
                        .on_hover_text("Ctrl+Z")
                        .clicked()
                        {
                            self.undo(ctx);
                        }
                    });
                });
                ui.add_enabled_ui(
                    !busy && !self.confirm_close && self.pending_mode.is_none(),
                    |ui| {
                        ui.horizontal_wrapped(|ui| match self.mode {
                            Mode::Pan => {
                                ui.label(
                                    egui::RichText::new("滚轮缩放 · 左键拖动图片").color(pal.text),
                                );
                                ui.label(
                                    egui::RichText::new(
                                        "裁剪时按住空格拖动画布；F 适配，0 实际大小",
                                    )
                                    .size(12.0)
                                    .color(pal.dim),
                                );
                            }
                            Mode::Crop => {
                                let before = self.crop;
                                let mut c = self.crop;
                                ui.label("X");
                                ui.add_sized(
                                    [54.0, 28.0],
                                    egui::DragValue::new(&mut c.x)
                                        .clamp_range(0..=self.size().0 - 1),
                                );
                                ui.label("Y");
                                ui.add_sized(
                                    [54.0, 28.0],
                                    egui::DragValue::new(&mut c.y)
                                        .clamp_range(0..=self.size().1 - 1),
                                );
                                c.w = c.w.min(self.size().0 - c.x);
                                c.h = c.h.min(self.size().1 - c.y);
                                let cw = dim(ui, "宽", &mut c.w);
                                let ch = dim(ui, "高", &mut c.h);
                                if let Some((rw, rh)) = self.ratio() {
                                    if cw {
                                        c.h = Plan::paired_dimension(c.w, rh, rw);
                                    } else if ch {
                                        c.w = Plan::paired_dimension(c.h, rw, rh);
                                    }
                                }
                                c.w = c.w.min(self.size().0 - c.x).max(1);
                                c.h = c.h.min(self.size().1 - c.y).max(1);
                                if c != before {
                                    self.crop = c;
                                }
                                let old = self.crop_ratio;
                                egui::ComboBox::from_id_source("iv-crop-ratio")
                                    .width(88.0)
                                    .selected_text(
                                        [
                                            "自由比例",
                                            "原图比例",
                                            "1:1",
                                            "4:3",
                                            "3:4",
                                            "16:9",
                                            "9:16",
                                        ][self.crop_ratio],
                                    )
                                    .show_ui(ui, |ui| {
                                        for (i, n) in [
                                            "自由比例",
                                            "原图比例",
                                            "1:1",
                                            "4:3",
                                            "3:4",
                                            "16:9",
                                            "9:16",
                                        ]
                                        .into_iter()
                                        .enumerate()
                                        {
                                            ui.selectable_value(&mut self.crop_ratio, i, n);
                                        }
                                    });
                                if old != self.crop_ratio {
                                    if let Some(r) = self.ratio() {
                                        self.crop = Crop::centered_ratio(self.size(), r);
                                    }
                                }
                                if command(ui, "全选", true).clicked() {
                                    self.reset_draft();
                                }
                                if command(ui, "应用裁剪", self.pending()).clicked() {
                                    self.apply_draft(ctx);
                                }
                                if command(ui, "取消", self.pending()).clicked() {
                                    self.reset_draft();
                                }
                            }
                            Mode::Rotate => {
                                if command(ui, "左转 90°", !self.pending()).clicked() {
                                    self.start_op(ctx, Operation::Quarter(-1));
                                }
                                if command(ui, "右转 90°", !self.pending()).clicked() {
                                    self.start_op(ctx, Operation::Quarter(1));
                                }
                                if command(ui, "水平翻转", !self.pending()).clicked() {
                                    self.start_op(ctx, Operation::FlipHorizontal);
                                }
                                if command(ui, "垂直翻转", !self.pending()).clicked() {
                                    self.start_op(ctx, Operation::FlipVertical);
                                }
                                ui.separator();
                                ui.label("角度");
                                ui.add_sized(
                                    [64.0, 28.0],
                                    egui::DragValue::new(&mut self.angle)
                                        .clamp_range(-180.0..=180.0)
                                        .speed(0.2)
                                        .suffix("°"),
                                );
                                if command(ui, "应用旋转", self.pending()).clicked() {
                                    self.apply_draft(ctx);
                                }
                                if command(ui, "取消", self.pending()).clicked() {
                                    self.reset_draft();
                                }
                            }
                            Mode::Resize => {
                                if dim(ui, "宽", &mut self.width) && self.lock_ratio {
                                    self.height = Plan::paired_dimension(
                                        self.width,
                                        self.size().1,
                                        self.size().0,
                                    );
                                }
                                if dim(ui, "高", &mut self.height) && self.lock_ratio {
                                    self.width = Plan::paired_dimension(
                                        self.height,
                                        self.size().0,
                                        self.size().1,
                                    );
                                }
                                ui.label("px");
                                if ui.checkbox(&mut self.lock_ratio, "锁定比例").changed()
                                    && self.lock_ratio
                                {
                                    self.height = Plan::paired_dimension(
                                        self.width,
                                        self.size().1,
                                        self.size().0,
                                    );
                                }
                                for (name, n, d) in [("50%", 1, 2), ("100%", 1, 1), ("200%", 2, 1)]
                                {
                                    if ui.button(name).clicked() {
                                        self.width = (self.size().0 * n / d).max(1);
                                        self.height = (self.size().1 * n / d).max(1);
                                    }
                                }
                                egui::ComboBox::from_id_source("iv-inline-filter")
                                    .width(122.0)
                                    .selected_text(if self.filter == Filter::Smooth {
                                        "平滑（双线性）"
                                    } else {
                                        "最近邻（像素图）"
                                    })
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut self.filter,
                                            Filter::Smooth,
                                            "平滑（双线性）",
                                        );
                                        ui.selectable_value(
                                            &mut self.filter,
                                            Filter::Nearest,
                                            "最近邻（像素图）",
                                        );
                                    });
                                if command(
                                    ui,
                                    "应用尺寸",
                                    self.pending()
                                        && image_edit::pixel_bytes(self.width, self.height).is_ok(),
                                )
                                .clicked()
                                {
                                    self.apply_draft(ctx);
                                }
                                if command(ui, "取消", self.pending()).clicked() {
                                    self.reset_draft();
                                }
                            }
                        });
                    },
                );
            });
        egui::TopBottomPanel::bottom("iv-inline-edit-status")
            .frame(
                egui::Frame::none()
                    .fill(pal.overlay)
                    .inner_margin(egui::Margin::symmetric(14.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(7.0, 6.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} × {} px", self.size().0, self.size().1))
                            .strong(),
                    );
                    ui.separator();
                    if ui
                        .button("−")
                        .on_hover_text("缩小视图，不改变像素尺寸")
                        .clicked()
                    {
                        self.view.zoom(
                            [self.last_viewport.x * 0.5, self.last_viewport.y * 0.5],
                            0.8,
                        );
                        self.fit_pending = false;
                    }
                    ui.label(format!("{:.0}%", self.view.scale * 100.0));
                    if ui
                        .button("＋")
                        .on_hover_text("放大视图，不改变像素尺寸")
                        .clicked()
                    {
                        self.view.zoom(
                            [self.last_viewport.x * 0.5, self.last_viewport.y * 0.5],
                            1.25,
                        );
                        self.fit_pending = false;
                    }
                    if ui.button("适配").clicked() {
                        self.fit_pending = true;
                    }
                    if ui.button("1:1").clicked() {
                        self.view.scale = 1.0;
                        self.view.center(
                            self.preview_size(),
                            [self.last_viewport.x, self.last_viewport.y],
                        );
                        self.fit_pending = false;
                    }
                    ui.checkbox(&mut self.checkerboard, "透明网格");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if command(
                            ui,
                            "另存为 PNG…",
                            !busy
                                && !self.pending()
                                && !self.confirm_close
                                && self.pending_mode.is_none(),
                        )
                        .on_hover_text("Ctrl+S；仅保存新文件，不覆盖原图")
                        .clicked()
                        {
                            action = Action::Save;
                        }
                        if let Some(path) = &self.saved_path {
                            if command(
                                ui,
                                "打开结果",
                                !busy
                                    && !self.pending()
                                    && self.saved_revision == self.document.current.id,
                            )
                            .clicked()
                            {
                                action = Action::Open(path.clone());
                            }
                        }
                        if busy {
                            ui.spinner();
                        }
                    });
                });
                if self.confirm_close {
                    ui.horizontal(|ui| {
                        ui.label("退出并放弃未保存的编辑？");
                        if command(ui, "放弃编辑", !busy).clicked() {
                            self.close_requested = true;
                        }
                        if command(ui, "继续编辑", true).clicked() {
                            self.confirm_close = false;
                        }
                    });
                } else if let Some(mode) = self.pending_mode {
                    ui.horizontal(|ui| {
                        ui.label("当前调整尚未应用");
                        if command(ui, "应用并切换", !busy).clicked() {
                            self.after_apply_mode = Some(mode);
                            self.pending_mode = None;
                            self.apply_draft(ctx);
                        }
                        if command(ui, "丢弃调整", !busy).clicked() {
                            self.reset_draft();
                            self.mode = mode;
                            self.pending_mode = None;
                        }
                        if command(ui, "继续调整", !busy).clicked() {
                            self.pending_mode = None;
                        }
                    });
                } else {
                    let invalid = if self.mode == Mode::Resize {
                        image_edit::pixel_bytes(self.width, self.height).err()
                    } else if self.mode == Mode::Rotate {
                        edit_ops::rotated_size(self.size(), self.angle).err()
                    } else {
                        None
                    };
                    let text = if let Some(e) = invalid.as_ref().or(self.error.as_ref()) {
                        e.clone()
                    } else if self.save_job.is_some() {
                        "正在另存… 原图保持不变".into()
                    } else if self.edit_job.is_some() {
                        "正在应用编辑…".into()
                    } else if self.pending() {
                        match self.mode {
                            Mode::Crop => format!(
                                "选区 {} × {} px · 拖动八个手柄或框选；Enter 应用，Esc 取消",
                                self.crop.w, self.crop.h
                            ),
                            Mode::Rotate => format!(
                                "预览 {:.1}° · 输出 {} × {} px，空白保留透明；Enter 应用",
                                self.angle,
                                self.preview_size().0,
                                self.preview_size().1
                            ),
                            _ => format!(
                                "目标 {} × {} px · 这是修改像素尺寸；Enter 应用，Esc 取消",
                                self.width, self.height
                            ),
                        }
                    } else if let Some(p) = &self.saved_path {
                        if self.document.current.id == self.saved_revision {
                            format!(
                                "已保存：{}",
                                p.file_name().unwrap_or_default().to_string_lossy()
                            )
                        } else {
                            "存在未保存编辑 · Ctrl+Z 撤销 · 另存不会覆盖原图".into()
                        }
                    } else {
                        self.document.current.source.note.clone()
                    };
                    let text = if self.size().0.max(self.size().1) > 4096 {
                        format!("{text} · 预览长边≤4096，导出为完整像素")
                    } else {
                        text
                    };
                    ui.add(
                        egui::Label::new(egui::RichText::new(&text).size(11.5).color(pal.dim))
                            .truncate(true),
                    )
                    .on_hover_text(text);
                }
            });
        self.poll_draft(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.canvas))
            .show(ctx, |ui| {
                self.draw_canvas(
                    ui,
                    pal,
                    !busy && !self.confirm_close && self.pending_mode.is_none(),
                );
            });
        if self.close_requested {
            Action::Close
        } else {
            action
        }
    }
    fn draw_canvas(&mut self, ui: &mut egui::Ui, pal: &Palette, enabled: bool) {
        let viewport = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(viewport, Sense::click_and_drag());
        let p = ui.painter().with_clip_rect(viewport);
        let size = self.preview_size();
        if self.last_viewport != viewport.size() {
            self.fit_pending = true;
            self.last_viewport = viewport.size();
        }
        if self.fit_pending {
            self.view.fit(size, [viewport.width(), viewport.height()]);
            self.fit_pending = false;
        }
        p.rect_filled(
            viewport,
            0.0,
            self.background
                .map(|rgb| Color32::from_rgb(rgb[0], rgb[1], rgb[2]))
                .unwrap_or(pal.canvas),
        );
        if self.checkerboard {
            let tile = 16.0;
            for y in 0..(viewport.height() / tile).ceil() as u32 {
                for x in 0..(viewport.width() / tile).ceil() as u32 {
                    if (x + y) % 2 == 0 {
                        p.rect_filled(
                            Rect::from_min_size(
                                viewport.min + Vec2::new(x as f32 * tile, y as f32 * tile),
                                Vec2::splat(tile),
                            ),
                            0.0,
                            pal.btn_hover,
                        );
                    }
                }
            }
        }
        let pointer = response
            .interact_pointer_pos()
            .or_else(|| ui.input(|i| i.pointer.hover_pos()));
        if enabled && response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            let zoom = ui.input(|i| i.zoom_delta());
            if let Some(pos) = pointer {
                if scroll.abs() > 0.5 || zoom != 1.0 {
                    self.view.zoom(
                        [pos.x - viewport.min.x, pos.y - viewport.min.y],
                        (scroll / 400.0).exp() * zoom,
                    );
                }
            }
        }
        let rect = Rect::from_min_size(
            viewport.min + Vec2::from(self.view.offset),
            Vec2::new(size.0 as f32, size.1 as f32) * self.view.scale,
        );
        if self.texture_revision != Some(self.document.current.id) {
            p.text(
                viewport.center(),
                egui::Align2::CENTER_CENTER,
                "正在更新图像…",
                egui::FontId::proportional(15.0),
                pal.text,
            );
            return;
        }
        let draft = self
            .draft_texture
            .as_ref()
            .filter(|(key, _)| Some(*key) == self.draft_key());
        if let Some(tex) = draft.map(|(_, tex)| tex).or(self.texture.as_ref()) {
            if draft.is_none() && self.mode == Mode::Rotate && self.pending() {
                let angle = self.angle.to_radians() as f32;
                let (s, c) = angle.sin_cos();
                let source_size =
                    Vec2::new(self.size().0 as f32, self.size().1 as f32) * self.view.scale;
                let mut mesh = egui::epaint::Mesh::with_texture(tex.id());
                for uv in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
                    let v = Vec2::new((uv[0] - 0.5) * source_size.x, (uv[1] - 0.5) * source_size.y);
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: rect.center() + Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y),
                        uv: Pos2::new(uv[0], uv[1]),
                        color: Color32::WHITE,
                    });
                }
                mesh.indices = vec![0, 1, 2, 0, 2, 3];
                p.add(egui::Shape::mesh(mesh));
            } else {
                p.image(
                    tex.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            p.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, pal.border));
        }
        let image_point = |pos: Pos2| {
            [
                (pos.x - rect.left()) / self.view.scale,
                (pos.y - rect.top()) / self.view.scale,
            ]
        };
        let c = self.crop;
        let selected = Rect::from_min_size(
            rect.min + Vec2::new(c.x as f32, c.y as f32) * self.view.scale,
            Vec2::new(c.w as f32, c.h as f32) * self.view.scale,
        );
        let handles = [
            selected.left_top(),
            selected.center_top(),
            selected.right_top(),
            selected.right_center(),
            selected.right_bottom(),
            selected.center_bottom(),
            selected.left_bottom(),
            selected.left_center(),
        ];
        let hit = pointer.and_then(|pos| handles.iter().position(|pt| pt.distance(pos) <= 9.0));
        let pan = ui.input(|i| {
            i.key_down(egui::Key::Space) || i.pointer.middle_down() || i.pointer.secondary_down()
        }) || self.mode != Mode::Crop;
        if enabled && response.drag_started() {
            if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                self.drag = if pan {
                    Some(Drag::Pan {
                        start: pos,
                        offset: self.view.offset,
                    })
                } else if let Some(index) = handles.iter().position(|pt| pt.distance(pos) <= 9.0) {
                    Some(Drag::Handle { index, crop: c })
                } else if rect.contains(pos) {
                    if selected.contains(pos) && c != Crop::full(self.size().0, self.size().1) {
                        Some(Drag::Move {
                            start: image_point(pos),
                            crop: c,
                        })
                    } else {
                        Some(Drag::New(image_point(pos)))
                    }
                } else {
                    None
                };
            }
        }
        if enabled && response.dragged() {
            if let (Some(drag), Some(pos)) = (self.drag, pointer) {
                match drag {
                    Drag::Pan { start, offset } => {
                        self.view.offset =
                            [offset[0] + pos.x - start.x, offset[1] + pos.y - start.y];
                    }
                    Drag::Move { start, crop } => {
                        let b = image_point(pos);
                        self.crop = crop.translated(
                            (b[0] - start[0]).round() as i64,
                            (b[1] - start[1]).round() as i64,
                            self.size(),
                        );
                    }
                    Drag::Handle { index, crop } => {
                        self.crop = edit_ops::resize_crop(
                            crop,
                            index,
                            image_point(pos),
                            self.size(),
                            self.ratio(),
                        );
                    }
                    Drag::New(a) => {
                        let b = image_point(pos);
                        let mut crop = Crop::from_points(a, b, self.size());
                        if let Some(r) = self.ratio() {
                            let fit = Crop::centered_ratio((crop.w, crop.h), r);
                            crop.w = fit.w;
                            crop.h = fit.h;
                            if b[0] < a[0] {
                                crop.x = (a[0].clamp(crop.w as f32, self.size().0 as f32) as u32)
                                    - crop.w;
                            }
                            if b[1] < a[1] {
                                crop.y = (a[1].clamp(crop.h as f32, self.size().1 as f32) as u32)
                                    - crop.h;
                            }
                        }
                        self.crop = crop;
                    }
                }
            }
        }
        if response.drag_stopped() {
            self.drag = None;
        }
        if self.mode == Mode::Crop {
            let c = self.crop;
            let sel = Rect::from_min_size(
                rect.min + Vec2::new(c.x as f32, c.y as f32) * self.view.scale,
                Vec2::new(c.w as f32, c.h as f32) * self.view.scale,
            );
            for mask in [
                Rect::from_min_max(rect.min, Pos2::new(rect.right(), sel.top())),
                Rect::from_min_max(Pos2::new(rect.left(), sel.bottom()), rect.max),
                Rect::from_min_max(Pos2::new(rect.left(), sel.top()), sel.left_bottom()),
                Rect::from_min_max(sel.right_top(), Pos2::new(rect.right(), sel.bottom())),
            ] {
                p.rect_filled(mask, 0.0, Color32::from_black_alpha(150));
            }
            p.rect_stroke(sel, 0.0, Stroke::new(1.5_f32, pal.accent));
            for n in [1.0, 2.0] {
                let x = sel.left() + sel.width() * n / 3.0;
                let y = sel.top() + sel.height() * n / 3.0;
                p.line_segment(
                    [Pos2::new(x, sel.top()), Pos2::new(x, sel.bottom())],
                    Stroke::new(0.8_f32, Color32::from_white_alpha(140)),
                );
                p.line_segment(
                    [Pos2::new(sel.left(), y), Pos2::new(sel.right(), y)],
                    Stroke::new(0.8_f32, Color32::from_white_alpha(140)),
                );
            }
            for pt in [
                sel.left_top(),
                sel.center_top(),
                sel.right_top(),
                sel.right_center(),
                sel.right_bottom(),
                sel.center_bottom(),
                sel.left_bottom(),
                sel.left_center(),
            ] {
                p.rect_filled(
                    Rect::from_center_size(pt, Vec2::splat(8.0)),
                    2.0,
                    Color32::WHITE,
                );
                p.rect_stroke(
                    Rect::from_center_size(pt, Vec2::splat(8.0)),
                    2.0,
                    Stroke::new(1.0_f32, pal.accent),
                );
            }
        }
        if enabled && response.hovered() {
            ui.ctx().set_cursor_icon(if pan {
                if self.drag.is_some() {
                    egui::CursorIcon::Grabbing
                } else {
                    egui::CursorIcon::Grab
                }
            } else if let Some(h) = hit {
                match h {
                    0 | 4 => egui::CursorIcon::ResizeNwSe,
                    2 | 6 => egui::CursorIcon::ResizeNeSw,
                    1 | 5 => egui::CursorIcon::ResizeVertical,
                    _ => egui::CursorIcon::ResizeHorizontal,
                }
            } else if pointer.is_some_and(|pos| selected.contains(pos)) && self.pending() {
                egui::CursorIcon::Move
            } else {
                egui::CursorIcon::Crosshair
            });
        }
    }
}
