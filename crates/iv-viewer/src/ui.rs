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

    /// 当前主题（默认深色）。
    pub fn current(ctx: &egui::Context) -> Self {
        ctx.data_mut(|d| *d.get_temp_mut_or(Self::id(), ThemeMode::Dark))
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
    /// 悬浮胶囊底色（半透明）
    pub overlay: Color32,
    /// 悬浮胶囊描边
    pub overlay_border: Color32,
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
    /// 常规控件描边
    pub w_border: Color32,
    /// extreme_bg（文本输入等最深控件底）
    pub extreme: Color32,
    pub err_bg: Color32,
    pub err_border: Color32,
    pub err_text: Color32,
    /// 悬浮胶囊投影
    pub shadow: egui::epaint::Shadow,
    /// 玻璃窗口底色（属性/设置等 egui::Window 的半透明底）
    pub glass_window: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// 当前主题配色板。
pub fn palette(ctx: &egui::Context) -> Palette {
    match ThemeMode::current(ctx) {
        // 中性烟灰玻璃 + 少量鼠尾草绿强调。
        ThemeMode::Dark => Palette {
            is_dark: true,
            bg: rgb(0x17, 0x1a, 0x1c),
            bar: rgb(0x24, 0x29, 0x2c),
            canvas: rgb(0x17, 0x1a, 0x1c),
            text: rgb(0xf1, 0xf4, 0xf3),
            text_bright: Color32::WHITE,
            dim: rgb(0xb6, 0xc0, 0xbb),
            faint: rgb(0x93, 0xa0, 0x9a),
            border: Color32::from_white_alpha(28),
            accent: rgb(0x9f, 0xca, 0xb4),
            selected_text: rgb(0x16, 0x27, 0x1f),
            overlay: Color32::from_rgba_unmultiplied(0x24, 0x29, 0x2c, 184),
            overlay_border: Color32::from_white_alpha(22),
            icon: rgb(0xc9, 0xd1, 0xcd),
            btn_hover: Color32::from_white_alpha(20),
            btn_pressed: Color32::from_white_alpha(38),
            w_bg: rgb(0x2b, 0x30, 0x33),
            w_hover: rgb(0x34, 0x3a, 0x3d),
            w_border: rgb(0x48, 0x50, 0x4d),
            extreme: rgb(0x12, 0x15, 0x16),
            err_bg: Color32::from_rgba_unmultiplied(0x46, 0x2a, 0x27, 148),
            err_border: rgb(0x6b, 0x40, 0x3b),
            err_text: rgb(0xe8, 0x9a, 0x90),
            shadow: egui::epaint::Shadow {
                offset: egui::vec2(0.0, 6.0),
                blur: 20.0,
                spread: 0.0,
                color: Color32::from_black_alpha(110),
            },
            glass_window: Color32::from_rgba_unmultiplied(0x24, 0x29, 0x2c, 219),
        },
        // 浅色对应中性乳白玻璃，避免给待查看图片附加明显色罩。
        ThemeMode::Light => Palette {
            is_dark: false,
            bg: rgb(0xe6, 0xe8, 0xe7),
            bar: rgb(0xf5, 0xf7, 0xf6),
            canvas: rgb(0xe6, 0xe8, 0xe7),
            text: rgb(0x25, 0x31, 0x2c),
            text_bright: rgb(0x18, 0x21, 0x1d),
            dim: rgb(0x53, 0x63, 0x5a),
            faint: rgb(0x65, 0x74, 0x6c),
            border: Color32::from_black_alpha(31),
            accent: rgb(0x35, 0x6c, 0x52),
            selected_text: Color32::WHITE,
            overlay: Color32::from_rgba_unmultiplied(0xf5, 0xf7, 0xf6, 209),
            overlay_border: Color32::from_black_alpha(36),
            icon: rgb(0x43, 0x54, 0x4b),
            btn_hover: Color32::from_black_alpha(15),
            btn_pressed: Color32::from_black_alpha(28),
            w_bg: rgb(0xec, 0xef, 0xed),
            w_hover: rgb(0xe2, 0xe7, 0xe4),
            w_border: rgb(0xb8, 0xc0, 0xbc),
            extreme: rgb(0xdc, 0xe1, 0xde),
            err_bg: Color32::from_rgba_unmultiplied(0xfb, 0xe7, 0xe3, 158),
            err_border: rgb(0xe0, 0xb5, 0xae),
            err_text: rgb(0xa1, 0x4e, 0x46),
            shadow: egui::epaint::Shadow {
                offset: egui::vec2(0.0, 5.0),
                blur: 18.0,
                spread: 0.0,
                color: Color32::from_rgba_unmultiplied(0x74, 0x8a, 0x7c, 66),
            },
            glass_window: Color32::from_rgba_unmultiplied(0xf5, 0xf7, 0xf6, 230),
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
    v.selection.bg_fill = pal.accent;
    v.hyperlink_color = pal.accent;
    v.widgets.inactive.weak_bg_fill = pal.w_bg;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0f32, pal.text);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0f32, pal.w_border);
    v.widgets.inactive.rounding = Rounding::same(10.0);
    v.widgets.hovered.weak_bg_fill = pal.w_hover;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0f32, pal.text_bright);
    v.widgets.hovered.rounding = Rounding::same(10.0);
    v.widgets.active.bg_fill = pal.accent;
    v.widgets.active.fg_stroke = Stroke::new(1.0f32, pal.selected_text);
    v.widgets.active.rounding = Rounding::same(10.0);
    v.widgets.open.weak_bg_fill = pal.w_bg;
    v.widgets.open.rounding = Rounding::same(10.0);
    // 关闭悬停/按下时的尺寸膨胀：现代 UI 保持稳定尺寸
    v.widgets.inactive.expansion = 0.0;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.expansion = 0.0;
    v.widgets.open.expansion = 0.0;
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0f32, pal.dim);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0f32, pal.border);
    v.window_rounding = Rounding::same(16.0);
    v.menu_rounding = Rounding::same(14.0);
    // 浮层投影：小偏移 + 大羽化
    let (pa, wa) = if pal.is_dark { (96, 110) } else { (35, 55) };
    v.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 12.0,
        spread: 0.0,
        color: Color32::from_black_alpha(pa),
    };
    v.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 8.0),
        blur: 20.0,
        spread: 0.0,
        color: Color32::from_black_alpha(wa),
    };
    // 玻璃描边：窗口/浮层统一细亮边
    v.window_stroke = Stroke::new(1.0f32, pal.overlay_border);
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

/// 悬浮工具条容器：半透明玻璃底（背后图像经 shader 实时模糊）+ 细亮边 + 大圆角 + 柔和投影。
pub fn capsule(pal: &Palette) -> egui::Frame {
    egui::Frame::default()
        .fill(pal.overlay)
        .stroke(Stroke::new(1.0f32, pal.overlay_border))
        .rounding(Rounding::same(24.0))
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .shadow(pal.shadow)
}

/// 沿上半圈连续渐隐的玻璃高光，避免固定水平线在圆角前突然截断。
pub fn paint_glass_sheen(p: &egui::Painter, rect: Rect, pal: &Palette) {
    let max_alpha = if pal.is_dark { 36.0 } else { 96.0 };
    let radius = (rect.height() * 0.5).clamp(2.0, 24.0);
    let mut points = Vec::with_capacity(31);
    for i in 0..=6 {
        let angle = std::f32::consts::PI + std::f32::consts::FRAC_PI_2 * i as f32 / 6.0;
        points.push(Pos2::new(
            rect.left() + radius + angle.cos() * radius,
            rect.top() + radius + angle.sin() * radius,
        ));
    }
    for i in 1..=16 {
        points.push(Pos2::new(
            egui::lerp(
                rect.left() + radius..=rect.right() - radius,
                i as f32 / 16.0,
            ),
            rect.top(),
        ));
    }
    for i in 1..=6 {
        let angle = std::f32::consts::PI * 1.5 + std::f32::consts::FRAC_PI_2 * i as f32 / 6.0;
        points.push(Pos2::new(
            rect.right() - radius + angle.cos() * radius,
            rect.top() + radius + angle.sin() * radius,
        ));
    }
    for i in 1..points.len() {
        let t = i as f32 / (points.len() - 1) as f32;
        let alpha = (max_alpha * (std::f32::consts::PI * t).sin().sqrt()) as u8;
        p.line_segment(
            [points[i - 1], points[i]],
            Stroke::new(1.0_f32, Color32::from_white_alpha(alpha)),
        );
    }
}

/// 胶囊内的细分隔线（1px 竖线，占 6px 宽）。
pub fn sep(ui: &mut egui::Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(6.0, 16.0), Sense::hover());
    ui.painter().vline(
        rect.center().x,
        rect.y_range(),
        Stroke::new(1.0f32, pal.border),
    );
}

/// 右键菜单容器：半透明玻璃底 + 圆角 + 亮边 + 投影。
/// （egui 内置右键菜单不响应 Esc 且状态不可控，故菜单壳自绘，内容仍用标准按钮）
pub fn menu_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::default()
        .fill(pal.glass_window)
        .stroke(Stroke::new(1.0f32, pal.overlay_border))
        .rounding(Rounding::same(16.0))
        .inner_margin(egui::Margin::symmetric(8.0, 8.0))
        .shadow(pal.shadow)
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
    let inner = rect.shrink2(Vec2::new(2.0, 1.0));
    if selected {
        ui.painter()
            .rect_filled(inner, Rounding::same(8.0), pal.accent);
    } else if resp.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(inner, Rounding::same(8.0), pal.btn_pressed);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(inner, Rounding::same(8.0), pal.btn_hover);
    }
    let fg = if selected {
        pal.selected_text
    } else {
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
    if resp.has_focus() {
        ui.painter()
            .rect_stroke(inner, Rounding::same(8.0), Stroke::new(1.5_f32, pal.accent));
    }
    resp
}

pub fn menu_section(ui: &mut egui::Ui, label: &str, pal: &Palette) {
    ui.add_space(5.0);
    ui.label(RichText::new(label).size(11.5).color(pal.dim));
    ui.add_space(2.0);
}

pub fn menu_sep(ui: &mut egui::Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(
        rect.left() + 8.0..=rect.right() - 8.0,
        rect.center().y,
        Stroke::new(1.0_f32, pal.border),
    );
}

/* ============================= 设置页组件 ============================= */

/// 设置页分组卡片：圆角半透明底 + 细边 + 加粗标题，内容竖排。
pub fn settings_card(
    ui: &mut egui::Ui,
    pal: &Palette,
    title: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    let fill = if pal.is_dark {
        Color32::from_black_alpha(70)
    } else {
        Color32::from_white_alpha(95)
    };
    egui::Frame::default()
        .fill(fill)
        .stroke(Stroke::new(1.0f32, pal.border))
        .rounding(Rounding::same(14.0))
        .inner_margin(egui::Margin::symmetric(14.0, 12.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).color(pal.text).strong().size(14.5));
            ui.add_space(9.0);
            add(ui);
        });
}

/// 设置行：左侧标签（+ 可选灰色描述），右侧控件右对齐。
pub fn setting_row(
    ui: &mut egui::Ui,
    pal: &Palette,
    label: &str,
    desc: &str,
    widget: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.add_space(2.0);
            ui.label(RichText::new(label).color(pal.text).size(13.5));
            if !desc.is_empty() {
                ui.label(RichText::new(desc).small().color(pal.faint));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), widget);
    });
}

/// 滑条设置行：标签（固定宽对齐）+ 滑条（占满中间）+ 右侧数值。
/// `pct` 为 true 时按百分比显示。返回本帧是否被拖动改变。
pub fn slider_row(
    ui: &mut egui::Ui,
    pal: &Palette,
    label: &str,
    val: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    pct: bool,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        // 标签固定宽，多条滑条纵向对齐
        let (lr, _) = ui.allocate_exact_size(Vec2::new(62.0, 20.0), Sense::hover());
        ui.painter().text(
            lr.left_center(),
            Align2::LEFT_CENTER,
            label,
            FontId::new(13.0, FontFamily::Proportional),
            pal.dim,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let txt = if pct {
                format!("{:.0}%", *val * 100.0)
            } else {
                format!("{val:.2}")
            };
            ui.label(RichText::new(txt).color(pal.dim).monospace().size(12.0));
            ui.add_space(8.0);
            let w = ui.available_width();
            let r = ui.add_sized(
                Vec2::new(w.max(80.0), 18.0),
                egui::Slider::new(val, range).show_value(false),
            );
            changed = r.changed();
        });
    });
    changed
}

/// 开关（switch）：自绘滑块，玻璃拟态风格，点击切换。
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, pal: &Palette) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(40.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let p = ui.painter();
        let t = if *on { 1.0f32 } else { 0.0 };
        let hovering = resp.hovered();
        // 轨道
        let track = if *on {
            pal.accent
        } else if pal.is_dark {
            Color32::from_white_alpha(if hovering { 34 } else { 24 })
        } else {
            Color32::from_black_alpha(if hovering { 40 } else { 28 })
        };
        p.rect_filled(rect, Rounding::same(11.0), track);
        if !*on {
            p.rect_stroke(
                rect,
                Rounding::same(11.0),
                Stroke::new(1.0f32, pal.overlay_border),
            );
        }
        // 滑块（圆）
        let cx = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, t);
        let knob = if *on { Color32::WHITE } else { pal.icon };
        p.circle_filled(Pos2::new(cx, rect.center().y), 7.5, knob);
    }
    resp
}

/* ================================ 图标 ================================ */

/// 手绘矢量图标集（16×16 基准，随按钮尺寸缩放）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Open,
    Prev,
    Next,
    Fit,
    Actual,
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
        let enabled = ui.is_enabled();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let hovering = enabled && resp.hovered();
        let p = ui.painter();
        let r = Rounding::same(9.0);
        let inner = rect.shrink(2.0);
        if enabled && active {
            p.rect_filled(inner, r, pal.accent);
            if hovering {
                p.rect_stroke(
                    inner,
                    r,
                    Stroke::new(1.0_f32, Color32::from_white_alpha(90)),
                );
            }
        } else if pressed {
            let fill = if danger {
                Color32::from_rgba_unmultiplied(0xc8, 0x4f, 0x4f, 118)
            } else {
                pal.btn_pressed
            };
            p.rect_filled(inner, r, fill);
            p.rect_stroke(inner, r, Stroke::new(1.0_f32, pal.overlay_border));
        } else if hovering {
            let fill = if danger {
                Color32::from_rgba_unmultiplied(0xc8, 0x4f, 0x4f, 72)
            } else {
                pal.btn_hover
            };
            p.rect_filled(inner, r, fill);
        }
        let fg = if !enabled {
            pal.faint
        } else if active {
            pal.selected_text
        } else if danger && (hovering || pressed) {
            Color32::WHITE
        } else if hovering {
            pal.text_bright
        } else {
            pal.icon
        };
        paint_icon(
            p,
            icon,
            Rect::from_center_size(rect.center(), Vec2::splat(16.0)),
            fg,
        );
        if resp.has_focus() {
            p.rect_stroke(
                rect.shrink(0.75),
                Rounding::same(10.0),
                Stroke::new(1.5_f32, pal.accent),
            );
        }
    }
    resp
}

/// 通道等紧凑选择项使用的 32pt 高分段按钮。
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
    let inner = rect.shrink(2.0);
    if selected {
        ui.painter()
            .rect_filled(inner, Rounding::same(9.0), pal.accent);
    } else if resp.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(inner, Rounding::same(9.0), pal.btn_pressed);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(inner, Rounding::same(9.0), pal.btn_hover);
    }
    let color = if selected {
        pal.selected_text
    } else {
        pal.icon
    };
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font, color));
    ui.painter().galley(
        Pos2::new(
            rect.center().x - galley.size().x / 2.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    if resp.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(0.75),
            Rounding::same(10.0),
            Stroke::new(1.5_f32, pal.accent),
        );
    }
    resp
}

/* ================================ 徽章 ================================ */

/// 按格式/压缩名选择徽章配色（随主题明暗切换）。
fn badge_colors(text: &str, dark: bool) -> (Color32, Color32) {
    let t = text.trim();
    let fg_bg = |fg: (u8, u8, u8), bg: (u8, u8, u8)| {
        (
            Color32::from_rgb(fg.0, fg.1, fg.2),
            Color32::from_rgb(bg.0, bg.1, bg.2),
        )
    };
    if dark {
        if t.starts_with("BC") || t.starts_with("DXT") {
            fg_bg((0x5a, 0xd4, 0xc8), (0x1b, 0x31, 0x34))
        } else if t.eq_ignore_ascii_case("DDS") {
            fg_bg((0xb4, 0x8e, 0xff), (0x2a, 0x24, 0x40))
        } else if t.eq_ignore_ascii_case("HDR") || t.eq_ignore_ascii_case("EXR") {
            fg_bg((0xff, 0xb4, 0x54), (0x39, 0x2c, 0x1b))
        } else if t.eq_ignore_ascii_case("PSD") {
            fg_bg((0x6f, 0xc3, 0xff), (0x1c, 0x2b, 0x3a))
        } else {
            fg_bg((0xab, 0xaf, 0xb8), (0x27, 0x2a, 0x31))
        }
    } else {
        if t.starts_with("BC") || t.starts_with("DXT") {
            fg_bg((0x0f, 0x76, 0x6e), (0xd7, 0xee, 0xec))
        } else if t.eq_ignore_ascii_case("DDS") {
            fg_bg((0x7c, 0x3a, 0xed), (0xec, 0xe4, 0xfd))
        } else if t.eq_ignore_ascii_case("HDR") || t.eq_ignore_ascii_case("EXR") {
            fg_bg((0xb4, 0x5f, 0x06), (0xfd, 0xec, 0xd2))
        } else if t.eq_ignore_ascii_case("PSD") {
            fg_bg((0x1d, 0x6f, 0xb8), (0xdc, 0xec, 0xfb))
        } else {
            fg_bg((0x5a, 0x60, 0x6b), (0xe3, 0xe5, 0xea))
        }
    }
}

/// 徽章：固定 32pt 高，与操作区共享垂直中心。
pub fn badge(ui: &mut egui::Ui, text: &str, pal: &Palette) -> egui::Response {
    let (fg, bg) = badge_colors(text, pal.is_dark);
    let font = egui::FontId::new(11.0, FontFamily::Proportional);
    // layout_no_wrap：徽章文字绝不换行
    let galley = ui.fonts(|f| f.layout_no_wrap(format!(" {text} "), font, fg));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(galley.size().x, 32.0), Sense::hover());
    // 色块高度取文字高度 + 上下各 2px，垂直居中于行
    let pill = Rect::from_center_size(
        rect.center(),
        Vec2::new(galley.size().x, galley.size().y + 4.0),
    );
    ui.painter().rect_filled(pill, Rounding::same(4.0), bg);
    ui.painter().galley(pill.left_top(), galley, fg);
    resp
}

/* ============================== 空状态占位 ============================== */

/// 画布底色：对角双色柔和渐变（玻璃拟态需要鲜活背景衬托，图像边缘/空状态下可见）。
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
        (rgb(0x20, 0x24, 0x26), rgb(0x15, 0x18, 0x1a))
    } else {
        (rgb(0xf0, 0xf2, 0xf1), rgb(0xdd, 0xe1, 0xdf))
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
    paint_image_icon(
        painter,
        egui::Rect::from_center_size(egui::Pos2::new(cx, top + 48.0), egui::vec2(128.0, 96.0)),
        pal.faint,
    );
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
