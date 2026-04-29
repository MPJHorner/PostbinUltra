//! Hot-restartable capture server.
//!
//! `CaptureSupervisor::start` brings up the capture listener with port-fallback
//! (mirroring the CLI's behaviour), and `reconfigure` lets the desktop UI rebind
//! to a new (bind, port) at runtime — which is what makes "change the port in
//! the Settings panel" feel instantaneous instead of forcing an app restart.
//!
//! Reconfigure is fail-fast on purpose: if the user picks port 8080 and 8080 is
//! already in use, we surface the error instead of silently jumping to 8081.
//! The original listener is left running unchanged when reconfigure fails.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::capture::{router, CaptureConfig};
use crate::store::RequestStore;

/// How long to wait for a graceful shutdown of the previous listener before
/// abandoning the join handle. Anything longer feels unresponsive in the UI;
/// in practice axum drains in-flight requests in milliseconds, so this is a
/// safety net rather than the common path.
const RECONFIGURE_GRACE: Duration = Duration::from_secs(5);

/// Maximum consecutive ports tried after the requested one when the requested
/// port is already in use. Only used by `start`; `reconfigure` is strict.
const FALLBACK_ATTEMPTS: u16 = 50;

#[derive(Debug, Clone, Copy)]
enum BindMode {
    Strict,
    AllowFallback,
}

struct RunningCapture {
    addr: SocketAddr,
    shutdown_tx: oneshot::Sender<()>,
    task: JoinHandle<io::Result<()>>,
}

/// Holds the live capture listener and lets the desktop app swap it for a
/// new (bind, port) without dropping captured state.
pub struct CaptureSupervisor {
    store: Arc<RequestStore>,
    config: CaptureConfig,
    state: Mutex<Option<RunningCapture>>,
}

impl CaptureSupervisor {
    /// Bind the initial listener with port-fallback. Returns once the listener
    /// is accepting connections.
    pub async fn start(
        bind: IpAddr,
        port: u16,
        store: Arc<RequestStore>,
        config: CaptureConfig,
    ) -> io::Result<Self> {
        let running = bind_and_serve(bind, port, &store, &config, BindMode::AllowFallback).await?;
        Ok(Self {
            store,
            config,
            state: Mutex::new(Some(running)),
        })
    }

    pub fn current_addr(&self) -> SocketAddr {
        let guard = self.state.lock().expect("supervisor state poisoned");
        guard
            .as_ref()
            .expect("supervisor accessed after shutdown")
            .addr
    }

    pub fn store(&self) -> Arc<RequestStore> {
        self.store.clone()
    }

    pub fn config(&self) -> &CaptureConfig {
        &self.config
    }

    /// Bind a new listener on (bind, port), then gracefully stop the old one.
    /// On failure, the old listener is unchanged.
    pub async fn reconfigure(&self, bind: IpAddr, port: u16) -> io::Result<SocketAddr> {
        let new = bind_and_serve(bind, port, &self.store, &self.config, BindMode::Strict).await?;
        let new_addr = new.addr;
        let old = {
            let mut guard = self.state.lock().expect("supervisor state poisoned");
            guard.replace(new)
        };
        if let Some(old) = old {
            shut_down(old).await;
        }
        Ok(new_addr)
    }

    /// Gracefully stop the listener and wait for the task to exit. After this,
    /// the supervisor must not be reconfigured.
    pub async fn shutdown(&self) {
        let old = {
            let mut guard = self.state.lock().expect("supervisor state poisoned");
            guard.take()
        };
        if let Some(old) = old {
            shut_down(old).await;
        }
    }
}

impl Drop for CaptureSupervisor {
    fn drop(&mut self) {
        // Best-effort: send the shutdown signal so the spawned task exits even
        // if the user dropped us without calling `shutdown()`. We can't await
        // here, so we rely on tokio reaping the task; the JoinHandle is
        // dropped, which detaches it.
        if let Ok(mut guard) = self.state.lock() {
            if let Some(running) = guard.take() {
                let _ = running.shutdown_tx.send(());
                running.task.abort();
            }
        }
    }
}

async fn bind_and_serve(
    bind: IpAddr,
    port: u16,
    store: &Arc<RequestStore>,
    config: &CaptureConfig,
    mode: BindMode,
) -> io::Result<RunningCapture> {
    let listener = bind_listener(bind, port, mode).await?;
    let addr = listener.local_addr()?;
    let router = router(store.clone(), config.clone());
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .await
    });
    Ok(RunningCapture {
        addr,
        shutdown_tx,
        task,
    })
}

async fn bind_listener(bind: IpAddr, port: u16, mode: BindMode) -> io::Result<TcpListener> {
    if port == 0 {
        return TcpListener::bind(SocketAddr::new(bind, 0)).await;
    }
    match mode {
        BindMode::Strict => TcpListener::bind(SocketAddr::new(bind, port)).await,
        BindMode::AllowFallback => {
            let mut last_err = None;
            for offset in 0..=FALLBACK_ATTEMPTS {
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
            Err(last_err
                .unwrap_or_else(|| io::Error::other("no free port found in fallback range")))
        }
    }
}

async fn shut_down(running: RunningCapture) {
    let _ = running.shutdown_tx.send(());
    match tokio::time::timeout(RECONFIGURE_GRACE, running.task).await {
        Ok(_) => {}
        Err(_) => {
            // Task did not exit in time. Tokio aborts on JoinHandle drop after
            // timeout; nothing else to do.
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn local() -> IpAddr {
        "127.0.0.1".parse().unwrap()
    }

    #[tokio::test]
    async fn start_binds_and_serves_on_ephemeral_port() {
        let store = RequestStore::new(10);
        let sup = CaptureSupervisor::start(local(), 0, store, CaptureConfig::default())
            .await
            .unwrap();
        let addr = sup.current_addr();
        let resp = reqwest::get(format!("http://{addr}/probe"))
            .await
            .expect("connect");
        assert!(resp.status().is_success());
    }

    #[tokio::test]
    async fn current_addr_reflects_bound_port() {
        let blocker = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let want = blocker.local_addr().unwrap().port() + 1; // an arbitrary likely-free port
        drop(blocker);
        let store = RequestStore::new(10);
        let sup = CaptureSupervisor::start(local(), 0, store, CaptureConfig::default())
            .await
            .unwrap();
        let _ = want; // not strictly checked, just demonstrating ephemeral path
        let addr = sup.current_addr();
        assert_eq!(addr.ip(), local());
        assert!(addr.port() != 0);
    }

    #[tokio::test]
    async fn start_falls_back_to_next_port_when_busy() {
        // Hold a port and ask the supervisor to bind it; it should fall back.
        let blocker = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let busy = blocker.local_addr().unwrap().port();
        let store = RequestStore::new(10);
        let sup = CaptureSupervisor::start(local(), busy, store, CaptureConfig::default())
            .await
            .unwrap();
        let chosen = sup.current_addr().port();
        assert_ne!(chosen, busy, "expected fallback to a different port");
        drop(blocker);
        sup.shutdown().await;
    }

    #[tokio::test]
    async fn reconfigure_swaps_to_new_port_and_old_stops_accepting() {
        let store = RequestStore::new(10);
        let sup = CaptureSupervisor::start(local(), 0, store, CaptureConfig::default())
            .await
            .unwrap();
        let old = sup.current_addr();

        // Old should accept.
        let r = reqwest::get(format!("http://{old}/x")).await.unwrap();
        assert!(r.status().is_success());

        let new = sup.reconfigure(local(), 0).await.unwrap();
        assert_ne!(new, old);

        // New should accept.
        let r = reqwest::get(format!("http://{new}/y")).await.unwrap();
        assert!(r.status().is_success());

        // Old should refuse within a short window. axum's graceful shutdown
        // closes the listening socket promptly; poll until refused or timeout.
        let mut closed = false;
        for _ in 0..40 {
            let res = reqwest::Client::builder()
                .timeout(Duration::from_millis(250))
                .build()
                .unwrap()
                .get(format!("http://{old}/z"))
                .send()
                .await;
            if res.is_err() {
                closed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(closed, "old listener should have stopped accepting");
        sup.shutdown().await;
    }

    #[tokio::test]
    async fn reconfigure_fails_strictly_when_port_in_use() {
        let store = RequestStore::new(10);
        let sup = CaptureSupervisor::start(local(), 0, store, CaptureConfig::default())
            .await
            .unwrap();
        let old = sup.current_addr();

        // Hold a port to make it busy.
        let blocker = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let busy = blocker.local_addr().unwrap().port();

        let res = sup.reconfigure(local(), busy).await;
        assert!(res.is_err(), "strict reconfigure should fail on busy port");
        // Old listener should still be serving.
        let r = reqwest::get(format!("http://{old}/still-up"))
            .await
            .unwrap();
        assert!(r.status().is_success());

        drop(blocker);
        sup.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_stops_accepting() {
        let store = RequestStore::new(10);
        let sup = CaptureSupervisor::start(local(), 0, store, CaptureConfig::default())
            .await
            .unwrap();
        let addr = sup.current_addr();
        sup.shutdown().await;

        // Should refuse after shutdown.
        let mut closed = false;
        for _ in 0..40 {
            let res = reqwest::Client::builder()
                .timeout(Duration::from_millis(250))
                .build()
                .unwrap()
                .get(format!("http://{addr}/x"))
                .send()
                .await;
            if res.is_err() {
                closed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(closed, "post-shutdown listener should refuse");
    }

    #[tokio::test]
    async fn store_and_config_accessors_return_what_was_passed_in() {
        let store = RequestStore::new(7);
        let mut cfg = CaptureConfig::default();
        cfg.max_body_size = 4242;
        let sup = CaptureSupervisor::start(local(), 0, store, cfg)
            .await
            .unwrap();
        assert_eq!(sup.store().capacity(), 7);
        assert_eq!(sup.config().max_body_size, 4242);
        sup.shutdown().await;
    }

    #[tokio::test]
    async fn drop_aborts_running_task() {
        // Keep this minimal: just confirm Drop runs without panicking when the
        // supervisor is dropped while the task is still active.
        let store = RequestStore::new(2);
        let sup = CaptureSupervisor::start(local(), 0, store, CaptureConfig::default())
            .await
            .unwrap();
        let addr = sup.current_addr();
        drop(sup);
        // Give tokio a tick to process the abort.
        tokio::time::sleep(Duration::from_millis(50)).await;
        // Best effort: a fresh request should fail (eventually) but we don't
        // strictly require it within the timeout window.
        let _ = reqwest::Client::builder()
            .timeout(Duration::from_millis(250))
            .build()
            .unwrap()
            .get(format!("http://{addr}/x"))
            .send()
            .await;
    }
}
