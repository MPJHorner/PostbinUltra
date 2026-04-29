//! Lightweight syntax highlighters for the Body tab.
//!
//! Hand-rolled tokenizers that emit an egui `LayoutJob`. We intentionally
//! avoid pulling in `syntect` (40 MB of grammars + onig) since the bodies we
//! render are short fragments of well-known formats — JSON, XML / HTML, and
//! key=value lines — and a couple hundred lines of focused code beats a heavy
//! dependency for that.
//!
//! Pure functions, no egui drawing here. They produce a `LayoutJob` which the
//! caller hands to `egui::Label::new(...)` (or a TextEdit layouter).

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, TextStyle};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub punct: Color32,
    pub key: Color32,
    pub string: Color32,
    pub number: Color32,
    pub keyword: Color32,
    pub tag: Color32,
    pub attr: Color32,
    pub comment: Color32,
    pub fg: Color32,
}

impl Palette {
    pub fn for_ctx(ctx: &egui::Context) -> Self {
        if ctx.global_style().visuals.dark_mode {
            Self {
                punct: Color32::from_rgb(0x84, 0x8b, 0xa1),
                key: Color32::from_rgb(0x9c, 0xdc, 0xfe),
                string: Color32::from_rgb(0xa6, 0xe2, 0x2e),
                number: Color32::from_rgb(0xff, 0xb8, 0x60),
                keyword: Color32::from_rgb(0xc0, 0x8b, 0xff),
                tag: Color32::from_rgb(0x6e, 0xa8, 0xff),
                attr: Color32::from_rgb(0xff, 0xb8, 0x60),
                comment: Color32::from_rgb(0x6c, 0x72, 0x80),
                fg: Color32::from_rgb(0xe8, 0xea, 0xed),
            }
        } else {
            Self {
                punct: Color32::from_rgb(0x6a, 0x71, 0x80),
                key: Color32::from_rgb(0x06, 0x55, 0xa3),
                string: Color32::from_rgb(0x1f, 0x80, 0x35),
                number: Color32::from_rgb(0xa6, 0x4d, 0x06),
                keyword: Color32::from_rgb(0x7c, 0x47, 0xc5),
                tag: Color32::from_rgb(0x3a, 0x6a, 0xd9),
                attr: Color32::from_rgb(0xa6, 0x4d, 0x06),
                comment: Color32::from_rgb(0x8a, 0x92, 0xa0),
                fg: Color32::from_rgb(0x1b, 0x1f, 0x27),
            }
        }
    }
}

fn fmt(font: &FontId, color: Color32) -> TextFormat {
    TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    }
}

fn mono(ctx: &egui::Context) -> FontId {
    ctx.global_style()
        .text_styles
        .get(&TextStyle::Monospace)
        .cloned()
        .unwrap_or_else(|| FontId::monospace(12.5))
}

/// Highlight JSON. Resilient to malformed input — falls back to plain text on
/// errors so the user always sees something.
pub fn json_layout(text: &str, ctx: &egui::Context) -> LayoutJob {
    let pal = Palette::for_ctx(ctx);
    let font = mono(ctx);
    let mut job = LayoutJob::default();

    // Rather than parse a real AST, we tokenize the pretty-printed text
    // character-by-character. Strings track whether the next non-whitespace
    // character is `:` to colour them as keys vs. values.
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'{' | b'}' | b'[' | b']' | b',' | b':' => {
                job.append(
                    std::str::from_utf8(&bytes[i..=i]).unwrap(),
                    0.0,
                    fmt(&font, pal.punct),
                );
                i += 1;
            }
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                let s = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                let is_key = peek_next_non_ws(bytes, i) == Some(b':');
                let color = if is_key { pal.key } else { pal.string };
                job.append(s, 0.0, fmt(&font, color));
            }
            b't' | b'f' | b'n' if is_keyword(bytes, i) => {
                let len = if bytes[i] == b'f' { 5 } else { 4 };
                let s = std::str::from_utf8(&bytes[i..i + len]).unwrap_or("");
                job.append(s, 0.0, fmt(&font, pal.keyword));
                i += len;
            }
            b'-' | b'0'..=b'9' => {
                let start = i;
                if bytes[i] == b'-' {
                    i += 1;
                }
                while i < bytes.len()
                    && (bytes[i].is_ascii_digit()
                        || bytes[i] == b'.'
                        || bytes[i] == b'e'
                        || bytes[i] == b'E'
                        || bytes[i] == b'+'
                        || bytes[i] == b'-')
                {
                    i += 1;
                }
                let s = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                job.append(s, 0.0, fmt(&font, pal.number));
            }
            _ => {
                // Whitespace + anything we don't classify: emit as plain.
                let start = i;
                while i < bytes.len() && !is_json_significant(bytes[i]) {
                    i += 1;
                }
                if i == start {
                    i += 1;
                }
                let s = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                job.append(s, 0.0, fmt(&font, pal.fg));
            }
        }
    }
    job
}

fn is_json_significant(b: u8) -> bool {
    matches!(
        b,
        b'{' | b'}' | b'[' | b']' | b',' | b':' | b'"' | b't' | b'f' | b'n' | b'-' | b'0'..=b'9'
    )
}

fn is_keyword(bytes: &[u8], i: usize) -> bool {
    bytes[i..].starts_with(b"true")
        || bytes[i..].starts_with(b"false")
        || bytes[i..].starts_with(b"null")
}

fn peek_next_non_ws(bytes: &[u8], mut i: usize) -> Option<u8> {
    while i < bytes.len() {
        if !bytes[i].is_ascii_whitespace() {
            return Some(bytes[i]);
        }
        i += 1;
    }
    None
}

/// Highlight XML / HTML. Tags, attribute names, attribute values, and text
/// content get distinct colours. Unbalanced or invalid markup degrades to
/// plain text safely.
pub fn xml_layout(text: &str, ctx: &egui::Context) -> LayoutJob {
    let pal = Palette::for_ctx(ctx);
    let font = mono(ctx);
    let mut job = LayoutJob::default();

    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Comment <!-- ... -->
            if bytes[i..].starts_with(b"<!--") {
                let start = i;
                i += 4;
                while i + 2 < bytes.len() && &bytes[i..i + 3] != b"-->" {
                    i += 1;
                }
                if i + 2 < bytes.len() {
                    i += 3;
                } else {
                    i = bytes.len();
                }
                let s = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                job.append(s, 0.0, fmt(&font, pal.comment));
                continue;
            }
            // Open `<` punctuation
            job.append("<", 0.0, fmt(&font, pal.punct));
            i += 1;
            // Optional `/` or `?` or `!`
            while i < bytes.len() && matches!(bytes[i], b'/' | b'?' | b'!') {
                let s = std::str::from_utf8(&bytes[i..=i]).unwrap();
                job.append(s, 0.0, fmt(&font, pal.punct));
                i += 1;
            }
            // Tag name
            let start = i;
            while i < bytes.len() && is_name_char(bytes[i]) {
                i += 1;
            }
            if i > start {
                let s = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
                job.append(s, 0.0, fmt(&font, pal.tag));
            }
            // Attributes until `>`
            while i < bytes.len() && bytes[i] != b'>' {
                if bytes[i].is_ascii_whitespace() {
                    let ws_start = i;
                    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                        i += 1;
                    }
                    job.append(
                        std::str::from_utf8(&bytes[ws_start..i]).unwrap_or(" "),
                        0.0,
                        fmt(&font, pal.fg),
                    );
                    continue;
                }
                if bytes[i] == b'/' || bytes[i] == b'?' {
                    let s = std::str::from_utf8(&bytes[i..=i]).unwrap();
                    job.append(s, 0.0, fmt(&font, pal.punct));
                    i += 1;
                    continue;
                }
                // Attribute name
                let attr_start = i;
                while i < bytes.len() && is_name_char(bytes[i]) {
                    i += 1;
                }
                if i > attr_start {
                    let s = std::str::from_utf8(&bytes[attr_start..i]).unwrap_or("");
                    job.append(s, 0.0, fmt(&font, pal.attr));
                }
                if i < bytes.len() && bytes[i] == b'=' {
                    job.append("=", 0.0, fmt(&font, pal.punct));
                    i += 1;
                }
                // Attribute value (quoted)
                if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                    let quote = bytes[i];
                    let v_start = i;
                    i += 1;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                    let s = std::str::from_utf8(&bytes[v_start..i]).unwrap_or("");
                    job.append(s, 0.0, fmt(&font, pal.string));
                } else {
                    // Bare or malformed: just step.
                    if i < bytes.len() {
                        let s = std::str::from_utf8(&bytes[i..=i]).unwrap();
                        job.append(s, 0.0, fmt(&font, pal.fg));
                        i += 1;
                    }
                }
            }
            if i < bytes.len() && bytes[i] == b'>' {
                job.append(">", 0.0, fmt(&font, pal.punct));
                i += 1;
            }
        } else {
            // Text content until next `<`
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            let s = std::str::from_utf8(&bytes[start..i]).unwrap_or("");
            job.append(s, 0.0, fmt(&font, pal.fg));
        }
    }
    job
}

fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b':' | b'-' | b'_' | b'.')
}

/// Plain monospace, no highlighting. Convenience for the Body tab to reuse
/// the same `LayoutJob` rendering path.
pub fn plain_layout(text: &str, ctx: &egui::Context, color: Color32) -> LayoutJob {
    let font = mono(ctx);
    let mut job = LayoutJob::default();
    job.append(text, 0.0, fmt(&font, color));
    job
}

/// Heuristic: pick a highlighter from the content-type and body shape.
/// Returns `None` for unknown / non-text formats — caller should render plain.
pub fn detect(content_type: Option<&str>, text: &str) -> Highlighter {
    let ct = content_type
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let main = ct.split(';').next().unwrap_or("").trim();
    if main == "application/json" || main.ends_with("+json") || looks_like_json(text) {
        return Highlighter::Json;
    }
    if main == "application/xml"
        || main == "text/xml"
        || main == "text/html"
        || main.ends_with("+xml")
        || looks_like_xml(text)
    {
        return Highlighter::Xml;
    }
    Highlighter::None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Highlighter {
    Json,
    Xml,
    None,
}

fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

fn looks_like_xml(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('<') && trimmed.contains('>')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> egui::Context {
        egui::Context::default()
    }

    #[test]
    fn detect_json_by_content_type() {
        assert_eq!(detect(Some("application/json"), ""), Highlighter::Json);
        assert_eq!(
            detect(Some("application/vnd.api+json"), ""),
            Highlighter::Json
        );
    }

    #[test]
    fn detect_xml_by_content_type() {
        assert_eq!(detect(Some("application/xml"), ""), Highlighter::Xml);
        assert_eq!(detect(Some("text/html"), ""), Highlighter::Xml);
        assert_eq!(detect(Some("application/atom+xml"), ""), Highlighter::Xml);
    }

    #[test]
    fn detect_falls_back_on_shape_when_no_content_type() {
        assert_eq!(detect(None, "{\"a\":1}"), Highlighter::Json);
        assert_eq!(detect(None, "<root/>"), Highlighter::Xml);
        assert_eq!(detect(None, "hello"), Highlighter::None);
    }

    #[test]
    fn json_layout_emits_sections_for_keys_and_values() {
        let job = json_layout("{\"a\": 1}", &ctx());
        assert!(!job.sections.is_empty());
        // The job should contain at least: { "a" : 1 } => 5+ sections
        assert!(job.sections.len() >= 5);
    }

    #[test]
    fn json_layout_handles_arrays_and_keywords() {
        let job = json_layout("[true, false, null, -3.14]", &ctx());
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn xml_layout_handles_tags_attrs_and_text() {
        let job = xml_layout("<a href=\"x\">hi</a>", &ctx());
        assert!(job.sections.len() >= 4);
    }

    #[test]
    fn xml_layout_handles_comments() {
        let job = xml_layout("<!-- skip --><x/>", &ctx());
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn json_layout_safe_on_invalid_input() {
        // Should not panic.
        let _ = json_layout("{not really json", &ctx());
    }
}
