//! # Top-level entrypoint, kept separate so it can be excluded from coverage.
//!
//! Everything in this file talks to something a unit test runner cannot
//! deterministically drive:
//!
//! - [`run`] orchestrates the whole process and blocks on a Ctrl+C signal.
//! - [`wait_for_shutdown`] reads OS signals via [`tokio::signal::ctrl_c`].
//! - [`spawn_update_check`] hits the GitHub releases API.
//! - [`open_browser`] shells out to `open` / `xdg-open` / `cmd /C start`.
//!
//! The pure orchestration that *can* be tested (binding listeners, building
//! configs, the printer task, the log writer task, `Running` lifecycle) lives
//! in [`crate::app`] and is fully exercised by the unit + integration tests.
//!
//! `codecov.yml`, `.github/workflows/ci.yml`, and `Makefile` all add
//! `src/entrypoint.rs` to their `--ignore-filename-regex` so that running
//! coverage does not show a false 0% on these branches. See `CLAUDE.md` for
//! the full coverage policy.

use anyhow::Result;
use tokio::signal;
use tokio::task::JoinHandle;

use crate::app::{self, Running};
use crate::cli::Cli;
use crate::output::{Printer, PrinterOptions};
use crate::update;

/// Top-level entrypoint used by `main`. Starts the servers and waits for
/// either Ctrl+C or a server to crash.
///
/// Excluded from coverage: blocks on `signal::ctrl_c()` which a test runner
/// cannot fire deterministically, and orchestrates the network-bound update
/// check.
pub async fn run(cli: Cli) -> Result<()> {
    let printer = Printer::new(PrinterOptions::from_cli(cli.no_cli, cli.json, cli.verbose));
    let running = app::start(&cli, printer.clone()).await?;
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

/// Spawn a background task that asks GitHub if a newer release exists.
///
/// Excluded from coverage: every error path resolves to a silent no-op so an
/// offline machine never sees noise. The pure version-comparison logic
/// (`parse_semver`, `is_newer` in `update.rs`) is unit-tested directly.
pub(crate) fn spawn_update_check(printer: Printer) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Some(latest) = update::check_latest_version().await {
            printer.print_update_available(update::current_version(), &latest);
        }
    })
}

/// Wait for either Ctrl+C, a capture-server crash, or the UI server to stop.
///
/// Excluded from coverage: the `signal::ctrl_c()` arm is the *normal* shutdown
/// path in production but cannot be exercised from a test without sending a
/// real signal to the test process.
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

/// Open `url` in the user's default browser.
///
/// Excluded from coverage: would launch a real browser if invoked, so we have
/// no good way to call it from a test. The construction of the command line
/// is straightforward enough that visual review is the right gate.
#[cfg(not(target_os = "windows"))]
pub(crate) fn open_browser(url: &str) -> std::io::Result<()> {
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
pub(crate) fn open_browser(url: &str) -> std::io::Result<()> {
    std::process::Command::new("cmd")
        .args(["/C", "start", url])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

// These tests still exercise real behaviour and guard against regressions.
// They do not contribute to coverage numbers because this file is in the
// `--ignore-filename-regex` list (see `make coverage`).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::PrinterOptions;
    use clap::Parser;

    fn cli_for(args: &[&str]) -> Cli {
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
    async fn wait_for_shutdown_returns_when_capture_task_finishes() {
        // Build a Running by hand so we can immediately abort the capture task,
        // simulating the "server stopped" branch in `wait_for_shutdown`. This
        // is the testable arm of the select! — `signal::ctrl_c` cannot be
        // fired deterministically from a test runner, so it stays uncovered
        // and is the reason this whole file is excluded from coverage.
        let c = cli_for(&["-p", "0", "-u", "0"]);
        let running = app::start(&c, quiet_printer()).await.unwrap();
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
        let res =
            tokio::time::timeout(std::time::Duration::from_secs(2), wait_for_shutdown(mock)).await;
        assert!(res.is_ok(), "wait_for_shutdown didn't return in time");
    }
}
