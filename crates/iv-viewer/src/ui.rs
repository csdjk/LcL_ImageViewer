//! 主题与通用 UI 组件：深/浅双主题配色、字体（系统字体加载）、矢量图标、
//! 悬浮胶囊容器、图标按钮、徽章、空状态占位。

use eframe::egui;
use eframe::egui::{Color32, FontFamily, FontId, Pos2, Rect, Rounding, Sense, Stroke, TextStyle, Vec2};

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
    /// 悬浮胶囊底色（半透明）
    pub overlay: Color32,
    /// 悬浮胶囊描边
    pub overlay_border: Color32,
    /// 图标常态色
    pub icon: Color32,
    /// 图标按钮悬停底
    pub btn_hover: Color32,
    /// 开关按钮激活底
    pub btn_on_bg: Color32,
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
    /// 新拟态：右下暗阴影色（软凸起/凹陷的暗侧）
    pub neu_dark: Color32,
    /// 新拟态：左上亮高光色（软凸起的亮侧）
    pub neu_light: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// 当前主题配色板。
pub fn palette(ctx: &egui::Context) -> Palette {
    match ThemeMode::current(ctx) {
        // 深色新拟态：深绿灰底 + 柔和黑影 + 微亮高光，accent 为浅苔绿
        ThemeMode::Dark => Palette {
            is_dark: true,
            bg: rgb(0x26, 0x2d, 0x29),
            bar: rgb(0x26, 0x2d, 0x29),
            canvas: rgb(0x20, 0x27, 0x24),
            text: rgb(0xd3, 0xde, 0xd6),
            text_bright: Color32::WHITE,
            dim: rgb(0x8b, 0x9a, 0x90),
            faint: rgb(0x5c, 0x6a, 0x61),
            border: rgb(0x37, 0x41, 0x3b),
            accent: rgb(0x8f, 0xbc, 0xa4),
            overlay: Color32::from_rgba_unmultiplied(0x2a, 0x32, 0x2d, 242),
            overlay_border: Color32::from_white_alpha(22),
            icon: rgb(0xbd, 0xca, 0xc1),
            btn_hover: Color32::from_white_alpha(14),
            btn_on_bg: Color32::from_rgba_unmultiplied(0x8f, 0xbc, 0xa4, 38),
            w_bg: rgb(0x2d, 0x35, 0x30),
            w_hover: rgb(0x36, 0x40, 0x3a),
            w_border: rgb(0x3d, 0x48, 0x42),
            extreme: rgb(0x1c, 0x22, 0x1f),
            err_bg: rgb(0x3d, 0x2a, 0x28),
            err_border: rgb(0x6b, 0x40, 0x3b),
            err_text: rgb(0xe8, 0x9a, 0x90),
            shadow: egui::epaint::Shadow {
                offset: egui::vec2(0.0, 8.0),
                blur: 22.0,
                spread: 0.0,
                color: Color32::from_black_alpha(85),
            },
            neu_dark: Color32::from_black_alpha(120),
            neu_light: Color32::from_rgba_unmultiplied(0x46, 0x52, 0x4b, 110),
        },
        // 浅色新拟态（治愈系主场）：雾感鼠尾草绿底，元素与底同色、靠双向软阴影塑形
        ThemeMode::Light => Palette {
            is_dark: false,
            bg: rgb(0xe6, 0xef, 0xe9),
            bar: rgb(0xe6, 0xef, 0xe9),
            canvas: rgb(0xdc, 0xe7, 0xdf),
            text: rgb(0x3f, 0x52, 0x48),
            text_bright: rgb(0x2c, 0x3b, 0x32),
            dim: rgb(0x6d, 0x7f, 0x74),
            faint: rgb(0x9a, 0xaa, 0xa0),
            border: rgb(0xc3, 0xd1, 0xc7),
            accent: rgb(0x5d, 0x8f, 0x74),
            overlay: Color32::from_rgba_unmultiplied(0xea, 0xf2, 0xec, 246),
            overlay_border: Color32::from_white_alpha(130),
            icon: rgb(0x5a, 0x6f, 0x62),
            btn_hover: Color32::from_white_alpha(70),
            btn_on_bg: Color32::from_rgba_unmultiplied(0x5d, 0x8f, 0x74, 26),
            w_bg: rgb(0xdf, 0xe8, 0xe1),
            w_hover: rgb(0xd5, 0xe1, 0xd8),
            w_border: rgb(0xc3, 0xd1, 0xc7),
            extreme: rgb(0xd8, 0xe2, 0xda),
            err_bg: rgb(0xf6, 0xe3, 0xe0),
            err_border: rgb(0xe0, 0xb5, 0xae),
            err_text: rgb(0xa1, 0x4e, 0x46),
            shadow: egui::epaint::Shadow {
                offset: egui::vec2(0.0, 10.0),
                blur: 26.0,
                spread: 0.0,
                color: Color32::from_rgba_unmultiplied(0x74, 0x8a, 0x7c, 60),
            },
            neu_dark: Color32::from_rgba_unmultiplied(0x9f, 0xb5, 0xa8, 130),
            neu_light: Color32::from_white_alpha(200),
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
    v.window_fill = pal.bar;
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
    v.widgets.active.fg_stroke = Stroke::new(1.0f32, Color32::WHITE);
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
    // 可交互元素使用手型指针
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    ctx.set_visuals(v);

    install_fonts(ctx);

    ctx.style_mut(|s| {
        s.text_styles.insert(TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Small, FontId::new(12.0, FontFamily::Proportional));
        s.text_styles.insert(TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace));
    });
}

/* ============================== 悬浮胶囊 ============================== */

/// 悬浮工具条容器：近不透明底 + 高光细边 + 大圆角 + 柔和投影。
pub fn capsule(pal: &Palette) -> egui::Frame {
    egui::Frame::default()
        .fill(pal.overlay)
        .stroke(Stroke::new(1.0f32, pal.overlay_border))
        .rounding(Rounding::same(22.0))
        .inner_margin(egui::Margin::symmetric(9.0, 6.0))
        .shadow(pal.shadow)
}

/// 胶囊内的细分隔线（1px 竖线，占 6px 宽）。
pub fn sep(ui: &mut egui::Ui, pal: &Palette) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(6.0, 16.0), Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0f32, pal.border));
}

/// 右键菜单容器：不透明底 + 圆角 + 描边 + 投影。
/// （egui 内置右键菜单不响应 Esc 且状态不可控，故菜单壳自绘，内容仍用标准按钮）
pub fn menu_frame(pal: &Palette) -> egui::Frame {
    egui::Frame::default()
        .fill(pal.bar)
        .stroke(Stroke::new(1.0f32, pal.overlay_border))
        .rounding(Rounding::same(16.0))
        .inner_margin(egui::Margin::symmetric(5.0, 5.0))
        .shadow(pal.shadow)
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
    }
}

/* ============================== 图标按钮 ============================== */

/// 新拟态软凸起：右下暗阴影 + 左上亮高光（用 Shadow::tessellate 叠两层 mesh）。
fn paint_neu_raise(p: &egui::Painter, rect: Rect, rounding: Rounding, pal: &Palette) {
    let dark = egui::epaint::Shadow {
        offset: egui::vec2(3.0, 4.0),
        blur: 9.0,
        spread: 0.0,
        color: pal.neu_dark,
    };
    let light = egui::epaint::Shadow {
        offset: egui::vec2(-3.0, -3.0),
        blur: 9.0,
        spread: 0.0,
        color: pal.neu_light,
    };
    p.add(egui::Shape::mesh(dark.tessellate(rect, rounding)));
    p.add(egui::Shape::mesh(light.tessellate(rect, rounding)));
}

/// 新拟态凹陷：稍暗的底 + 顶内暗线 / 底内亮线（egui 无内阴影，用边线模拟）。
fn paint_neu_inset(p: &egui::Painter, rect: Rect, rounding: Rounding, pal: &Palette) {
    let fill = if pal.is_dark {
        Color32::from_black_alpha(36)
    } else {
        Color32::from_rgba_unmultiplied(0x9f, 0xb5, 0xa8, 44)
    };
    p.rect_filled(rect, rounding, fill);
    // 顶/左边偏暗（光从左上来，凹陷处上沿背光）
    let top = Rect::from_min_size(rect.min, egui::vec2(rect.width(), 1.6));
    p.rect_filled(top, Rounding::ZERO, pal.neu_dark);
    // 底/左边偏亮
    let bot = Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - 1.6),
        rect.max,
    );
    p.rect_filled(bot, Rounding::ZERO, pal.neu_light);
}

/// 图标按钮：28×26，常态透明，悬停软凸起，开关激活态凹陷 + 强调色图标。
pub fn icon_btn(ui: &mut egui::Ui, icon: Icon, active: bool, pal: &Palette) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(28.0, 26.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let hovering = enabled && (resp.hovered() || resp.is_pointer_button_down_on());
        let p = ui.painter();
        let r = Rounding::same(9.0);
        let inner = rect.shrink(1.5);
        if enabled && active {
            paint_neu_inset(p, inner, r, pal);
        } else if enabled && hovering {
            paint_neu_raise(p, inner, r, pal);
            p.rect_filled(inner, r, pal.btn_hover);
        }
        let fg = if !enabled {
            pal.faint
        } else if active {
            pal.accent
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
    }
    resp
}

/* ================================ 徽章 ================================ */

/// 按格式/压缩名选择徽章配色（随主题明暗切换）。
fn badge_colors(text: &str, dark: bool) -> (Color32, Color32) {
    let t = text.trim();
    let fg_bg = |fg: (u8, u8, u8), bg: (u8, u8, u8)| {
        (Color32::from_rgb(fg.0, fg.1, fg.2), Color32::from_rgb(bg.0, bg.1, bg.2))
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

/// 画一个小圆角徽章（格式/压缩标签），返回其响应（hover tooltip 可叠加）。
pub fn badge(ui: &mut egui::Ui, text: &str, pal: &Palette) -> egui::Response {
    let (fg, bg) = badge_colors(text, pal.is_dark);
    let font = egui::FontId::new(11.0, FontFamily::Proportional);
    // layout_no_wrap：徽章文字绝不换行
    let galley = ui.fonts(|f| f.layout_no_wrap(format!(" {text} "), font, fg));
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().rect_filled(rect, Rounding::same(4.0), bg);
    ui.painter().galley(rect.left_top(), galley, fg);
    resp
}

/* ============================== 空状态占位 ============================== */

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

    if let Some(bytes) = std::fs::read("C:/Windows/Fonts/segoeui.ttf").ok() {
        fonts.font_data.insert("segoe_ui".into(), egui::FontData::from_owned(bytes));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "segoe_ui".into());
        changed = true;
    }
    if let Some(bytes) = std::fs::read("C:/Windows/Fonts/consola.ttf").ok() {
        fonts.font_data.insert("consolas".into(), egui::FontData::from_owned(bytes));
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "consolas".into());
        changed = true;
    }
    if let Some(bytes) = std::fs::read("C:/Windows/Fonts/msyh.ttc").ok() {
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
