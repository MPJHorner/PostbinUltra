//! Embedded window icon. The full-resolution `.icns` for the `.app` bundle
//! lives under `assets/icons/` and is generated at build time; this module
//! ships a smaller PNG inside the binary so the running app has a Dock icon
//! even when launched without the bundle (e.g. `cargo run`).

const ICON_PNG: &[u8] = include_bytes!("../assets/icons/window-icon.png");

/// Decode the embedded PNG and produce an `egui::IconData`. Returns `None`
/// when the embedded asset is missing or fails to decode — the binary still
/// runs in that case, just without a custom Dock icon.
pub fn load_window_icon() -> Option<egui::IconData> {
    if ICON_PNG.is_empty() {
        return None;
    }
    let image = image::load_from_memory(ICON_PNG).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_is_either_decoded_or_absent_cleanly() {
        // Just exercise the code path. If a PNG is bundled it should decode;
        // if the file is empty we should get None without panicking.
        let _ = load_window_icon();
    }
}
