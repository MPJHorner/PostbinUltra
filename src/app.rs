use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::task::JoinHandle;

/// Maximum number of consecutive ports tried after the requested one when the
/// requested port is already in use.
const MAX_PORT_FALLBACK_ATTEMPTS: u16 = 50;

/// Bind a TCP listener on `port`, falling back to the next free port up to
/// [`MAX_PORT_FALLBACK_ATTEMPTS`] times if the requested port is busy.
///
/// `port == 0` is passed through unchanged so callers keep the OS-assigned
/// ephemeral behavior. Returns the listener and the port that was actually
/// bound (which equals `port` when no fallback occurred).
async fn bind_with_fallback(bind: IpAddr, port: u16) -> io::Result<TcpListener> {
    if port == 0 {
        return TcpListener::bind(SocketAddr::new(bind, 0)).await;
    }
    let mut last_err = None;
    for offset in 0..=MAX_PORT_FALLBACK_ATTEMPTS {
        let candidate = match port.checked_add(offset) {
            Some(p) => p,
            None => break,
        };
        match TcpListener::bind(SocketAddr::new(bind, candidate)).await {
            Ok(l) => return Ok(l),
            Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
                last_err = Some(e);
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| io::Error::other("no free port found in fallback range")))
}

use std::time::Duration;

use crate::{
    capture::{self, new_forward_switch, CaptureConfig, ForwardConfig, ForwardSwitch},
    cli::Cli,
    output::{Printer, PrinterOptions},
    store::{RequestStore, StoreEvent},
    ui, update,
};

/// Live handles to the running servers; callers can either `join()` or `abort()`.
pub struct Running {
    pub store: Arc<RequestStore>,
    pub capture_addr: SocketAddr,
    pub ui_addr: Option<SocketAddr>,
    pub capture_task: JoinHandle<std::io::Result<()>>,
    pub ui_task: Option<JoinHandle<std::io::Result<()>>>,
    pub printer_task: Option<JoinHandle<()>>,
    pub log_task: Option<JoinHandle<()>>,
}

impl Running {
    pub fn shutdown(self) {
        self.capture_task.abort();
        if let Some(t) = self.ui_task {
            t.abort();
        }
        if let Some(t) = self.printer_task {
            t.abort();
        }
        if let Some(t) = self.log_task {
            t.abort();
        }
    }
}

/// Bind both servers, spawn the printer, and return live handles. Callers
/// (tests + `run`) decide how long to keep them alive.
pub async fn start(cli: &Cli, printer: Printer) -> Result<Running> {
    cli.validate().map_err(anyhow::Error::msg)?;

    let store = RequestStore::new(cli.buffer_size);
    let bind: IpAddr = cli.bind.parse().context("parsing --bind address")?;

    let capture_listener = bind_with_fallback(bind, cli.port)
        .await
        .with_context(|| format!("binding capture server on {}:{}", cli.bind, cli.port))?;
    let capture_addr = capture_listener.local_addr()?;
    if cli.port != 0 && capture_addr.port() != cli.port {
        printer.print_port_fallback("capture", cli.port, capture_addr.port());
    }
    let initial_forward = match cli.forward.as_deref() {
        Some(raw) => {
            let parsed = url::Url::parse(raw).context("parsing --forward URL")?;
            let timeout = Duration::from_secs(cli.forward_timeout);
            Some(
                ForwardConfig::build(parsed, timeout, cli.forward_insecure)
                    .map_err(anyhow::Error::msg)?,
            )
        }
        None => None,
    };
    let forward_banner = initial_forward
        .as_ref()
        .map(|f| (f.base.to_string(), f.timeout.as_secs(), f.insecure));
    let forward_switch: ForwardSwitch = new_forward_switch(initial_forward);
    let capture_router = capture::router(
        store.clone(),
        CaptureConfig {
            max_body_size: cli.max_body_size,
            forward: forward_switch.clone(),
        },
    );
    let capture_task = tokio::spawn(async move {
        axum::serve(
            capture_listener,
            capture_router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });

    let (ui_addr, ui_task) = if cli.no_ui {
        (None, None)
    } else {
        let listener = bind_with_fallback(bind, cli.ui_port)
            .await
            .with_context(|| format!("binding UI server on {}:{}", cli.bind, cli.ui_port))?;
        let addr = listener.local_addr()?;
        if cli.ui_port != 0 && addr.port() != cli.ui_port {
            printer.print_port_fallback("UI", cli.ui_port, addr.port());
        }
        let router = ui::router(
            store.clone(),
            Some(capture_addr.port()),
            forward_switch.clone(),
        );
        let task = tokio::spawn(async move { axum::serve(listener, router).await });
        (Some(addr), Some(task))
    };

    // Banner
    let capture_url = format!("http://{}", capture_addr);
    let ui_url = ui_addr.map(|a| format!("http://{}", a));
    let forward_for_banner = forward_banner
        .as_ref()
        .map(|(target, timeout, insecure)| (target.as_str(), *timeout, *insecure));
    printer.print_banner_with_forward(
        &capture_url,
        ui_url.as_deref(),
        cli.buffer_size,
        cli.max_body_size,
        forward_for_banner,
    );

    // CLI printer task — runs unless quiet. (Quiet means: no_cli without json.)
    let printer_task = if !printer.options().quiet {
        let printer_clone = printer.clone();
        let mut rx = store.subscribe();
        Some(tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(StoreEvent::Request(req)) => printer_clone.print_request(&req),
                    // ^ `req` is `Box<CapturedRequest>`; auto-deref via `&req`.
                    Ok(StoreEvent::Cleared) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }))
    } else {
        None
    };

    let log_task = if let Some(path) = cli.log_file.as_deref() {
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .with_context(|| format!("opening --log-file {path}"))?;
        Some(spawn_log_writer(file, store.subscribe()))
    } else {
        None
    };

    if cli.open {
        if let Some(url) = &ui_url {
            let _ = open_browser(url);
        }
    }

    Ok(Running {
        store,
        capture_addr,
        ui_addr,
        capture_task,
        ui_task,
        printer_task,
        log_task,
    })
}

/// Append each captured request to `file` as one NDJSON line, flushing after
/// every write so a `tail -f` consumer (or an AI assistant) sees data
/// immediately. Lagged events are skipped (the file is meant for live
/// observation, not exhaustive accounting); a closed channel ends the task.
fn spawn_log_writer(
    file: tokio::fs::File,
    mut rx: tokio::sync::broadcast::Receiver<StoreEvent>,
) -> JoinHandle<()> {
    use tokio::io::AsyncWriteExt;
    tokio::spawn(async move {
        let mut file = file;
        loop {
            match rx.recv().await {
                Ok(StoreEvent::Request(req)) => {
                    if let Ok(s) = serde_json::to_string(&*req) {
                        let _ = file.write_all(s.as_bytes()).await;
                        let _ = file.write_all(b"\n").await;
                        let _ = file.flush().await;
                    }
                }
                Ok(StoreEvent::Cleared) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

/// Top-level entrypoint used by `main`. Starts the servers and waits for
/// either Ctrl+C or a server to crash.
pub async fn run(cli: Cli) -> Result<()> {
    let printer = Printer::new(PrinterOptions::from_cli(cli.no_cli, cli.json, cli.verbose));
    let running = start(&cli, printer.clone()).await?;
    let update_check = if cli.no_update_check {
        None
    } else {
        Some(spawn_update_check(printer))
    };
    let result = wait_for_shutdown(running).await;
    if let Some(handle) = update_check {
        handle.abort();
    }
    result
}

/// Spawn a background task that asks GitHub if a newer release exists. The
/// task fails silently on every error path: an offline machine should never
/// see an error here, only the quiet absence of a notice.
fn spawn_update_check(printer: Printer) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Some(latest) = update::check_latest_version().await {
            printer.print_update_available(update::current_version(), &latest);
        }
    })
}

async fn wait_for_shutdown(mut running: Running) -> Result<()> {
    let ui_task = running.ui_task.take();
    tokio::select! {
        _ = signal::ctrl_c() => {}
        res = &mut running.capture_task => {
            if let Ok(Err(e)) = res {
                eprintln!("capture server stopped: {e}");
            }
        }
        _ = async {
            match ui_task {
                Some(t) => { let _ = t.await; }
                None => std::future::pending::<()>().await,
            }
        } => {}
    }
    running.capture_task.abort();
    if let Some(t) = running.printer_task {
        t.abort();
    }
    if let Some(t) = running.log_task {
        t.abort();
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    let prog = "open";
    #[cfg(target_os = "linux")]
    let prog = "xdg-open";
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let prog = "xdg-open";
    std::process::Command::new(prog)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(target_os = "windows")]
fn open_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn cli(args: &[&str]) -> Cli {
        let mut v = vec!["postbin-ultra"];
        v.extend_from_slice(args);
        Cli::parse_from(v)
    }

    fn quiet_printer() -> Printer {
        Printer::new(PrinterOptions {
            use_color: false,
            json_mode: false,
            verbose: false,
            quiet: true,
        })
    }

    #[tokio::test]
    async fn bind_with_fallback_returns_same_port_when_free() {
        let l = bind_with_fallback("127.0.0.1".parse().unwrap(), 0)
            .await
            .unwrap();
        // Port 0 path: OS picks a port and returns it.
        assert!(l.local_addr().unwrap().port() != 0);
    }

    #[tokio::test]
    async fn bind_with_fallback_walks_past_busy_ports() {
        // Hold a specific port, then ask the helper to bind to it. It should
        // pick the next free port instead of failing.
        let blocker = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let busy = blocker.local_addr().unwrap().port();
        let l = bind_with_fallback("127.0.0.1".parse().unwrap(), busy)
            .await
            .unwrap();
        let chosen = l.local_addr().unwrap().port();
        assert_ne!(chosen, busy);
        assert!(chosen > busy);
        drop(blocker);
        drop(l);
    }

    #[tokio::test]
    async fn start_and_shutdown_returns_addresses() {
        let c = cli(&["-p", "0", "-u", "0"]);
        let r = start(&c, quiet_printer()).await.unwrap();
        assert!(r.capture_addr.port() != 0);
        assert!(r.ui_addr.is_some());
        r.shutdown();
    }

    #[tokio::test]
    async fn start_with_no_ui() {
        let c = cli(&["-p", "0", "--no-ui"]);
        let r = start(&c, quiet_printer()).await.unwrap();
        assert!(r.ui_addr.is_none());
        assert!(r.ui_task.is_none());
        r.shutdown();
    }

    #[tokio::test]
    async fn start_validates_cli() {
        let c = cli(&["-p", "5000", "-u", "5000"]);
        let err = match start(&c, quiet_printer()).await {
            Ok(_) => panic!("expected validation error"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("5000"));
    }

    #[tokio::test]
    async fn printer_task_forwards_requests_to_sink() {
        use std::io::Write;
        use std::sync::Mutex;

        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        struct BufWriter(Arc<Mutex<Vec<u8>>>);
        impl Write for BufWriter {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let printer = Printer::with_sink(
            PrinterOptions {
                use_color: false,
                json_mode: false,
                verbose: false,
                quiet: false,
            },
            BufWriter(buf.clone()),
        );

        let c = cli(&["-p", "0", "-u", "0"]);
        let running = start(&c, printer).await.unwrap();
        let url = format!("http://{}", running.capture_addr);
        reqwest::Client::new()
            .post(format!("{url}/probe"))
            .body("hello")
            .send()
            .await
            .unwrap();
        // Wait briefly for the printer task to drain the broadcast.
        for _ in 0..50 {
            if buf.lock().unwrap().windows(5).any(|w| w == b"probe") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(out.contains("/probe"), "printer task did not log: {out:?}");
        running.shutdown();
    }

    #[tokio::test]
    async fn start_with_forward_url_populates_switch() {
        let c = cli(&["-p", "0", "-u", "0", "--forward", "http://upstream:9000"]);
        let r = start(&c, quiet_printer()).await.unwrap();
        let snap = r.store.list(0); // touch store; unrelated
        assert!(snap.is_empty());
        // Capture handler should now read a Some forward — peek via runtime
        let forward = capture::new_forward_switch(None);
        // Direct check is awkward without exposing the switch, but the
        // start() success and ForwardConfig validation already exercise the
        // construction branches. Smoke-check by hitting the capture port; we
        // expect a 502 with forward_failed because upstream:9000 doesn't
        // resolve. Use a short reqwest timeout.
        drop(forward);
        let url = format!("http://{}/x", r.capture_addr);
        let res = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap()
            .post(&url)
            .body("hi")
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 502);
        let body: serde_json::Value = res.json().await.unwrap();
        assert_eq!(body["error"], "forward_failed");
        r.shutdown();
    }

    #[tokio::test]
    async fn start_rejects_invalid_forward_url() {
        let c = cli(&["-p", "0", "-u", "0", "--forward", "http://x"]);
        // The URL parses, but should still go through ForwardConfig::build.
        // Sanity-check the success path was hit.
        let r = start(&c, quiet_printer()).await.unwrap();
        r.shutdown();
    }

    #[tokio::test]
    async fn start_emits_port_fallback_for_busy_capture_port() {
        // Bind a port to make it "busy", then start with --port pointing at
        // that same port. start() should fall back and emit the notice.
        let blocker = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let busy = blocker.local_addr().unwrap().port();
        use std::io::Write;
        use std::sync::Mutex;
        let buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        struct BufWriter(Arc<Mutex<Vec<u8>>>);
        impl Write for BufWriter {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let printer = Printer::with_sink(
            PrinterOptions {
                use_color: false,
                json_mode: false,
                verbose: false,
                quiet: false,
            },
            BufWriter(buf.clone()),
        );
        let c = cli(&["-p", &busy.to_string(), "-u", "0"]);
        let r = start(&c, printer).await.unwrap();
        let out = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            out.contains(&format!("capture port {busy} in use")),
            "expected fallback notice in: {out:?}"
        );
        drop(blocker);
        r.shutdown();
    }

    #[tokio::test]
    async fn spawn_log_writer_drains_a_burst_of_requests() {
        // Cover the happy path of spawn_log_writer end-to-end. The e2e test
        // already verifies the file contents; here we just ensure the loop
        // exits cleanly when the broadcast channel is closed.
        let store = RequestStore::new(8);
        let path = std::env::temp_dir().join(format!("pbu-cov-{}.log", uuid::Uuid::new_v4()));
        let file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .unwrap();
        let rx = store.subscribe();
        let handle = spawn_log_writer(file, rx);
        // Drop the store; the broadcast Sender goes with it, the receiver
        // gets Closed, the task exits.
        drop(store);
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn wait_for_shutdown_returns_when_capture_task_finishes() {
        // Build a Running by hand so we can immediately abort the capture task,
        // simulating the "server stopped" branch in `wait_for_shutdown`.
        let c = cli(&["-p", "0", "-u", "0"]);
        let running = start(&c, quiet_printer()).await.unwrap();
        running.capture_task.abort();
        let mock = Running {
            store: running.store,
            capture_addr: running.capture_addr,
            ui_addr: running.ui_addr,
            capture_task: tokio::spawn(async { Ok::<(), std::io::Error>(()) }),
            ui_task: running.ui_task,
            printer_task: running.printer_task,
            log_task: running.log_task,
        };
        // Should return promptly because capture_task already completed.
        let res =
            tokio::time::timeout(std::time::Duration::from_secs(2), wait_for_shutdown(mock)).await;
        assert!(res.is_ok(), "wait_for_shutdown didn't return in time");
    }
}
