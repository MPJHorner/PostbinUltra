use std::io::{IsTerminal, Write};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Local};
use owo_colors::OwoColorize;

use crate::request::CapturedRequest;

#[derive(Clone, Debug)]
pub struct PrinterOptions {
    pub use_color: bool,
    pub json_mode: bool,
    pub verbose: bool,
    pub quiet: bool,
}

impl PrinterOptions {
    pub fn from_cli(no_cli: bool, json: bool, verbose: bool) -> Self {
        let stdout = std::io::stdout();
        let isatty = stdout.is_terminal();
        let no_color_env = std::env::var_os("NO_COLOR").is_some();
        Self {
            use_color: isatty && !no_color_env && !no_cli && !json,
            json_mode: json,
            verbose,
            quiet: no_cli && !json,
        }
    }
}

#[derive(Clone)]
pub struct Printer {
    opts: PrinterOptions,
    sink: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl Printer {
    pub fn new(opts: PrinterOptions) -> Self {
        let sink: Box<dyn Write + Send> = Box::new(std::io::stdout());
        Self {
            opts,
            sink: Arc::new(Mutex::new(sink)),
        }
    }

    /// Returns a printer that writes into the given buffer; used by tests.
    pub fn with_sink<W: Write + Send + 'static>(opts: PrinterOptions, sink: W) -> Self {
        Self {
            opts,
            sink: Arc::new(Mutex::new(Box::new(sink))),
        }
    }

    pub fn options(&self) -> &PrinterOptions {
        &self.opts
    }

    pub fn print_request(&self, req: &CapturedRequest) {
        if self.opts.quiet {
            return;
        }
        if self.opts.json_mode {
            if let Ok(s) = serde_json::to_string(req) {
                self.write_line(&s);
            }
            return;
        }
        let line = self.format_line(req);
        self.write_line(&line);
        if self.opts.verbose {
            self.write_verbose_extras(req);
        }
    }

    fn write_verbose_extras(&self, req: &CapturedRequest) {
        let last = req.headers.len().saturating_sub(1);
        for (i, (k, v)) in req.headers.iter().enumerate() {
            let connector = if i == last && req.body.is_empty() {
                "                └─"
            } else {
                "                ├─"
            };
            self.write_line(&format!("{connector} {k}: {v}"));
        }
        let preview = body_preview(&req.body, 200);
        if !preview.is_empty() {
            self.write_line(&format!("                └─ body: {preview}"));
        }
    }

    pub fn format_line(&self, req: &CapturedRequest) -> String {
        let local: DateTime<Local> = req.received_at.with_timezone(&Local);
        let ts = local.format("%H:%M:%S%.3f").to_string();
        let method_padded = format!("{:<7}", req.method);
        let path_full = if req.query.is_empty() {
            req.path.clone()
        } else {
            format!("{}?{}", req.path, req.query)
        };
        let path_disp = truncate_middle(&path_full, 38);
        let size = humansize::format_size(req.body_bytes_received as u64, humansize::BINARY);
        let ct = req
            .content_type()
            .map(|v| v.split(';').next().unwrap_or(v).trim().to_string())
            .unwrap_or_else(|| "—".to_string());
        let from = format!("from {}", req.remote_addr);
        let trunc = if req.body_truncated {
            " [truncated]"
        } else {
            ""
        };

        if self.opts.use_color {
            format!(
                "  {ts}  {method}  {path:<38}  {size:>10}  {ct:<24}  {from}{trunc}",
                ts = ts.dimmed(),
                method = colored_method(&req.method, &method_padded),
                path = path_disp,
                size = size.bright_black(),
                ct = ct.bright_black(),
                from = from.dimmed(),
                trunc = trunc.bright_yellow(),
            )
        } else {
            format!("  {ts}  {method_padded}  {path_disp:<38}  {size:>10}  {ct:<24}  {from}{trunc}")
        }
    }

    /// Notify that a requested port was busy and the server fell back to
    /// `actual`. Suppressed in quiet/json modes for the same reason as the
    /// banner — keeps NDJSON output strictly machine-readable.
    pub fn print_port_fallback(&self, label: &str, requested: u16, actual: u16) {
        if self.opts.json_mode || self.opts.quiet {
            return;
        }
        if self.opts.use_color {
            self.write_line(&format!(
                "  {}  {label} port {requested} in use — using {actual}",
                "!".bright_yellow(),
                label = label.bright_white(),
                requested = requested.to_string().bright_white(),
                actual = actual.to_string().bright_green(),
            ));
        } else {
            self.write_line(&format!(
                "  ! {label} port {requested} in use — using {actual}"
            ));
        }
    }

    /// One-line notice that a newer release exists. Suppressed in quiet/json
    /// modes so machine-readable output stays clean.
    pub fn print_update_available(&self, current: &str, latest: &str) {
        if self.opts.json_mode || self.opts.quiet {
            return;
        }
        if self.opts.use_color {
            self.write_line(&format!(
                "  {}  update available: v{current} -> v{latest}  (run `postbin-ultra --update`)",
                "↑".bright_green(),
                current = current.bright_white(),
                latest = latest.bright_green(),
            ));
        } else {
            self.write_line(&format!(
                "  ^ update available: v{current} -> v{latest}  (run `postbin-ultra --update`)"
            ));
        }
    }

    pub fn print_banner(
        &self,
        capture_url: &str,
        ui_url: Option<&str>,
        buffer: usize,
        max_body: usize,
    ) {
        self.print_banner_with_forward(capture_url, ui_url, buffer, max_body, None)
    }

    /// Banner variant that includes a `Forward` line when proxy mode is on.
    /// `forward` is `(target_url, timeout_secs, insecure)`.
    pub fn print_banner_with_forward(
        &self,
        capture_url: &str,
        ui_url: Option<&str>,
        buffer: usize,
        max_body: usize,
        forward: Option<(&str, u64, bool)>,
    ) {
        if self.opts.json_mode || self.opts.quiet {
            return;
        }
        let version = env!("CARGO_PKG_VERSION");
        let max = humansize::format_size(max_body as u64, humansize::BINARY);
        if self.opts.use_color {
            self.write_line("");
            self.write_line(&format!(
                "  {} {}",
                "▶".bright_blue(),
                format!("PostbinUltra v{version}").bold()
            ));
            self.write_line(&format!(
                "    {}  {}   {}",
                "Capture".bright_black(),
                capture_url.bright_white().underline(),
                "(any method, any path)".dimmed()
            ));
            if let Some(u) = ui_url {
                self.write_line(&format!(
                    "    {}  {}",
                    "Web UI ".bright_black(),
                    u.bright_white().underline()
                ));
            }
            self.write_line(&format!(
                "    {}  {} requests · {} max body",
                "Buffer ".bright_black(),
                buffer,
                max
            ));
            if let Some((target, timeout_secs, insecure)) = forward {
                let suffix = if insecure {
                    format!("(timeout {timeout_secs}s, insecure)")
                } else {
                    format!("(timeout {timeout_secs}s)")
                };
                self.write_line(&format!(
                    "    {}  -> {}  {}",
                    "Forward".bright_black(),
                    target.bright_white().underline(),
                    suffix.dimmed()
                ));
            }
            self.write_line("");
            self.write_line(&format!(
                "  {}",
                "Waiting for requests… (Ctrl+C to quit)".dimmed()
            ));
            self.write_line("");
        } else {
            self.write_line("");
            self.write_line(&format!("  PostbinUltra v{version}"));
            self.write_line(&format!(
                "    Capture  {capture_url}   (any method, any path)"
            ));
            if let Some(u) = ui_url {
                self.write_line(&format!("    Web UI   {u}"));
            }
            self.write_line(&format!("    Buffer   {buffer} requests · {max} max body"));
            if let Some((target, timeout_secs, insecure)) = forward {
                let suffix = if insecure {
                    format!("(timeout {timeout_secs}s, insecure)")
                } else {
                    format!("(timeout {timeout_secs}s)")
                };
                self.write_line(&format!("    Forward  -> {target}  {suffix}"));
            }
            self.write_line("");
            self.write_line("  Waiting for requests… (Ctrl+C to quit)");
            self.write_line("");
        }
    }

    fn write_line(&self, s: &str) {
        let mut sink = self.sink.lock().expect("printer sink poisoned");
        let _ = writeln!(sink, "{s}");
        let _ = sink.flush();
    }
}

fn colored_method(method: &str, padded: &str) -> String {
    match method {
        "GET" => padded.bright_cyan().to_string(),
        "POST" => padded.bright_blue().to_string(),
        "PUT" => padded.bright_yellow().to_string(),
        "PATCH" => padded.bright_magenta().to_string(),
        "DELETE" => padded.bright_red().to_string(),
        "OPTIONS" | "HEAD" => padded.bright_black().to_string(),
        _ => padded.bright_white().to_string(),
    }
}

fn truncate_middle(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let half = max.saturating_sub(1) / 2;
    let chars: Vec<char> = s.chars().collect();
    let head: String = chars.iter().take(half).collect();
    let tail: String = chars
        .iter()
        .rev()
        .take(max - half - 1)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{head}…{tail}")
}

pub(crate) fn body_preview(body: &[u8], max: usize) -> String {
    if body.is_empty() {
        return String::new();
    }
    match std::str::from_utf8(body) {
        Ok(s) => {
            let trimmed = s.replace(['\n', '\r', '\t'], " ");
            let collapsed: String = trimmed
                .chars()
                .scan(false, |prev_space, c| {
                    let is_space = c == ' ';
                    let keep = !(is_space && *prev_space);
                    *prev_space = is_space;
                    Some(if keep { Some(c) } else { None })
                })
                .flatten()
                .collect();
            if collapsed.chars().count() > max {
                let mut out: String = collapsed.chars().take(max).collect();
                out.push('…');
                out
            } else {
                collapsed
            }
        }
        Err(_) => format!("<{} bytes binary>", body.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use chrono::Utc;
    use std::sync::Arc;
    use uuid::Uuid;

    fn req(method: &str, body: &'static [u8], headers: Vec<(&str, &str)>) -> CapturedRequest {
        CapturedRequest {
            id: Uuid::nil(),
            received_at: Utc::now(),
            method: method.into(),
            path: "/foo".into(),
            query: "a=1".into(),
            version: "HTTP/1.1".into(),
            remote_addr: "127.0.0.1:1234".into(),
            headers: headers
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
            body: Bytes::from_static(body),
            body_truncated: false,
            body_bytes_received: body.len(),
        }
    }

    #[test]
    fn format_line_includes_method_path_size_ct() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let p = Printer::new(opts);
        let r = req(
            "POST",
            b"{\"x\":1}",
            vec![("content-type", "application/json")],
        );
        let line = p.format_line(&r);
        assert!(line.contains("POST"));
        assert!(line.contains("/foo?a=1"));
        assert!(line.contains("application/json"));
        assert!(line.contains("from 127.0.0.1:1234"));
    }

    #[test]
    fn format_line_handles_no_query_no_ct() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let p = Printer::new(opts);
        let mut r = req("GET", b"", vec![]);
        r.query = String::new();
        let line = p.format_line(&r);
        assert!(line.contains("GET"));
        assert!(line.contains("/foo "));
        assert!(line.contains("—"));
    }

    #[test]
    fn format_line_truncated_marker_when_color_off() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let p = Printer::new(opts);
        let mut r = req("POST", b"abc", vec![]);
        r.body_truncated = true;
        let line = p.format_line(&r);
        assert!(line.contains("[truncated]"));
    }

    #[test]
    fn print_request_writes_one_line_in_default_mode() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_request(&req("GET", b"hi", vec![("x", "y")]));
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert_eq!(out.lines().count(), 1);
        assert!(out.contains("GET"));
    }

    #[test]
    fn print_request_verbose_writes_headers_and_body() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: true,
            quiet: false,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_request(&req(
            "POST",
            b"hello",
            vec![("content-type", "text/plain"), ("x-other", "z")],
        ));
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(out.contains("content-type: text/plain"));
        assert!(out.contains("x-other: z"));
        assert!(out.contains("hello"));
    }

    #[test]
    fn print_request_json_mode_emits_ndjson() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: true,
            verbose: false,
            quiet: false,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_request(&req("PUT", b"hi", vec![]));
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
        assert_eq!(parsed["method"], "PUT");
        assert_eq!(parsed["body"], "hi");
    }

    #[test]
    fn quiet_mode_writes_nothing() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: true,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_request(&req("GET", b"x", vec![]));
        p.print_banner("http://x", None, 10, 1024);
        assert!(buf.lock().unwrap().is_empty());
    }

    #[test]
    fn banner_includes_urls_and_buffer() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_banner(
            "http://127.0.0.1:9000",
            Some("http://127.0.0.1:9001"),
            1000,
            10485760,
        );
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(out.contains("PostbinUltra"));
        assert!(out.contains("http://127.0.0.1:9000"));
        assert!(out.contains("http://127.0.0.1:9001"));
        assert!(out.contains("1000"));
        assert!(out.contains("10 MiB"));
    }

    #[test]
    fn port_fallback_notice_includes_both_ports() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_port_fallback("capture", 9000, 9002);
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(out.contains("capture"));
        assert!(out.contains("9000"));
        assert!(out.contains("9002"));
    }

    #[test]
    fn port_fallback_notice_quiet_writes_nothing() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: true,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_port_fallback("capture", 9000, 9002);
        assert!(buf.lock().unwrap().is_empty());
    }

    #[test]
    fn banner_with_no_ui_skips_ui_line() {
        let opts = PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_banner("http://127.0.0.1:9000", None, 100, 1024);
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(out.contains("Capture"));
        assert!(!out.contains("Web UI"));
    }

    #[test]
    fn colored_method_branches_all() {
        for m in [
            "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "WEIRD",
        ] {
            let s = colored_method(m, m);
            assert!(s.contains(m));
        }
    }

    #[test]
    fn body_preview_handles_text_binary_long_empty() {
        assert_eq!(body_preview(b"", 10), "");
        assert_eq!(body_preview(b"hi\nthere", 10), "hi there");
        let long = vec![b'a'; 500];
        let p = body_preview(&long, 10);
        assert!(p.ends_with('…'));
        let bin = vec![0xff, 0xfe, 0xfd];
        assert_eq!(body_preview(&bin, 10), "<3 bytes binary>");
    }

    #[test]
    fn truncate_middle_works() {
        assert_eq!(truncate_middle("abc", 10), "abc");
        let t = truncate_middle("abcdefghijklmnop", 7);
        assert!(t.contains('…'));
        assert!(t.chars().count() <= 7);
    }

    #[test]
    fn options_from_cli_quiet_when_no_cli_and_not_json() {
        let opts = PrinterOptions::from_cli(true, false, false);
        assert!(opts.quiet);
        assert!(!opts.use_color);

        let opts = PrinterOptions::from_cli(true, true, false);
        assert!(!opts.quiet);
        assert!(opts.json_mode);
    }

    #[test]
    fn banner_color_path_contains_brand() {
        let opts = PrinterOptions {
            use_color: true,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let p = Printer::with_sink(opts, BufWriter(buf.clone()));
        p.print_banner("http://x", Some("http://y"), 1, 1024);
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(out.contains("PostbinUltra"));
    }

    #[test]
    fn format_line_color_path_branches() {
        let opts = PrinterOptions {
            use_color: true,
            json_mode: false,
            verbose: false,
            quiet: false,
        };
        let p = Printer::new(opts);
        let mut r = req("POST", b"x", vec![("content-type", "application/json")]);
        r.body_truncated = true;
        let line = p.format_line(&r);
        // Color codes present
        assert!(line.contains("POST"));
        assert!(line.contains("[truncated]"));
    }

    /// Buffer wrapper that lets us share an `Arc<Mutex<Vec<u8>>>` between the
    /// test and the printer.
    struct BufWriter(Arc<Mutex<Vec<u8>>>);
    impl Write for BufWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
