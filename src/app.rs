use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::task::JoinHandle;

use crate::{
    capture::{self, CaptureConfig},
    cli::Cli,
    output::{Printer, PrinterOptions},
    store::{RequestStore, StoreEvent},
    ui,
};

/// Live handles to the running servers; callers can either `join()` or `abort()`.
pub struct Running {
    pub store: Arc<RequestStore>,
    pub capture_addr: SocketAddr,
    pub ui_addr: Option<SocketAddr>,
    pub capture_task: JoinHandle<std::io::Result<()>>,
    pub ui_task: Option<JoinHandle<std::io::Result<()>>>,
    pub printer_task: Option<JoinHandle<()>>,
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
    }
}

/// Bind both servers, spawn the printer, and return live handles. Callers
/// (tests + `run`) decide how long to keep them alive.
pub async fn start(cli: &Cli, printer: Printer) -> Result<Running> {
    cli.validate().map_err(anyhow::Error::msg)?;

    let store = RequestStore::new(cli.buffer_size);
    let bind: IpAddr = cli.bind.parse().context("parsing --bind address")?;

    let capture_listener = TcpListener::bind(SocketAddr::new(bind, cli.port))
        .await
        .with_context(|| format!("binding capture server on {}:{}", cli.bind, cli.port))?;
    let capture_addr = capture_listener.local_addr()?;
    let capture_router = capture::router(
        store.clone(),
        CaptureConfig {
            max_body_size: cli.max_body_size,
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
        let listener = TcpListener::bind(SocketAddr::new(bind, cli.ui_port))
            .await
            .with_context(|| format!("binding UI server on {}:{}", cli.bind, cli.ui_port))?;
        let addr = listener.local_addr()?;
        let router = ui::router(store.clone());
        let task = tokio::spawn(async move { axum::serve(listener, router).await });
        (Some(addr), Some(task))
    };

    // Banner
    let capture_url = format!("http://{}", capture_addr);
    let ui_url = ui_addr.map(|a| format!("http://{}", a));
    printer.print_banner(
        &capture_url,
        ui_url.as_deref(),
        cli.buffer_size,
        cli.max_body_size,
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
    })
}

/// Top-level entrypoint used by `main`. Starts the servers and waits for
/// either Ctrl+C or a server to crash.
pub async fn run(cli: Cli) -> Result<()> {
    let printer = Printer::new(PrinterOptions::from_cli(cli.no_cli, cli.json, cli.verbose));
    let running = start(&cli, printer).await?;
    wait_for_shutdown(running).await
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
        };
        // Should return promptly because capture_task already completed.
        let res =
            tokio::time::timeout(std::time::Duration::from_secs(2), wait_for_shutdown(mock)).await;
        assert!(res.is_ok(), "wait_for_shutdown didn't return in time");
    }
}
