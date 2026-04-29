use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::request::{CapturedRequest, ForwardOutcome};

/// Events broadcast when the store changes. Used by the SSE stream and the CLI
/// printer task. The request is boxed so the enum stays small even when bodies
/// are large.
#[derive(Debug, Clone)]
pub enum StoreEvent {
    Request(Box<CapturedRequest>),
    /// A request's `forward` outcome was set or replaced (live forward
    /// completed, or a Replay finished). Carries the full updated request so
    /// subscribers don't have to hit the store again.
    ForwardUpdated(Box<CapturedRequest>),
    Cleared,
}

/// Bounded in-memory ring buffer of captured requests with a broadcast channel
/// that fans out new arrivals to every subscriber (CLI + SSE clients).
pub struct RequestStore {
    inner: Mutex<VecDeque<CapturedRequest>>,
    capacity: usize,
    tx: broadcast::Sender<StoreEvent>,
}

impl RequestStore {
    pub fn new(capacity: usize) -> Arc<Self> {
        let cap = capacity.max(1);
        let (tx, _) = broadcast::channel(256);
        Arc::new(Self {
            inner: Mutex::new(VecDeque::with_capacity(cap)),
            capacity: cap,
            tx,
        })
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn push(&self, req: CapturedRequest) {
        {
            let mut q = self.inner.lock().expect("store poisoned");
            if q.len() >= self.capacity {
                q.pop_front();
            }
            q.push_back(req.clone());
        }
        let _ = self.tx.send(StoreEvent::Request(Box::new(req)));
    }

    /// Returns up to `limit` most-recent requests, newest first.
    pub fn list(&self, limit: usize) -> Vec<CapturedRequest> {
        let q = self.inner.lock().expect("store poisoned");
        q.iter().rev().take(limit).cloned().collect()
    }

    pub fn get(&self, id: Uuid) -> Option<CapturedRequest> {
        let q = self.inner.lock().expect("store poisoned");
        q.iter().find(|r| r.id == id).cloned()
    }

    /// Append a forward attempt to the matching request's history and
    /// broadcast a [`StoreEvent::ForwardUpdated`]. The first appended outcome
    /// is the live forward; subsequent ones are user-triggered Replays.
    /// No-op if `id` isn't in the buffer (e.g. the request was already
    /// evicted by a long-running replay). Returns the updated request so the
    /// caller can act on it without a second lookup.
    pub fn append_forward(&self, id: Uuid, outcome: ForwardOutcome) -> Option<CapturedRequest> {
        let updated = {
            let mut q = self.inner.lock().expect("store poisoned");
            let req = q.iter_mut().find(|r| r.id == id)?;
            req.forwards.push(outcome);
            req.clone()
        };
        let _ = self
            .tx
            .send(StoreEvent::ForwardUpdated(Box::new(updated.clone())));
        Some(updated)
    }

    pub fn clear(&self) {
        {
            let mut q = self.inner.lock().expect("store poisoned");
            q.clear();
        }
        let _ = self.tx.send(StoreEvent::Cleared);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StoreEvent> {
        self.tx.subscribe()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("store poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use chrono::Utc;

    fn req(path: &str) -> CapturedRequest {
        CapturedRequest {
            id: Uuid::new_v4(),
            received_at: Utc::now(),
            method: "GET".into(),
            path: path.into(),
            query: String::new(),
            version: "HTTP/1.1".into(),
            remote_addr: "127.0.0.1:1".into(),
            headers: vec![],
            body: Bytes::new(),
            body_truncated: false,
            body_bytes_received: 0,
            forwards: Vec::new(),
        }
    }

    #[test]
    fn capacity_minimum_one() {
        let s = RequestStore::new(0);
        assert_eq!(s.capacity(), 1);
    }

    #[test]
    fn push_list_get_clear() {
        let s = RequestStore::new(10);
        let r1 = req("/a");
        let r2 = req("/b");
        s.push(r1.clone());
        s.push(r2.clone());
        assert_eq!(s.len(), 2);
        let list = s.list(10);
        // newest first
        assert_eq!(list[0].id, r2.id);
        assert_eq!(list[1].id, r1.id);
        assert!(s.get(r1.id).is_some());
        assert!(s.get(Uuid::new_v4()).is_none());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn evicts_oldest_when_full() {
        let s = RequestStore::new(2);
        let r1 = req("/a");
        let r2 = req("/b");
        let r3 = req("/c");
        s.push(r1.clone());
        s.push(r2.clone());
        s.push(r3.clone());
        assert_eq!(s.len(), 2);
        let list = s.list(10);
        assert_eq!(list[0].id, r3.id);
        assert_eq!(list[1].id, r2.id);
        // r1 was evicted
        assert!(s.get(r1.id).is_none());
    }

    #[test]
    fn list_limit_respected() {
        let s = RequestStore::new(100);
        for i in 0..50 {
            s.push(req(&format!("/{i}")));
        }
        assert_eq!(s.list(10).len(), 10);
        assert_eq!(s.list(100).len(), 50);
    }

    #[tokio::test]
    async fn subscribers_receive_events() {
        let s = RequestStore::new(10);
        let mut rx1 = s.subscribe();
        let mut rx2 = s.subscribe();
        let r = req("/x");
        s.push(r.clone());
        match rx1.recv().await.unwrap() {
            StoreEvent::Request(got) => assert_eq!(got.id, r.id),
            StoreEvent::Cleared => panic!("expected request event"),
            #[allow(unreachable_patterns)]
            StoreEvent::ForwardUpdated(_) => panic!("expected request event"),
        }
        match rx2.recv().await.unwrap() {
            StoreEvent::Request(got) => assert_eq!(got.id, r.id),
            StoreEvent::Cleared => panic!("expected request event"),
            #[allow(unreachable_patterns)]
            StoreEvent::ForwardUpdated(_) => panic!("expected request event"),
        }
        s.clear();
        assert!(matches!(rx1.recv().await.unwrap(), StoreEvent::Cleared));
    }

    #[tokio::test]
    async fn append_forward_pushes_outcome_and_broadcasts() {
        use crate::request::{ForwardBody, ForwardStatus};
        let s = RequestStore::new(10);
        let r = req("/x");
        let id = r.id;
        s.push(r);
        let mut rx = s.subscribe();
        let outcome = ForwardOutcome {
            started_at: Utc::now(),
            upstream_url: "https://api.example.com/x".into(),
            status: ForwardStatus::Success {
                status_code: 200,
                headers: vec![],
                body: ForwardBody::Utf8 { text: "ok".into() },
                body_size: 2,
                duration_ms: 5,
            },
        };
        let updated = s.append_forward(id, outcome.clone()).unwrap();
        assert_eq!(updated.forwards.len(), 1);
        assert_eq!(updated.latest_forward(), Some(&outcome));
        match rx.recv().await.unwrap() {
            StoreEvent::ForwardUpdated(got) => {
                assert_eq!(got.id, id);
                assert_eq!(got.forwards.len(), 1);
            }
            other => panic!("expected ForwardUpdated, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn append_forward_accumulates_replays() {
        use crate::request::{ForwardBody, ForwardStatus};
        let s = RequestStore::new(10);
        let r = req("/x");
        let id = r.id;
        s.push(r);
        let make = |code: u16| ForwardOutcome {
            started_at: Utc::now(),
            upstream_url: "https://x".into(),
            status: ForwardStatus::Success {
                status_code: code,
                headers: vec![],
                body: ForwardBody::Utf8 {
                    text: code.to_string(),
                },
                body_size: 3,
                duration_ms: 1,
            },
        };
        s.append_forward(id, make(200)).unwrap();
        s.append_forward(id, make(500)).unwrap();
        let stored = s.get(id).unwrap();
        assert_eq!(stored.forwards.len(), 2);
        match &stored.latest_forward().unwrap().status {
            ForwardStatus::Success { status_code, .. } => assert_eq!(*status_code, 500),
            _ => panic!("expected success"),
        }
    }

    #[test]
    fn append_forward_returns_none_for_missing_id() {
        use crate::request::ForwardStatus;
        let s = RequestStore::new(10);
        let outcome = ForwardOutcome {
            started_at: Utc::now(),
            upstream_url: "https://x".into(),
            status: ForwardStatus::Skipped {
                reason: "n/a".into(),
            },
        };
        assert!(s.append_forward(Uuid::new_v4(), outcome).is_none());
    }

    #[test]
    fn subscribe_with_no_listeners_does_not_panic() {
        let s = RequestStore::new(10);
        // No subscribers; push should not error
        s.push(req("/x"));
        s.clear();
    }
}
