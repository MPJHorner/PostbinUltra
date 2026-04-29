use base64::Engine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use uuid::Uuid;

/// A single captured HTTP request, including its raw body bytes.
#[derive(Debug, Clone)]
pub struct CapturedRequest {
    pub id: Uuid,
    pub received_at: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub query: String,
    pub version: String,
    pub remote_addr: String,
    pub headers: Vec<(String, String)>,
    pub body: Bytes,
    pub body_truncated: bool,
    pub body_bytes_received: usize,
    /// Forward attempts for this request, oldest first. The first entry is
    /// the live forward (if forwarding was on at the time of capture);
    /// subsequent entries are user-triggered Replays. Empty means this
    /// request was never forwarded.
    pub forwards: Vec<ForwardOutcome>,
}

impl CapturedRequest {
    /// Latest forward attempt, if any. Convenience for callers that only care
    /// about the most recent outcome (CLI printer, the SSE summary, etc).
    pub fn latest_forward(&self) -> Option<&ForwardOutcome> {
        self.forwards.last()
    }
}

impl CapturedRequest {
    pub fn content_type(&self) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
    }
}

/// Result of forwarding a captured request upstream. Built once per attempt;
/// re-running the forward (Replay button) overwrites the existing outcome on
/// the captured request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForwardOutcome {
    pub started_at: DateTime<Utc>,
    pub upstream_url: String,
    pub status: ForwardStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ForwardStatus {
    /// Upstream returned a response (any HTTP status — success or 5xx).
    Success {
        status_code: u16,
        headers: Vec<(String, String)>,
        body: ForwardBody,
        body_size: usize,
        duration_ms: u64,
    },
    /// We deliberately didn't forward (e.g. body was truncated).
    Skipped { reason: String },
    /// Forwarding was attempted but failed before we got a response (DNS,
    /// timeout, TLS handshake, etc).
    Error { message: String, duration_ms: u64 },
}

/// Body of a forwarded response. UTF-8 stored as a string for ergonomics; raw
/// bytes stored as base64 to keep this serializable as plain JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "encoding", rename_all = "lowercase")]
pub enum ForwardBody {
    Utf8 { text: String },
    Base64 { data: String },
}

impl ForwardBody {
    pub fn from_bytes(b: &[u8]) -> Self {
        match std::str::from_utf8(b) {
            Ok(s) => Self::Utf8 {
                text: s.to_string(),
            },
            Err(_) => Self::Base64 {
                data: base64::engine::general_purpose::STANDARD.encode(b),
            },
        }
    }

    /// Decode back to bytes for re-display in the desktop UI.
    pub fn into_bytes(&self) -> Vec<u8> {
        match self {
            Self::Utf8 { text } => text.as_bytes().to_vec(),
            Self::Base64 { data } => base64::engine::general_purpose::STANDARD
                .decode(data)
                .unwrap_or_default(),
        }
    }
}

impl Serialize for CapturedRequest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("CapturedRequest", 15)?;
        s.serialize_field("id", &self.id)?;
        s.serialize_field("received_at", &self.received_at)?;
        s.serialize_field("method", &self.method)?;
        s.serialize_field("path", &self.path)?;
        s.serialize_field("query", &self.query)?;
        s.serialize_field("version", &self.version)?;
        s.serialize_field("remote_addr", &self.remote_addr)?;
        s.serialize_field("headers", &self.headers)?;
        s.serialize_field("body_truncated", &self.body_truncated)?;
        s.serialize_field("body_bytes_received", &self.body_bytes_received)?;
        s.serialize_field("body_size", &self.body.len())?;
        match std::str::from_utf8(&self.body) {
            Ok(text) => {
                s.serialize_field("body", text)?;
                s.serialize_field("body_encoding", "utf8")?;
            }
            Err(_) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&self.body);
                s.serialize_field("body", &b64)?;
                s.serialize_field("body_encoding", "base64")?;
            }
        }
        s.serialize_field("forwards", &self.forwards)?;
        // Convenience alias so single-attempt consumers (logs, simple JSON
        // viewers) don't need to look at the array. Mirrors `latest_forward`.
        s.serialize_field("forward", &self.forwards.last())?;
        s.end()
    }
}

/// Owned, fully-deserialisable mirror of [`CapturedRequest`] used by tests and
/// by HTTP clients that consume the JSON API.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CapturedRequestJson {
    pub id: Uuid,
    pub received_at: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub query: String,
    pub version: String,
    pub remote_addr: String,
    pub headers: Vec<(String, String)>,
    pub body_truncated: bool,
    pub body_bytes_received: usize,
    pub body_size: usize,
    pub body: String,
    pub body_encoding: String,
    #[serde(default)]
    pub forwards: Vec<ForwardOutcome>,
    /// Convenience alias serialized alongside `forwards` — the latest entry.
    #[serde(default)]
    pub forward: Option<ForwardOutcome>,
}

impl CapturedRequestJson {
    /// Returns the body as raw bytes, decoding base64 if needed.
    pub fn body_bytes(&self) -> Vec<u8> {
        match self.body_encoding.as_str() {
            "base64" => base64::engine::general_purpose::STANDARD
                .decode(&self.body)
                .unwrap_or_default(),
            _ => self.body.as_bytes().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(body: Bytes) -> CapturedRequest {
        CapturedRequest {
            id: Uuid::nil(),
            received_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            method: "POST".into(),
            path: "/foo".into(),
            query: "a=1".into(),
            version: "HTTP/1.1".into(),
            remote_addr: "127.0.0.1:1234".into(),
            headers: vec![
                ("content-type".into(), "application/json".into()),
                ("set-cookie".into(), "a=1".into()),
                ("set-cookie".into(), "b=2".into()),
            ],
            body,
            body_truncated: false,
            body_bytes_received: 0,
            forwards: Vec::new(),
        }
    }

    #[test]
    fn serializes_utf8_body_as_string() {
        let req = sample(Bytes::from_static(b"hello world"));
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(v["body"], "hello world");
        assert_eq!(v["body_encoding"], "utf8");
        assert_eq!(v["body_size"], 11);
    }

    #[test]
    fn serializes_binary_body_as_base64() {
        let req = sample(Bytes::from_static(&[0xff, 0xfe, 0x00, 0x01]));
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        assert_eq!(v["body_encoding"], "base64");
        assert_eq!(v["body"], "//4AAQ==");
    }

    #[test]
    fn preserves_duplicate_headers_in_order() {
        let req = sample(Bytes::new());
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&req).unwrap()).unwrap();
        let headers = v["headers"].as_array().unwrap();
        assert_eq!(headers.len(), 3);
        assert_eq!(headers[1][0], "set-cookie");
        assert_eq!(headers[1][1], "a=1");
        assert_eq!(headers[2][0], "set-cookie");
        assert_eq!(headers[2][1], "b=2");
    }

    #[test]
    fn content_type_is_case_insensitive() {
        let mut req = sample(Bytes::new());
        req.headers = vec![("Content-Type".into(), "text/plain".into())];
        assert_eq!(req.content_type(), Some("text/plain"));
    }

    #[test]
    fn captured_request_json_decodes_base64() {
        let json = CapturedRequestJson {
            id: Uuid::nil(),
            received_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            method: "GET".into(),
            path: "/".into(),
            query: String::new(),
            version: "HTTP/1.1".into(),
            remote_addr: "127.0.0.1:1".into(),
            headers: vec![],
            body_truncated: false,
            body_bytes_received: 4,
            body_size: 4,
            body: "//4AAQ==".into(),
            body_encoding: "base64".into(),
            forwards: Vec::new(),
            forward: None,
        };
        assert_eq!(json.body_bytes(), vec![0xff, 0xfe, 0x00, 0x01]);
    }

    #[test]
    fn captured_request_json_decodes_utf8() {
        let json = CapturedRequestJson {
            id: Uuid::nil(),
            received_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            method: "GET".into(),
            path: "/".into(),
            query: String::new(),
            version: "HTTP/1.1".into(),
            remote_addr: "127.0.0.1:1".into(),
            headers: vec![],
            body_truncated: false,
            body_bytes_received: 5,
            body_size: 5,
            body: "hello".into(),
            body_encoding: "utf8".into(),
            forwards: Vec::new(),
            forward: None,
        };
        assert_eq!(json.body_bytes(), b"hello");
    }

    #[test]
    fn forward_outcome_round_trips_through_json() {
        let outcome = ForwardOutcome {
            started_at: DateTime::<Utc>::from_timestamp(123, 0).unwrap(),
            upstream_url: "https://api.example.com/v1/foo".into(),
            status: ForwardStatus::Success {
                status_code: 201,
                headers: vec![("content-type".into(), "application/json".into())],
                body: ForwardBody::Utf8 { text: "{}".into() },
                body_size: 2,
                duration_ms: 42,
            },
        };
        let s = serde_json::to_string(&outcome).unwrap();
        let back: ForwardOutcome = serde_json::from_str(&s).unwrap();
        assert_eq!(outcome, back);
    }

    #[test]
    fn forward_body_handles_binary() {
        let b = ForwardBody::from_bytes(&[0xff, 0xfe, 0x00]);
        assert!(matches!(b, ForwardBody::Base64 { .. }));
        assert_eq!(b.into_bytes(), vec![0xff, 0xfe, 0x00]);
    }

    #[test]
    fn forward_body_handles_text() {
        let b = ForwardBody::from_bytes(b"hi");
        assert!(matches!(b, ForwardBody::Utf8 { .. }));
        assert_eq!(b.into_bytes(), b"hi");
    }
}
