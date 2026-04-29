//! `eframe::App` impl for the desktop UI.
//!
//! Pure rendering + event wiring. All the state mutations the UI triggers go
//! through `crate::state::AppState` so the testable logic stays out of egui.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use eframe::egui::{self, Color32, RichText, Stroke};
use postbin_ultra::capture::{new_forward_switch, CaptureConfig, ForwardConfig};
use postbin_ultra::request::{CapturedRequest, ForwardStatus};
use postbin_ultra::settings::{ForwardSettings, Settings, Theme as ThemePref};
use postbin_ultra::store::{RequestStore, StoreEvent};
use postbin_ultra::supervisor::CaptureSupervisor;
use tokio::sync::broadcast;

use crate::format::{format_body, FormattedBody};
use crate::highlight::{self, Highlighter};
use crate::state::{
    AppEvent, AppState, BodyFormat, DetailTab, SettingsTab, METHOD_CHIPS, OTHER_BUCKET,
};
use crate::theme;
use crate::widgets;

const STATUS_TTL: Duration = Duration::from_secs(3);

// Icon glyphs. Picked from Unicode planes that egui's bundled fonts render
// reliably (NotoEmoji + emoji-icon-font ship in the default font set).
const ICON_SETTINGS: &str = "\u{2699}"; // ⚙
const ICON_SUN: &str = "\u{2600}"; // ☀
const ICON_MOON: &str = "\u{1F319}"; // 🌙
const ICON_AUTO: &str = "\u{1F313}"; // 🌓 (system / auto)
const ICON_PAUSE: &str = "\u{23F8}"; // ⏸
const ICON_PLAY: &str = "\u{25B6}"; // ▶
const ICON_TRASH: &str = "\u{1F5D1}"; // 🗑
const ICON_FORWARD: &str = "\u{2197}"; // ↗
const ICON_FILTER: &str = "\u{1F50D}"; // 🔍

pub struct DesktopApp {
    state: AppState,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
}

impl DesktopApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        runtime: tokio::runtime::Handle,
        settings: Settings,
        settings_path: PathBuf,
    ) -> anyhow::Result<Self> {
        let store = RequestStore::new(settings.buffer_size);
        let forward_switch = new_forward_switch(forward_from_settings(&settings.forward)?);
        let cfg = CaptureConfig {
            max_body_size: settings.max_body_size,
            forward: forward_switch,
        };
        let bind = settings
            .bind
            .parse()
            .context("parsing bind address from settings")?;
        let supervisor = runtime
            .block_on(CaptureSupervisor::start(
                bind,
                settings.port,
                store.clone(),
                cfg,
            ))
            .context("starting capture server")?;
        let supervisor = Arc::new(supervisor);

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        let ctx = cc.egui_ctx.clone();
        let store_for_relay = store.clone();
        runtime.spawn(spawn_relay(
            store_for_relay.subscribe(),
            event_tx,
            ctx.clone(),
        ));

        crate::fonts::install(&ctx);
        theme::apply(&ctx, settings.theme);

        let state = AppState {
            supervisor,
            requests: vec![],
            selected: None,
            filter: String::new(),
            method_filter_off: HashSet::new(),
            settings: settings.clone(),
            settings_path,
            editing_settings: false,
            settings_tab: SettingsTab::default(),
            pending_settings: settings,
            pending_settings_error: None,
            runtime,
            detail_tab: DetailTab::default(),
            body_format: BodyFormat::default(),
            paused: false,
            status_message: None,
            forward_selection: std::collections::HashMap::new(),
            forward_flash: None,
        };

        Ok(Self { state, event_rx })
    }
}

async fn spawn_relay(
    mut rx: broadcast::Receiver<StoreEvent>,
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ctx: egui::Context,
) {
    loop {
        match rx.recv().await {
            Ok(StoreEvent::Request(req)) => {
                if tx.send(AppEvent::Request(req)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
            Ok(StoreEvent::ForwardUpdated(req)) => {
                if tx.send(AppEvent::ForwardUpdated(req)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
            Ok(StoreEvent::Cleared) => {
                if tx.send(AppEvent::Cleared).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

fn forward_from_settings(f: &ForwardSettings) -> anyhow::Result<Option<ForwardConfig>> {
    if !f.enabled || f.url.is_empty() {
        return Ok(None);
    }
    let url = url::Url::parse(&f.url).context("parsing forward URL")?;
    let cfg = ForwardConfig::build(url, Duration::from_secs(f.timeout_secs), f.insecure)
        .map_err(anyhow::Error::msg)?;
    Ok(Some(cfg))
}

impl eframe::App for DesktopApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(ev) = self.event_rx.try_recv() {
            self.state.push_event(ev);
        }

        let ctx = ui.ctx().clone();
        theme::apply(&ctx, self.state.settings.theme);

        #[cfg(target_os = "macos")]
        self.render_mac_titlebar(ui);
        self.render_top_bar(ui);
        self.render_methods_bar(ui);
        self.render_list(ui);
        self.render_detail(ui);
        self.render_settings_dialog(&ctx);

        self.handle_hotkeys(&ctx);
    }
}

impl DesktopApp {
    // ────────────────────────── macOS title strip ──────────────────────────

    /// Custom title strip drawn under macOS's transparent title region (we
    /// turned the system title bar invisible with `with_fullsize_content_view`).
    /// Solid black, "Postbin Ultra" centered in white. Traffic lights still
    /// sit on top in the OS-reserved area on the left.
    #[cfg(target_os = "macos")]
    fn render_mac_titlebar(&self, parent: &mut egui::Ui) {
        let height = 28.0;
        egui::Panel::top("mac-titlebar")
            .exact_size(height)
            .frame(
                egui::Frame::default()
                    .fill(Color32::BLACK)
                    .stroke(Stroke::NONE),
            )
            .show_inside(parent, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter_at(rect);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Postbin Ultra",
                    egui::FontId::proportional(13.0),
                    Color32::WHITE,
                );
            });
    }

    // ────────────────────────── Top bar ──────────────────────────

    fn render_top_bar(&mut self, parent: &mut egui::Ui) {
        let ctx = parent.ctx().clone();
        let bar_bg = theme::elev_bg(&ctx);
        let border = theme::border_color(&ctx);
        egui::Panel::top("topbar")
            .exact_size(56.0)
            .frame(
                egui::Frame::default()
                    .fill(bar_bg)
                    .stroke(Stroke::new(1.0, border))
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 0,
                        bottom: 0,
                    }),
            )
            .show_inside(parent, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.heading(RichText::new("Postbin Ultra").color(theme::ACCENT));
                    ui.add_space(14.0);

                    let url = self.state.capture_url();
                    let pill = widgets::label_pill(ui, "Capture", &url);
                    if pill.clicked() {
                        ctx.copy_text(url.clone());
                        self.state.status("Capture URL copied");
                    }
                    pill.on_hover_text("Click to copy the capture URL");

                    ui.add_space(10.0);

                    // Filter input with leading icon
                    let filter_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.state.filter)
                            .desired_width(220.0)
                            .hint_text(format!("{ICON_FILTER}  filter — method, path, header"))
                            .margin(egui::vec2(8.0, 5.0)),
                    );
                    if !self.state.filter.is_empty() {
                        if ui.small_button("✕").on_hover_text("Clear filter").clicked() {
                            self.state.filter.clear();
                        }
                        let _ = filter_resp; // keep response alive
                    }

                    let visible_count = self.state.filtered_requests().len();
                    let total = self.state.requests.len();
                    let counter = if visible_count == total {
                        format!("{} captured", total)
                    } else {
                        format!("{} of {} captured", visible_count, total)
                    };
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(counter)
                            .color(theme::dim_text_color(&ctx))
                            .small(),
                    );

                    if let Some(status) = self.state.current_status(STATUS_TTL) {
                        ui.add_space(8.0);
                        ui.label(RichText::new(status).color(theme::ACCENT).small().italics());
                    }

                    // Right-side controls.
                    //
                    // Each action is wrapped in `push_id` with a stable salt
                    // so egui's auto-id counter doesn't drift between passes
                    // when the icon glyph changes (theme cycles between
                    // sun/moon/auto, pause flips between ⏸/▶, etc). Without
                    // these, egui logs "changed id between passes" warnings
                    // because the rect-based fallback ids hash position +
                    // counter, both of which can shift with content.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.push_id("topbar-settings", |ui| {
                            if widgets::icon_button(ui, ICON_SETTINGS, "Settings (,)").clicked() {
                                self.open_settings();
                            }
                        });

                        ui.add_space(2.0);
                        ui.push_id("topbar-theme", |ui| {
                            let (theme_glyph, theme_label) = match self.state.settings.theme {
                                ThemePref::System => (ICON_AUTO, "Theme: system (T)"),
                                ThemePref::Dark => (ICON_MOON, "Theme: dark (T)"),
                                ThemePref::Light => (ICON_SUN, "Theme: light (T)"),
                            };
                            if widgets::icon_button(ui, theme_glyph, theme_label).clicked() {
                                self.cycle_theme();
                            }
                        });

                        ui.add_space(2.0);
                        ui.push_id("topbar-pause", |ui| {
                            let (pause_glyph, pause_label) = if self.state.paused {
                                (ICON_PLAY, "Resume capture (P)")
                            } else {
                                (ICON_PAUSE, "Pause capture (P)")
                            };
                            if widgets::icon_toggle(ui, pause_glyph, pause_label, self.state.paused)
                                .clicked()
                            {
                                self.state.paused = !self.state.paused;
                            }
                        });

                        ui.add_space(2.0);
                        ui.push_id("topbar-clear", |ui| {
                            if widgets::icon_button_colored(
                                ui,
                                ICON_TRASH,
                                "Clear captures (Shift+X)",
                                theme::DANGER,
                            )
                            .clicked()
                            {
                                self.clear_all();
                            }
                        });

                        ui.add_space(8.0);
                        ui.push_id("topbar-forward", |ui| {
                            self.render_forward_pill(ui);
                        });

                        ui.add_space(8.0);
                        widgets::status_dot(ui, true);
                    });
                });
            });
    }

    fn render_forward_pill(&mut self, ui: &mut egui::Ui) {
        let f = &self.state.settings.forward;
        let (label, value, accent) = if f.enabled && !f.url.is_empty() {
            let host = url::Url::parse(&f.url)
                .ok()
                .and_then(|u| u.host_str().map(|s| s.to_string()))
                .unwrap_or_else(|| f.url.clone());
            (
                "Forward",
                format!("{ICON_FORWARD} {host}"),
                Some(theme::ACCENT),
            )
        } else if !f.url.is_empty() {
            ("Forward", "off".to_string(), None)
        } else {
            ("Forward", "not set".to_string(), None)
        };
        let pill = widgets::label_pill_with_color(ui, label, &value, accent);
        let resp = pill.on_hover_text(if f.url.is_empty() {
            "Click to configure upstream URL"
        } else if f.enabled {
            "Click to open forward settings (Shift+click toggles off)"
        } else {
            "Click to open forward settings (Shift+click toggles on)"
        });
        if resp.clicked() {
            let shift = ui.input(|i| i.modifiers.shift);
            if shift && !f.url.is_empty() {
                self.toggle_forward_enabled();
            } else {
                self.open_settings_on(SettingsTab::Forward);
            }
        }
    }

    // ────────────────────────── Method chips ──────────────────────────

    fn render_methods_bar(&mut self, parent: &mut egui::Ui) {
        let ctx = parent.ctx().clone();
        egui::Panel::top("methods-bar")
            .exact_size(40.0)
            .frame(
                egui::Frame::default()
                    .fill(theme::elev_bg(&ctx))
                    .stroke(Stroke::new(1.0, theme::border_color(&ctx)))
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 6,
                        bottom: 6,
                    }),
            )
            .show_inside(parent, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new("METHODS")
                            .small()
                            .color(theme::dim_text_color(&ctx))
                            .strong(),
                    );
                    ui.add_space(8.0);
                    let buckets: Vec<&str> = METHOD_CHIPS
                        .iter()
                        .copied()
                        .chain(std::iter::once(OTHER_BUCKET))
                        .collect();
                    for bucket in buckets {
                        let selected = self.state.method_visible(bucket);
                        if widgets::method_chip(ui, bucket, selected).clicked() {
                            self.state.toggle_method(bucket);
                        }
                    }
                    ui.add_space(8.0);
                    if !self.state.method_filter_off.is_empty()
                        && ui
                            .small_button("Reset")
                            .on_hover_text("Show all methods")
                            .clicked()
                    {
                        self.state.reset_method_filter();
                    }
                });
            });
    }

    // ────────────────────────── Request list ──────────────────────────

    fn render_list(&mut self, parent: &mut egui::Ui) {
        let ctx = parent.ctx().clone();
        egui::Panel::left("list-pane")
            .resizable(true)
            .default_size(360.0)
            .min_size(280.0)
            .max_size(560.0)
            .frame(
                egui::Frame::default()
                    .fill(theme::elev_bg(&ctx))
                    .stroke(Stroke::new(1.0, theme::border_color(&ctx)))
                    .inner_margin(egui::Margin::same(0)),
            )
            .show_inside(parent, |ui| {
                let visible = self.state.filtered_requests();
                if visible.is_empty() {
                    self.render_empty_state(ui);
                    return;
                }
                // Each row prints a relative time ("12s ago", "5m ago"). egui
                // only redraws on input or our explicit broadcasts, so without
                // this nudge the labels would freeze between captures. 1s is
                // plenty fast for human-readable relative time and egui
                // de-dupes redundant repaint requests.
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs(1));
                let mut newly_selected: Option<uuid::Uuid> = None;
                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for req in &visible {
                            let selected = self.state.selected == Some(req.id);
                            let resp = render_request_row(ui, req, selected);
                            if resp.clicked() {
                                newly_selected = Some(req.id);
                            }
                        }
                    });
                if let Some(id) = newly_selected {
                    self.state.selected = Some(id);
                }
            });
    }

    fn render_empty_state(&self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(
                RichText::new("Waiting for requests")
                    .heading()
                    .color(theme::ACCENT),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Send anything to the capture URL")
                    .color(theme::muted_text_color(&ctx)),
            );
            ui.add_space(20.0);
            let snippet = format!("curl -X POST {}/hello -d 'world'", self.state.capture_url());
            egui::Frame::new()
                .fill(theme::elev2_bg(&ctx))
                .stroke(Stroke::new(1.0, theme::border_color(&ctx)))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.label(RichText::new(&snippet).monospace().small());
                });
            ui.add_space(8.0);
            if ui.small_button("Copy").clicked() {
                ui.ctx().copy_text(snippet);
            }
        });
    }

    // ────────────────────────── Detail pane ──────────────────────────

    fn render_detail(&mut self, parent: &mut egui::Ui) {
        let ctx = parent.ctx().clone();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(theme::elev_bg(&ctx))
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 18,
                        top: 14,
                        bottom: 14,
                    }),
            )
            .show_inside(parent, |ui| {
                let Some(req) = self.state.selected_request().cloned() else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(
                            RichText::new("Select a request to inspect")
                                .color(theme::dim_text_color(&ctx))
                                .heading(),
                        );
                    });
                    return;
                };
                self.render_detail_header(ui, &req);
                ui.add_space(10.0);
                self.render_tab_bar(ui, &req);
                ui.add_space(8.0);
                // The Body tab pins its toolbar + action row and only scrolls
                // the JSON / XML card in the middle, so it lays itself out
                // directly. Other tabs are simple lists / tables that can scroll
                // as a whole.
                match self.state.detail_tab {
                    DetailTab::Body => self.render_body(ui, &req),
                    DetailTab::Headers => {
                        egui::ScrollArea::vertical()
                            .id_salt("headers-scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| render_headers(ui, &req));
                    }
                    DetailTab::Query => {
                        egui::ScrollArea::vertical()
                            .id_salt("query-scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| render_query(ui, &req));
                    }
                    DetailTab::Raw => {
                        egui::ScrollArea::vertical()
                            .id_salt("raw-scroll")
                            .auto_shrink([false; 2])
                            .show(ui, |ui| render_raw(ui, &req));
                    }
                    DetailTab::Forwarded => self.render_forwarded(ui, &req),
                }
            });
    }

    fn render_detail_header(&self, ui: &mut egui::Ui, req: &CapturedRequest) {
        let ctx = ui.ctx().clone();
        let dim = theme::dim_text_color(&ctx);
        let path = if req.query.is_empty() {
            req.path.clone()
        } else {
            format!("{}?{}", req.path, req.query)
        };

        // Top: method + path. Path is the hero — shown big, monospace, strong.
        // We compute the right-side metadata first so it can right-align without
        // fighting the path label for space.
        ui.horizontal(|ui| {
            widgets::method_badge_sized(ui, &req.method, 68.0);
            ui.add_space(12.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Copy URL")
                    .on_hover_text("Copy method + path")
                    .clicked()
                {
                    ctx.copy_text(format!("{} {}", req.method, path));
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&path)
                                .monospace()
                                .strong()
                                .size(15.0)
                                .color(ui.visuals().strong_text_color()),
                        )
                        .truncate(),
                    );
                });
            });
        });
        ui.add_space(4.0);
        // Bottom: timestamp · size · remote — small dim text, separated by mid-dots.
        ui.horizontal(|ui| {
            let ts = req
                .received_at
                .with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M:%S%.3f")
                .to_string();
            ui.label(RichText::new(ts).small().color(dim));
            ui.label(RichText::new(" · ").small().color(dim));
            ui.label(
                RichText::new(humansize::format_size(
                    req.body_bytes_received as u64,
                    humansize::BINARY,
                ))
                .small()
                .color(dim),
            );
            ui.label(RichText::new(" · ").small().color(dim));
            ui.label(
                RichText::new(format!("from {}", req.remote_addr))
                    .small()
                    .color(dim),
            );
            ui.label(RichText::new(" · ").small().color(dim));
            ui.label(RichText::new(&req.version).small().color(dim));
            if req.body_truncated {
                ui.add_space(8.0);
                ui.colored_label(theme::WARNING, RichText::new("truncated").small());
            }
        });
    }

    fn render_tab_bar(&mut self, ui: &mut egui::Ui, req: &CapturedRequest) {
        // Forwarded tab is conditional: it only shows when the captured
        // request has been forwarded (or replayed) at least once. If we land
        // on a request that has lost its forward outcome and Forwarded was
        // selected, fall back to Body so the user isn't stuck on a blank tab.
        let has_forward = !req.forwards.is_empty();
        if matches!(self.state.detail_tab, DetailTab::Forwarded) && !has_forward {
            self.state.detail_tab = DetailTab::Body;
        }

        ui.horizontal(|ui| {
            for (tab, label) in [
                (DetailTab::Body, "Body"),
                (DetailTab::Headers, "Headers"),
                (DetailTab::Query, "Query"),
                (DetailTab::Raw, "Raw"),
            ] {
                let selected = self.state.detail_tab == tab;
                if tab_button(ui, label, selected).clicked() {
                    self.state.detail_tab = tab;
                }
            }
            if has_forward {
                let selected = self.state.detail_tab == DetailTab::Forwarded;
                let label = forwarded_tab_label(req);
                if tab_button(ui, &label, selected).clicked() {
                    self.state.detail_tab = DetailTab::Forwarded;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if matches!(self.state.detail_tab, DetailTab::Body) {
                    egui::ComboBox::from_id_salt("body-format")
                        .width(110.0)
                        .selected_text(format_label(self.state.body_format))
                        .show_ui(ui, |ui| {
                            for fmt in [
                                BodyFormat::Auto,
                                BodyFormat::Pretty,
                                BodyFormat::Raw,
                                BodyFormat::Hex,
                            ] {
                                ui.selectable_value(
                                    &mut self.state.body_format,
                                    fmt,
                                    format_label(fmt),
                                );
                            }
                        });
                    ui.label(
                        RichText::new("Format")
                            .color(theme::dim_text_color(ui.ctx()))
                            .small(),
                    );
                }
            });
        });
    }

    fn render_body(&self, ui: &mut egui::Ui, req: &CapturedRequest) {
        let ctx = ui.ctx().clone();
        let formatted: FormattedBody =
            format_body(&req.body, req.content_type(), self.state.body_format);

        let pick = match self.state.body_format {
            BodyFormat::Auto | BodyFormat::Pretty => {
                highlight::detect(req.content_type(), &formatted.text)
            }
            BodyFormat::Raw | BodyFormat::Hex => Highlighter::None,
        };
        let show_tree =
            matches!(self.state.body_format, BodyFormat::Auto) && pick == Highlighter::Json;
        let body_text_for_actions = formatted.text.clone();

        // The body is laid out as three stacked regions: a fixed-height toolbar
        // at the top, a fixed-height action row at the bottom, and a scrollable
        // card filling the gap between them. We pre-allocate the bottom row
        // first (using `with_layout(bottom_up)`) so the scroll area knows
        // exactly how much vertical space it can claim.
        let action_row_h = 36.0;

        // ── Top toolbar: stays pinned at the top, never scrolls. ──────────
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(humansize::format_size(
                    req.body_bytes_received as u64,
                    humansize::BINARY,
                ))
                .small()
                .color(theme::muted_text_color(&ctx)),
            );
            if let Some(ct) = req.content_type() {
                ui.add_space(8.0);
                content_type_chip(ui, ct);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if show_tree {
                    if ui
                        .small_button("Expand all")
                        .on_hover_text("Open every collapsible JSON node")
                        .clicked()
                    {
                        crate::tree::set_all_open(&ctx, &formatted.text, true);
                    }
                    if ui
                        .small_button("Collapse all")
                        .on_hover_text("Close every collapsible JSON node")
                        .clicked()
                    {
                        crate::tree::set_all_open(&ctx, &formatted.text, false);
                    }
                }
            });
        });
        ui.add_space(8.0);

        if let Some(notice) = &formatted.notice {
            ui.label(
                RichText::new(notice)
                    .color(theme::dim_text_color(&ctx))
                    .small()
                    .italics(),
            );
            ui.add_space(6.0);
        }

        // Reserve the action row at the bottom first; the scroll area then
        // claims the remaining vertical space above it.
        let scroll_h = (ui.available_height() - action_row_h - 8.0).max(80.0);

        if !formatted.text.is_empty() {
            let body_text = formatted.text.clone();
            egui::ScrollArea::vertical()
                .id_salt("body-card-scroll")
                .max_height(scroll_h)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    crate::tree::body_card(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        let rendered_tree = if show_tree {
                            crate::tree::try_render(ui, &body_text)
                        } else {
                            false
                        };
                        if !rendered_tree {
                            let layout_job = match pick {
                                Highlighter::Json => highlight::json_layout(&body_text, ui.ctx()),
                                Highlighter::Xml => highlight::xml_layout(&body_text, ui.ctx()),
                                Highlighter::None => highlight::plain_layout(
                                    &body_text,
                                    ui.ctx(),
                                    ui.visuals().text_color(),
                                ),
                            };
                            ui.add(egui::Label::new(layout_job).selectable(true));
                        }
                    });
                });
        } else {
            // Even with empty bodies we want the action row to land in the
            // same place, so claim the scroll area's height as filler.
            ui.allocate_space(egui::vec2(ui.available_width(), scroll_h));
        }

        ui.add_space(8.0);

        // ── Bottom action row: pinned, never scrolls. ─────────────────────
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Copy body")
                    .on_hover_text("Copy the body text to the clipboard")
                    .clicked()
                {
                    ctx.copy_text(body_text_for_actions.clone());
                }
                ui.add_space(4.0);
                if ui
                    .small_button("Download raw")
                    .on_hover_text("Save the raw body bytes to the Downloads folder")
                    .clicked()
                {
                    let _ = save_body_to_disk(&req.body, req);
                }
            });
        });
    }

    // ────────────────────────── Forwarded tab ──────────────────────────

    fn render_forwarded(&mut self, ui: &mut egui::Ui, req: &CapturedRequest) {
        let ctx = ui.ctx().clone();
        let id = req.id;
        let n = req.forwards.len();
        if n == 0 {
            return;
        }
        let current_idx = self.state.forward_index_for(req).unwrap_or(n - 1);
        let outcome = req.forwards[current_idx].clone();
        let pinned = self.state.forward_selection.contains_key(&id);
        let flash = self.state.forward_flash;

        // ── Action row: count + Latest + Replay ────────────────────────
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} attempt{}", n, if n == 1 { "" } else { "s" })).strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("Replay")
                    .on_hover_text(
                        "Re-fire this captured request through the current forward target",
                    )
                    .clicked()
                {
                    self.spawn_replay(id);
                }
                if pinned
                    && ui
                        .small_button("Follow latest")
                        .on_hover_text("Stop pinning to this row and follow new replays")
                        .clicked()
                {
                    self.state.forward_selection.remove(&id);
                }
            });
        });
        ui.add_space(8.0);

        // ── Attempts table ─────────────────────────────────────────────
        let mut new_pin: Option<usize> = None;
        egui::Frame::new()
            .fill(theme::elev_bg(&ctx))
            .stroke(Stroke::new(1.0, theme::border_color(&ctx)))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                forward_table_header(ui);
                // Newest first reads naturally — most recent replay on top.
                for (idx, attempt) in req.forwards.iter().enumerate().rev() {
                    let selected = idx == current_idx;
                    let flashing = flash.is_some_and(|(rid, fidx, t)| {
                        rid == id
                            && fidx == idx
                            && t.elapsed() < std::time::Duration::from_millis(900)
                    });
                    if forward_table_row(ui, idx, attempt, selected, flashing) {
                        new_pin = Some(idx);
                    }
                    if flashing {
                        ctx.request_repaint_after(std::time::Duration::from_millis(50));
                    }
                }
            });
        if let Some(idx) = new_pin {
            self.state.forward_selection.insert(id, idx);
        }
        ui.add_space(10.0);

        // ── Detail of the selected attempt ──────────────────────────────
        egui::ScrollArea::vertical()
            .id_salt("forwarded-detail-scroll")
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                forward_outcome_detail(ui, &outcome);
            });
    }

    // ────────────────────────── Settings dialog ──────────────────────────

    fn render_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.state.editing_settings {
            return;
        }
        let mut open = self.state.editing_settings;
        let mut save_clicked = false;
        let mut cancel_clicked = false;
        let mut reset_clicked = false;
        egui::Window::new("Settings")
            .open(&mut open)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size([560.0, 460.0])
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .inner_margin(egui::Margin::same(0))
                    .corner_radius(egui::CornerRadius::same(12)),
            )
            .show(ctx, |ui| {
                let dialog_w = 560.0;
                ui.set_min_width(dialog_w);
                ui.set_max_width(dialog_w);
                ui.spacing_mut().item_spacing.y = 0.0;

                // ── Header strip with tabs ──────────────────────────────
                egui::Frame::new()
                    .fill(theme::elev_bg(ui.ctx()))
                    .stroke(Stroke::NONE)
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 18,
                        top: 14,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Settings")
                                    .heading()
                                    .strong()
                                    .color(ui.visuals().strong_text_color()),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if widgets::close_button(ui, "Close (Esc)").clicked() {
                                        cancel_clicked = true;
                                    }
                                },
                            );
                        });
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            for (tab, label) in [
                                (SettingsTab::Capture, "Capture"),
                                (SettingsTab::Forward, "Forward"),
                                (SettingsTab::Appearance, "Appearance"),
                                (SettingsTab::Advanced, "Advanced"),
                            ] {
                                let selected = self.state.settings_tab == tab;
                                if settings_tab_button(ui, label, selected).clicked() {
                                    self.state.settings_tab = tab;
                                }
                            }
                        });
                        // Bottom hairline under the tab strip.
                        let p = ui.painter();
                        let r = ui.max_rect();
                        p.line_segment(
                            [
                                egui::pos2(r.left() - 18.0, r.bottom() + 4.0),
                                egui::pos2(r.right() + 18.0, r.bottom() + 4.0),
                            ],
                            Stroke::new(1.0, theme::border_color(ui.ctx())),
                        );
                    });

                // ── Body of the active tab ──────────────────────────────
                egui::Frame::new()
                    .fill(ui.visuals().window_fill)
                    .inner_margin(egui::Margin {
                        left: 22,
                        right: 22,
                        top: 18,
                        bottom: 6,
                    })
                    .show(ui, |ui| {
                        ui.set_min_height(260.0);
                        ui.spacing_mut().item_spacing.y = 10.0;
                        match self.state.settings_tab {
                            SettingsTab::Capture => self.render_settings_capture(ui),
                            SettingsTab::Forward => self.render_settings_forward(ui),
                            SettingsTab::Appearance => self.render_settings_appearance(ui),
                            SettingsTab::Advanced => self.render_settings_advanced(ui),
                        }

                        if let Some(err) = &self.state.pending_settings_error {
                            ui.add_space(8.0);
                            egui::Frame::new()
                                .fill(theme::DANGER.linear_multiply(0.18))
                                .stroke(Stroke::new(1.0, theme::DANGER))
                                .corner_radius(egui::CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(err).color(theme::DANGER).strong());
                                });
                        }
                    });

                // ── Footer ──────────────────────────────────────────────
                egui::Frame::new()
                    .fill(theme::elev_bg(ui.ctx()))
                    .stroke(Stroke {
                        width: 1.0,
                        color: theme::border_color(ui.ctx()),
                    })
                    .inner_margin(egui::Margin {
                        left: 18,
                        right: 18,
                        top: 12,
                        bottom: 12,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new("Reset to defaults")
                                        .corner_radius(egui::CornerRadius::same(6))
                                        .min_size(egui::vec2(140.0, 32.0)),
                                )
                                .clicked()
                            {
                                reset_clicked = true;
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let save_text =
                                        RichText::new("Save").color(Color32::WHITE).strong();
                                    if ui
                                        .add(
                                            egui::Button::new(save_text)
                                                .fill(theme::ACCENT)
                                                .corner_radius(egui::CornerRadius::same(6))
                                                .min_size(egui::vec2(96.0, 32.0)),
                                        )
                                        .clicked()
                                    {
                                        save_clicked = true;
                                    }
                                    ui.add_space(4.0);
                                    if ui
                                        .add(
                                            egui::Button::new("Cancel")
                                                .corner_radius(egui::CornerRadius::same(6))
                                                .min_size(egui::vec2(80.0, 32.0)),
                                        )
                                        .clicked()
                                    {
                                        cancel_clicked = true;
                                    }
                                },
                            );
                        });
                    });
            });
        if !open {
            cancel_clicked = true;
        }
        if reset_clicked {
            self.state.pending_settings = Settings::default();
            self.state.pending_settings_error = None;
        }
        if cancel_clicked {
            self.state.editing_settings = false;
            self.state.pending_settings = self.state.settings.clone();
            self.state.pending_settings_error = None;
        }
        if save_clicked {
            match self.apply_pending_settings() {
                Ok(()) => {
                    self.state.editing_settings = false;
                    self.state.pending_settings_error = None;
                    self.state.status("Settings saved");
                }
                Err(e) => {
                    self.state.pending_settings_error = Some(format!("{e:#}"));
                }
            }
        }
    }

    fn render_settings_capture(&mut self, ui: &mut egui::Ui) {
        // Two-column grid keeps the dialog compact.
        settings_grid(ui, |grid| {
            grid.row("Bind address", |ui| {
                styled_text_input(ui, &mut self.state.pending_settings.bind, "0.0.0.0");
            });
            grid.row("Port", |ui| {
                styled_drag_input(
                    ui,
                    egui::DragValue::new(&mut self.state.pending_settings.port)
                        .range(0u16..=65535u16)
                        .speed(1.0),
                );
            });
            grid.row("Buffer (requests)", |ui| {
                styled_drag_input(
                    ui,
                    egui::DragValue::new(&mut self.state.pending_settings.buffer_size)
                        .range(1usize..=100_000usize)
                        .speed(10.0)
                        .suffix(" reqs"),
                );
            });
            grid.row("Max body size", |ui| {
                styled_drag_input(
                    ui,
                    egui::DragValue::new(&mut self.state.pending_settings.max_body_size)
                        .range(1usize..=usize::MAX)
                        .speed(1024.0)
                        .suffix(" bytes"),
                );
            });
        });
    }

    fn render_settings_forward(&mut self, ui: &mut egui::Ui) {
        ui.add(toggle_row(
            "Forward each captured request upstream",
            &mut self.state.pending_settings.forward.enabled,
        ));
        ui.add_space(6.0);
        ui.add_enabled_ui(self.state.pending_settings.forward.enabled, |ui| {
            settings_grid(ui, |grid| {
                grid.row("Upstream URL", |ui| {
                    styled_text_input(
                        ui,
                        &mut self.state.pending_settings.forward.url,
                        "https://api.example.com",
                    );
                });
                grid.row("Timeout", |ui| {
                    styled_drag_input(
                        ui,
                        egui::DragValue::new(&mut self.state.pending_settings.forward.timeout_secs)
                            .range(1u64..=600u64)
                            .suffix(" s"),
                    );
                });
            });
            ui.add_space(4.0);
            ui.add(toggle_row(
                "Skip TLS verification (dev only)",
                &mut self.state.pending_settings.forward.insecure,
            ));
        });
    }

    fn render_settings_appearance(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Theme")
                .small()
                .strong()
                .color(theme::muted_text_color(ui.ctx())),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            for (variant, glyph, label) in [
                (ThemePref::System, ICON_AUTO, "System"),
                (ThemePref::Dark, ICON_MOON, "Dark"),
                (ThemePref::Light, ICON_SUN, "Light"),
            ] {
                let selected = self.state.pending_settings.theme == variant;
                if theme_card(ui, glyph, label, selected).clicked() {
                    self.state.pending_settings.theme = variant;
                }
                ui.add_space(8.0);
            }
        });
    }

    fn render_settings_advanced(&mut self, ui: &mut egui::Ui) {
        settings_grid(ui, |grid| {
            grid.row("Log file", |ui| {
                let mut log_path = self
                    .state
                    .pending_settings
                    .log_file
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                let changed =
                    styled_text_input(ui, &mut log_path, "(leave blank to disable file logging)")
                        .changed();
                if changed {
                    self.state.pending_settings.log_file = if log_path.trim().is_empty() {
                        None
                    } else {
                        Some(log_path.into())
                    };
                }
            });
        });
        ui.add_space(8.0);
        ui.add(toggle_row(
            "Skip update check on startup",
            &mut self.state.pending_settings.no_update_check,
        ));
    }

    // ────────────────────────── Hotkeys ──────────────────────────

    fn handle_hotkeys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::J)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
            {
                self.state.select_relative(1);
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::K)
                || i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
            {
                self.state.select_relative(-1);
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::G) {
                self.state.select_first_visible();
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::P) {
                self.state.paused = !self.state.paused;
            }
            if i.consume_key(egui::Modifiers::SHIFT, egui::Key::X) {
                self.clear_all();
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::T) {
                self.cycle_theme();
            }
            for (key, tab) in [
                (egui::Key::Num1, DetailTab::Body),
                (egui::Key::Num2, DetailTab::Headers),
                (egui::Key::Num3, DetailTab::Query),
                (egui::Key::Num4, DetailTab::Raw),
            ] {
                if i.consume_key(egui::Modifiers::NONE, key) {
                    self.state.detail_tab = tab;
                }
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Comma)
                && !self.state.editing_settings
            {
                self.state.editing_settings = true;
                self.state.pending_settings = self.state.settings.clone();
                self.state.pending_settings_error = None;
            }
        });
    }

    fn apply_pending_settings(&mut self) -> anyhow::Result<()> {
        self.state
            .pending_settings
            .validate()
            .map_err(anyhow::Error::msg)?;
        let new_settings = self.state.pending_settings.clone();
        new_settings
            .save(&self.state.settings_path)
            .with_context(|| {
                format!("writing settings to {}", self.state.settings_path.display())
            })?;

        let forward_cfg = forward_from_settings(&new_settings.forward)?;
        let switch = self.state.supervisor.config().forward.clone();
        let runtime = self.state.runtime.clone();
        runtime.block_on(async move {
            *switch.write().await = forward_cfg;
        });

        let bind: std::net::IpAddr = new_settings.bind.parse().context("parsing bind")?;
        let want_port = new_settings.port;
        let current_addr = self.state.supervisor.current_addr();
        let need_rebind =
            bind != current_addr.ip() || (want_port != 0 && want_port != current_addr.port());
        if need_rebind {
            let supervisor = self.state.supervisor.clone();
            self.state
                .runtime
                .block_on(async move { supervisor.reconfigure(bind, want_port).await })?;
        }

        self.state.settings = new_settings;
        Ok(())
    }

    fn open_settings(&mut self) {
        self.open_settings_on(self.state.settings_tab);
    }

    /// Re-fire the captured request through the *current* forward target.
    /// Runs entirely in the background tokio runtime; the result lands back
    /// in the UI via the existing store broadcast (`StoreEvent::ForwardUpdated`).
    fn spawn_replay(&self, id: uuid::Uuid) {
        let store = self.state.supervisor.store();
        let switch = self.state.supervisor.config().forward.clone();
        let runtime = self.state.runtime.clone();

        let Some(req) = store.get(id) else {
            return;
        };

        runtime.spawn(async move {
            let forward = switch.read().await.clone();
            let outcome = match forward {
                None => postbin_ultra::request::ForwardOutcome {
                    started_at: chrono::Utc::now(),
                    upstream_url: String::new(),
                    status: postbin_ultra::request::ForwardStatus::Skipped {
                        reason: "no forward target configured — set an Upstream URL in Settings → Forward".into(),
                    },
                },
                Some(cfg) => {
                    // Reconstruct the request inputs that capture.rs needs.
                    let mut method = http::Method::GET;
                    if let Ok(m) = req.method.parse::<http::Method>() {
                        method = m;
                    }
                    let mut headers = http::HeaderMap::with_capacity(req.headers.len());
                    for (k, v) in &req.headers {
                        if let (Ok(name), Ok(val)) = (
                            http::HeaderName::from_bytes(k.as_bytes()),
                            http::HeaderValue::from_str(v),
                        ) {
                            headers.append(name, val);
                        }
                    }
                    let remote: std::net::SocketAddr = req
                        .remote_addr
                        .parse()
                        .unwrap_or_else(|_| "127.0.0.1:0".parse().unwrap());
                    postbin_ultra::capture::do_forward(
                        &cfg,
                        method,
                        &req.path,
                        &req.query,
                        &headers,
                        remote,
                        req.body.clone(),
                        req.body_truncated,
                    )
                    .await
                }
            };
            let _ = store.append_forward(id, outcome);
        });
    }

    fn open_settings_on(&mut self, tab: SettingsTab) {
        self.state.editing_settings = true;
        self.state.settings_tab = tab;
        self.state.pending_settings = self.state.settings.clone();
        self.state.pending_settings_error = None;
    }

    fn cycle_theme(&mut self) {
        self.state.settings.theme = match self.state.settings.theme {
            ThemePref::System => ThemePref::Dark,
            ThemePref::Dark => ThemePref::Light,
            ThemePref::Light => ThemePref::System,
        };
        let _ = self.state.settings.save(&self.state.settings_path);
    }

    fn toggle_forward_enabled(&mut self) {
        self.state.settings.forward.enabled = !self.state.settings.forward.enabled;
        // Persist + push to supervisor.
        let cfg = match forward_from_settings(&self.state.settings.forward) {
            Ok(c) => c,
            Err(_) => return,
        };
        let switch = self.state.supervisor.config().forward.clone();
        let runtime = self.state.runtime.clone();
        runtime.block_on(async move {
            *switch.write().await = cfg;
        });
        let _ = self.state.settings.save(&self.state.settings_path);
        let msg = if self.state.settings.forward.enabled {
            "Forward enabled"
        } else {
            "Forward disabled"
        };
        self.state.status(msg);
    }

    fn clear_all(&mut self) {
        self.state.requests.clear();
        self.state.selected = None;
        let store = self.state.supervisor.store();
        store.clear();
    }
}

fn format_label(fmt: BodyFormat) -> &'static str {
    match fmt {
        BodyFormat::Auto => "Auto",
        BodyFormat::Pretty => "Pretty",
        BodyFormat::Raw => "Raw",
        BodyFormat::Hex => "Hex",
    }
}

/// Two-column settings grid: small label on the left, input on the right,
/// inputs all the same width so the dialog reads as a coherent form.
struct SettingsGrid<'a> {
    ui: &'a mut egui::Ui,
    label_width: f32,
    input_width: f32,
}

impl<'a> SettingsGrid<'a> {
    fn row(&mut self, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
        self.ui.horizontal(|ui| {
            ui.set_min_height(32.0);
            ui.add_sized(
                [self.label_width, 30.0],
                egui::Label::new(
                    RichText::new(label)
                        .small()
                        .strong()
                        .color(theme::muted_text_color(ui.ctx())),
                )
                .selectable(false),
            );
            // Right side: fixed width so all inputs align.
            let input_w = self.input_width;
            ui.scope(|ui| {
                ui.set_min_width(input_w);
                ui.set_max_width(input_w);
                contents(ui);
            });
        });
    }
}

fn settings_grid(ui: &mut egui::Ui, build: impl FnOnce(&mut SettingsGrid<'_>)) {
    let total = ui.available_width();
    let label_w = 130.0;
    let input_w = (total - label_w - 12.0).max(220.0);
    let mut grid = SettingsGrid {
        ui,
        label_width: label_w,
        input_width: input_w,
    };
    build(&mut grid);
}

/// Polished, full-width text input. Returns the inner Response so callers
/// can react to `.changed()`.
fn styled_text_input(ui: &mut egui::Ui, value: &mut String, placeholder: &str) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(placeholder)
            .desired_width(f32::INFINITY)
            .margin(egui::vec2(10.0, 7.0))
            .font(egui::TextStyle::Body),
    )
}

/// DragValue wrapped to feel like a numeric input field rather than a chip.
fn styled_drag_input(ui: &mut egui::Ui, drag: egui::DragValue<'_>) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(Stroke::new(1.0, theme::border_color(ui.ctx())))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 6));
    frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(drag);
            });
        })
        .response
}

/// Big, full-width checkbox row — gives toggles a proper presence in the
/// dialog instead of looking lost in line with text.
fn toggle_row<'a>(label: &'a str, value: &'a mut bool) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| {
        let frame = egui::Frame::new()
            .fill(theme::elev2_bg(ui.ctx()))
            .stroke(Stroke::new(1.0, theme::border_color(ui.ctx())))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(12, 10));
        frame
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                widgets::nice_checkbox(ui, value, label);
            })
            .response
    }
}

/// One of three theme cards (System / Dark / Light) — a chunky, glanceable
/// option tile.
fn theme_card(ui: &mut egui::Ui, glyph: &str, label: &str, selected: bool) -> egui::Response {
    let bg = if selected {
        theme::accent_soft(ui.ctx())
    } else {
        theme::elev2_bg(ui.ctx())
    };
    let border = if selected {
        theme::ACCENT
    } else {
        theme::border_color(ui.ctx())
    };
    let frame = egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(18, 14));
    // Lock to a fixed footprint — vertical_centered is greedy and would
    // otherwise expand each card to fill the dialog width.
    let card_size = egui::vec2(150.0, 86.0);
    let inner = frame
        .show(ui, |ui| {
            ui.set_min_size(card_size);
            ui.set_max_size(card_size);
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(glyph).size(22.0));
                ui.add_space(4.0);
                ui.label(RichText::new(label).strong().color(if selected {
                    theme::ACCENT_STRONG
                } else {
                    ui.visuals().text_color()
                }));
            });
        })
        .response;
    inner
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Tab button used inside the settings dialog header.
fn settings_tab_button(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let text_color = if selected {
        theme::ACCENT
    } else {
        theme::muted_text_color(ui.ctx())
    };
    let frame = egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .inner_margin(egui::Margin {
            left: 14,
            right: 14,
            top: 8,
            bottom: 10,
        });
    let resp = frame
        .show(ui, |ui| {
            ui.label(RichText::new(label).strong().size(13.5).color(text_color));
        })
        .response;
    // Underline for active tab.
    if selected {
        let r = resp.rect;
        ui.painter().line_segment(
            [
                egui::pos2(r.left() + 4.0, r.bottom() - 1.0),
                egui::pos2(r.right() - 4.0, r.bottom() - 1.0),
            ],
            Stroke::new(2.0, theme::ACCENT),
        );
    }
    resp.interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Tab-strip button with an underline accent when selected.
fn tab_button(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let text_color = if selected {
        ui.visuals().text_color()
    } else {
        theme::muted_text_color(ui.ctx())
    };
    let frame = egui::Frame::new()
        .fill(if selected {
            theme::accent_soft(ui.ctx())
        } else {
            Color32::TRANSPARENT
        })
        .stroke(Stroke::NONE)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 6));
    let resp = frame
        .show(ui, |ui| {
            ui.label(RichText::new(label).strong().color(text_color).size(13.0));
        })
        .response;
    resp.interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Compact single-row layout matching the web UI:
///   `[METHOD]  /api/v1/some/long/path…           42s ago`
///   `                                              123 B`
/// Manual rect allocation gives pixel-perfect three-column layout: fixed
/// badge | flexible truncated path | fixed right metadata column.
fn render_request_row(ui: &mut egui::Ui, req: &CapturedRequest, selected: bool) -> egui::Response {
    const ROW_H: f32 = 38.0;
    const PAD_X: f32 = 14.0;
    const BADGE_W: f32 = 60.0;
    const BADGE_GAP: f32 = 10.0;
    const RIGHT_W: f32 = 72.0; // wide enough for "59m ago" / "999 KiB"
    const RIGHT_GAP: f32 = 10.0;

    let ctx = ui.ctx().clone();
    let dim = theme::dim_text_color(&ctx);

    let id = ui.make_persistent_id(("req-row", req.id));
    let full_w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(full_w, ROW_H), egui::Sense::click());

    // ── Background, hairline divider, accent left bar ─────────────────
    let bg = if selected {
        theme::accent_soft(&ctx)
    } else if resp.hovered() {
        theme::soft_bg(&ctx)
    } else {
        Color32::TRANSPARENT
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::CornerRadius::ZERO, bg);
    painter.line_segment(
        [
            egui::pos2(rect.left(), rect.bottom() - 0.5),
            egui::pos2(rect.right(), rect.bottom() - 0.5),
        ],
        Stroke::new(1.0, theme::border_color(&ctx)),
    );
    if selected {
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(3.0, ROW_H)),
            egui::CornerRadius::ZERO,
            theme::ACCENT,
        );
    }

    // ── Method badge — left-anchored, vertically centred ──────────────
    let cy = rect.center().y;
    let badge_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + PAD_X + BADGE_W / 2.0, cy),
        egui::vec2(BADGE_W, 24.0),
    );
    {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(badge_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        widgets::method_badge_sized(&mut child, &req.method, BADGE_W);
    }

    // ── Right column — time-ago over size, right-aligned ──────────────
    let right_rect = egui::Rect::from_min_max(
        egui::pos2(rect.right() - PAD_X - RIGHT_W, rect.top() + 4.0),
        egui::pos2(rect.right() - PAD_X, rect.bottom() - 4.0),
    );
    let time_ago = crate::state::humanize_relative(req.received_at, chrono::Utc::now());
    let size = humansize::format_size(req.body_bytes_received as u64, humansize::BINARY);
    {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(right_rect)
                .layout(egui::Layout::top_down(egui::Align::Max)),
        );
        child.spacing_mut().item_spacing.y = 1.0;
        child
            .add(egui::Label::new(RichText::new(time_ago).size(10.5).color(dim)).selectable(false));
        child.add(egui::Label::new(RichText::new(size).size(10.5).color(dim)).selectable(false));
    }

    // ── Path — fills the gap between badge and right column, truncates ──
    let path = if req.query.is_empty() {
        req.path.clone()
    } else {
        format!("{}?{}", req.path, req.query)
    };
    let path_left = rect.left() + PAD_X + BADGE_W + BADGE_GAP;
    let path_right = rect.right() - PAD_X - RIGHT_W - RIGHT_GAP;
    let path_rect = egui::Rect::from_min_max(
        egui::pos2(path_left, cy - 9.0),
        egui::pos2(path_right, cy + 9.0),
    );
    {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(path_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        let path_color = if selected {
            child.visuals().strong_text_color()
        } else {
            child.visuals().text_color()
        };
        child.add(
            egui::Label::new(RichText::new(path).monospace().color(path_color).size(12.5))
                .truncate(),
        );
    }

    // Click target covers the whole row.
    ui.interact(rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn render_headers(ui: &mut egui::Ui, req: &CapturedRequest) {
    if req.headers.is_empty() {
        ui.label(RichText::new("(no headers)").italics());
        return;
    }
    egui::Grid::new("headers-grid")
        .num_columns(2)
        .spacing([18.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for (k, v) in &req.headers {
                ui.label(
                    RichText::new(k)
                        .monospace()
                        .strong()
                        .color(theme::ACCENT_STRONG),
                );
                ui.label(RichText::new(v).monospace());
                ui.end_row();
            }
        });
}

fn render_query(ui: &mut egui::Ui, req: &CapturedRequest) {
    if req.query.is_empty() {
        ui.label(RichText::new("(no query string)").italics());
        return;
    }
    let pairs: Vec<(&str, &str)> = req
        .query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (k, v)
        })
        .collect();
    egui::Grid::new("query-grid")
        .num_columns(2)
        .spacing([18.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            for (k, v) in pairs {
                ui.label(
                    RichText::new(k)
                        .monospace()
                        .strong()
                        .color(theme::ACCENT_STRONG),
                );
                ui.label(RichText::new(v).monospace());
                ui.end_row();
            }
        });
}

fn render_raw(ui: &mut egui::Ui, req: &CapturedRequest) {
    let text = build_raw(req);
    let layout = highlight::plain_layout(&text, ui.ctx(), ui.visuals().text_color());
    egui::Frame::new()
        .fill(theme::elev2_bg(ui.ctx()))
        .stroke(Stroke::new(1.0, theme::border_color(ui.ctx())))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.add(egui::Label::new(layout).selectable(true));
        });
}

/// Small monospaced badge showing the content-type. Sits next to the body
/// toolbar so users can tell at a glance what they're looking at.
/// Tab strip label for the Forwarded tab — shows status code (or "err"/"skip")
/// inline so users can see the outcome without clicking through.
fn forwarded_tab_label(req: &CapturedRequest) -> String {
    let Some(outcome) = req.latest_forward() else {
        return "Forwarded".into();
    };
    let suffix = match &outcome.status {
        ForwardStatus::Success { status_code, .. } => format!(" {}", status_code),
        ForwardStatus::Skipped { .. } => " skip".into(),
        ForwardStatus::Error { .. } => " err".into(),
    };
    let count = req.forwards.len();
    if count > 1 {
        format!("Forwarded ({}){}", count, suffix)
    } else {
        format!("Forwarded{}", suffix)
    }
}

/// Header row for the attempts table — small uppercase labels with the same
/// column widths as `forward_table_row`.
fn forward_table_header(ui: &mut egui::Ui) {
    let dim = theme::dim_text_color(ui.ctx());
    egui::Frame::new()
        .fill(theme::elev2_bg(ui.ctx()))
        .stroke(Stroke::NONE)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let head = |t: &str| RichText::new(t).small().strong().color(dim);
                ui.add_sized([34.0, 16.0], egui::Label::new(head("#")).selectable(false));
                ui.add_sized(
                    [88.0, 16.0],
                    egui::Label::new(head("STATUS")).selectable(false),
                );
                ui.add_sized(
                    [108.0, 16.0],
                    egui::Label::new(head("TIME")).selectable(false),
                );
                ui.add_sized(
                    [62.0, 16.0],
                    egui::Label::new(head("LATENCY")).selectable(false),
                );
                ui.label(head("UPSTREAM"));
            });
        });
    // Hairline under the header row.
    let r = ui.min_rect();
    ui.painter().line_segment(
        [
            egui::pos2(r.left(), r.bottom() - 0.5),
            egui::pos2(r.right(), r.bottom() - 0.5),
        ],
        Stroke::new(1.0, theme::border_color(ui.ctx())),
    );
}

/// Single row in the attempts table. Returns `true` when the user clicks it
/// (caller pins this index as the selection).
fn forward_table_row(
    ui: &mut egui::Ui,
    idx: usize,
    outcome: &postbin_ultra::request::ForwardOutcome,
    selected: bool,
    flashing: bool,
) -> bool {
    let ctx = ui.ctx().clone();
    let dim = theme::dim_text_color(&ctx);
    let bg = if flashing {
        theme::ACCENT.linear_multiply(0.45)
    } else if selected {
        theme::accent_soft(&ctx)
    } else {
        Color32::TRANSPARENT
    };
    let frame = egui::Frame::new()
        .fill(bg)
        .stroke(Stroke::NONE)
        .inner_margin(egui::Margin::symmetric(10, 8));
    let row_resp = frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add_sized(
                    [34.0, 18.0],
                    egui::Label::new(
                        RichText::new(format!("#{}", idx + 1))
                            .monospace()
                            .small()
                            .color(dim),
                    )
                    .selectable(false),
                );
                ui.scope(|ui| {
                    ui.set_max_width(88.0);
                    forward_status_pill(ui, &outcome.status);
                });
                let time = outcome
                    .started_at
                    .with_timezone(&chrono::Local)
                    .format("%H:%M:%S%.3f")
                    .to_string();
                ui.add_sized(
                    [108.0, 18.0],
                    egui::Label::new(RichText::new(time).monospace().small().color(dim))
                        .selectable(false),
                );
                let lat = forward_duration_ms(&outcome.status)
                    .map(|d| format!("{} ms", d))
                    .unwrap_or_default();
                ui.add_sized(
                    [62.0, 18.0],
                    egui::Label::new(RichText::new(lat).monospace().small().color(dim))
                        .selectable(false),
                );
                ui.add(
                    egui::Label::new(RichText::new(&outcome.upstream_url).monospace().small())
                        .truncate(),
                );
            });
        })
        .response;
    // Hairline divider between rows.
    let r = row_resp.rect;
    ui.painter().line_segment(
        [
            egui::pos2(r.left(), r.bottom() - 0.5),
            egui::pos2(r.right(), r.bottom() - 0.5),
        ],
        Stroke::new(1.0, theme::border_color(&ctx)),
    );
    let id = ui.make_persistent_id(("forward-row", idx));
    let resp = ui
        .interact(row_resp.rect, id, egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    resp.clicked()
}

/// Detail panel for a single forward attempt — upstream URL, headers grid,
/// body card. Pure rendering, no `&mut self` borrow.
fn forward_outcome_detail(ui: &mut egui::Ui, outcome: &postbin_ultra::request::ForwardOutcome) {
    let ctx = ui.ctx().clone();
    let dim = theme::dim_text_color(&ctx);

    // Upstream URL row.
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Upstream")
                .small()
                .strong()
                .color(theme::muted_text_color(&ctx)),
        );
        ui.add(
            egui::Label::new(
                RichText::new(&outcome.upstream_url)
                    .monospace()
                    .color(ui.visuals().text_color()),
            )
            .truncate(),
        );
        if ui.small_button("Copy URL").clicked() {
            ctx.copy_text(outcome.upstream_url.clone());
        }
    });
    ui.add_space(8.0);

    match &outcome.status {
        ForwardStatus::Skipped { reason } => {
            forward_notice(ui, theme::WARNING, "Skipped", reason);
        }
        ForwardStatus::Error { message, .. } => {
            forward_notice(ui, theme::DANGER, "Error", message);
        }
        ForwardStatus::Success {
            headers,
            body,
            body_size,
            ..
        } => {
            ui.label(
                RichText::new("RESPONSE HEADERS")
                    .small()
                    .strong()
                    .color(theme::dim_text_color(&ctx)),
            );
            ui.add_space(4.0);
            if headers.is_empty() {
                ui.label(RichText::new("(no headers)").italics());
            } else {
                egui::Frame::new()
                    .fill(theme::elev2_bg(&ctx))
                    .stroke(Stroke::new(1.0, theme::border_color(&ctx)))
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        egui::Grid::new("forwarded-headers-grid")
                            .num_columns(2)
                            .spacing([18.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                for (k, v) in headers {
                                    ui.label(
                                        RichText::new(k)
                                            .monospace()
                                            .strong()
                                            .color(theme::ACCENT_STRONG),
                                    );
                                    ui.label(RichText::new(v).monospace());
                                    ui.end_row();
                                }
                            });
                    });
            }
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("RESPONSE BODY")
                        .small()
                        .strong()
                        .color(theme::dim_text_color(&ctx)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(humansize::format_size(*body_size as u64, humansize::BINARY))
                        .small()
                        .color(dim),
                );
                let ct = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.as_str());
                if let Some(ct) = ct {
                    ui.add_space(8.0);
                    content_type_chip(ui, ct);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Copy body").clicked() {
                        let text = match body {
                            postbin_ultra::request::ForwardBody::Utf8 { text } => text.clone(),
                            postbin_ultra::request::ForwardBody::Base64 { .. } => {
                                "(binary body — use Download)".to_string()
                            }
                        };
                        ctx.copy_text(text);
                    }
                });
            });
            ui.add_space(6.0);

            let bytes = body.into_bytes();
            let formatted_text = match std::str::from_utf8(&bytes) {
                Ok(s) => s.to_string(),
                Err(_) => format!("<{} bytes binary>", bytes.len()),
            };
            let pick = highlight::detect(
                headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .map(|(_, v)| v.as_str()),
                &formatted_text,
            );
            crate::tree::body_card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                let rendered_tree = if pick == Highlighter::Json {
                    crate::tree::try_render(ui, &formatted_text)
                } else {
                    false
                };
                if !rendered_tree {
                    let layout_job = match pick {
                        Highlighter::Json => highlight::json_layout(&formatted_text, ui.ctx()),
                        Highlighter::Xml => highlight::xml_layout(&formatted_text, ui.ctx()),
                        Highlighter::None => highlight::plain_layout(
                            &formatted_text,
                            ui.ctx(),
                            ui.visuals().text_color(),
                        ),
                    };
                    ui.add(egui::Label::new(layout_job).selectable(true));
                }
            });
        }
    }
}

/// Pill that summarizes the forward outcome, colored by HTTP status class.
fn forward_status_pill(ui: &mut egui::Ui, status: &ForwardStatus) {
    let (label, color) = match status {
        ForwardStatus::Success { status_code, .. } => {
            let color = match *status_code / 100 {
                2 => theme::SUCCESS,
                3 => theme::ACCENT,
                4 => theme::WARNING,
                _ => theme::DANGER,
            };
            (format!("HTTP {}", status_code), color)
        }
        ForwardStatus::Skipped { .. } => ("Skipped".to_string(), theme::WARNING),
        ForwardStatus::Error { .. } => ("Error".to_string(), theme::DANGER),
    };
    let frame = egui::Frame::new()
        .fill(color.linear_multiply(0.18))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(10, 4));
    frame.show(ui, |ui| {
        ui.label(
            RichText::new(label)
                .monospace()
                .strong()
                .color(color)
                .size(11.5),
        );
    });
}

fn forward_duration_ms(status: &ForwardStatus) -> Option<u64> {
    match status {
        ForwardStatus::Success { duration_ms, .. } => Some(*duration_ms),
        ForwardStatus::Error { duration_ms, .. } => Some(*duration_ms),
        ForwardStatus::Skipped { .. } => None,
    }
}

/// Boxed message used by the Skipped / Error variants — colored card with a
/// heading ("Skipped" / "Error") and the underlying reason.
fn forward_notice(ui: &mut egui::Ui, color: egui::Color32, heading: &str, detail: &str) {
    let frame = egui::Frame::new()
        .fill(color.linear_multiply(0.18))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(14, 12));
    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(RichText::new(heading).strong().color(color).size(13.5));
        ui.add_space(4.0);
        ui.label(RichText::new(detail).monospace().size(12.0));
    });
}

fn content_type_chip(ui: &mut egui::Ui, ct: &str) {
    let main = ct.split(';').next().unwrap_or(ct).trim();
    let frame = egui::Frame::new()
        .fill(theme::elev2_bg(ui.ctx()))
        .stroke(Stroke::new(1.0, theme::border_color(ui.ctx())))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(8, 2));
    frame.show(ui, |ui| {
        ui.label(
            RichText::new(main)
                .monospace()
                .small()
                .color(theme::ACCENT_STRONG),
        );
    });
}

/// Write the raw body bytes to `~/Downloads/postbin-<id>.bin` (or a sensible
/// fallback). Best-effort; errors flow up to the caller's status line.
fn save_body_to_disk(body: &[u8], req: &CapturedRequest) -> std::io::Result<()> {
    let dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let ext = match req.content_type().and_then(|c| c.split(';').next()) {
        Some("application/json") => "json",
        Some("application/xml") | Some("text/xml") => "xml",
        Some("text/html") => "html",
        Some("text/plain") => "txt",
        Some("text/csv") => "csv",
        Some("image/png") => "png",
        Some("image/jpeg") => "jpg",
        _ => "bin",
    };
    let name = format!("postbin-{}.{}", &req.id.simple(), ext);
    let path = dir.join(name);
    std::fs::write(&path, body)
}

/// Renders the raw HTTP request line + headers + body preview. Used by the
/// Raw tab; pulled out so it can be unit-tested without egui.
pub fn build_raw(req: &CapturedRequest) -> String {
    let path = if req.query.is_empty() {
        req.path.clone()
    } else {
        format!("{}?{}", req.path, req.query)
    };
    let mut out = format!("{} {} {}\n", req.method, path, req.version);
    for (k, v) in &req.headers {
        out.push_str(&format!("{k}: {v}\n"));
    }
    out.push('\n');
    match std::str::from_utf8(&req.body) {
        Ok(s) => out.push_str(s),
        Err(_) => out.push_str(&format!("<{} bytes binary>", req.body.len())),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use chrono::Utc;
    use uuid::Uuid;

    fn req_for(method: &str, body: &'static [u8], headers: &[(&str, &str)]) -> CapturedRequest {
        CapturedRequest {
            id: Uuid::nil(),
            received_at: Utc::now(),
            method: method.into(),
            path: "/foo".into(),
            query: "a=1".into(),
            version: "HTTP/1.1".into(),
            remote_addr: "127.0.0.1:1".into(),
            headers: headers
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
            body: Bytes::from_static(body),
            body_truncated: false,
            body_bytes_received: body.len(),
            forwards: Vec::new(),
        }
    }

    #[test]
    fn build_raw_emits_request_line_headers_and_body() {
        let req = req_for("POST", b"hello", &[("content-type", "text/plain")]);
        let raw = build_raw(&req);
        assert!(raw.starts_with("POST /foo?a=1 HTTP/1.1\n"));
        assert!(raw.contains("content-type: text/plain"));
        assert!(raw.ends_with("hello"));
    }

    #[test]
    fn build_raw_handles_no_query() {
        let mut req = req_for("GET", b"", &[]);
        req.query = String::new();
        let raw = build_raw(&req);
        assert!(raw.starts_with("GET /foo HTTP/1.1\n"));
    }

    #[test]
    fn build_raw_marks_binary_body() {
        let body: &'static [u8] = &[0xff, 0xfe, 0x00];
        let req = req_for("POST", body, &[]);
        let raw = build_raw(&req);
        assert!(raw.contains("<3 bytes binary>"));
    }

    #[test]
    fn forward_from_settings_returns_none_when_disabled_or_empty() {
        let mut f = ForwardSettings::default();
        assert!(forward_from_settings(&f).unwrap().is_none());
        f.enabled = true;
        f.url = String::new();
        assert!(forward_from_settings(&f).unwrap().is_none());
    }

    #[test]
    fn forward_from_settings_builds_config_when_enabled_with_url() {
        let f = ForwardSettings {
            enabled: true,
            url: "https://api.example.com/v1".into(),
            timeout_secs: 12,
            insecure: true,
        };
        let cfg = forward_from_settings(&f).unwrap().unwrap();
        assert_eq!(cfg.base.as_str(), "https://api.example.com/v1");
        assert_eq!(cfg.timeout, Duration::from_secs(12));
        assert!(cfg.insecure);
    }

    #[test]
    fn forward_from_settings_rejects_bad_url() {
        let f = ForwardSettings {
            enabled: true,
            url: "not a url".into(),
            timeout_secs: 30,
            insecure: false,
        };
        assert!(forward_from_settings(&f).is_err());
    }

    #[test]
    fn format_label_covers_all_modes() {
        for fmt in [
            BodyFormat::Auto,
            BodyFormat::Pretty,
            BodyFormat::Raw,
            BodyFormat::Hex,
        ] {
            let label = format_label(fmt);
            assert!(!label.is_empty());
        }
    }
}
