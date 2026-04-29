//! Persistent user-facing settings for the Postbin Ultra desktop app.
//!
//! Loaded from a JSON file in the platform config dir at startup, edited
//! through the in-app Settings dialog, and written back atomically. Lib-only:
//! the struct shape lives here, the desktop crate owns the UI.

use std::io;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Forward / proxy-mode subsection. Keeping the `enabled` flag separate from
/// `url` lets the user toggle proxy mode off without losing the URL they had
/// configured.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForwardSettings {
    pub enabled: bool,
    pub url: String,
    pub timeout_secs: u64,
    pub insecure: bool,
}

impl Default for ForwardSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            timeout_secs: 30,
            insecure: false,
        }
    }
}

/// Theme preference. `System` follows the OS appearance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

/// All user-configurable settings, persisted as JSON in the platform config
/// directory. `#[serde(default)]` lets us add and remove fields between
/// versions without breaking on-disk files — unknown legacy keys
/// (e.g. `ui_port`, `serve_web_ui` from pre-2.0) are silently dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub port: u16,
    pub bind: String,
    pub max_body_size: usize,
    pub buffer_size: usize,
    pub no_update_check: bool,
    pub forward: ForwardSettings,
    pub log_file: Option<PathBuf>,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 9000,
            bind: "127.0.0.1".into(),
            max_body_size: 10 * 1024 * 1024,
            buffer_size: 1000,
            no_update_check: false,
            forward: ForwardSettings::default(),
            log_file: None,
            theme: Theme::System,
        }
    }
}

impl Settings {
    /// `~/Library/Application Support/PostbinUltra/settings.json` on macOS,
    /// `$XDG_CONFIG_HOME/postbin-ultra/settings.json` on Linux,
    /// `%APPDATA%\PostbinUltra\settings.json` on Windows.
    /// Returns `None` only when `dirs::config_dir()` cannot resolve a path
    /// (very rare; e.g. unset `$HOME` on Unix).
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("PostbinUltra").join("settings.json"))
    }

    /// Read settings from `path`, falling back to [`Settings::default`] for
    /// any failure (missing file, malformed JSON, IO error). The desktop app
    /// surfaces "we couldn't read your settings, using defaults" exactly once
    /// at launch — the trade-off is that a corrupted file does not block the
    /// app from starting.
    pub fn load_or_default(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str::<Self>(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Atomic write: pretty-prints to a sibling `.tmp` and then renames into
    /// place so a crash mid-write never leaves the user with a half-written
    /// settings file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        self.bind
            .parse::<IpAddr>()
            .map_err(|_| format!("invalid bind address: {}", self.bind))?;
        if self.max_body_size == 0 {
            return Err("max body size must be greater than zero".into());
        }
        if self.buffer_size == 0 {
            return Err("buffer size must be greater than zero".into());
        }
        if self.forward.enabled {
            let parsed = url::Url::parse(&self.forward.url)
                .map_err(|e| format!("invalid forward URL '{}': {e}", self.forward.url))?;
            match parsed.scheme() {
                "http" | "https" => {}
                other => return Err(format!("forward URL must use http or https, got '{other}'")),
            }
        }
        if self.forward.timeout_secs == 0 {
            return Err("forward timeout must be greater than zero".into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_are_sensible() {
        let s = Settings::default();
        assert_eq!(s.port, 9000);
        assert_eq!(s.bind, "127.0.0.1");
        assert_eq!(s.max_body_size, 10 * 1024 * 1024);
        assert_eq!(s.buffer_size, 1000);
        assert!(!s.no_update_check);
        assert!(!s.forward.enabled);
        assert_eq!(s.forward.timeout_secs, 30);
        assert!(s.log_file.is_none());
        assert_eq!(s.theme, Theme::System);
    }

    #[test]
    fn validate_accepts_defaults() {
        Settings::default().validate().unwrap();
    }

    #[test]
    fn validate_rejects_bad_bind() {
        let mut s = Settings::default();
        s.bind = "not-an-ip".into();
        let err = s.validate().unwrap_err();
        assert!(err.contains("invalid bind"));
    }

    #[test]
    fn validate_rejects_zero_capacities() {
        let mut s = Settings::default();
        s.max_body_size = 0;
        assert!(s.validate().is_err());
        let mut s = Settings::default();
        s.buffer_size = 0;
        assert!(s.validate().is_err());
        let mut s = Settings::default();
        s.forward.timeout_secs = 0;
        assert!(s.validate().is_err());
    }

    #[test]
    fn validate_forward_url_only_when_enabled() {
        let mut s = Settings::default();
        s.forward.enabled = false;
        s.forward.url = "not a url".into();
        s.validate().unwrap();

        s.forward.enabled = true;
        let err = s.validate().unwrap_err();
        assert!(err.contains("invalid forward URL"));
    }

    #[test]
    fn validate_forward_scheme_must_be_http() {
        let mut s = Settings::default();
        s.forward.enabled = true;
        s.forward.url = "ftp://example.com".into();
        let err = s.validate().unwrap_err();
        assert!(err.contains("http or https"));
    }

    #[test]
    fn save_load_round_trip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let mut s = Settings::default();
        s.port = 7777;
        s.bind = "0.0.0.0".into();
        s.forward.enabled = true;
        s.forward.url = "https://api.example.com/v1".into();
        s.theme = Theme::Dark;
        s.save(&path).unwrap();
        let loaded = Settings::load_or_default(&path);
        assert_eq!(s, loaded);
    }

    #[test]
    fn load_returns_default_for_missing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let s = Settings::load_or_default(&path);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn load_returns_default_for_invalid_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let s = Settings::load_or_default(&path);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn load_tolerates_partial_json_via_serde_default() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("partial.json");
        std::fs::write(&path, br#"{"port":1234}"#).unwrap();
        let s = Settings::load_or_default(&path);
        assert_eq!(s.port, 1234);
        assert_eq!(s.bind, "127.0.0.1");
        assert_eq!(s.max_body_size, 10 * 1024 * 1024);
    }

    #[test]
    fn load_drops_unknown_legacy_fields() {
        // Pre-2.0 settings files include `ui_port` and `serve_web_ui`. They
        // have to be silently dropped so existing installs upgrade cleanly.
        let dir = tempdir().unwrap();
        let path = dir.path().join("legacy.json");
        std::fs::write(
            &path,
            br#"{"port":9000,"ui_port":9001,"serve_web_ui":true,"bind":"127.0.0.1"}"#,
        )
        .unwrap();
        let s = Settings::load_or_default(&path);
        assert_eq!(s.port, 9000);
        assert_eq!(s.bind, "127.0.0.1");
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let path = dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("settings.json");
        Settings::default().save(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn theme_serializes_lowercase() {
        let mut s = Settings::default();
        s.theme = Theme::Dark;
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"theme\":\"dark\""));
    }

    #[test]
    fn default_path_resolves_under_config_dir() {
        let path = Settings::default_path();
        if let Some(p) = path {
            // Compare path components rather than the string form so the test
            // passes on Windows (backslash separators) and on Unix.
            let mut iter = p.components().rev();
            assert_eq!(
                iter.next().map(|c| c.as_os_str().to_owned()),
                Some(std::ffi::OsString::from("settings.json"))
            );
            assert_eq!(
                iter.next().map(|c| c.as_os_str().to_owned()),
                Some(std::ffi::OsString::from("PostbinUltra"))
            );
        }
    }
}
