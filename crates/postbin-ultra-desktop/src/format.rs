//! Body formatters used by the Body tab.
//!
//! All functions take the raw body bytes and the request's content-type
//! (already lowercased) and return a single string the UI prints in
//! a monospace pane. Pure functions, no egui dependency, fully unit-tested.

use crate::state::BodyFormat;

const MAX_PRETTY_BYTES: usize = 4 * 1024 * 1024;
const HEX_BYTES_PER_LINE: usize = 16;
const HEX_MAX_LINES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedBody {
    pub text: String,
    /// `true` when the rendered text is monospaced (JSON, hex, raw).
    pub monospaced: bool,
    /// Optional notice the UI can render above the body, e.g. "binary".
    pub notice: Option<String>,
}

pub fn format_body(body: &[u8], content_type: Option<&str>, mode: BodyFormat) -> FormattedBody {
    if body.is_empty() {
        return FormattedBody {
            text: String::new(),
            monospaced: true,
            notice: Some("empty body".into()),
        };
    }
    let ct = content_type.map(|s| s.to_ascii_lowercase());
    let ct_main = ct.as_deref().map(strip_params);
    match mode {
        BodyFormat::Auto => format_auto(body, ct_main),
        BodyFormat::Pretty => format_pretty(body, ct_main),
        BodyFormat::Raw => format_raw(body),
        BodyFormat::Hex => format_hex(body),
    }
}

fn format_auto(body: &[u8], content_type_main: Option<&str>) -> FormattedBody {
    if let Ok(text) = std::str::from_utf8(body) {
        if matches!(content_type_main, Some("application/json")) || looks_like_json(text) {
            if let Some(pretty) = pretty_json(text) {
                return FormattedBody {
                    text: pretty,
                    monospaced: true,
                    notice: None,
                };
            }
        }
        if matches!(content_type_main, Some("application/x-www-form-urlencoded"))
            || (content_type_main.is_none() && looks_like_form(text))
        {
            return FormattedBody {
                text: pretty_form(text),
                monospaced: true,
                notice: Some("decoded application/x-www-form-urlencoded".into()),
            };
        }
        return FormattedBody {
            text: text.to_string(),
            monospaced: true,
            notice: None,
        };
    }
    // Binary body fallback: hex preview.
    let mut out = format_hex(body);
    out.notice = Some(format!("{} bytes binary (hex view)", body.len()));
    out
}

fn format_pretty(body: &[u8], content_type_main: Option<&str>) -> FormattedBody {
    let Ok(text) = std::str::from_utf8(body) else {
        return FormattedBody {
            text: format_hex(body).text,
            monospaced: true,
            notice: Some("binary body, hex view".into()),
        };
    };
    if matches!(content_type_main, Some("application/json")) || looks_like_json(text) {
        if let Some(pretty) = pretty_json(text) {
            return FormattedBody {
                text: pretty,
                monospaced: true,
                notice: None,
            };
        }
    }
    if matches!(content_type_main, Some("application/x-www-form-urlencoded"))
        || (content_type_main.is_none() && looks_like_form(text))
    {
        return FormattedBody {
            text: pretty_form(text),
            monospaced: true,
            notice: Some("decoded application/x-www-form-urlencoded".into()),
        };
    }
    FormattedBody {
        text: text.to_string(),
        monospaced: true,
        notice: None,
    }
}

fn format_raw(body: &[u8]) -> FormattedBody {
    match std::str::from_utf8(body) {
        Ok(s) => FormattedBody {
            text: s.to_string(),
            monospaced: true,
            notice: None,
        },
        Err(_) => FormattedBody {
            text: format_hex(body).text,
            monospaced: true,
            notice: Some("binary body, hex view".into()),
        },
    }
}

fn format_hex(body: &[u8]) -> FormattedBody {
    let mut out = String::with_capacity(body.len() * 4);
    let total_lines = body.len().div_ceil(HEX_BYTES_PER_LINE);
    let truncated = total_lines > HEX_MAX_LINES;
    let visible_lines = total_lines.min(HEX_MAX_LINES);
    for line in 0..visible_lines {
        let start = line * HEX_BYTES_PER_LINE;
        let end = (start + HEX_BYTES_PER_LINE).min(body.len());
        out.push_str(&format!("{:08x}  ", start));
        for offset in 0..HEX_BYTES_PER_LINE {
            let idx = start + offset;
            if idx < end {
                out.push_str(&format!("{:02x} ", body[idx]));
            } else {
                out.push_str("   ");
            }
            if offset == 7 {
                out.push(' ');
            }
        }
        out.push(' ');
        for &byte in &body[start..end] {
            out.push(if (0x20..=0x7e).contains(&byte) {
                byte as char
            } else {
                '.'
            });
        }
        out.push('\n');
    }
    let notice = if truncated {
        Some(format!(
            "hex view truncated to first {} bytes ({} total)",
            HEX_MAX_LINES * HEX_BYTES_PER_LINE,
            body.len()
        ))
    } else {
        None
    };
    FormattedBody {
        text: out,
        monospaced: true,
        notice,
    }
}

fn pretty_json(text: &str) -> Option<String> {
    if text.len() > MAX_PRETTY_BYTES {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    serde_json::to_string_pretty(&v).ok()
}

fn pretty_form(text: &str) -> String {
    let mut out = String::new();
    let mut first = true;
    for pair in text.split('&').filter(|p| !p.is_empty()) {
        if first {
            first = false;
        } else {
            out.push('\n');
        }
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        out.push_str(&decode_form(k));
        out.push_str(" = ");
        out.push_str(&decode_form(v));
    }
    out
}

fn decode_form(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_digit(bytes[i + 1]);
                let lo = hex_digit(bytes[i + 2]);
                if let (Some(hi), Some(lo)) = (hi, lo) {
                    out.push((hi << 4) | lo);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            _ => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + b - b'a'),
        b'A'..=b'F' => Some(10 + b - b'A'),
        _ => None,
    }
}

fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with('{') || trimmed.starts_with('[')
}

fn looks_like_form(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "%+-_.&=~/".contains(c))
        && text.contains('=')
}

fn strip_params(ct: &str) -> &str {
    ct.split(';').next().unwrap_or(ct).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_body_renders_notice() {
        let f = format_body(&[], None, BodyFormat::Auto);
        assert!(f.text.is_empty());
        assert_eq!(f.notice.as_deref(), Some("empty body"));
    }

    #[test]
    fn auto_formats_json_body_pretty() {
        let f = format_body(
            br#"{"a":1,"b":[2,3]}"#,
            Some("application/json"),
            BodyFormat::Auto,
        );
        assert!(f.text.contains("\n"));
        assert!(f.text.contains("\"a\": 1"));
        assert!(f.notice.is_none());
    }

    #[test]
    fn auto_falls_back_to_text_for_unknown_ct() {
        let f = format_body(b"hello", Some("text/plain"), BodyFormat::Auto);
        assert_eq!(f.text, "hello");
        assert!(f.monospaced);
    }

    #[test]
    fn auto_detects_json_without_content_type() {
        let f = format_body(br#"{"x":1}"#, None, BodyFormat::Auto);
        assert!(f.text.contains("\"x\": 1"));
    }

    #[test]
    fn auto_decodes_form_url_encoded() {
        let f = format_body(
            b"name=Jane%20Doe&age=30",
            Some("application/x-www-form-urlencoded"),
            BodyFormat::Auto,
        );
        assert!(f.text.contains("name = Jane Doe"));
        assert!(f.text.contains("age = 30"));
        assert_eq!(
            f.notice.as_deref(),
            Some("decoded application/x-www-form-urlencoded")
        );
    }

    #[test]
    fn auto_handles_binary_with_hex_view() {
        // Use lone high bytes which form invalid UTF-8 so the auto formatter
        // falls through to the binary/hex preview branch.
        let body: Vec<u8> = vec![0xff, 0xfe, 0xfd, 0xfc, 0xfb, 0xfa];
        let f = format_body(&body, None, BodyFormat::Auto);
        assert!(f.text.contains("00000000"));
        assert!(f.notice.as_deref().unwrap().contains("binary"));
    }

    #[test]
    fn pretty_mode_falls_back_to_text_for_non_json() {
        let f = format_body(b"plain", Some("text/plain"), BodyFormat::Pretty);
        assert_eq!(f.text, "plain");
    }

    #[test]
    fn pretty_mode_pretties_form_data_for_form_ct() {
        let f = format_body(
            b"k=v",
            Some("application/x-www-form-urlencoded"),
            BodyFormat::Pretty,
        );
        assert!(f.text.contains("k = v"));
    }

    #[test]
    fn pretty_mode_renders_binary_as_hex() {
        let body = vec![0xff, 0xfe, 0x00];
        let f = format_body(&body, None, BodyFormat::Pretty);
        assert!(f.text.contains("ff fe 00"));
    }

    #[test]
    fn raw_mode_outputs_text_when_utf8() {
        let f = format_body(b"hello", None, BodyFormat::Raw);
        assert_eq!(f.text, "hello");
    }

    #[test]
    fn raw_mode_falls_back_to_hex_for_binary() {
        let body = vec![0xff, 0xfe];
        let f = format_body(&body, None, BodyFormat::Raw);
        assert!(f.text.contains("ff fe"));
    }

    #[test]
    fn hex_mode_includes_offset_and_ascii_column() {
        let f = format_body(b"hello world", None, BodyFormat::Hex);
        assert!(f.text.starts_with("00000000  "));
        assert!(f.text.contains("hello world"));
    }

    #[test]
    fn hex_truncates_huge_bodies() {
        let body = vec![0u8; HEX_MAX_LINES * HEX_BYTES_PER_LINE * 2];
        let f = format_body(&body, None, BodyFormat::Hex);
        assert!(f.notice.unwrap().contains("truncated"));
    }

    #[test]
    fn pretty_json_handles_huge_bodies_gracefully() {
        let big = vec![b'a'; MAX_PRETTY_BYTES + 10];
        let f = format_body(&big, Some("application/json"), BodyFormat::Auto);
        // Falls back to text/utf8 since JSON parsing skipped.
        assert_eq!(f.notice, None);
    }

    #[test]
    fn decode_form_handles_plus_and_percent_encoding() {
        assert_eq!(decode_form("a+b"), "a b");
        assert_eq!(decode_form("a%20b"), "a b");
        assert_eq!(decode_form("a%XYb"), "a%XYb");
        assert_eq!(decode_form("ascii"), "ascii");
    }

    #[test]
    fn looks_like_json_detection() {
        assert!(looks_like_json("  { }"));
        assert!(looks_like_json("[1,2]"));
        assert!(!looks_like_json("not json"));
    }

    #[test]
    fn looks_like_form_detection() {
        assert!(looks_like_form("a=b&c=d"));
        assert!(looks_like_form("only=key"));
        assert!(!looks_like_form("not a form"));
        assert!(!looks_like_form(""));
    }

    #[test]
    fn strip_params_drops_charset_segment() {
        assert_eq!(strip_params("application/json"), "application/json");
        assert_eq!(
            strip_params("application/json; charset=utf-8"),
            "application/json"
        );
        assert_eq!(strip_params("text/plain;boundary=foo"), "text/plain");
    }
}
