//! Self-update support: a one-shot `--update` command and a non-blocking
//! startup check that prints a hint when a newer release is available.
//!
//! The startup check is best-effort. Any failure (no network, GitHub down,
//! unparseable response, slow connection that exceeds the timeout) resolves
//! silently so we never spam the terminal of a user who is offline.

use std::time::Duration;

use anyhow::Result;
use tokio::task;

const REPO_OWNER: &str = "MPJHorner";
const REPO_NAME: &str = "PostbinUltra";
const BIN_NAME: &str = "postbin-ultra";

/// Hard cap on how long the startup check is allowed to run before we give up.
/// Kept tight on purpose: a slow GitHub response should never delay the banner.
const CHECK_TIMEOUT: Duration = Duration::from_secs(3);

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Outcome of a `--update` invocation.
pub enum UpdateOutcome {
    Updated { from: String, to: String },
    AlreadyLatest(String),
}

/// Hits GitHub releases for the latest tag. Returns `Some(version)` only when
/// it is strictly newer than the running binary. Any error path returns
/// `None` so callers can treat "no update" and "failed to check" identically.
pub async fn check_latest_version() -> Option<String> {
    let join = task::spawn_blocking(fetch_latest_release_blocking);
    let fetched = tokio::time::timeout(CHECK_TIMEOUT, join).await.ok()?.ok()?;
    let latest = fetched?;
    if is_newer(&latest, current_version()) {
        Some(latest)
    } else {
        None
    }
}

fn fetch_latest_release_blocking() -> Option<String> {
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .build()
        .ok()?
        .fetch()
        .ok()?;
    let latest = releases.first()?;
    Some(latest.version.clone())
}

/// Download and replace the running binary with the latest release. Blocks
/// while the download runs; not safe to call from inside a tokio runtime.
pub fn run_self_update() -> Result<UpdateOutcome> {
    let from = current_version().to_string();
    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .show_download_progress(true)
        .current_version(&from)
        .build()?
        .update()?;
    let to = status.version().to_string();
    if status.updated() {
        Ok(UpdateOutcome::Updated { from, to })
    } else {
        Ok(UpdateOutcome::AlreadyLatest(to))
    }
}

fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_semver(remote), parse_semver(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

fn parse_semver(raw: &str) -> Option<(u32, u32, u32)> {
    let stripped = raw.trim().trim_start_matches('v');
    let mut parts = stripped.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch_raw = parts.next()?;
    let patch_digits: String = patch_raw
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let patch = patch_digits.parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_handles_v_prefix_and_pre_release() {
        assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("0.1.0-beta.1"), Some((0, 1, 0)));
        assert_eq!(parse_semver("garbage"), None);
        assert_eq!(parse_semver("1.2"), None);
    }

    #[test]
    fn is_newer_compares_correctly() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("garbage", "0.1.0"));
    }

    #[test]
    fn current_version_matches_cargo_pkg_version() {
        assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
    }
}
