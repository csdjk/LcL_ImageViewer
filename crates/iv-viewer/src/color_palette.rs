//! Compact background picker. HSV is evaluated in sRGB, matching displayed hex values.
//! Keep hue separately from RGB so editing black/white/gray never resets the hue slider.
use crate::ui::Palette;
use eframe::egui::{self, Color32, Key, Pos2, Rect, Sense, Stroke, Vec2};

const SWATCHES: [[u8; 3]; 8] = [
    [255, 255, 255],
    [128, 128, 128],
    [0, 0, 0],
    [239, 68, 68],
    [245, 158, 11],
    [34, 197, 94],
    [59, 130, 246],
    [168, 85, 247],
];

pub struct ColorPalette {
    hue: f32,
    saturation: f32,
    value: f32,
    hex: String,
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::new([128, 128, 128])
    }
}

impl ColorPalette {
    pub fn new(rgb: [u8; 3]) -> Self {
        let mut state = Self {
            hue: 0.0,
            saturation: 1.0,
            value: 1.0,
            hex: String::new(),
        };
        state.set_rgb(rgb);
        state
    }

    pub fn rgb(&self) -> [u8; 3] {
        hsv_rgb(self.hue, self.saturation, self.value)
    }

    pub fn set_rgb(&mut self, rgb: [u8; 3]) {
        let [r, g, b] = rgb.map(|c| f32::from(c) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        self.value = max;
        if max > 0.0 {
            self.saturation = delta / max;
        }
        if delta > 0.0 {
            let hue = if max == r {
                (g - b) / delta
            } else if max == g {
                (b - r) / delta + 2.0
            } else {
                (r - g) / delta + 4.0
            };
            self.hue = (hue / 6.0).rem_euclid(1.0);
        }
        self.hex = hex(rgb);
    }

    fn refresh_hex(&mut self) {
        self.hex = hex(self.rgb());
    }

    /// Only valid complete RGB values may change the preview; partial edits stay in the field.
    fn apply_hex(&mut self) -> bool {
        let Some(rgb) = parse_hex(&self.hex) else {
            return false;
        };
        let editing = self.hex.clone();
        self.set_rgb(rgb);
        self.hex = editing;
        true
    }

    pub fn valid_hex(&self) -> bool {
        parse_hex(&self.hex).is_some()
    }

    /// Returns true only when the selected RGB changes. Nothing is written into the image.
    pub fn show(&mut self, ui: &mut egui::Ui, pal: &Palette) -> bool {
        let before = self.rgb();
        let mut touched = false;
        let width = ui.available_width();
        ui.spacing_mut().item_spacing.y = 8.0;

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, 146.0), Sense::click_and_drag());
        let response = response
            .on_hover_cursor(egui::CursorIcon::Crosshair)
            .on_hover_text("点击或拖动选色：左右调整饱和度，上下调整明暗");
        if response.clicked() || response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                [self.saturation, self.value] = sv_at(rect, pos);
                self.refresh_hex();
                touched = true;
                response.request_focus();
            }
        }
        if response.has_focus() {
            let delta = arrow_delta(ui);
            if delta != Vec2::ZERO {
                self.saturation = (self.saturation + delta.x).clamp(0.0, 1.0);
                self.value = (self.value - delta.y).clamp(0.0, 1.0);
                self.refresh_hex();
            }
        }
        paint_gradient(ui.painter(), rect, 24, 16, |s, y| {
            hsv_rgb(self.hue, s, 1.0 - y)
        });
        ui.painter()
            .rect_stroke(rect, 0.0, Stroke::new(1.0_f32, pal.dim.gamma_multiply(0.5)));
        let selected = Pos2::new(
            rect.left() + self.saturation * rect.width(),
            rect.bottom() - self.value * rect.height(),
        );
        let marker = rect.shrink(5.0).clamp(selected);
        ui.painter()
            .circle_stroke(marker, 5.0, Stroke::new(3.0_f32, Color32::BLACK));
        ui.painter()
            .circle_stroke(marker, 5.0, Stroke::new(1.5_f32, Color32::WHITE));

        let (hue_rect, response) =
            ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::click_and_drag());
        let response = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("拖动选择色相");
        if response.clicked() || response.dragged_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                self.hue = ((pos.x - hue_rect.left()) / hue_rect.width()).clamp(0.0, 1.0);
                self.refresh_hex();
                touched = true;
                response.request_focus();
            }
        }
        if response.has_focus() {
            let delta = arrow_delta(ui).x;
            if delta != 0.0 {
                self.hue = (self.hue + delta).clamp(0.0, 1.0);
                self.refresh_hex();
            }
        }
        paint_gradient(ui.painter(), hue_rect, 48, 1, |h, _| hsv_rgb(h, 1.0, 1.0));
        let x = (hue_rect.left() + self.hue * hue_rect.width())
            .clamp(hue_rect.left() + 3.0, hue_rect.right() - 3.0);
        let handle =
            Rect::from_center_size(Pos2::new(x, hue_rect.center().y), Vec2::new(6.0, 22.0));
        ui.painter()
            .rect_stroke(handle, 2.0, Stroke::new(3.0_f32, Color32::BLACK));
        ui.painter()
            .rect_stroke(handle, 2.0, Stroke::new(1.5_f32, Color32::WHITE));

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let rgb = self.rgb();
            let (preview, _) = ui.allocate_exact_size(Vec2::new(30.0, 28.0), Sense::hover());
            ui.painter().rect_filled(preview, 6.0, color(rgb));
            ui.painter()
                .rect_stroke(preview, 6.0, Stroke::new(1.0_f32, pal.dim));
            ui.label(egui::RichText::new("HEX").size(11.0).color(pal.dim));
            let edit = ui.add_sized(
                [156.0, 28.0],
                egui::TextEdit::singleline(&mut self.hex)
                    .id_source("iv-background-hex")
                    .font(egui::TextStyle::Monospace)
                    .char_limit(7)
                    .desired_width(156.0)
                    .hint_text("#RRGGBB"),
            );
            if edit.changed() {
                touched |= self.apply_hex();
            }
            if edit.lost_focus() && self.valid_hex() {
                self.refresh_hex();
            }
        });
        let valid = self.valid_hex();
        ui.label(
            egui::RichText::new(if valid {
                "拖动实时预览，也可输入色号"
            } else {
                "请输入六位色号，例如 #3B82F6"
            })
            .size(11.0)
            .color(if valid { pal.dim } else { pal.err_text }),
        );
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let side = (width - 7.0 * 6.0) / 8.0;
            for rgb in SWATCHES {
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
                let inset = rect.shrink(2.0);
                ui.painter().rect_filled(inset, 4.0, color(rgb));
                ui.painter().rect_stroke(
                    inset,
                    4.0,
                    Stroke::new(1.0_f32, pal.dim.gamma_multiply(0.5)),
                );
                if self.rgb() == rgb || response.hovered() {
                    ui.painter()
                        .rect_stroke(rect, 5.0, Stroke::new(1.5_f32, pal.accent));
                }
                if response.on_hover_text(hex(rgb)).clicked() {
                    self.set_rgb(rgb);
                    touched = true;
                }
            }
        });
        touched || before != self.rgb()
    }
}

fn arrow_delta(ui: &egui::Ui) -> Vec2 {
    ui.input(|i| {
        let step = if i.modifiers.shift { 0.05 } else { 0.01 };
        Vec2::new(
            (i.key_pressed(Key::ArrowRight) as i32 - i.key_pressed(Key::ArrowLeft) as i32) as f32,
            (i.key_pressed(Key::ArrowDown) as i32 - i.key_pressed(Key::ArrowUp) as i32) as f32,
        ) * step
    })
}

fn color([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}
fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}
fn parse_hex(text: &str) -> Option<[u8; 3]> {
    let text = text.trim().strip_prefix('#').unwrap_or(text.trim());
    if text.len() != 6 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some([
        u8::from_str_radix(&text[0..2], 16).ok()?,
        u8::from_str_radix(&text[2..4], 16).ok()?,
        u8::from_str_radix(&text[4..6], 16).ok()?,
    ])
}
fn sv_at(rect: Rect, pos: Pos2) -> [f32; 2] {
    [
        ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
        (1.0 - (pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
    ]
}
fn hsv_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let sector = h.rem_euclid(1.0) * 6.0;
    let chroma = v * s;
    let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let [r, g, b] = match sector as u32 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    [r, g, b].map(|c| ((c + v - chroma).clamp(0.0, 1.0) * 255.0).round() as u8)
}
fn paint_gradient(
    p: &egui::Painter,
    rect: Rect,
    columns: u32,
    rows: u32,
    sample: impl Fn(f32, f32) -> [u8; 3],
) {
    let mut mesh = egui::Mesh::default();
    for y in 0..=rows {
        for x in 0..=columns {
            let u = x as f32 / columns as f32;
            let v = y as f32 / rows as f32;
            mesh.colored_vertex(
                Pos2::new(
                    rect.left() + u * rect.width(),
                    rect.top() + v * rect.height(),
                ),
                color(sample(u, v)),
            );
        }
    }
    for y in 0..rows {
        for x in 0..columns {
            let i = y * (columns + 1) + x;
            mesh.add_triangle(i, i + 1, i + columns + 1);
            mesh.add_triangle(i + 1, i + columns + 2, i + columns + 1);
        }
    }
    p.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgb_round_trip_preserves_srgb_byte_values() {
        for r in [0, 1, 23, 64, 128, 214, 255] {
            for g in [0, 1, 23, 64, 128, 214, 255] {
                for b in [0, 1, 23, 64, 128, 214, 255] {
                    assert_eq!(ColorPalette::new([r, g, b]).rgb(), [r, g, b]);
                }
            }
        }
    }
    #[test]
    fn black_white_gray_keep_hue_until_a_chromatic_color_is_selected() {
        let mut state = ColorPalette::new([0, 0, 255]);
        for rgb in [[0, 0, 0], [255, 255, 255], [128, 128, 128]] {
            state.set_rgb(rgb);
            assert!((state.hue - 2.0 / 3.0).abs() < 0.00001);
            assert_eq!(state.rgb(), rgb);
        }
        state.saturation = 1.0;
        state.value = 1.0;
        assert_eq!(state.rgb(), [0, 0, 255]);
    }
    #[test]
    fn sv_area_has_correct_edges_and_clamps_outside_drag() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(200.0, 100.0));
        assert_eq!(sv_at(rect, rect.left_top()), [0.0, 1.0]);
        assert_eq!(sv_at(rect, rect.right_bottom()), [1.0, 0.0]);
        assert_eq!(sv_at(rect, Pos2::new(500.0, -1.0)), [1.0, 1.0]);
        assert_eq!(sv_at(rect, rect.center()), [0.5, 0.5]);
    }
    #[test]
    fn hue_bar_wraps_red_and_all_primary_colors_match() {
        for (h, expected) in [
            (0.0, [255, 0, 0]),
            (1.0 / 6.0, [255, 255, 0]),
            (2.0 / 6.0, [0, 255, 0]),
            (3.0 / 6.0, [0, 255, 255]),
            (4.0 / 6.0, [0, 0, 255]),
            (5.0 / 6.0, [255, 0, 255]),
            (1.0, [255, 0, 0]),
        ] {
            assert_eq!(hsv_rgb(h, 1.0, 1.0), expected);
        }
    }
    #[test]
    fn incomplete_or_invalid_hex_never_overwrites_current_color() {
        let mut state = ColorPalette::new([12, 34, 56]);
        for text in ["", "#", "#123", "#GG0000", "黑色", "#12345678"] {
            state.hex = text.into();
            assert!(!state.apply_hex());
            assert_eq!(state.rgb(), [12, 34, 56]);
        }
        state.hex = "abCDeF".into();
        assert!(state.apply_hex());
        assert_eq!(state.rgb(), [171, 205, 239]);
        state.refresh_hex();
        assert_eq!(state.hex, "#ABCDEF");
    }
}
