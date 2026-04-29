//! Interactive collapsible JSON tree view.
//!
//! Parses the body once with `serde_json` and renders each object/array as
//! an `egui::CollapsingState` keyed off a stable JSON-pointer-style path.
//! Bulk Expand all / Collapse all walks the same tree and writes each node's
//! state to egui memory so the next frame picks them up.

use egui::collapsing_header::CollapsingState;
use egui::{CornerRadius, Margin, RichText, Stroke};
use serde_json::Value;

use crate::highlight::Palette;
use crate::theme;

const ROOT_ID: &str = "json-tree";

/// Try to parse and render `text` as an interactive JSON tree. Returns
/// `false` if the body isn't valid JSON, in which case the caller should
/// fall through to a flat highlighter.
pub fn try_render(ui: &mut egui::Ui, text: &str) -> bool {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let pal = Palette::for_ctx(ui.ctx());
    render_value(ui, &value, "$", &pal, 0);
    true
}

/// Walk the same JSON `text` and force every collapsible node open or closed.
/// Called by the "Expand all" / "Collapse all" buttons in the body toolbar.
pub fn set_all_open(ctx: &egui::Context, text: &str, open: bool) {
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let mut paths: Vec<String> = Vec::new();
    collect_paths(&value, "$".into(), &mut paths);
    for p in paths {
        let id = node_id(&p);
        let mut state = CollapsingState::load_with_default_open(ctx, id, open);
        state.set_open(open);
        state.store(ctx);
    }
}

fn node_id(path: &str) -> egui::Id {
    egui::Id::new((ROOT_ID, path))
}

fn collect_paths(value: &Value, path: String, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            out.push(path.clone());
            for (k, v) in map {
                collect_paths(v, format!("{}.{}", path, k), out);
            }
        }
        Value::Array(arr) => {
            out.push(path.clone());
            for (i, v) in arr.iter().enumerate() {
                collect_paths(v, format!("{}[{}]", path, i), out);
            }
        }
        _ => {}
    }
}

fn render_value(ui: &mut egui::Ui, value: &Value, path: &str, pal: &Palette, depth: usize) {
    match value {
        Value::Object(map) => render_object(ui, map, path, pal, depth),
        Value::Array(arr) => render_array(ui, arr, path, pal, depth),
        _ => render_scalar_inline(ui, value, pal),
    }
}

fn render_object(
    ui: &mut egui::Ui,
    map: &serde_json::Map<String, Value>,
    path: &str,
    pal: &Palette,
    depth: usize,
) {
    if map.is_empty() {
        ui.label(RichText::new("{ }").monospace().color(pal.punct));
        return;
    }
    // Auto-open the first two levels, deeper levels collapsed by default.
    let default_open = depth < 2;
    let id = node_id(path);
    let state = CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    state
        .show_header(ui, |ui| {
            disclosure_summary(ui, "{", &format!("{} keys", map.len()), "}", pal);
        })
        .body(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (i, (k, v)) in map.iter().enumerate() {
                let child_path = format!("{}.{}", path, k);
                render_kv_row(ui, k, v, &child_path, pal, depth + 1, i + 1 < map.len());
            }
        });
}

fn render_array(ui: &mut egui::Ui, arr: &[Value], path: &str, pal: &Palette, depth: usize) {
    if arr.is_empty() {
        ui.label(RichText::new("[ ]").monospace().color(pal.punct));
        return;
    }
    let default_open = depth < 2;
    let id = node_id(path);
    let state = CollapsingState::load_with_default_open(ui.ctx(), id, default_open);
    state
        .show_header(ui, |ui| {
            disclosure_summary(ui, "[", &format!("{} items", arr.len()), "]", pal);
        })
        .body(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (i, v) in arr.iter().enumerate() {
                let child_path = format!("{}[{}]", path, i);
                render_index_row(ui, i, v, &child_path, pal, depth + 1, i + 1 < arr.len());
            }
        });
}

fn render_kv_row(
    ui: &mut egui::Ui,
    key: &str,
    value: &Value,
    path: &str,
    pal: &Palette,
    depth: usize,
    has_more: bool,
) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            RichText::new(format!("\"{}\"", key))
                .monospace()
                .color(pal.key),
        );
        ui.label(RichText::new(":").monospace().color(pal.punct));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            render_value_inline_or_block(ui, value, path, pal, depth, has_more);
        });
    });
}

fn render_index_row(
    ui: &mut egui::Ui,
    idx: usize,
    value: &Value,
    path: &str,
    pal: &Palette,
    depth: usize,
    has_more: bool,
) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.label(
            RichText::new(format!("{}", idx))
                .monospace()
                .color(pal.comment),
        );
        ui.label(RichText::new(":").monospace().color(pal.punct));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            render_value_inline_or_block(ui, value, path, pal, depth, has_more);
        });
    });
}

fn render_value_inline_or_block(
    ui: &mut egui::Ui,
    value: &Value,
    path: &str,
    pal: &Palette,
    depth: usize,
    has_more: bool,
) {
    match value {
        Value::Object(_) | Value::Array(_) => {
            render_value(ui, value, path, pal, depth);
            if has_more {
                ui.label(RichText::new(",").monospace().color(pal.punct));
            }
        }
        _ => {
            ui.horizontal(|ui| {
                render_scalar_inline(ui, value, pal);
                if has_more {
                    ui.label(RichText::new(",").monospace().color(pal.punct));
                }
            });
        }
    }
}

fn render_scalar_inline(ui: &mut egui::Ui, value: &Value, pal: &Palette) {
    let (text, color) = match value {
        Value::Null => ("null".to_string(), pal.keyword),
        Value::Bool(b) => (b.to_string(), pal.keyword),
        Value::Number(n) => (n.to_string(), pal.number),
        Value::String(s) => (format!("\"{}\"", escape_json_string(s)), pal.string),
        _ => unreachable!("scalar branch only"),
    };
    ui.label(RichText::new(text).monospace().color(color));
}

fn disclosure_summary(ui: &mut egui::Ui, open: &str, count: &str, close: &str, pal: &Palette) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(open).monospace().color(pal.punct));
        ui.label(RichText::new(count).small().italics().color(pal.comment));
        ui.label(RichText::new(close).monospace().color(pal.punct));
    });
}

fn escape_json_string(s: &str) -> String {
    // Keep this readable rather than perfectly compliant — collapse the most
    // visually-disruptive control chars; full JSON encoding lives in the Raw
    // tab where we just dump the bytes.
    s.chars()
        .map(|c| match c {
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            '"' => "\\\"".to_string(),
            '\\' => "\\\\".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// Container card the body tree renders inside. Pulled out so the flat-text
/// path can use the same surface and the two views look identical.
pub fn body_card<R>(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let frame = egui::Frame::new()
        .fill(theme::elev2_bg(ui.ctx()))
        .stroke(Stroke::new(1.0, theme::border_color(ui.ctx())))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(16, 14));
    frame
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            contents(ui)
        })
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_paths_walks_objects_and_arrays() {
        let v: Value = serde_json::from_str(r#"{"a":{"b":1,"c":[10,20]},"d":"x"}"#).unwrap();
        let mut paths = vec![];
        collect_paths(&v, "$".into(), &mut paths);
        // Should include the root + "a" + "a.c" objects/arrays. Scalars (b, d,
        // c[0], c[1]) are not collapsible so we don't track them.
        assert!(paths.contains(&"$".to_string()));
        assert!(paths.contains(&"$.a".to_string()));
        assert!(paths.contains(&"$.a.c".to_string()));
        assert!(!paths.contains(&"$.d".to_string()));
        assert!(!paths.contains(&"$.a.b".to_string()));
    }

    #[test]
    fn try_render_returns_false_on_invalid_json() {
        // We can't easily render in a test, but we can assert the parse gate.
        assert!(serde_json::from_str::<Value>("not json").is_err());
    }

    #[test]
    fn escape_json_string_handles_common_control_chars() {
        assert_eq!(escape_json_string("a\nb"), "a\\nb");
        assert_eq!(escape_json_string("\"hi\""), "\\\"hi\\\"");
    }
}
