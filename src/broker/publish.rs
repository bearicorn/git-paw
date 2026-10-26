//! Helpers for constructing and publishing `BrokerMessage` values to a
//! running broker over HTTP.
//!
//! These helpers live in the library so any caller outside the broker
//! process (e.g. `commands::supervisor::supervisor` self-registration) can publish messages
//! using the same wire format and error handling.

use std::time::Duration;

use crate::broker::WaveStatus;
use crate::broker::client::BrokerClient;
use crate::broker::messages::{
    ArtifactPayload, BlockedPayload, BrokerMessage, QuestionPayload, StatusPayload,
};
use crate::error::PawError;

/// Builds an `agent.status` broker message with the given fields.
///
/// The `modified_files` list is always empty; this helper is intended for
/// status pings (boot announcements, merge results, supervisor heartbeats),
/// not for artifact-style messages.
///
/// `cli` populates the optional `StatusPayload.cli` field — set it when the
/// caller knows which CLI is publishing (typically the supervisor self-
/// registration path, where the CLI name is resolved from
/// `[supervisor].cli`). Pass `None` for coding-agent paths, which rely on
/// the broker's watch-target map to populate the dashboard CLI column.
///
/// `phase` and `activity` are intentionally not exposed through this helper.
/// Callers that want to publish a phase label or an activity phrase SHALL
/// construct the [`BrokerMessage::Status`] directly with a fully-populated
/// [`StatusPayload`].
pub fn build_status_message(
    agent_id: &str,
    status: &str,
    message: Option<String>,
    cli: Option<&str>,
) -> BrokerMessage {
    BrokerMessage::Status {
        agent_id: agent_id.to_string(),
        payload: StatusPayload {
            status: status.to_string(),
            modified_files: Vec::new(),
            message,
            cli: cli.map(str::to_string),
            phase: None,
            detail: None,
            activity: None,
        },
    }
}

/// Builds an `agent.artifact` broker message, matching the `broker.sh
/// artifact` shape exactly (`status` is always `"done"` — the code-less DONE
/// fallback; the post-commit-hook path never goes through this helper).
pub fn build_artifact_message(
    agent_id: &str,
    exports: Vec<String>,
    modified_files: Vec<String>,
) -> BrokerMessage {
    BrokerMessage::Artifact {
        agent_id: agent_id.to_string(),
        payload: ArtifactPayload {
            status: "done".to_string(),
            exports,
            modified_files,
        },
    }
}

/// Builds an `agent.blocked` broker message, matching the `broker.sh
/// blocked <needs> <from>` shape exactly.
pub fn build_blocked_message(agent_id: &str, needs: &str, from: &str) -> BrokerMessage {
    BrokerMessage::Blocked {
        agent_id: agent_id.to_string(),
        payload: BlockedPayload {
            needs: needs.to_string(),
            from: from.to_string(),
        },
    }
}

/// Builds an `agent.question` broker message, matching the `broker.sh
/// question <text>` shape exactly.
pub fn build_question_message(agent_id: &str, question: &str) -> BrokerMessage {
    BrokerMessage::Question {
        agent_id: agent_id.to_string(),
        payload: QuestionPayload {
            question: question.to_string(),
        },
    }
}

/// POSTs a [`BrokerMessage`] to the broker's `/publish` endpoint through
/// [`BrokerClient`].
///
/// Used to publish status updates from a process other than the broker host
/// (the broker runs in its own host process, `git paw __broker`). Uses the
/// in-process client instead of an external `curl` invocation to avoid
/// shelling out and the associated permission prompt overhead.
///
/// Errors are returned but not fatal at most call sites — the caller decides
/// whether to fail or continue.
pub fn publish_to_broker_http(broker_url: &str, msg: &BrokerMessage) -> Result<(), PawError> {
    BrokerClient::new(broker_url).publish(msg)
}

/// Registers a live filesystem watch target on the running broker via
/// `POST /watch` through [`BrokerClient`], with the spec the agent was
/// launched from when there is one.
///
/// Used by `git paw add` to give a hot-added worktree the same watcher
/// coverage a start-time agent has, so it surfaces in `/status` from worktree
/// activity even before its CLI self-publishes (capability
/// `broker-live-watch-registration`). The launcher of a fresh launch
/// registers every agent the same way once its surface exists.
///
/// Errors are returned but intended to be non-fatal at the call site: a broker
/// that is down or predates the endpoint simply leaves the agent to
/// self-register via its boot block, exactly as in v0.6.0.
pub fn register_watch_target_http(
    broker_url: &str,
    agent_id: &str,
    worktree_path: &std::path::Path,
    cli: &str,
    spec_id: Option<&str>,
) -> Result<(), PawError> {
    BrokerClient::new(broker_url).watch(agent_id, worktree_path, cli, spec_id)
}

/// A `GET /log` answer: the entries, and the epoch of the broker that
/// served them.
///
/// The epoch is what makes a cursor safe across a broker restart. Sequence
/// numbers restart at zero on every start, and a cursor never rewinds, so a
/// reader that kept its old cursor would skip the new broker's entries
/// forever. See [`CursorReader`].
#[derive(Debug, Clone)]
pub struct BrokerLogPage {
    /// Entries newer than the requested cursor, oldest first.
    pub entries: Vec<BrokerLogEntry>,
    /// The serving broker instance's epoch (start time, Unix seconds).
    pub epoch: u64,
}

/// Tracks a broker log cursor across broker restarts.
///
/// Holds the last sequence seen and the epoch it was seen under. When the
/// epoch changes, the cursor is meaningless — [`Self::observe_epoch`] resets
/// it to `0` so the next read starts from the new broker's first entry. The
/// first epoch observed is recorded without a reset, because there is no
/// earlier cursor to invalidate.
#[derive(Debug, Default)]
pub struct CursorReader {
    cursor: u64,
    epoch: Option<u64>,
}

impl CursorReader {
    /// A reader starting from the beginning of whatever broker it first
    /// reaches.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The sequence number to request entries after.
    #[must_use]
    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    /// Records the epoch just observed, resetting the cursor to `0` when it
    /// differs from the previously recorded one. Returns `true` when a reset
    /// happened.
    pub fn observe_epoch(&mut self, epoch: u64) -> bool {
        match self.epoch {
            Some(previous) if previous == epoch => false,
            Some(_) => {
                self.cursor = 0;
                self.epoch = Some(epoch);
                true
            }
            // First sight of any broker: nothing to invalidate.
            None => {
                self.epoch = Some(epoch);
                false
            }
        }
    }

    /// Advances the cursor to `seq` when it is newer than the current one.
    pub fn advance_to(&mut self, seq: u64) {
        self.cursor = self.cursor.max(seq);
    }
}

/// One entry from the broker's `GET /log` response, preserving the
/// publish-time wall-clock timestamp alongside the message.
///
/// [`fetch_log_over_http`] discards the timestamp; callers that need to
/// reconstruct relative state (e.g. intent TTL expiry, conflict
/// `detected_at`) use [`fetch_log_entries_over_http`] instead.
#[derive(Debug, Clone)]
pub struct BrokerLogEntry {
    /// Sequence number assigned at publish time.
    pub seq: u64,
    /// Wall-clock seconds since the Unix epoch when the message was published.
    pub timestamp_unix_secs: u64,
    /// The original broker message.
    pub message: BrokerMessage,
}

/// Fetches the broker's message log over HTTP via `GET /log`, preserving
/// each entry's sequence number and publish timestamp.
///
/// `since` is the caller's cursor: only entries with a greater sequence
/// number are returned, and `0` asks for the whole log. A reader that
/// consumes the log incrementally passes the highest sequence it has already
/// seen, so each poll costs the new entries rather than the full history.
///
/// `wait` long-polls: when it is `Some` and nothing is newer than `since`,
/// the broker holds the request until an entry is appended or the wait
/// elapses (it caps the wait at 60 seconds). The socket read timeout is
/// extended past the requested wait so the client does not give up on a
/// request the broker is legitimately still holding.
///
/// Returns entries in chronological order (oldest first). Errors are
/// returned; the caller decides whether to fail or fall back to empty.
pub fn fetch_log_entries_over_http(
    broker_url: &str,
    since: u64,
    wait: Option<Duration>,
) -> Result<Vec<BrokerLogEntry>, PawError> {
    Ok(fetch_log_page_over_http(broker_url, since, wait)?.entries)
}

/// [`fetch_log_entries_over_http`] that also reports the serving broker's
/// epoch, for a reader advancing a cursor across restarts.
pub fn fetch_log_page_over_http(
    broker_url: &str,
    since: u64,
    wait: Option<Duration>,
) -> Result<BrokerLogPage, PawError> {
    BrokerClient::new(broker_url).log_page(since, wait)
}

/// Fetches the broker's wave-progress object over HTTP via `GET /status`.
///
/// Returns `Ok(None)` when the broker answers without a `wave` object (a
/// broker predating this field), and `Err` when it cannot be reached or
/// answers with something unparseable. Callers treat both as "no wave to
/// report" and print nothing extra.
pub fn fetch_wave_over_http(broker_url: &str) -> Result<Option<WaveStatus>, PawError> {
    Ok(BrokerClient::new(broker_url).status()?.wave)
}

/// Fetches the broker's full message log over HTTP via `GET /log`.
///
/// Returns the parsed `BrokerMessage` entries in chronological order
/// (oldest first). Useful for any code that needs to read broker state
/// from outside the broker host process.
///
/// Errors are returned; the caller decides whether to fail or fall back
/// to an empty log.
pub fn fetch_log_over_http(broker_url: &str) -> Result<Vec<BrokerMessage>, PawError> {
    Ok(fetch_log_entries_over_http(broker_url, 0, None)?
        .into_iter()
        .map(|e| e.message)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// `mcp-agent-publish` scenario "Rejected message does not corrupt the
    /// protocol stream": a broker that responds with a non-202 status (the
    /// shape of a rejection) surfaces as an `Err`, not a panic — the shared
    /// path every MCP publish tool routes through.
    #[test]
    fn publish_to_broker_http_returns_err_on_non_202_without_panicking() {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let body = "bad request";
                let response = format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let msg = build_status_message("feat-x", "working", None, None);
        let result = publish_to_broker_http(&format!("http://{addr}"), &msg);
        assert!(
            result.is_err(),
            "a rejected (non-202) response must surface as an error, not panic"
        );
        server.join().unwrap();
    }

    /// The cursor and the wait reach the broker on the wire, in the shape
    /// `GET /log?since=<n>&wait=<secs>` — the request `wait_for_events`,
    /// `sweep.sh events` and the drive-loop observer all depend on.
    #[test]
    fn fetch_log_entries_sends_the_cursor_and_the_wait() {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let read = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..read]).to_string();
            let body = r#"{"entries":[],"last_seq":4}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            request
        });

        let entries = fetch_log_entries_over_http(
            &format!("http://{addr}"),
            4,
            Some(Duration::from_secs(30)),
        )
        .expect("the reader must parse the broker's answer");
        assert!(entries.is_empty());

        let request = server.join().unwrap();
        let request_line = request.lines().next().unwrap_or_default();
        assert!(
            request_line.contains("since=4") && request_line.contains("wait=30"),
            "cursor and wait must both reach the broker; got {request_line:?}"
        );
    }

    /// Without a wait the reader asks for an immediate answer — no `wait`
    /// parameter at all, so a pre-long-poll broker behaves exactly as before.
    #[test]
    fn fetch_log_entries_without_a_wait_sends_no_wait_parameter() {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 2048];
            let read = stream.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..read]).to_string();
            let body = r#"{"entries":[],"last_seq":0}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
            request
        });

        let _ = fetch_log_over_http(&format!("http://{addr}"));

        let request = server.join().unwrap();
        let request_line = request.lines().next().unwrap_or_default();
        assert!(
            request_line.contains("since=0") && !request_line.contains("wait="),
            "a whole-log read must not long-poll; got {request_line:?}"
        );
    }

    #[test]
    fn build_status_message_with_explicit_cli_populates_cli_field() {
        let msg = build_status_message(
            "supervisor",
            "working",
            Some("Supervisor booting".to_string()),
            Some("claude"),
        );
        let BrokerMessage::Status { agent_id, payload } = msg else {
            panic!("expected BrokerMessage::Status");
        };
        assert_eq!(agent_id, "supervisor");
        assert_eq!(payload.status, "working");
        assert_eq!(payload.message.as_deref(), Some("Supervisor booting"));
        assert_eq!(payload.cli.as_deref(), Some("claude"));
        assert_eq!(payload.phase, None);
    }

    /// Task 3.3 E2E: a hot-added worktree registered through the real
    /// `register_watch_target_http` helper against a live broker surfaces the
    /// agent in `/status` from worktree activity — not via CLI
    /// self-registration. Closes git-paw-add's deferred 6.1/6.2.
    #[test]
    fn register_watch_target_http_surfaces_hot_added_agent() {
        use std::process::Command;

        use crate::broker::{BrokerState, ProbeResult, probe_broker, start_broker};
        use crate::config::BrokerConfig;

        let tmp = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .expect("git command failed");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "test"]);
        run(&["commit", "--allow-empty", "-m", "root", "-q"]);

        // Use a port range outside the other broker tests' `19_xxx` space so a
        // parallel sibling can never bind it first and leave us reattaching to
        // a disconnected state.
        let config = BrokerConfig {
            enabled: true,
            #[allow(clippy::cast_possible_truncation)]
            port: 20_300 + (std::process::id() as u16 % 100),
            bind: "127.0.0.1".to_string(),
            ..Default::default()
        };
        // A leftover broker on this port (e.g. a fast rerun) makes the test
        // inconclusive — skip rather than reattach to a disconnected state.
        if probe_broker(&config.url()) != ProbeResult::NoListener {
            return;
        }
        let state = BrokerState::new(None);
        // If the port is busy the test is inconclusive — not a failure.
        let Ok(handle) = start_broker(&config, state, Vec::new()) else {
            return;
        };

        // Register the hot-added worktree over HTTP via the new endpoint.
        register_watch_target_http(&config.url(), "feat-hot", tmp.path(), "claude", None)
            .expect("broker must accept the live watch registration");

        // Dirty the worktree so the watcher has activity to surface.
        std::fs::write(tmp.path().join("hot.rs"), "fn hot() {}").unwrap();

        let mut found = false;
        for _ in 0..40 {
            std::thread::sleep(Duration::from_millis(250));
            if handle.state.read().agents.contains_key("feat-hot") {
                found = true;
                break;
            }
        }
        assert!(
            found,
            "the hot-added worktree must surface feat-hot in /status via the watcher"
        );
        drop(handle);
    }

    #[test]
    fn build_status_message_with_none_cli_omits_cli_key_from_json() {
        let msg = build_status_message("feat-x", "working", None, None);
        let BrokerMessage::Status { ref payload, .. } = msg else {
            panic!("expected BrokerMessage::Status");
        };
        assert_eq!(payload.cli, None);
        assert_eq!(payload.phase, None);

        let json = serde_json::to_string(&msg).unwrap();
        assert!(
            !json.contains("\"cli\""),
            "cli key must be omitted from JSON when None; got {json}"
        );
        assert!(
            !json.contains("\"phase\""),
            "phase key must be omitted from JSON when None; got {json}"
        );
    }

    #[test]
    fn build_artifact_message_matches_broker_sh_shape() {
        let msg = build_artifact_message(
            "feat-x",
            vec!["foo".to_string()],
            vec!["src/lib.rs".to_string()],
        );
        let BrokerMessage::Artifact { agent_id, payload } = msg else {
            panic!("expected BrokerMessage::Artifact");
        };
        assert_eq!(agent_id, "feat-x");
        assert_eq!(payload.status, "done");
        assert_eq!(payload.exports, vec!["foo".to_string()]);
        assert_eq!(payload.modified_files, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn build_blocked_message_matches_broker_sh_shape() {
        let msg = build_blocked_message("feat-x", "peer review", "feat-y");
        let BrokerMessage::Blocked { agent_id, payload } = msg else {
            panic!("expected BrokerMessage::Blocked");
        };
        assert_eq!(agent_id, "feat-x");
        assert_eq!(payload.needs, "peer review");
        assert_eq!(payload.from, "feat-y");
    }

    #[test]
    fn build_question_message_matches_broker_sh_shape() {
        let msg = build_question_message("feat-x", "should I proceed?");
        let BrokerMessage::Question { agent_id, payload } = msg else {
            panic!("expected BrokerMessage::Question");
        };
        assert_eq!(agent_id, "feat-x");
        assert_eq!(payload.question, "should I proceed?");
    }
}
