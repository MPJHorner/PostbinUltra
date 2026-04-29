//! Custom egui theme tuned for the Postbin Ultra brand.
//!
//! Two design priorities: (1) read well at-a-glance — high-contrast method
//! badges, dimmed metadata; (2) feel native — soft shadows, lavender accent
//! that matches the existing web UI so users moving between the two don't
//! feel disoriented. Palette tokens here mirror `crates/postbin-ultra/ui/style.css`.

use egui::{Color32, CornerRadius, FontId, Margin, Stroke, TextStyle, Visuals};

use postbin_ultra::settings::Theme as ThemePref;

// ── Brand ──────────────────────────────────────────────────────────────
pub const ACCENT: Color32 = Color32::from_rgb(0x7c, 0x8c, 0xff);
pub const ACCENT_STRONG: Color32 = Color32::from_rgb(0x9c, 0xaa, 0xff);
pub const ACCENT_SOFT_DARK: Color32 = Color32::from_rgba_premultiplied(0x4a, 0x55, 0x99, 0x55);
pub const ACCENT_SOFT_LIGHT: Color32 = Color32::from_rgba_premultiplied(0x7c, 0x8c, 0xff, 0x33);

// ── Dark surfaces ──────────────────────────────────────────────────────
pub const BG_DARK: Color32 = Color32::from_rgb(0x0b, 0x0d, 0x12);
pub const BG_ELEV_DARK: Color32 = Color32::from_rgb(0x13, 0x16, 0x1d);
pub const BG_ELEV2_DARK: Color32 = Color32::from_rgb(0x18, 0x1c, 0x25);
pub const BG_SOFT_DARK: Color32 = Color32::from_rgb(0x1d, 0x21, 0x2c);
pub const BORDER_DARK: Color32 = Color32::from_rgb(0x22, 0x26, 0x2f);
pub const BORDER_STRONG_DARK: Color32 = Color32::from_rgb(0x2c, 0x31, 0x40);
pub const TEXT_DARK: Color32 = Color32::from_rgb(0xe8, 0xea, 0xed);
pub const TEXT_MUTED_DARK: Color32 = Color32::from_rgb(0x9b, 0xa1, 0xad);
pub const TEXT_DIM_DARK: Color32 = Color32::from_rgb(0x6c, 0x72, 0x80);

// ── Light surfaces ─────────────────────────────────────────────────────
pub const BG_LIGHT: Color32 = Color32::from_rgb(0xf6, 0xf7, 0xf9);
pub const BG_ELEV_LIGHT: Color32 = Color32::from_rgb(0xff, 0xff, 0xff);
pub const BG_ELEV2_LIGHT: Color32 = Color32::from_rgb(0xf0, 0xf2, 0xf5);
pub const BG_SOFT_LIGHT: Color32 = Color32::from_rgb(0xe9, 0xec, 0xf1);
pub const BORDER_LIGHT: Color32 = Color32::from_rgb(0xdd, 0xe1, 0xe8);
pub const BORDER_STRONG_LIGHT: Color32 = Color32::from_rgb(0xc9, 0xcf, 0xd9);
pub const TEXT_LIGHT: Color32 = Color32::from_rgb(0x1b, 0x1f, 0x27);
pub const TEXT_MUTED_LIGHT: Color32 = Color32::from_rgb(0x55, 0x5c, 0x6a);
pub const TEXT_DIM_LIGHT: Color32 = Color32::from_rgb(0x8a, 0x92, 0xa0);

// ── Status ─────────────────────────────────────────────────────────────
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0x6b, 0x6b);
pub const WARNING: Color32 = Color32::from_rgb(0xff, 0xb8, 0x60);
pub const SUCCESS: Color32 = Color32::from_rgb(0x4c, 0xd9, 0x7e);

/// Apply the theme to an egui context. Re-callable on each frame after the
/// user toggles the preference; egui internally diffs the style so this is
/// effectively free unless the dark/light decision changes.
pub fn apply(ctx: &egui::Context, pref: ThemePref) {
    let dark = is_dark(ctx, pref);
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.selection.bg_fill = ACCENT.linear_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;

    let radius = CornerRadius::same(6);
    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = radius;

    if dark {
        visuals.panel_fill = BG_DARK;
        visuals.window_fill = BG_ELEV_DARK;
        // Used as the TextEdit fill; lighter than panel so inputs read as
        // surfaces rather than holes punched in the dialog.
        visuals.extreme_bg_color = BG_SOFT_DARK;
        visuals.faint_bg_color = BG_ELEV2_DARK;
        visuals.code_bg_color = BG_ELEV2_DARK;
        visuals.window_stroke = Stroke::new(1.0, BORDER_DARK);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_DARK);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_DARK);
        visuals.widgets.inactive.bg_fill = BG_ELEV2_DARK;
        visuals.widgets.inactive.weak_bg_fill = BG_ELEV2_DARK;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG_DARK);
        visuals.widgets.hovered.bg_fill = BG_SOFT_DARK;
        visuals.widgets.hovered.weak_bg_fill = BG_SOFT_DARK;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        visuals.widgets.active.bg_fill = BG_SOFT_DARK;
        visuals.override_text_color = Some(TEXT_DARK);
    } else {
        visuals.panel_fill = BG_LIGHT;
        visuals.window_fill = BG_ELEV_LIGHT;
        visuals.extreme_bg_color = BG_ELEV_LIGHT;
        visuals.faint_bg_color = BG_ELEV2_LIGHT;
        visuals.code_bg_color = BG_ELEV2_LIGHT;
        visuals.window_stroke = Stroke::new(1.0, BORDER_LIGHT);
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_LIGHT);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_LIGHT);
        visuals.widgets.inactive.bg_fill = BG_ELEV_LIGHT;
        visuals.widgets.inactive.weak_bg_fill = BG_ELEV_LIGHT;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG_LIGHT);
        visuals.widgets.hovered.bg_fill = BG_ELEV2_LIGHT;
        visuals.widgets.hovered.weak_bg_fill = BG_ELEV2_LIGHT;
        visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
        visuals.widgets.active.bg_fill = BG_ELEV2_LIGHT;
        visuals.override_text_color = Some(TEXT_LIGHT);
    }

    visuals.widgets.noninteractive.corner_radius = radius;
    visuals.widgets.inactive.corner_radius = radius;
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.open.corner_radius = radius;

    ctx.set_visuals(visuals);

    let mut style = (*ctx.global_style()).clone();
    // Sized to mirror the web stylesheet: 13px body, 12.5px mono, 18px heading.
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(12.5));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(16.5));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.5));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(11.0));
    style.spacing.item_spacing = egui::vec2(8.0, 5.0);
    style.spacing.window_margin = Margin::same(16);
    style.spacing.menu_margin = Margin::same(8);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size = egui::vec2(28.0, 28.0);
    ctx.set_global_style(style);
}

pub fn is_dark(ctx: &egui::Context, pref: ThemePref) -> bool {
    match pref {
        ThemePref::Dark => true,
        ThemePref::Light => false,
        ThemePref::System => detect_system_dark(ctx),
    }
}

pub fn dim_text_color(ctx: &egui::Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        TEXT_DIM_DARK
    } else {
        TEXT_DIM_LIGHT
    }
}

pub fn muted_text_color(ctx: &egui::Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        TEXT_MUTED_DARK
    } else {
        TEXT_MUTED_LIGHT
    }
}

pub fn border_color(ctx: &egui::Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        BORDER_DARK
    } else {
        BORDER_LIGHT
    }
}

pub fn elev_bg(ctx: &egui::Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        BG_ELEV_DARK
    } else {
        BG_ELEV_LIGHT
    }
}

pub fn elev2_bg(ctx: &egui::Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        BG_ELEV2_DARK
    } else {
        BG_ELEV2_LIGHT
    }
}

pub fn soft_bg(ctx: &egui::Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        BG_SOFT_DARK
    } else {
        BG_SOFT_LIGHT
    }
}

pub fn accent_soft(ctx: &egui::Context) -> Color32 {
    if ctx.global_style().visuals.dark_mode {
        ACCENT_SOFT_DARK
    } else {
        ACCENT_SOFT_LIGHT
    }
}

pub fn method_color(method: &str) -> Color32 {
    match method.to_ascii_uppercase().as_str() {
        "GET" => Color32::from_rgb(0x4c, 0xd9, 0x7e),
        "POST" => Color32::from_rgb(0x6e, 0xa8, 0xff),
        "PUT" => Color32::from_rgb(0xff, 0xb8, 0x60),
        "PATCH" => Color32::from_rgb(0xc0, 0x8b, 0xff),
        "DELETE" => DANGER,
        "OPTIONS" => Color32::from_rgb(0x4d, 0xd0, 0xe1),
        "HEAD" => Color32::from_rgb(0xb0, 0xbe, 0xc5),
        _ => Color32::from_rgb(0x9b, 0xa1, 0xad),
    }
}

fn detect_system_dark(ctx: &egui::Context) -> bool {
    // egui exposes the OS-reported preference via `system_theme()` — falls back
    // to whatever style is currently active when the OS doesn't report.
    match ctx.system_theme() {
        Some(egui::Theme::Dark) => true,
        Some(egui::Theme::Light) => false,
        None => ctx.global_style().visuals.dark_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_color_distinct_per_known_verb() {
        let methods = [
            "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "WEIRD",
        ];
        let colors: Vec<Color32> = methods.iter().map(|m| method_color(m)).collect();
        let primary: std::collections::HashSet<_> = colors[..5].iter().collect();
        assert_eq!(primary.len(), 5);
    }

    #[test]
    fn method_color_is_case_insensitive() {
        assert_eq!(method_color("post"), method_color("POST"));
        assert_eq!(method_color("Get"), method_color("GET"));
    }
}
