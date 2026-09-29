//! 编辑器专用控件；复用查看器的新拟态表面，不修改全局主题或编辑数据。
use super::{button_face, paint_focus, surface_shapes, Palette, CONTROL_RADIUS};
use eframe::egui::{self, Color32, FontFamily, FontId, Rect, Response, Sense, Stroke, Vec2};

pub const HEIGHT: f32 = 32.0;
pub const GAP: f32 = 8.0;
pub const FIELD_WIDTH: f32 = 68.0;
pub const NOTICE_HEIGHT: f32 = 40.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Secondary,
    Primary,
    Destructive,
}

pub fn configure(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = Vec2::new(GAP, GAP);
    ui.spacing_mut().interact_size.y = HEIGHT;
    ui.spacing_mut().button_padding = Vec2::new(10.0, 6.0);
    ui.spacing_mut().combo_height = 280.0;
    for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
        ui.style_mut()
            .text_styles
            .insert(text, FontId::proportional(13.0));
    }
}

fn sized_button(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    selected: bool,
    role: Role,
) -> Response {
    let pal = super::palette(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, HEIGHT), Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, label));
    if ui.is_rect_visible(rect) {
        let is_primary = role == Role::Primary && ui.is_enabled();
        let mut fg = button_face(ui, &response, selected || is_primary, &pal);
        if is_primary {
            fg = pal.selected_text;
        }
        if role == Role::Destructive && ui.is_enabled() {
            fg = pal.err_text;
        }
        let font = FontId::new(13.0, FontFamily::Proportional);
        let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), font, fg));
        ui.painter()
            .with_clip_rect(rect)
            .galley(rect.center() - galley.size() * 0.5, galley, fg);
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn button(ui: &mut egui::Ui, label: &str, enabled: bool, role: Role) -> Response {
    let pal = super::palette(ui.ctx());
    let width = ui.fonts(|f| {
        f.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), pal.text)
            .size()
            .x
    });
    let min_width = if label.chars().count() <= 1 {
        HEIGHT
    } else {
        52.0
    };
    ui.add_enabled_ui(enabled, |ui| {
        sized_button(ui, label, (width + 22.0).max(min_width), false, role)
    })
    .inner
}

pub fn tab(ui: &mut egui::Ui, label: &str, selected: bool, width: f32) -> Response {
    sized_button(ui, label, width, selected, Role::Secondary)
}

pub fn toggle(ui: &mut egui::Ui, label: &str, value: &mut bool) -> Response {
    let pal = super::palette(ui.ctx());
    let text =
        ui.fonts(|f| f.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), pal.text));
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(text.size().x + 36.0, HEIGHT), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *value, label));
    let fg = button_face(ui, &response, *value, &pal);
    let check =
        Rect::from_center_size(rect.left_center() + Vec2::new(14.0, 0.0), Vec2::splat(10.0));
    if *value {
        ui.painter().line_segment(
            [check.left_center(), check.center_bottom()],
            Stroke::new(1.6_f32, fg),
        );
        ui.painter().line_segment(
            [check.center_bottom(), check.right_top()],
            Stroke::new(1.6_f32, fg),
        );
    } else {
        ui.painter()
            .rect_stroke(check, 3.0, Stroke::new(1.0_f32, fg));
    }
    let galley = ui.fonts(|f| f.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), fg));
    ui.painter().galley(
        egui::pos2(rect.left() + 26.0, rect.center().y - galley.size().y * 0.5),
        galley,
        fg,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

// Native DragValue/TextEdit and ComboBox retain editing, popup and keyboard behavior.
// Their backing surface is inserted before native text, not painted over it.
fn native_style(ui: &mut egui::Ui, pal: &Palette) {
    configure(ui);
    let v = ui.visuals_mut();
    v.button_frame = false;
    v.extreme_bg_color = Color32::TRANSPARENT;
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
        &mut v.widgets.noninteractive,
    ] {
        w.bg_fill = Color32::TRANSPARENT;
        w.weak_bg_fill = Color32::TRANSPARENT;
        w.bg_stroke = Stroke::NONE;
        w.rounding = egui::Rounding::same(CONTROL_RADIUS);
        w.expansion = 0.0;
    }
    v.widgets.inactive.fg_stroke.color = pal.text;
    v.widgets.hovered.fg_stroke.color = pal.text_bright;
    v.widgets.active.fg_stroke.color = pal.selected_text;
    v.widgets.noninteractive.fg_stroke.color = pal.faint;
}

pub fn field(ui: &mut egui::Ui, widget: impl egui::Widget, width: f32) -> Response {
    let pal = super::palette(ui.ctx());
    let layer = ui.painter().add(egui::Shape::Noop);
    let response = ui
        .scope(|ui| {
            native_style(ui, &pal);
            ui.style_mut().override_text_style = Some(egui::TextStyle::Monospace);
            ui.add_sized([width, HEIGHT], widget)
        })
        .inner;
    let fill = if response.hovered() && ui.is_enabled() {
        pal.w_hover
    } else {
        pal.extreme
    };
    ui.painter().set(
        layer,
        surface_shapes(
            response.rect.shrink(2.0),
            CONTROL_RADIUS,
            fill,
            true,
            0.0,
            &pal,
        ),
    );
    if ui.is_enabled() && response.has_focus() {
        paint_focus(ui.painter(), response.rect.shrink(1.0), &pal);
    }
    response
}

pub fn combo<R>(
    ui: &mut egui::Ui,
    id: &str,
    selected: &str,
    width: f32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<Option<R>> {
    let pal = super::palette(ui.ctx());
    let layer = ui.painter().add(egui::Shape::Noop);
    let result = ui
        .scope(|ui| {
            native_style(ui, &pal);
            egui::ComboBox::from_id_source(id)
                .width(width)
                .selected_text(egui::RichText::new(selected).size(13.0))
                .show_ui(ui, |ui| {
                    // Popup remains an opaque themed surface, never a transparent native widget.
                    ui.visuals_mut().extreme_bg_color = pal.extreme;
                    ui.visuals_mut().widgets.inactive.bg_fill = pal.w_bg;
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = pal.btn_hover;
                    ui.visuals_mut().selection.bg_fill = pal.btn_pressed;
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.spacing_mut().interact_size.y = HEIGHT;
                    content(ui)
                })
        })
        .inner;
    let fill = if result.response.hovered() && ui.is_enabled() {
        pal.btn_hover
    } else {
        pal.w_bg
    };
    ui.painter().set(
        layer,
        surface_shapes(
            result.response.rect.shrink(2.0),
            CONTROL_RADIUS,
            fill,
            false,
            if ui.is_enabled() { 0.5 } else { 0.0 },
            &pal,
        ),
    );
    if ui.is_enabled() && result.response.has_focus() {
        paint_focus(ui.painter(), result.response.rect.shrink(1.0), &pal);
    }
    result
}

/// 参数行标签与输入框共用32点行盒，不受自动换行的顶对齐影响。
pub fn caption(ui: &mut egui::Ui, text: &str) {
    let pal = super::palette(ui.ctx());
    let galley =
        ui.fonts(|f| f.layout_no_wrap(text.to_owned(), FontId::proportional(13.0), pal.dim));
    let (rect, _) = ui.allocate_exact_size(Vec2::new(galley.size().x, HEIGHT), Sense::hover());
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, pal.dim);
}

pub fn group_gap(ui: &mut egui::Ui) {
    ui.add_space(6.0);
}

/// 恒定高度的提示/确认行；不会因待应用提示出现而改变画布高度。
pub fn notice<R>(
    ui: &mut egui::Ui,
    emphasized: bool,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let pal = super::palette(ui.ctx());
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, NOTICE_HEIGHT), Sense::hover());
    if emphasized {
        ui.painter()
            .rect_filled(rect, CONTROL_RADIUS, pal.btn_pressed);
    }
    let mut child = ui.child_ui(
        rect.shrink2(Vec2::new(8.0, 4.0)),
        egui::Layout::left_to_right(egui::Align::Center),
    );
    child.spacing_mut().item_spacing.x = GAP;
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    let inner = content(&mut child);
    egui::InnerResponse::new(
        inner,
        ui.interact(rect, ui.id().with("editor-notice"), Sense::hover()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(theme: super::super::ThemeMode, enabled: bool) -> Vec<Rect> {
        let ctx = egui::Context::default();
        super::super::ThemeMode::apply_to(&ctx, theme);
        let mut rects = Vec::new();
        let mut n = 16384_u32;
        let mut checked = false;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(880.0, 560.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    configure(ui);
                    ui.add_enabled_ui(enabled, |ui| {
                        rects.push(button(ui, "应用裁剪", true, Role::Primary).rect);
                        rects.push(button(ui, "取消", true, Role::Secondary).rect);
                        rects.push(button(ui, "丢弃调整", true, Role::Destructive).rect);
                        rects.push(tab(ui, "裁剪", true, 62.0).rect);
                        rects.push(toggle(ui, "锁定比例", &mut checked).rect);
                        rects.push(field(ui, egui::DragValue::new(&mut n), FIELD_WIDTH).rect);
                        rects.push(
                            combo(ui, "test-format", "自由比例", 110.0, |ui| {
                                ui.label("1:1");
                            })
                            .response
                            .rect,
                        );
                    });
                });
            },
        );
        assert_eq!(n, 16384);
        assert!(!checked);
        rects
    }
    #[test]
    fn all_controls_share_height_in_both_themes_and_disabled_state() {
        for theme in [
            super::super::ThemeMode::Light,
            super::super::ThemeMode::Dark,
        ] {
            for enabled in [true, false] {
                for r in frame(theme, enabled) {
                    assert!((r.height() - HEIGHT).abs() < 0.1, "{r:?}");
                }
            }
        }
    }
    #[test]
    fn disabling_controls_does_not_change_layout() {
        assert_eq!(
            frame(super::super::ThemeMode::Dark, true),
            frame(super::super::ThemeMode::Dark, false)
        );
    }
    #[test]
    fn notice_has_stable_height_with_or_without_actions() {
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let a = notice(ui, false, |ui| {
                    ui.label("原图保持不变");
                })
                .response
                .rect;
                let b = notice(ui, true, |ui| {
                    button(ui, "继续调整", true, Role::Primary);
                })
                .response
                .rect;
                assert_eq!(a.height(), NOTICE_HEIGHT);
                assert_eq!(a.size(), b.size());
            });
        });
    }
}
