//! Small reusable widgets shared across the top bar, list, and detail panes.
//!
//! All widgets accept an `egui::Ui` and return a `Response` (or paint into
//! the parent ui directly). They consult `crate::theme` for colours so the
//! whole app changes shape when the user toggles light/dark.

use egui::{Color32, CornerRadius, Margin, Response, RichText, Sense, Stroke, Ui, Vec2};

use crate::theme;

/// Outline-style HTTP-method badge: colored text on a faint background with a
/// matching colored 1px border. Mirrors `style.css .method-badge` exactly:
/// fixed-width, never greedy, never stretches to fill its container.
pub fn method_badge_sized(ui: &mut Ui, method: &str, min_width: f32) -> Response {
    let color = theme::method_color(method);
    let border = color.linear_multiply(0.5);
    let bg = theme::elev2_bg(ui.ctx());
    let text = method.to_uppercase();
    let font_id = egui::FontId::monospace(10.5);
    // Painter::layout_no_wrap gives us text dimensions without needing mutable
    // access to Fonts (which `Context::fonts` doesn't expose).
    let galley = ui.painter().layout_no_wrap(text, font_id, color);
    let pad = egui::vec2(8.0, 4.0);
    let w = (galley.size().x + pad.x * 2.0).max(min_width);
    let h = galley.size().y + pad.y * 2.0;
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(4), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, border),
        egui::epaint::StrokeKind::Inside,
    );
    let text_pos = rect.center() - galley.size() * 0.5;
    painter.galley(text_pos, galley, color);
    resp
}

/// "Pill" frame that shows a label + value. Click target. Used for the
/// Capture URL pill and the Forward indicator in the top bar.
pub fn label_pill(ui: &mut Ui, label: &str, value: &str) -> Response {
    label_pill_with_color(ui, label, value, None)
}

pub fn label_pill_with_color(
    ui: &mut Ui,
    label: &str,
    value: &str,
    accent: Option<Color32>,
) -> Response {
    let bg = theme::elev2_bg(ui.ctx());
    let border = accent.unwrap_or_else(|| theme::border_color(ui.ctx()));
    let frame = egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin {
            left: 10,
            right: 10,
            top: 5,
            bottom: 5,
        });
    let resp = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(label.to_uppercase())
                        .small()
                        .color(theme::dim_text_color(ui.ctx()))
                        .strong(),
                );
                ui.add_space(2.0);
                ui.label(RichText::new(value).monospace().strong());
            });
        })
        .response;
    resp.interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Square icon button. Single glyph, fixed size, tooltip on hover.
/// `active` paints the button as if it were currently engaged (e.g. paused).
pub fn icon_button(ui: &mut Ui, glyph: &str, tooltip: &str) -> Response {
    icon_button_inner(ui, glyph, tooltip, false, None)
}

pub fn icon_toggle(ui: &mut Ui, glyph: &str, tooltip: &str, active: bool) -> Response {
    icon_button_inner(ui, glyph, tooltip, active, None)
}

pub fn icon_button_colored(ui: &mut Ui, glyph: &str, tooltip: &str, color: Color32) -> Response {
    icon_button_inner(ui, glyph, tooltip, false, Some(color))
}

fn icon_button_inner(
    ui: &mut Ui,
    glyph: &str,
    tooltip: &str,
    active: bool,
    color_override: Option<Color32>,
) -> Response {
    let size = Vec2::splat(30.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    let visuals = ui.visuals();
    let dark = visuals.dark_mode;
    let hover = resp.hovered();
    let bg = if active {
        theme::accent_soft(ui.ctx())
    } else if hover {
        if dark {
            theme::BG_SOFT_DARK
        } else {
            theme::BG_ELEV2_LIGHT
        }
    } else {
        Color32::TRANSPARENT
    };
    let border = if active {
        theme::ACCENT
    } else if hover {
        theme::border_color(ui.ctx())
    } else {
        Color32::TRANSPARENT
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, CornerRadius::same(6), bg);
    if border != Color32::TRANSPARENT {
        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, border),
            egui::epaint::StrokeKind::Inside,
        );
    }
    let glyph_color = color_override.unwrap_or(if active {
        theme::ACCENT
    } else {
        theme::muted_text_color(ui.ctx())
    });
    let font = egui::FontId::proportional(15.0);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        font,
        glyph_color,
    );
    if !tooltip.is_empty() {
        resp = resp.on_hover_text(tooltip);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Selectable method-filter chip. Renders as an outline pill that lights up
/// in the method's colour when `selected`.
pub fn method_chip(ui: &mut Ui, method: &str, selected: bool) -> Response {
    let color = theme::method_color(method);
    let bg = if selected {
        color.linear_multiply(0.18)
    } else {
        theme::elev2_bg(ui.ctx())
    };
    let stroke_color = if selected {
        color
    } else {
        theme::border_color(ui.ctx())
    };
    let text_color = if selected {
        color
    } else {
        theme::muted_text_color(ui.ctx())
    };
    let frame = egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, stroke_color))
        .corner_radius(CornerRadius::same(255))
        .inner_margin(Margin {
            left: 9,
            right: 9,
            top: 3,
            bottom: 3,
        });
    let resp = frame
        .show(ui, |ui| {
            ui.label(
                RichText::new(method.to_uppercase())
                    .monospace()
                    .color(text_color)
                    .strong()
                    .size(10.5),
            );
        })
        .response;
    resp.interact(Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Properly-visible checkbox: an 18×18 box with a heavy border that fills
/// with the brand accent and shows a white tick when on. The whole row
/// (box + label) is one click target. Replaces egui's tiny default checkbox
/// inside the settings dialog.
pub fn nice_checkbox(ui: &mut Ui, value: &mut bool, label: &str) -> Response {
    let box_size: f32 = 18.0;
    let gap: f32 = 10.0;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let text_color = ui.visuals().text_color();
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font, text_color);
    let h = box_size.max(galley.size().y) + 6.0;
    let w = box_size + gap + galley.size().x + 4.0;

    let (rect, mut resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }

    let painter = ui.painter_at(rect);
    let box_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 2.0, rect.center().y - box_size / 2.0),
        Vec2::splat(box_size),
    );
    let hovered = resp.hovered();
    let bg = if *value {
        theme::ACCENT
    } else {
        ui.visuals().extreme_bg_color
    };
    let border = if *value {
        theme::ACCENT
    } else if hovered {
        theme::ACCENT_STRONG
    } else {
        theme::muted_text_color(ui.ctx())
    };
    painter.rect_filled(box_rect, CornerRadius::same(4), bg);
    painter.rect_stroke(
        box_rect,
        CornerRadius::same(4),
        Stroke::new(1.5, border),
        egui::epaint::StrokeKind::Inside,
    );

    if *value {
        // Tick mark — two stroke segments forming a check.
        let c = box_rect.center();
        let p1 = egui::pos2(c.x - 4.5, c.y - 0.0);
        let p2 = egui::pos2(c.x - 1.0, c.y + 3.5);
        let p3 = egui::pos2(c.x + 5.0, c.y - 3.5);
        let stroke = Stroke::new(2.0, Color32::WHITE);
        painter.line_segment([p1, p2], stroke);
        painter.line_segment([p2, p3], stroke);
    }

    let label_pos = egui::pos2(
        box_rect.right() + gap,
        rect.center().y - galley.size().y / 2.0,
    );
    painter.galley(label_pos, galley, text_color);

    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Square close button that paints an "X" with stroke segments — no font
/// dependency, so it renders identically across platforms regardless of
/// which Unicode glyphs the bundled fonts cover.
pub fn close_button(ui: &mut Ui, tooltip: &str) -> Response {
    let size = Vec2::splat(28.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    let hover = resp.hovered();
    let painter = ui.painter_at(rect);
    let bg = if hover {
        theme::soft_bg(ui.ctx())
    } else {
        Color32::TRANSPARENT
    };
    let border = if hover {
        theme::border_color(ui.ctx())
    } else {
        Color32::TRANSPARENT
    };
    painter.rect_filled(rect, CornerRadius::same(6), bg);
    if border != Color32::TRANSPARENT {
        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, border),
            egui::epaint::StrokeKind::Inside,
        );
    }
    let stroke_color = if hover {
        theme::DANGER
    } else {
        theme::muted_text_color(ui.ctx())
    };
    let c = rect.center();
    let r = 5.0;
    let stroke = Stroke::new(1.5, stroke_color);
    painter.line_segment(
        [egui::pos2(c.x - r, c.y - r), egui::pos2(c.x + r, c.y + r)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(c.x - r, c.y + r), egui::pos2(c.x + r, c.y - r)],
        stroke,
    );
    if !tooltip.is_empty() {
        resp = resp.on_hover_text(tooltip);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Small status dot used on the right edge of the top bar.
pub fn status_dot(ui: &mut Ui, ok: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    let color = if ok { theme::SUCCESS } else { theme::DANGER };
    ui.painter().circle_filled(rect.center(), 4.0, color);
}

#[cfg(test)]
mod tests {
    // Widgets are tested indirectly via the desktop app's state tests and
    // visual smoke testing. egui widgets aren't easily unit-tested outside
    // a real frame loop.
}
