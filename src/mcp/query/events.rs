//! Broker event reads for an orchestrator that works a turn at a time.
//!
//! A turn-based reader that samples state misses anything that happens and
//! un-happens between two samples. Reading the broker's ordered log by cursor
//! cannot miss an event, and long-polling that cursor means the reader learns
//! about one as soon as it exists rather than on its next sample. When the
//! wait elapses with nothing new, the wave object still tells the caller
//! whether the quiet is healthy or a stall.
//!
//! Every read here degrades to an empty, successful answer when no broker is
//! reachable — never an error.

use std::time::Duration;

use rmcp::schemars;
use serde::Serialize;

use crate::broker::WaveStatus;
use crate::broker::publish::fetch_log_page_over_http;
use crate::mcp::RepoContext;

/// Default long-poll, in seconds, when the caller names none.
pub const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

/// Upper bound on the long-poll, in seconds — the same cap the broker's
/// `GET /log` enforces, applied here so the tool never asks for more than it
/// can be served.
pub const MAX_TIMEOUT_SECONDS: u64 = 60;

/// One entry from the broker's message log.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct EventEntry {
    /// Sequence number assigned at publish time.
    pub seq: u64,
    /// Wall-clock seconds since the Unix epoch when the message was published.
    pub timestamp_unix_secs: u64,
    /// The broker message, in its wire shape.
    pub message: serde_json::Value,
}

/// The broker's wave-progress object, mirrored at the MCP boundary.
#[derive(Debug, Clone, Copy, Serialize, schemars::JsonSchema)]
pub struct WaveSnapshot {
    /// Wall-clock seconds since the Unix epoch at the last observed progress.
    pub last_progress_unix_secs: u64,
    /// Seconds elapsed since the last observed progress.
    pub idle_seconds: u64,
    /// Configured stall threshold in seconds; `0` disables `stalled`.
    pub stall_threshold_seconds: u64,
    /// Whether the wave has been idle past its threshold.
    pub stalled: bool,
}

impl From<WaveStatus> for WaveSnapshot {
    fn from(w: WaveStatus) -> Self {
        Self {
            last_progress_unix_secs: w.last_progress_unix_secs,
            idle_seconds: w.idle_seconds,
            stall_threshold_seconds: w.stall_threshold_seconds,
            stalled: w.stalled,
        }
    }
}

/// The answer to a `wait_for_events` call.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct EventsSnapshot {
    /// Entries published after the requested cursor, oldest first.
    pub entries: Vec<EventEntry>,
    /// The new cursor: the highest returned sequence number, or the
    /// requested `since` when nothing was returned. Never rewinds.
    pub last_seq: u64,
    /// The serving broker instance's epoch (its start time, Unix seconds).
    ///
    /// Sequence numbers restart at zero whenever a broker starts, so a
    /// cursor only means anything against the epoch it came from. A client
    /// that sees this change must pass `since = 0` on its next call, or it
    /// will sit above everything the new broker appends and time out
    /// forever. Absent when no broker answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub epoch: Option<u64>,
    /// The broker's wave-progress object, so a waiter that timed out still
    /// learns whether the wave is stalled. Absent when no broker answered.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave: Option<WaveSnapshot>,
    /// `true` when no session broker was reachable. The call still succeeds —
    /// an orchestrator polling a repository with no live session gets an
    /// empty answer, not an error.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_session: bool,
}

impl EventsSnapshot {
    /// The answer when the read could not be performed at all (the blocking
    /// task carrying it was cancelled or panicked). Indistinguishable to the
    /// caller from "no broker": empty, cursor held, flagged — never an
    /// error, and never a rewound cursor.
    #[must_use]
    pub fn unavailable(since: u64) -> Self {
        Self::no_session(since)
    }

    /// The answer for a repository with no reachable broker: empty, cursor
    /// held at `since`, flagged.
    fn no_session(since: u64) -> Self {
        Self {
            entries: Vec::new(),
            last_seq: since,
            epoch: None,
            wave: None,
            no_session: true,
        }
    }
}

/// Returns the configured timeout, defaulted and capped.
#[must_use]
pub fn resolve_timeout_seconds(requested: Option<u64>) -> u64 {
    requested
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
        .min(MAX_TIMEOUT_SECONDS)
}

/// Long-polls the session broker's log cursor and reports the new entries,
/// the new cursor, and the wave's progress.
///
/// Returns at once with `no_session` when no broker is reachable — including
/// when the log read itself fails, since a broker that cannot be read is
/// indistinguishable to the caller from one that is not there.
#[must_use]
pub fn wait_for_events(ctx: &RepoContext, since: u64, timeout_seconds: u64) -> EventsSnapshot {
    let Some(url) = ctx.broker_url.as_deref() else {
        return EventsSnapshot::no_session(since);
    };
    let Ok(page) = fetch_log_page_over_http(
        url,
        since,
        Some(Duration::from_secs(resolve_timeout_seconds(Some(
            timeout_seconds,
        )))),
    ) else {
        return EventsSnapshot::no_session(since);
    };

    let last_seq = page.entries.iter().map(|e| e.seq).max().unwrap_or(since);
    let entries = page
        .entries
        .into_iter()
        .map(|e| EventEntry {
            seq: e.seq,
            timestamp_unix_secs: e.timestamp_unix_secs,
            message: serde_json::to_value(&e.message).unwrap_or(serde_json::Value::Null),
        })
        .collect();

    EventsSnapshot {
        entries,
        last_seq,
        epoch: Some(page.epoch),
        wave: crate::broker::publish::fetch_wave_over_http(url)
            .ok()
            .flatten()
            .map(Into::into),
        no_session: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `mcp-server` scenario "No broker returns immediately": a repository
    /// with no session answers at once, empty, with the cursor held.
    #[test]
    fn no_broker_returns_immediately_with_the_cursor_held() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = RepoContext {
            root: tmp.path().to_path_buf(),
            git_paw_dir: None,
            broker_url: None,
            server_name: "git-paw".to_string(),
            is_main_worktree: false,
            authority: None,
        };

        let started = std::time::Instant::now();
        let snapshot = wait_for_events(&ctx, 7, 30);

        assert!(snapshot.no_session);
        assert!(snapshot.entries.is_empty());
        assert_eq!(snapshot.last_seq, 7, "the cursor must not rewind");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "a missing broker must not be waited out"
        );
    }

    /// The timeout defaults to 30 seconds and is capped at the 60 the broker
    /// will serve, so a client asking for ten minutes gets one minute.
    #[test]
    fn the_timeout_is_defaulted_and_capped() {
        assert_eq!(resolve_timeout_seconds(None), 30);
        assert_eq!(resolve_timeout_seconds(Some(5)), 5);
        assert_eq!(resolve_timeout_seconds(Some(600)), 60);
    }

    /// `no_session` is omitted from the wire when false, so the ordinary
    /// answer carries no noise.
    #[test]
    fn no_session_is_omitted_when_a_broker_answered() {
        let snapshot = EventsSnapshot {
            entries: Vec::new(),
            last_seq: 3,
            epoch: Some(1_000),
            wave: None,
            no_session: false,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains("no_session"), "got {json}");
        assert!(json.contains("\"last_seq\":3"), "got {json}");
    }
}
