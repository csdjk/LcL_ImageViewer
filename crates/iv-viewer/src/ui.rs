//! 主题与通用 UI 组件：深/浅双主题配色、字体（系统字体加载）、矢量图标、
//! 悬浮胶囊容器、图标按钮、徽章、空状态占位。

use eframe::egui;
use eframe::egui::{
    Align2, Color32, FontFamily, FontId, Pos2, Rect, RichText, Rounding, Sense, Stroke, TextStyle,
    Vec2,
};

/* ================================ 主题 ================================ */

/// 深色 / 浅色主题。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    fn id() -> egui::Id {
        egui::Id::new("iv-theme-mode")
    }

    /// 当前主题（未保存偏好时默认浅色）。
    pub fn current(ctx: &egui::Context) -> Self {
        ctx.data_mut(|d| *d.get_temp_mut_or(Self::id(), ThemeMode::Light))
    }

    /// 应用主题：重设 visuals，并同步 Windows 系统标题栏明暗。
    pub fn apply_to(ctx: &egui::Context, mode: ThemeMode) {
        ctx.data_mut(|d| d.insert_temp(Self::id(), mode));
        apply(ctx);
        ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(match mode {
            ThemeMode::Dark => egui::SystemTheme::Dark,
            ThemeMode::Light => egui::SystemTheme::Light,
        }));
    }

    /// 切换主题，返回切换后的主题。
    pub fn toggle(ctx: &egui::Context) -> ThemeMode {
        let next = match Self::current(ctx) {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
        };
        Self::apply_to(ctx, next);
        next
    }
}

/// 主题配色板。用 `palette(ctx)` 取当前主题。
#[derive(Clone, Copy)]
pub struct Palette {
    pub is_dark: bool,
    /// 窗口/面板底色
    pub bg: Color32,
    /// 不透明条/窗口底色（属性窗口等）
    pub bar: Color32,
    /// 画布底色
    pub canvas: Color32,
    /// 主文字
    pub text: Color32,
    /// 高亮文字（悬停态）
    pub text_bright: Color32,
    /// 次要文字
    pub dim: Color32,
    /// 三级文字（提示/标签）
    pub faint: Color32,
    /// 分隔线/边框
    pub border: Color32,
    /// 强调色（选中/激活）
    pub accent: Color32,
    /// 强调色实底上的文字/图标色
    pub selected_text: Color32,
    /// 新拟态悬浮面板底色（不透明）
    pub overlay: Color32,
    /// 图标常态色
    pub icon: Color32,
    /// 图标按钮悬停底
    pub btn_hover: Color32,
    /// 图标按钮按下底
    pub btn_pressed: Color32,
    /// 常规控件（下拉框/滑条）底色
    pub w_bg: Color32,
    /// 常规控件悬停底
    pub w_hover: Color32,
    /// extreme_bg（文本输入等最深控件底）
    pub extreme: Color32,
    pub err_bg: Color32,
    pub err_border: Color32,
    pub err_text: Color32,
    /// 悬浮胶囊投影
    pub shadow: egui::epaint::Shadow,
    /// 窗口不透明底色；字段名保留以减少调用端变化
    pub glass_window: Color32,
    /// 同色系柔影：光源固定在左上方。
    pub shadow_light: Color32,
    pub shadow_dark: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// 新拟态共享尺寸（egui 逻辑点，随 DPI 缩放）。
pub const CONTROL_RADIUS: f32 = 12.0;
pub const CARD_RADIUS: f32 = 14.0;
pub const PANEL_RADIUS: f32 = 16.0;

/// 当前主题配色板：低饱和雾蓝灰，同色相的深浅变化，不给原图叠色。
pub fn palette(ctx: &egui::Context) -> Palette {
    match ThemeMode::current(ctx) {
        ThemeMode::Dark => Palette {
            is_dark: true,
            bg: rgb(0x25, 0x2e, 0x3d),
            bar: rgb(0x2c, 0x37, 0x48),
            canvas: rgb(0x25, 0x2e, 0x3d),
            text: rgb(0xe3, 0xea, 0xf4),
            text_bright: rgb(0xf5, 0xf8, 0xff),
            dim: rgb(0xbc, 0xc9, 0xdc),
            faint: rgb(0xa2, 0xb2, 0xc9),
            border: Color32::TRANSPARENT,
            accent: rgb(0xb1, 0xc7, 0xe6),
            selected_text: rgb(0xe0, 0xeb, 0xfa),
            overlay: rgb(0x2c, 0x37, 0x48),
            icon: rgb(0xc5, 0xd2, 0xe5),
            btn_hover: rgb(0x32, 0x3f, 0x52),
            btn_pressed: rgb(0x23, 0x2f, 0x42),
            w_bg: rgb(0x2e, 0x3a, 0x4d),
            w_hover: rgb(0x35, 0x43, 0x58),
            extreme: rgb(0x21, 0x2b, 0x3b),
            err_bg: rgb(0x3c, 0x32, 0x3a),
            err_border: Color32::TRANSPARENT,
            err_text: rgb(0xf0, 0xbd, 0xc4),
            shadow: egui::epaint::Shadow {
                offset: Vec2::new(5.0, 6.0),
                blur: 20.0,
                spread: 0.0,
                color: alpha(rgb(0x12, 0x1a, 0x27), 160),
            },
            glass_window: rgb(0x2c, 0x37, 0x48),
            shadow_light: rgb(0x4e, 0x60, 0x7b),
            shadow_dark: rgb(0x10, 0x18, 0x26),
        },
        ThemeMode::Light => Palette {
            is_dark: false,
            bg: rgb(0xe6, 0xeb, 0xf2),
            bar: rgb(0xe6, 0xeb, 0xf2),
            canvas: rgb(0xe6, 0xeb, 0xf2),
            text: rgb(0x34, 0x44, 0x5b),
            text_bright: rgb(0x25, 0x37, 0x50),
            dim: rgb(0x53, 0x65, 0x7b),
            faint: rgb(0x56, 0x69, 0x80),
            border: Color32::TRANSPARENT,
            accent: rgb(0x49, 0x65, 0x8e),
            selected_text: rgb(0x35, 0x51, 0x79),
            overlay: rgb(0xe6, 0xeb, 0xf2),
            icon: rgb(0x4b, 0x60, 0x7b),
            btn_hover: rgb(0xec, 0xf1, 0xf8),
            btn_pressed: rgb(0xd3, 0xde, 0xed),
            w_bg: rgb(0xe6, 0xeb, 0xf2),
            w_hover: rgb(0xee, 0xf3, 0xf9),
            extreme: rgb(0xd9, 0xe2, 0xee),
            err_bg: rgb(0xf0, 0xe2, 0xe6),
            err_border: Color32::TRANSPARENT,
            err_text: rgb(0x8b, 0x44, 0x58),
            shadow: egui::epaint::Shadow {
                offset: Vec2::new(5.0, 6.0),
                blur: 20.0,
                spread: 0.0,
                color: alpha(rgb(0x9a, 0xac, 0xc5), 105),
            },
            glass_window: rgb(0xe6, 0xeb, 0xf2),
            shadow_light: Color32::WHITE,
            shadow_dark: rgb(0x9a, 0xac, 0xc5),
        },
    }
}

/// 应用主题：配色 + 系统字体 + 字号。主题切换时会再次调用。
pub fn apply(ctx: &egui::Context) {
    let pal = palette(ctx);
    let mut v = if pal.is_dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = pal.bg;
    v.window_fill = pal.glass_window;
    v.extreme_bg_color = pal.extreme;
    v.faint_bg_color = pal.bar;
    v.selection.bg_fill = pal.btn_pressed;
    v.selection.stroke = Stroke::new(1.0_f32, pal.selected_text);
    v.hyperlink_color = pal.accent;
    for widget in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
        &mut v.widgets.noninteractive,
    ] {
        widget.bg_stroke = Stroke::NONE;
        widget.rounding = Rounding::same(CONTROL_RADIUS);
        widget.expansion = 0.0;
    }
    v.widgets.inactive.bg_fill = pal.w_bg;
    v.widgets.inactive.weak_bg_fill = pal.w_bg;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, pal.text);
    v.widgets.hovered.bg_fill = pal.w_hover;
    v.widgets.hovered.weak_bg_fill = pal.w_hover;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, pal.text_bright);
    v.widgets.active.bg_fill = pal.btn_pressed;
    v.widgets.active.weak_bg_fill = pal.btn_pressed;
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, pal.selected_text);
    v.widgets.open.bg_fill = pal.extreme;
    v.widgets.open.weak_bg_fill = pal.extreme;
    v.widgets.open.fg_stroke = Stroke::new(1.0_f32, pal.selected_text);
    v.widgets.noninteractive.bg_fill = pal.extreme;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.dim);
    v.window_rounding = Rounding::same(PANEL_RADIUS);
    v.menu_rounding = Rounding::same(CARD_RADIUS);
    v.popup_shadow = pal.shadow;
    v.window_shadow = pal.shadow;
    v.window_stroke = Stroke::NONE;

    // 可交互元素使用手型指针
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    ctx.set_visuals(v);

    install_fonts(ctx);

    ctx.style_mut(|s| {
        s.text_styles.insert(
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        );
        s.text_styles
            .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        s.text_styles.insert(
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        );
        s.text_styles.insert(
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        );
        s.text_styles.insert(
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        );
    });
}

/* ============================== 悬浮胶囊 ============================== */

fn alpha(color: Color32, a: u8) -> Color32 {
    // egui-wgpu 0.27 的首选非 sRGB 交换链在 gamma 空间混合。
    // 必须同步缩放 RGB/Alpha，避免线性预乘的彩色阴影在此后端变成发光描边。
    color.gamma_multiply(a as f32 / 255.0)
}

/// 圆角轮廓及外法线；内阴影逐层向内衰减，不用硬描边模拟凹陷。
fn rounded_outline(rect: Rect, radius: f32) -> Vec<(Pos2, Vec2)> {
    let radius = radius
        .min(rect.width() * 0.5)
        .min(rect.height() * 0.5)
        .max(0.0);
    let centers = [
        Pos2::new(rect.left() + radius, rect.top() + radius),
        Pos2::new(rect.right() - radius, rect.top() + radius),
        Pos2::new(rect.right() - radius, rect.bottom() - radius),
        Pos2::new(rect.left() + radius, rect.bottom() - radius),
    ];
    let mut points = Vec::with_capacity(36);
    for (corner, center) in centers.into_iter().enumerate() {
        for step in 0..=8 {
            let angle = std::f32::consts::PI
                + (corner as f32 + step as f32 / 8.0) * std::f32::consts::FRAC_PI_2;
            let normal = Vec2::new(angle.cos(), angle.sin());
            points.push((center + normal * radius, normal));
        }
    }
    points
}

fn inset_mesh(rect: Rect, radius: f32, pal: &Palette) -> egui::Mesh {
    const RINGS: usize = 7;
    const POINTS: usize = 36;
    let depth = 4.5_f32.min(rect.width() * 0.2).min(rect.height() * 0.2);
    let mut mesh = egui::Mesh::default();
    for ring in 0..RINGS {
        let t = ring as f32 / (RINGS - 1) as f32;
        let inset = depth * t;
        for (point, normal) in rounded_outline(rect.shrink(inset), (radius - inset).max(0.0)) {
            let direction = (normal.x + normal.y) * std::f32::consts::FRAC_1_SQRT_2;
            let (color, strength) = if direction < 0.0 {
                (pal.shadow_dark, if pal.is_dark { 155.0 } else { 110.0 })
            } else {
                (pal.shadow_light, if pal.is_dark { 60.0 } else { 150.0 })
            };
            let a = (direction.abs() * strength * (1.0 - t).powi(2)) as u8;
            mesh.colored_vertex(point, alpha(color, a));
        }
    }
    for ring in 0..RINGS - 1 {
        for i in 0..POINTS {
            let next = (i + 1) % POINTS;
            let a = (ring * POINTS + i) as u32;
            let b = (ring * POINTS + next) as u32;
            let c = ((ring + 1) * POINTS + i) as u32;
            let d = ((ring + 1) * POINTS + next) as u32;
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(b, d, c);
        }
    }
    mesh
}

/// 多层新拟态表面。凸起为左上柔光 + 右下柔影 + 接触阴影；凹陷为圆角内阴影。
fn surface_shapes(
    rect: Rect,
    radius: f32,
    fill: Color32,
    inset: bool,
    elevation: f32,
    pal: &Palette,
) -> egui::Shape {
    let rounding = Rounding::same(radius);
    let mut shapes = Vec::with_capacity(5);
    if !inset && elevation > 0.0 {
        for (offset, blur, color) in [
            (
                Vec2::splat(-2.5 * elevation),
                7.0 * elevation,
                alpha(pal.shadow_light, if pal.is_dark { 45 } else { 115 }),
            ),
            (
                Vec2::new(3.0, 3.5) * elevation,
                9.0 * elevation,
                alpha(pal.shadow_dark, if pal.is_dark { 145 } else { 110 }),
            ),
            (
                Vec2::new(0.5, 1.0),
                3.0,
                alpha(pal.shadow_dark, if pal.is_dark { 45 } else { 28 }),
            ),
        ] {
            let shadow = egui::epaint::Shadow {
                offset,
                blur,
                spread: 0.0,
                color,
            };
            shapes.push(egui::Shape::mesh(shadow.tessellate(rect, rounding)));
        }
    }
    shapes.push(egui::Shape::rect_filled(rect, rounding, fill));
    if inset {
        shapes.push(egui::Shape::mesh(inset_mesh(rect, radius, pal)));
    }
    egui::Shape::Vec(shapes)
}

/// 保留 Frame 的布局/响应合同，把全部阴影放在内容之前，防止覆盖文字。
pub struct NeuFrame {
    frame: egui::Frame,
    pal: Palette,
    radius: f32,
    elevation: f32,
}

impl NeuFrame {
    pub fn show<R>(
        self,
        ui: &mut egui::Ui,
        add: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let background = ui.painter().add(egui::Shape::Noop);
        let margin = self.frame.outer_margin;
        let response = self.frame.show(ui, add);
        ui.painter().set(
            background,
            surface_shapes(
                margin.shrink_rect(response.response.rect),
                self.radius,
                self.pal.overlay,
                false,
                self.elevation,
                &self.pal,
            ),
        );
        response
    }
}

pub fn capsule(pal: &Palette) -> NeuFrame {
    NeuFrame {
        frame: egui::Frame::default().inner_margin(egui::Margin::symmetric(12.0, 8.0)),
        pal: *pal,
        radius: PANEL_RADIUS,
        elevation: 1.6,
    }
}

/// 仅以留白区分功能组，不画硬分隔线；保持旧布局的宽度预算。
pub fn sep(ui: &mut egui::Ui, _pal: &Palette) {
    ui.allocate_exact_size(Vec2::new(6.0, 16.0), Sense::hover());
}

pub fn menu_frame(pal: &Palette) -> NeuFrame {
    NeuFrame {
        frame: egui::Frame::default().inner_margin(egui::Margin::symmetric(8.0, 8.0)),
        pal: *pal,
        radius: PANEL_RADIUS,
        elevation: 1.8,
    }
}

fn paint_focus(painter: &egui::Painter, rect: Rect, pal: &Palette) {
    // 焦点是唯一保留的语义轮廓：低强度外晕 + 清晰细线，而非装饰边框。
    painter.rect_stroke(
        rect.expand(1.0),
        CONTROL_RADIUS,
        Stroke::new(4.0_f32, alpha(pal.accent, 26)),
    );
    painter.rect_stroke(
        rect.expand(0.5),
        CONTROL_RADIUS,
        Stroke::new(1.0_f32, pal.accent),
    );
}

fn button_face(ui: &egui::Ui, resp: &egui::Response, selected: bool, pal: &Palette) -> Color32 {
    let enabled = ui.is_enabled();
    let pressed = enabled && resp.is_pointer_button_down_on();
    let hovering = enabled && resp.hovered();
    let fill = if selected || pressed {
        pal.btn_pressed
    } else if hovering {
        pal.btn_hover
    } else {
        pal.w_bg
    };
    ui.painter().add(surface_shapes(
        resp.rect.shrink(2.0),
        CONTROL_RADIUS,
        fill,
        selected || pressed,
        if !enabled {
            0.0
        } else if hovering {
            1.05
        } else {
            0.7
        },
        pal,
    ));
    if enabled && resp.has_focus() {
        paint_focus(ui.painter(), resp.rect.shrink(0.75), pal);
    }
    if !enabled {
        pal.faint
    } else if selected {
        pal.selected_text
    } else if hovering {
        pal.text_bright
    } else {
        pal.icon
    }
}

/// 右键菜单的整行操作项，快捷键固定右对齐，避免短文本形成零散小胶囊。
pub fn menu_item(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &str,
    selected: bool,
    pal: &Palette,
) -> egui::Response {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    let fg = if selected || resp.hovered() || resp.is_pointer_button_down_on() {
        button_face(ui, &resp, selected, pal)
    } else {
        if resp.has_focus() {
            paint_focus(ui.painter(), rect.shrink(1.0), pal);
        }
        pal.text
    };
    ui.painter().text(
        Pos2::new(rect.left() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(13.5, FontFamily::Proportional),
        fg,
    );
    if !shortcut.is_empty() {
        ui.painter().text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            shortcut,
            FontId::new(12.0, FontFamily::Monospace),
            if selected { pal.selected_text } else { pal.dim },
        );
    }
    resp
}

/* ============================= 设置页组件 ============================= */

/// 设置页只保留操作列表，不再嵌套分类卡片。
pub const SETTINGS_CONTENT_WIDTH: f32 = 360.0;
pub const SETTINGS_ROW_HEIGHT: f32 = 40.0;
pub const SETTINGS_HEADER_HEIGHT: f32 = 40.0;

/// 标题拖动区与关闭按钮严格分开，二者之间保留8点安全间隔。
/// 限制弹窗到客户区内；极小窗口也保证左上标题与关闭区域有可访问的位置。
pub fn clamp_settings_popup(rect: Rect, screen: Rect) -> Pos2 {
    let bounds = screen.shrink(8.0);
    Pos2::new(
        rect.left().clamp(bounds.left(), (bounds.right() - rect.width()).max(bounds.left())),
        rect.top().clamp(bounds.top(), (bounds.bottom() - rect.height()).max(bounds.top())),
    )
}

pub fn settings_header_rects(row: Rect) -> [Rect; 2] {
    let close = Rect::from_center_size(
        Pos2::new(row.right() - 16.0, row.center().y), Vec2::splat(32.0));
    let drag = Rect::from_min_max(row.min, Pos2::new(close.left() - 8.0, row.bottom()));
    [drag, close]
}

pub fn settings_header(ui: &mut egui::Ui, pal: &Palette) -> (egui::Response, bool) {
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), SETTINGS_HEADER_HEIGHT), Sense::hover());
    let [drag, close] = settings_header_rects(row);
    let response = ui.interact(drag, ui.id().with("settings-title-drag"), Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::Grab)
        .on_hover_text("按住左键或右键移动设置弹窗（主窗口不动）");
    ui.painter().text(drag.left_center(), Align2::LEFT_CENTER, "设置",
        FontId::new(18.0, FontFamily::Proportional), pal.text_bright);
    let closed = ui.allocate_ui_at_rect(close, |ui| {
        icon_btn(ui, Icon::Close, false, pal).on_hover_text("关闭设置 (Esc)").clicked()
    }).inner;
    (response, closed)
}

/// 紧凑设置行：固定行高，说明改成悬停提示，操作区统一右对齐。
pub fn setting_row(
    ui: &mut egui::Ui,
    pal: &Palette,
    label: &str,
    desc: &str,
    widget: impl FnOnce(&mut egui::Ui),
) {
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), SETTINGS_ROW_HEIGHT),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(RichText::new(label).color(pal.text).size(13.5)).on_hover_text(desc);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), widget);
        },
    );
}

/// 固定32点滑条行；直接分配轨道宽度，避免嵌套水平布局挤掉轨道。
pub fn compact_slider_row(
    ui: &mut egui::Ui,
    pal: &Palette,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) -> bool {
    let width = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(width, 32.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (label_rect, _) = ui.allocate_exact_size(Vec2::new(76.0, 24.0), Sense::hover());
            ui.painter().text(label_rect.left_center(), Align2::LEFT_CENTER, label,
                FontId::new(13.0, FontFamily::Proportional), pal.dim);
            ui.spacing_mut().slider_width = (width - 76.0 - 40.0 - 16.0).max(80.0);
            let response = ui.add(egui::Slider::new(value, range).show_value(false).trailing_fill(true));
            let (value_rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 24.0), Sense::hover());
            ui.painter().text(value_rect.right_center(), Align2::RIGHT_CENTER,
                format!("{:.0}%", *value * 100.0),
                FontId::new(12.0, FontFamily::Monospace), pal.dim);
            response.changed()
        },
    ).inner
}

/// 凹陷轨道 + 凸起滑块；状态同时通过位置和明暗表达。
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, pal: &Palette) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(44.0, 26.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        p.add(surface_shapes(
            rect,
            CONTROL_RADIUS,
            if *on { pal.btn_pressed } else { pal.extreme },
            true,
            0.0,
            pal,
        ));
        let cx = if *on {
            rect.right() - 13.0
        } else {
            rect.left() + 13.0
        };
        let knob = Rect::from_center_size(Pos2::new(cx, rect.center().y), Vec2::splat(18.0));
        p.add(surface_shapes(
            knob,
            9.0,
            if resp.hovered() {
                pal.btn_hover
            } else {
                pal.bar
            },
            false,
            0.65,
            pal,
        ));
        if *on {
            p.circle_filled(knob.center(), 2.0, pal.accent);
        }
        if resp.has_focus() {
            paint_focus(p, rect.expand(1.0), pal);
        }
    }
    resp
}

/* ================================ 图标 ================================ */

/// 手绘矢量图标集（16×16 基准，随按钮尺寸缩放）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Open,
    Subfolders,
    Prev,
    Next,
    Fit,
    Actual,
    Bounds,
    Play,
    Pause,
    Grid,
    Sun,
    Moon,
    Close,
    Settings,
    /// 窗口最小化（无边框自绘标题栏）
    Min,
    /// 窗口最大化
    Max,
    /// 窗口还原（最大化后）
    Restore,
}

/// 在 rect 内绘制图标。
pub fn paint_icon(p: &egui::Painter, icon: Icon, rect: Rect, color: Color32) {
    let c = rect.center();
    let u = rect.width().min(rect.height()) / 16.0; // 以 16px 为设计基准
    let st = Stroke::new(1.5 * u, color);
    match icon {
        Icon::Open => {
            // 文件夹：闭合轮廓 + 标签页凸起
            let l = c.x - 7.0 * u;
            let r = c.x + 7.0 * u;
            let t = c.y - 5.0 * u;
            let b = c.y + 5.0 * u;
            let tab = 4.0 * u;
            let pts = vec![
                Pos2::new(l, b),
                Pos2::new(l, t + 1.0 * u),
                Pos2::new(l + tab, t + 1.0 * u),
                Pos2::new(l + tab + 1.5 * u, t + 3.0 * u),
                Pos2::new(r, t + 3.0 * u),
                Pos2::new(r, b),
            ];
            p.add(egui::Shape::closed_line(pts, st));
        }
        Icon::Subfolders => {
            // 分支目录图标：区别于打开文件的文件夹轮廓。
            p.rect_stroke(Rect::from_center_size(c + Vec2::new(-3.0, -4.5) * u, Vec2::new(5.0, 4.0) * u), 0.7_f32, st);
            p.line_segment([c + Vec2::new(-3.0, -2.5) * u, c + Vec2::new(-3.0, 5.0) * u], st);
            for y in [0.0_f32, 5.0] {
                p.line_segment([c + Vec2::new(-3.0, y) * u, c + Vec2::new(2.0, y) * u], st);
                p.rect_stroke(Rect::from_center_size(c + Vec2::new(4.5, y) * u, Vec2::new(5.0, 3.5) * u), 0.7_f32, st);
            }
        }
        Icon::Prev | Icon::Next => {
            // 尖括号
            let dir = if icon == Icon::Prev { -1.0 } else { 1.0 };
            p.add(egui::Shape::line(
                vec![
                    Pos2::new(c.x - dir * 3.0 * u, c.y - 4.5 * u),
                    Pos2::new(c.x + dir * 3.0 * u, c.y),
                    Pos2::new(c.x - dir * 3.0 * u, c.y + 4.5 * u),
                ],
                st,
            ));
        }
        Icon::Fit => {
            // 四角括号（适配窗口）
            let s = rect.shrink(1.5 * u);
            let k = 3.6 * u;
            for (x, y, dx, dy) in [
                (s.left(), s.top(), 1.0, 1.0),
                (s.right(), s.top(), -1.0, 1.0),
                (s.left(), s.bottom(), 1.0, -1.0),
                (s.right(), s.bottom(), -1.0, -1.0),
            ] {
                p.add(egui::Shape::line(
                    vec![
                        Pos2::new(x + k * dx, y),
                        Pos2::new(x, y),
                        Pos2::new(x, y + k * dy),
                    ],
                    st,
                ));
            }
        }
        Icon::Bounds => {
            let b = rect.shrink(3.0 * u);
            p.rect_stroke(b, 0.0, Stroke::new(1.0 * u, color));
            for corner in [b.left_top(), b.right_top(), b.left_bottom(), b.right_bottom()] {
                p.rect_filled(Rect::from_center_size(corner, Vec2::splat(3.2 * u)), 0.0, color);
            }
        }
        Icon::Actual => {
            // 方框 + 中心点（实际大小 1:1）
            p.rect_stroke(rect.shrink(3.0 * u), Rounding::same(1.5 * u), st);
            p.circle_filled(c, 1.8 * u, color);
        }
        Icon::Play => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x - 2.8 * u, c.y - 4.5 * u),
                    Pos2::new(c.x + 4.5 * u, c.y),
                    Pos2::new(c.x - 2.8 * u, c.y + 4.5 * u),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            let w = 2.6 * u;
            let h = 9.0 * u;
            for dx in [-3.4 * u, 0.8 * u] {
                p.rect_filled(
                    Rect::from_min_size(Pos2::new(c.x + dx, c.y - h / 2.0), Vec2::new(w, h)),
                    Rounding::same(0.8 * u),
                    color,
                );
            }
        }
        Icon::Grid => {
            // 2×2 像素块（最近邻采样）
            let s = 4.6 * u;
            let half = (s + 1.8 * u) / 2.0;
            for (dx, dy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let min = Pos2::new(c.x + dx * half - s / 2.0, c.y + dy * half - s / 2.0);
                p.rect_filled(
                    Rect::from_min_size(min, Vec2::splat(s)),
                    Rounding::same(1.0 * u),
                    color,
                );
            }
        }
        Icon::Sun => {
            p.circle_stroke(c, 3.4 * u, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                p.line_segment(
                    [
                        Pos2::new(c.x + a.cos() * 4.8 * u, c.y + a.sin() * 4.8 * u),
                        Pos2::new(c.x + a.cos() * 6.8 * u, c.y + a.sin() * 6.8 * u),
                    ],
                    st,
                );
            }
        }
        Icon::Moon => {
            // ◐ 半月：外圆描边 + 半圆填充
            let r = 5.2 * u;
            p.circle_stroke(c, r, st);
            let mut pts = Vec::with_capacity(17);
            for i in 0..=16 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 16.0;
                pts.push(Pos2::new(c.x + a.cos() * r, c.y + a.sin() * r));
            }
            p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
        }
        Icon::Close => {
            let k = 4.2 * u;
            p.line_segment(
                [Pos2::new(c.x - k, c.y - k), Pos2::new(c.x + k, c.y + k)],
                st,
            );
            p.line_segment(
                [Pos2::new(c.x - k, c.y + k), Pos2::new(c.x + k, c.y - k)],
                st,
            );
        }
        Icon::Min => {
            // 底部一条横线
            let k = 4.2 * u;
            p.line_segment(
                [
                    Pos2::new(c.x - k, c.y + 4.0 * u),
                    Pos2::new(c.x + k, c.y + 4.0 * u),
                ],
                st,
            );
        }
        Icon::Max => {
            // 单个方框
            let s = 4.2 * u;
            p.rect_stroke(
                Rect::from_center_size(c, Vec2::splat(s * 2.0)),
                Rounding::same(1.0 * u),
                st,
            );
        }
        Icon::Restore => {
            // 前后两个方框（后窗偏右上，前窗偏左下）
            let s = 3.1 * u;
            let d = 1.8 * u;
            let back = Rect::from_center_size(c + Vec2::new(d, -d), Vec2::splat(s * 2.0));
            let front = Rect::from_center_size(c + Vec2::new(-d, d), Vec2::splat(s * 2.0));
            p.rect_stroke(back, Rounding::same(0.8 * u), st);
            p.rect_stroke(front, Rounding::same(0.8 * u), st);
        }
        Icon::Settings => {
            // 齿轮：外圈 8 齿 + 中心圆孔
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                p.line_segment(
                    [
                        Pos2::new(c.x + a.cos() * 4.6 * u, c.y + a.sin() * 4.6 * u),
                        Pos2::new(c.x + a.cos() * 6.6 * u, c.y + a.sin() * 6.6 * u),
                    ],
                    Stroke::new(2.2 * u, color),
                );
            }
            p.circle_stroke(c, 4.2 * u, st);
        }
    }
}

/// 工具栏文本：固定 32pt 高（与操作区同高），galley 垂直居中绘制。
/// 避免不同字号文本直接 ui.label 时因 rect 高度不一、中心对齐后基线参差不齐。
pub fn bar_label(
    ui: &mut egui::Ui,
    text: impl Into<String>,
    size: f32,
    color: Color32,
) -> egui::Response {
    let font = FontId::new(size, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(text.into(), font, color));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x, 32.0), Sense::hover());
    let gp = Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(gp, galley, color);
    resp
}

/// 等宽字体的工具栏文本（数字读数），同 bar_label 但用 Monospace 族。
pub fn bar_label_mono(
    ui: &mut egui::Ui,
    text: impl Into<String>,
    size: f32,
    color: Color32,
) -> egui::Response {
    let font = FontId::new(size, FontFamily::Monospace);
    let galley = ui.fonts(|f| f.layout_no_wrap(text.into(), font, color));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x, 32.0), Sense::hover());
    let gp = Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(gp, galley, color);
    resp
}

/// 按实际字体测宽的文件名槽：中间省略并优先保留扩展名。
pub fn filename_label(
    ui: &mut egui::Ui,
    text: &str,
    max_width: f32,
    color: Color32,
) -> egui::Response {
    let font = FontId::new(13.5, FontFamily::Proportional);
    let measure = |s: &str| {
        ui.fonts(|f| f.layout_no_wrap(s.to_owned(), font.clone(), color))
            .size()
            .x
    };
    let shown = if measure(text) <= max_width {
        text.to_owned()
    } else {
        let (stem, suffix) = text
            .rsplit_once('.')
            .map_or((text, String::new()), |(stem, ext)| {
                (stem, format!(".{ext}"))
            });
        let chars: Vec<char> = stem.chars().collect();
        let mut low = 0usize;
        let mut high = chars.len();
        while low < high {
            let mid = (low + high).div_ceil(2);
            let left = mid.div_ceil(2);
            let right = mid / 2;
            let candidate = format!(
                "{}…{}{}",
                chars[..left].iter().collect::<String>(),
                chars[chars.len() - right..].iter().collect::<String>(),
                suffix
            );
            if measure(&candidate) <= max_width {
                low = mid;
            } else {
                high = mid - 1;
            }
        }
        let left = low.div_ceil(2);
        let right = low / 2;
        format!(
            "{}…{}{}",
            chars[..left].iter().collect::<String>(),
            chars[chars.len() - right..].iter().collect::<String>(),
            suffix
        )
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(shown, font, color));
    let (rect, resp) = ui.allocate_exact_size(
        Vec2::new(max_width, 32.0),
        Sense::focusable_noninteractive(),
    );
    ui.painter().galley(
        Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    resp
}

/* ============================== 图标按钮 ============================== */

/// 两侧导航的安全边距避开6点系统缩放热区，矩形尺寸与命中区一致。
pub const SIDE_NAV_SIZE: Vec2 = Vec2::new(48.0, 64.0);
pub const SIDE_NAV_RADIUS: f32 = 16.0;
pub const SIDE_NAV_GAP: f32 = 18.0;

pub fn side_navigation_rects(screen: Rect) -> [Rect; 2] {
    let y = screen.center().y;
    [
        Rect::from_center_size(
            Pos2::new(screen.left() + SIDE_NAV_GAP + SIDE_NAV_SIZE.x / 2.0, y),
            SIDE_NAV_SIZE,
        ),
        Rect::from_center_size(
            Pos2::new(screen.right() - SIDE_NAV_GAP - SIDE_NAV_SIZE.x / 2.0, y),
            SIDE_NAV_SIZE,
        ),
    ]
}

fn navigation_tint(pal: &Palette, hovered: bool, pressed: bool, enabled: bool) -> Color32 {
    let opacity = if pressed {
        194
    } else if hovered {
        172
    } else {
        142
    };
    let color = if pal.is_dark {
        rgb(0x23, 0x2e, 0x40)
    } else {
        rgb(0xf0, 0xf5, 0xfc)
    };
    alpha(color, if enabled { opacity } else { 112 })
}

/// 只画半透明玻璃调色、高光和图标；背后场景模糊由app注册圆角区域交给GPU。
pub fn glass_navigation_button(
    ui: &mut egui::Ui,
    icon: Icon,
    enabled: bool,
    pal: &Palette,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        SIDE_NAV_SIZE,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let hovering = enabled && response.hovered();
        let pressed = enabled && response.is_pointer_button_down_on();
        let rounding = Rounding::same(SIDE_NAV_RADIUS);
        let shadow = egui::epaint::Shadow {
            offset: Vec2::new(0.0, if pressed { 1.0 } else { 3.0 }),
            blur: if hovering { 18.0 } else { 14.0 },
            spread: 0.0,
            color: Color32::from_black_alpha(if pal.is_dark { 72 } else { 32 }),
        };
        p.add(egui::Shape::mesh(shadow.tessellate(rect, rounding)));
        p.rect_filled(
            rect,
            rounding,
            navigation_tint(pal, hovering, pressed, enabled),
        );
        // 沿圆角衰减的上缘柔光，不添加厚重外圈或霓虹辉光。
        let mut sheen = egui::Mesh::default();
        sheen.colored_vertex(rect.center(), Color32::TRANSPARENT);
        let outline = rounded_outline(rect.shrink(0.7), SIDE_NAV_RADIUS - 0.7);
        for (point, normal) in &outline {
            let amount = ((-normal.x - normal.y) * 0.5).max(0.0);
            sheen.colored_vertex(
                *point,
                alpha(
                    Color32::WHITE,
                    (amount * if pal.is_dark { 18.0 } else { 32.0 }) as u8,
                ),
            );
        }
        for i in 0..outline.len() {
            sheen.add_triangle(0, (i + 1) as u32, ((i + 1) % outline.len() + 1) as u32);
        }
        p.add(egui::Shape::mesh(sheen));
        p.rect_stroke(
            rect.shrink(0.5),
            rounding,
            Stroke::new(
                0.75_f32,
                alpha(Color32::WHITE, if pal.is_dark { 38 } else { 100 }),
            ),
        );
        let fg = if enabled {
            pal.text_bright
        } else {
            alpha(pal.icon, 95)
        };
        let center = rect.center()
            + if pressed {
                Vec2::new(0.0, 1.0)
            } else {
                Vec2::ZERO
            };
        paint_icon(
            p,
            icon,
            Rect::from_center_size(center, Vec2::splat(21.0)),
            fg,
        );
        if enabled && response.has_focus() {
            p.rect_stroke(
                rect.expand(2.0),
                rounding,
                Stroke::new(1.25_f32, pal.accent),
            );
        }
    }
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

/// 32×32 图标操作区：默认、悬停、按下、选中、焦点和禁用状态互不混淆。
pub fn icon_btn(ui: &mut egui::Ui, icon: Icon, active: bool, pal: &Palette) -> egui::Response {
    icon_btn_impl(ui, icon, active, false, pal)
}

pub fn icon_btn_danger(ui: &mut egui::Ui, icon: Icon, pal: &Palette) -> egui::Response {
    icon_btn_impl(ui, icon, false, true, pal)
}

fn icon_btn_impl(
    ui: &mut egui::Ui,
    icon: Icon,
    active: bool,
    danger: bool,
    pal: &Palette,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(32.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let mut fg = button_face(ui, &resp, active, pal);
        if danger && ui.is_enabled() && (resp.hovered() || resp.is_pointer_button_down_on()) {
            fg = pal.err_text;
        }
        paint_icon(
            ui.painter(),
            icon,
            Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
            fg,
        );
    }
    resp
}

/// 通道/主题选项：相同占位，选中时压入表面，不再使用高饱和彩色实底。
pub fn segment_button(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    pal: &Palette,
) -> egui::Response {
    let font = FontId::new(12.0, FontFamily::Proportional);
    let width = ui
        .fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), pal.text))
        .size()
        .x
        + 14.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width.max(30.0), 32.0), Sense::click());
    let color = button_face(ui, &resp, selected, pal);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font, color));
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, color);
    resp
}

/// 设置/检查器文字按钮，共享图标按钮的材质与交互状态。
pub fn text_button(
    ui: &mut egui::Ui,
    label: &str,
    min_width: f32,
    pal: &Palette,
) -> egui::Response {
    let font = FontId::new(13.0, FontFamily::Proportional);
    let width = ui
        .fonts(|f| f.layout_no_wrap(label.to_owned(), font.clone(), pal.text))
        .size()
        .x
        + 24.0;
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(width.max(min_width), 32.0), Sense::click());
    let color = button_face(ui, &resp, false, pal);
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font, color));
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, color);
    resp
}

/* ================================ 徽章 ================================ */

/// 格式信息统一单色系；只使用文本语义区分，不借助无关彩色标签。
pub fn badge(ui: &mut egui::Ui, text: &str, pal: &Palette) -> egui::Response {
    let font = FontId::new(11.0, FontFamily::Proportional);
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_owned(), font, pal.dim));
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(galley.size().x + 14.0, 32.0), Sense::hover());
    let pill = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), 24.0));
    ui.painter().add(surface_shapes(
        pill,
        CONTROL_RADIUS,
        pal.extreme,
        true,
        0.0,
        pal,
    ));
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, pal.dim);
    resp
}

/* ============================== 空状态占位 ============================== */

/// 画布底色：极轻的同色系渐变，仅在图像外和空状态可见。
pub fn paint_canvas_bg(p: &egui::Painter, rect: Rect, pal: &Palette) {
    let (c1, c2) = canvas_gradient(pal);
    let mut mesh = egui::Mesh::default();
    // epaint 0.27 的 colored_vertex 无返回值，顶点索引即推入顺序
    mesh.colored_vertex(rect.left_top(), c1);
    mesh.colored_vertex(rect.right_top(), c1);
    mesh.colored_vertex(rect.right_bottom(), c2);
    mesh.colored_vertex(rect.left_bottom(), c2);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
}

/// 回退画布渐变色；CPU 画布与 GPU 统一场景必须共享同一来源。
pub fn canvas_gradient(pal: &Palette) -> (Color32, Color32) {
    if pal.is_dark {
        (rgb(0x28, 0x33, 0x44), rgb(0x22, 0x2b, 0x39))
    } else {
        (rgb(0xe9, 0xee, 0xf5), rgb(0xe1, 0xe7, 0xf0))
    }
}

/// 手绘"图片"矢量图标（圆角相框 + 太阳 + 山），用于空状态。
fn paint_image_icon(painter: &egui::Painter, rect: Rect, color: Color32) {
    let s = Stroke::new(2.5f32, color);
    let r = rect.shrink(8.0);
    painter.rect_stroke(r, Rounding::same(12.0), s);
    // 太阳
    painter.circle_stroke(
        egui::Pos2::new(r.left() + r.width() * 0.3, r.top() + r.height() * 0.3),
        r.width() * 0.07,
        s,
    );
    // 山（两段折线）
    let base = r.bottom() - r.height() * 0.22;
    let m1 = egui::Pos2::new(r.left() + r.width() * 0.22, base);
    let peak1 = egui::Pos2::new(r.left() + r.width() * 0.45, r.top() + r.height() * 0.52);
    let valley = egui::Pos2::new(r.left() + r.width() * 0.62, r.bottom() - r.height() * 0.30);
    let peak2 = egui::Pos2::new(r.left() + r.width() * 0.80, r.top() + r.height() * 0.40);
    let m2 = egui::Pos2::new(r.right() - r.width() * 0.10, base);
    painter.add(egui::Shape::line(vec![m1, peak1, valley, peak2, m2], s));
}

/// 画布空状态 / 加载中占位（居中，Painter 直接绘制）。
/// `loading` 为 Some(文件名) 时显示解码中动画。
pub fn draw_placeholder(painter: &egui::Painter, rect: Rect, loading: Option<&str>, pal: &Palette) {
    let cx = rect.center().x;
    let font = |size: f32| egui::FontId::new(size, FontFamily::Proportional);

    if let Some(name) = loading {
        // 加载中：点阵旋转 spinner + 文件名
        let top = rect.center().y - 40.0;
        let t = painter.ctx().input(|i| i.time) as f32;
        let n = 8u32;
        let step = t * 6.0; // 每秒跳 6 个点
        let active = (step as u32) % n;
        for i in 0..n {
            let a = i as f32 * (std::f32::consts::TAU / n as f32) - std::f32::consts::FRAC_PI_2;
            let p = egui::Pos2::new(cx + a.cos() * 11.0, top + a.sin() * 11.0);
            let bright = i == active;
            // 未点亮的按距离高亮点渐暗，形成拖尾
            let dist = (i + n - active) % n;
            let alpha = if bright {
                1.0
            } else {
                (1.0 - dist as f32 / n as f32) * 0.55
            };
            let color = if alpha >= 1.0 {
                pal.accent
            } else {
                Color32::from_rgba_unmultiplied(
                    pal.accent.r(),
                    pal.accent.g(),
                    pal.accent.b(),
                    (alpha * 255.0) as u8,
                )
            };
            painter.circle_filled(p, 2.4, color);
        }
        painter.text(
            egui::Pos2::new(cx, top + 26.0),
            egui::Align2::CENTER_TOP,
            format!("正在解码 {name} …"),
            font(16.0),
            pal.text,
        );
        return;
    }

    // 空状态：图标 + 主副文案 + 支持格式
    let top = rect.center().y - 130.0;
    let tile = Rect::from_center_size(Pos2::new(cx, top + 48.0), Vec2::splat(96.0));
    painter.add(surface_shapes(tile, PANEL_RADIUS, pal.bar, false, 2.0, pal));
    paint_image_icon(painter, tile.shrink(17.0), pal.icon);
    let mut y = top + 128.0;
    painter.text(
        egui::Pos2::new(cx, y),
        egui::Align2::CENTER_TOP,
        "将图片拖拽到此处",
        font(20.0),
        pal.text,
    );
    y += 38.0;
    painter.text(
        egui::Pos2::new(cx, y),
        egui::Align2::CENTER_TOP,
        "Ctrl + O 打开文件 · ← → 浏览目录 · 滚轮缩放 · T 切换主题",
        font(13.0),
        pal.dim,
    );
    y += 30.0;
    painter.text(
        egui::Pos2::new(cx, y),
        egui::Align2::CENTER_TOP,
        "PNG · JPG · WebP · GIF · TGA · DDS (BC1–BC7) · PSD · HDR · QOI · PNM",
        font(12.0),
        pal.faint,
    );
}

/* ================================ 字体 ================================ */

/// 加载 Windows 系统字体：Segoe UI（UI/数字）+ 微软雅黑（中文）+ Consolas（等宽）。
/// 任一字体缺失时跳过，保持 egui 默认。
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut changed = false;

    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/segoeui.ttf") {
        fonts
            .font_data
            .insert("segoe_ui".into(), egui::FontData::from_owned(bytes));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "segoe_ui".into());
        changed = true;
    }
    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/consola.ttf") {
        fonts
            .font_data
            .insert("consolas".into(), egui::FontData::from_owned(bytes));
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "consolas".into());
        changed = true;
    }
    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/msyh.ttc") {
        let mut fd = egui::FontData::from_owned(bytes);
        fd.index = 0; // ttc 第 0 个：微软雅黑常规
        fonts.font_data.insert("msyh".into(), fd);
        // 追加到两个族末尾：前面的字体缺中文字形时回落到雅黑
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("msyh".into());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("msyh".into());
        changed = true;
    }
    if changed {
        ctx.set_fonts(fonts);
    }
}

#[cfg(test)]
mod neumorphic_tests {
    use super::*;

    fn contrast(a: Color32, b: Color32) -> f32 {
        let luminance = |c: Color32| {
            let linear = |v: u8| {
                let x = v as f32 / 255.0;
                if x <= 0.04045 {
                    x / 12.92
                } else {
                    ((x + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
        };
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn fresh_theme_is_light() {
        assert!(ThemeMode::current(&egui::Context::default()) == ThemeMode::Light);
    }

    #[test]
    fn panels_are_opaque_and_text_remains_readable_in_both_themes() {
        let ctx = egui::Context::default();
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            ThemeMode::apply_to(&ctx, theme);
            let pal = palette(&ctx);
            assert_eq!(pal.overlay.a(), 255);
            assert_eq!(pal.glass_window.a(), 255);
            assert_eq!(ctx.style().visuals.window_stroke, Stroke::NONE);
            for fg in [pal.text, pal.dim, pal.faint] {
                assert!(
                    contrast(fg, pal.bar) >= 4.5,
                    "contrast {}",
                    contrast(fg, pal.bar)
                );
            }
            assert!(contrast(pal.selected_text, pal.btn_pressed) >= 4.5);
        }
    }

    #[test]
    fn colored_shadows_darken_in_the_gamma_framebuffer() {
        let ctx = egui::Context::default();
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            ThemeMode::apply_to(&ctx, theme);
            let pal = palette(&ctx);
            for opacity in [32, 110, 180] {
                let shadow = alpha(pal.shadow_dark, opacity);
                for (src, dst) in shadow.to_array()[..3].iter().zip(&pal.bar.to_array()[..3]) {
                    let blended = *src as f32 + *dst as f32 * (1.0 - shadow.a() as f32 / 255.0);
                    assert!(blended <= *dst as f32 + 1.0);
                }
            }
        }
    }

    #[test]
    fn surface_radii_match_the_style_contract() {
        for radius in [CONTROL_RADIUS, CARD_RADIUS, PANEL_RADIUS] {
            assert!((12.0..=16.0).contains(&radius));
        }
    }

    #[test]
    fn inset_shadow_mesh_is_bounded_and_fades_to_transparent() {
        let pal = palette(&egui::Context::default());
        for size in [
            Vec2::new(28.0, 28.0),
            Vec2::new(44.0, 26.0),
            Vec2::new(220.0, 32.0),
        ] {
            let rect = Rect::from_min_size(Pos2::ZERO, size);
            let mesh = inset_mesh(rect, CONTROL_RADIUS, &pal);
            assert_eq!(mesh.vertices.len(), 7 * 36);
            for vertex in &mesh.vertices {
                assert!(vertex.pos.x.is_finite() && vertex.pos.y.is_finite());
                assert!(rect.expand(0.001).contains(vertex.pos));
            }
            assert!(mesh
                .indices
                .iter()
                .all(|i| (*i as usize) < mesh.vertices.len()));
            assert!(mesh
                .vertices
                .iter()
                .rev()
                .take(36)
                .all(|v| v.color.a() == 0));
        }
    }
}

#[cfg(test)]
mod side_navigation_tests {
    use super::*;

    #[test]
    fn side_buttons_remain_symmetric_and_inside_resize_hotzone() {
        for size in [
            Vec2::new(880.0, 560.0),
            Vec2::new(1280.0, 860.0),
            Vec2::new(1920.0, 1080.0),
        ] {
            let screen = Rect::from_min_size(Pos2::new(17.0, 29.0), size);
            let [left, right] = side_navigation_rects(screen);
            assert_eq!(left.size(), SIDE_NAV_SIZE);
            assert_eq!(right.size(), SIDE_NAV_SIZE);
            assert_eq!(left.center().y, screen.center().y);
            assert_eq!(right.center().y, screen.center().y);
            assert_eq!(left.left() - screen.left(), SIDE_NAV_GAP);
            assert_eq!(screen.right() - right.right(), SIDE_NAV_GAP);
            assert!(screen.shrink(6.0).contains_rect(left));
            assert!(screen.shrink(6.0).contains_rect(right));
        }
    }

    #[test]
    fn navigation_is_translucent_in_both_themes_and_all_states() {
        let ctx = egui::Context::default();
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            ThemeMode::apply_to(&ctx, mode);
            let pal = palette(&ctx);
            for (hovered, pressed, enabled) in [
                (false, false, true),
                (true, false, true),
                (true, true, true),
                (false, false, false),
            ] {
                let tint = navigation_tint(&pal, hovered, pressed, enabled);
                assert!((96..220).contains(&tint.a()));
                assert!(tint.r() <= tint.a() && tint.g() <= tint.a() && tint.b() <= tint.a());
            }
            assert_eq!(pal.overlay.a(), 255); // new translucency must not leak into other panels
        }
    }
}

#[cfg(test)]
mod settings_layout_tests {
    use super::*;

    #[test]
    fn title_drag_region_never_overlaps_close_button() {
        for width in [280.0, 320.0, SETTINGS_CONTENT_WIDTH, 480.0] {
            let row = Rect::from_min_size(Pos2::new(16.0, 20.0), Vec2::new(width, SETTINGS_HEADER_HEIGHT));
            let [drag, close] = settings_header_rects(row);
            assert!(row.contains_rect(drag) && row.contains_rect(close));
            assert!(!drag.intersects(close));
            assert_eq!(close.left() - drag.right(), 8.0);
            assert_eq!(close.size(), Vec2::splat(32.0));
            assert_eq!(drag.height(), SETTINGS_HEADER_HEIGHT);
        }
    }

    #[test]
    fn settings_controls_fit_the_minimum_viewport() {
        let frame_margin = 32.0;
        let fixed_height = SETTINGS_HEADER_HEIGHT + 4.0 + 8.0 + 20.0 + frame_margin;
        let expanded_height = fixed_height + 6.0 * SETTINGS_ROW_HEIGHT + 3.0 * 32.0;
        assert!(expanded_height < 560.0 - 48.0);
        assert!(SETTINGS_CONTENT_WIDTH + frame_margin < 880.0 - 48.0);
    }
}

#[cfg(test)]
mod settings_popup_drag_tests {
    use super::*;

    #[test]
    fn popup_moves_in_client_coordinates() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(880.0, 560.0));
        let popup = Rect::from_min_size(Pos2::new(244.0, 113.0), Vec2::new(392.0, 334.0));
        assert_eq!(clamp_settings_popup(popup.translate(Vec2::new(72.0, 36.0)), screen), Pos2::new(316.0, 149.0));
    }

    #[test]
    fn popup_is_kept_visible_at_all_edges() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(880.0, 560.0));
        for (at, expected) in [(Pos2::new(-1000.0, -1000.0), Pos2::new(8.0, 8.0)),
                               (Pos2::new(1000.0, 1000.0), Pos2::new(480.0, 218.0))] {
            assert_eq!(clamp_settings_popup(Rect::from_min_size(at, Vec2::new(392.0, 334.0)), screen), expected);
        }
    }

    #[test]
    fn expanded_popup_and_small_viewports_do_not_panic() {
        let popup = Rect::from_min_size(Pos2::new(400.0, 200.0), Vec2::new(392.0, 430.0));
        assert_eq!(clamp_settings_popup(popup, Rect::from_min_size(Pos2::ZERO, Vec2::new(880.0, 560.0))), Pos2::new(400.0, 122.0));
        assert_eq!(clamp_settings_popup(popup, Rect::from_min_size(Pos2::ZERO, Vec2::new(320.0, 240.0))), Pos2::new(8.0, 8.0));
    }
}

/// Bounds stay in image coordinates until transformed; do not clamp to the canvas
/// or an offscreen image would acquire false borders along the viewport edges.
pub fn image_bounds_rect(origin: Pos2, offset: Vec2, scale: f32, size: Vec2) -> Option<Rect> {
    if !scale.is_finite() || scale <= 0.0 || !size.is_finite() || size.min_elem() <= 0.0
        || !offset.is_finite() || !origin.x.is_finite() || !origin.y.is_finite() {
        return None;
    }
    let min = origin + offset;
    let scaled = size * scale;
    let max = min + scaled;
    if !scaled.is_finite() || !max.x.is_finite() || !max.y.is_finite() { return None; }
    Some(Rect::from_min_max(min, max))
}

pub fn paint_image_bounds(p: &egui::Painter, bounds: Rect, pal: &Palette) {
    // A slim dual-tone contour remains visible over white, dark and transparent pixels.
    // No fill, handles, hit targets or changes to the CPU pixels / sampling.
    let color = if pal.is_dark { Color32::from_rgb(102, 209, 255) }
        else { Color32::from_rgb(0, 116, 210) };
    p.rect_stroke(bounds, 0.0, Stroke::new(3.0_f32, Color32::from_black_alpha(160)));
    p.rect_stroke(bounds, 0.0, Stroke::new(1.25_f32, color));
}

#[cfg(test)]
mod image_bounds_tests {
    use super::*;
    #[test]
    fn full_image_bounds_follow_translation_scale_and_mip() {
        let b = image_bounds_rect(Pos2::new(10.0, 20.0), Vec2::new(30.0, 40.0),
            2.0, Vec2::new(256.0, 128.0)).unwrap();
        assert_eq!(b.min, Pos2::new(40.0, 60.0));
        assert_eq!(b.size(), Vec2::new(512.0, 256.0));
        let mip = image_bounds_rect(Pos2::ZERO, Vec2::ZERO, 2.0, Vec2::new(128.0, 64.0)).unwrap();
        assert_eq!(mip.size(), Vec2::new(256.0, 128.0));
    }
    #[test]
    fn offscreen_and_invalid_bounds_are_not_clamped_to_viewport() {
        let b = image_bounds_rect(Pos2::ZERO, Vec2::new(-400.0,-200.0),
            1.0, Vec2::new(100.0,100.0)).unwrap();
        assert_eq!(b.max, Pos2::new(-300.0,-100.0));
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(image_bounds_rect(Pos2::ZERO,Vec2::ZERO,scale,Vec2::splat(1.0)).is_none());
        }
    }
}
