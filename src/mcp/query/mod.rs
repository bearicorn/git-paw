//! Data-layer reads for the MCP server.
//!
//! Every function here returns plain Rust / `serde_json` values built from
//! deterministic sources: the broker's HTTP `/log`, files on disk, git
//! process output, and parsed configuration. **No MCP types appear in this
//! module** — that boundary (design D2) keeps the tool layer and any future
//! transport reusable on top of the same reads.
//!
//! Degradation contract (design D4): functions return empty / `None` results
//! when their data source is simply absent (no broker, no session, no
//! governance config). They return `Err` only for genuine misconfiguration —
//! a configured path that exists but cannot be read — so the tool layer can
//! surface it to the client as a protocol error.

pub mod conflicts;
pub mod docs;
pub mod git;
pub mod governance;
pub mod intents;
pub mod learnings;
pub mod session;
pub mod specs;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::PawError;

/// Current wall-clock time in seconds since the Unix epoch.
#[must_use]
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Resolves a possibly-relative configured path against the repository root.
#[must_use]
pub fn resolve_under_root(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

/// Reads a configured document.
///
/// - `None` configured → `Ok(None)` (graceful degradation).
/// - configured + readable → `Ok(Some(content))`.
/// - configured + present-but-unreadable / missing → `Err` (misconfiguration
///   the client should see).
pub fn read_optional_doc(
    repo_root: &Path,
    configured: Option<&Path>,
) -> Result<Option<String>, PawError> {
    let Some(rel) = configured else {
        return Ok(None);
    };
    let path = resolve_under_root(repo_root, rel);
    std::fs::read_to_string(&path).map(Some).map_err(|e| {
        PawError::McpError(format!(
            "configured governance path {} could not be read: {e}",
            path.display()
        ))
    })
}
