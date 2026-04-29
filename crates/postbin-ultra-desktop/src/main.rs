//! Native desktop entrypoint for Postbin Ultra.
//!
//! This binary is a native macOS / Linux / Windows app built on eframe. It
//! reuses the `postbin-ultra` library's capture server and store, but renders
//! captures with egui instead of the bundled web UI.

mod app;
mod fonts;
mod format;
mod highlight;
mod icon;
mod state;
mod theme;
mod tree;
mod widgets;

use std::path::PathBuf;

use eframe::egui;

use crate::app::DesktopApp;

fn main() -> eframe::Result<()> {
    init_tracing();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to start tokio runtime");
    let handle = runtime.handle().clone();

    let settings_path = postbin_ultra::settings::Settings::default_path()
        .unwrap_or_else(|| PathBuf::from("postbin-ultra.json"));
    let settings = postbin_ultra::settings::Settings::load_or_default(&settings_path);

    let icon = icon::load_window_icon();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Postbin Ultra")
        .with_app_id("co.uk.matthorner.postbin-ultra")
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([720.0, 460.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }

    // On macOS we draw our own black title strip with a centered title,
    // because the system gray translucent strip looks cheap next to the dark
    // app chrome. We keep the traffic lights (titlebar_buttons_shown), hide
    // the system title text, and let our content render edge-to-edge under
    // the title area.
    #[cfg(target_os = "macos")]
    {
        viewport = viewport
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(true)
            .with_titlebar_buttons_shown(true);
    }

    let opts = eframe::NativeOptions {
        viewport,
        persist_window: true,
        centered: true,
        ..Default::default()
    };

    let result = eframe::run_native(
        "Postbin Ultra",
        opts,
        Box::new(move |cc| {
            let app = DesktopApp::new(cc, handle.clone(), settings.clone(), settings_path.clone())
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                    format!("{e:#}").into()
                })?;
            Ok(Box::new(app))
        }),
    );

    drop(runtime);
    result
}

fn init_tracing() {
    use tracing_subscriber::fmt;
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,postbin_ultra=info,postbin_ultra_desktop=info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}
