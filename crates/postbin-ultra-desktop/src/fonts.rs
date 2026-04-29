//! Bundled fonts and font registration.
//!
//! Inter for proportional UI text, JetBrains Mono for code/JSON/headers.
//! Both shipped as embedded `.ttf` so the binary is self-contained on every
//! platform and the rendered UI looks identical to the web one.

use egui::{FontData, FontDefinitions, FontFamily};
use std::sync::Arc;

const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
const JBM_REGULAR: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");
const JBM_BOLD: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf");

/// Install our bundled fonts on the given context. Adds Inter + JetBrains
/// Mono ahead of egui's defaults so they take priority but fall through to
/// the bundled emoji / icon fonts for glyphs we don't ship (☀, ⚙, …).
pub fn install(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();

    defs.font_data.insert(
        "inter".into(),
        Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    defs.font_data.insert(
        "inter_semibold".into(),
        Arc::new(FontData::from_static(INTER_SEMIBOLD)),
    );
    defs.font_data.insert(
        "jbmono".into(),
        Arc::new(FontData::from_static(JBM_REGULAR)),
    );
    defs.font_data.insert(
        "jbmono_bold".into(),
        Arc::new(FontData::from_static(JBM_BOLD)),
    );

    // Proportional family: Inter first, then SemiBold for strong text, then
    // egui defaults (which provide emoji / icon fallback).
    let prop = defs.families.entry(FontFamily::Proportional).or_default();
    prop.insert(0, "inter".into());
    prop.insert(1, "inter_semibold".into());

    // Monospace family: JetBrains Mono ahead of egui's Hack default.
    let mono = defs.families.entry(FontFamily::Monospace).or_default();
    mono.insert(0, "jbmono".into());
    mono.insert(1, "jbmono_bold".into());

    ctx.set_fonts(defs);
}
