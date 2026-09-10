//! 主题与通用 UI 组件：配色、字体（系统字体加载）、徽章、矢量图标。

use eframe::egui;
use eframe::egui::{Color32, FontFamily, FontId, Rounding, Sense, Stroke, TextStyle};

/// 全局配色（深色美术工具风格）。
pub struct C;
impl C {
    /// 窗口/面板底色
    pub const BG: Color32 = Color32::from_rgb(0x16, 0x18, 0x1d);
    /// 工具栏/状态栏底色
    pub const BAR: Color32 = Color32::from_rgb(0x1a, 0x1d, 0x23);
    /// 画布底色（最深，突出图像）
    pub const CANVAS: Color32 = Color32::from_rgb(0x0d, 0x0e, 0x12);
    /// 分隔线/边框
    pub const BORDER: Color32 = Color32::from_rgb(0x2a, 0x2e, 0x37);
    /// 强调色（选中/激活）
    pub const ACCENT: Color32 = Color32::from_rgb(0x4c, 0x8d, 0xff);
    /// 主文字
    pub const TEXT: Color32 = Color32::from_rgb(0xd7, 0xda, 0xe0);
    /// 次要文字
    pub const DIM: Color32 = Color32::from_rgb(0x8b, 0x8f, 0x98);
    /// 错误横幅底色
    pub const ERR_BG: Color32 = Color32::from_rgb(0x3a, 0x20, 0x28);
    /// 错误边框
    pub const ERR_BORDER: Color32 = Color32::from_rgb(0x6e, 0x36, 0x44);
    /// 错误文字
    pub const ERR_TEXT: Color32 = Color32::from_rgb(0xff, 0x7b, 0x72);
}

/// 应用主题：配色 + 系统字体 + 字号。
pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = C::BG;
    v.window_fill = C::BAR;
    v.extreme_bg_color = Color32::from_rgb(0x10, 0x12, 0x16);
    v.faint_bg_color = C::BAR;
    v.selection.bg_fill = C::ACCENT;
    v.hyperlink_color = C::ACCENT;
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x23, 0x27, 0x30);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0f32, C::TEXT);
    v.widgets.inactive.rounding = Rounding::same(5.0);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2d, 0x32, 0x3e);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0f32, Color32::WHITE);
    v.widgets.hovered.rounding = Rounding::same(5.0);
    v.widgets.active.bg_fill = C::ACCENT;
    v.widgets.active.fg_stroke = Stroke::new(1.0f32, Color32::WHITE);
    v.widgets.active.rounding = Rounding::same(5.0);
    v.widgets.open.weak_bg_fill = Color32::from_rgb(0x23, 0x27, 0x30);
    v.widgets.open.rounding = Rounding::same(5.0);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0f32, C::DIM);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0f32, C::BORDER);
    v.window_rounding = Rounding::same(8.0);
    v.menu_rounding = Rounding::same(8.0);
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

/// 按格式/压缩名选择徽章配色：DDS 紫、BC* 青、HDR 橙、其余灰。
fn badge_colors(text: &str) -> (Color32, Color32) {
    let t = text.trim();
    let fg_bg = |fg: (u8, u8, u8), bg: (u8, u8, u8)| {
        (Color32::from_rgb(fg.0, fg.1, fg.2), Color32::from_rgb(bg.0, bg.1, bg.2))
    };
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
}

/// 画一个小圆角徽章（格式/压缩标签），返回其响应（hover tooltip 可叠加）。
pub fn badge(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let (fg, bg) = badge_colors(text);
    let font = egui::FontId::new(11.0, FontFamily::Proportional);
    // layout_no_wrap：徽章文字绝不换行（layout 的第 4 参是 wrap 宽度，传小值会逐字换行）
    let galley = ui.fonts(|f| f.layout_no_wrap(format!(" {text} "), font, fg));
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::same(4.0), bg);
    let pos = rect.left_top();
    ui.painter().galley(pos, galley, Color32::WHITE);
    resp
}

/// 手绘"图片"矢量图标（圆角相框 + 太阳 + 山），用于空状态。
pub fn paint_image_icon(painter: &egui::Painter, rect: egui::Rect) {
    let s = Stroke::new(2.5f32, Color32::from_rgb(0x4a, 0x4f, 0x5a));
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
pub fn draw_placeholder(painter: &egui::Painter, rect: egui::Rect, loading: Option<&str>) {
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
                C::ACCENT
            } else {
                Color32::from_rgba_unmultiplied(
                    C::ACCENT.r(),
                    C::ACCENT.g(),
                    C::ACCENT.b(),
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
            C::TEXT,
        );
        return;
    }

    // 空状态：图标 + 主副文案 + 支持格式
    let top = rect.center().y - 130.0;
    paint_image_icon(
        painter,
        egui::Rect::from_center_size(egui::Pos2::new(cx, top + 48.0), egui::vec2(128.0, 96.0)),
    );
    let mut y = top + 128.0;
    painter.text(
        egui::Pos2::new(cx, y),
        egui::Align2::CENTER_TOP,
        "将图片拖拽到此处",
        font(20.0),
        C::TEXT,
    );
    y += 38.0;
    painter.text(
        egui::Pos2::new(cx, y),
        egui::Align2::CENTER_TOP,
        "Ctrl + O 打开文件 · ← → 浏览目录 · 滚轮缩放",
        font(13.0),
        C::DIM,
    );
    y += 30.0;
    painter.text(
        egui::Pos2::new(cx, y),
        egui::Align2::CENTER_TOP,
        "PNG · JPG · WebP · GIF · TGA · DDS (BC1–BC7) · PSD · HDR · QOI · PNM",
        font(12.0),
        Color32::from_rgb(0x55, 0x5a, 0x64),
    );
}
