//! Helpers for constructing and publishing `BrokerMessage` values to a
//! running broker over HTTP.
//!
//! These helpers live in the library so any caller outside the broker
//! process (e.g. `cmd_supervisor` self-registration) can publish messages
//! using the same wire format and error handling.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use crate::broker::messages::{BrokerMessage, StatusPayload};
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
/// `phase` is intentionally not exposed through this helper. Callers that
/// want to publish a phase label SHALL construct the
/// [`BrokerMessage::Status`] directly with a fully-populated
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
        },
    }
}

/// POSTs a [`BrokerMessage`] to the broker's `/publish` endpoint over a raw
/// TCP HTTP/1.1 request.
///
/// Used to publish status updates from a process other than the broker host
/// (the broker runs in the dashboard pane). Uses a manual TCP write instead
/// of an external `curl` invocation to avoid shelling out and the associated
/// permission prompt overhead.
///
/// Errors are returned but not fatal at most call sites — the caller decides
/// whether to fail or continue.
pub fn publish_to_broker_http(broker_url: &str, msg: &BrokerMessage) -> Result<(), PawError> {
    let body = serde_json::to_string(msg)
        .map_err(|e| PawError::SessionError(format!("failed to serialize broker message: {e}")))?;

    let addr = broker_url.strip_prefix("http://").unwrap_or(broker_url);
    let socket_addr = if let Ok(a) = addr.parse() {
        a
    } else {
        use std::net::ToSocketAddrs;
        addr.to_socket_addrs()
            .map_err(|e| PawError::SessionError(format!("invalid broker address {addr}: {e}")))?
            .next()
            .ok_or_else(|| {
                PawError::SessionError(format!("broker address {addr} resolved to no addrs"))
            })?
    };

    let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500))
        .map_err(|e| PawError::SessionError(format!("failed to connect to broker: {e}")))?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();

    let request = format!(
        "POST /publish HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| PawError::SessionError(format!("failed to write broker request: {e}")))?;

    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    if !(response.starts_with("HTTP/1.1 202") || response.starts_with("HTTP/1.0 202")) {
        return Err(PawError::SessionError(format!(
            "broker rejected publish: {}",
            response.lines().next().unwrap_or("<empty>")
        )));
    }
    Ok(())
}

/// Registers a live filesystem watch target on the running broker via
/// `POST /watch` over a raw TCP HTTP/1.1 request.
///
/// Used by `git paw add` to give a hot-added worktree the same watcher
/// coverage a start-time agent has, so it surfaces in `/status` from worktree
/// activity even before its CLI self-publishes (capability
/// `broker-live-watch-registration`). Mirrors [`publish_to_broker_http`]'s
/// manual TCP write to avoid shelling out to `curl`.
///
/// Errors are returned but intended to be non-fatal at the call site: a broker
/// that is down or predates the endpoint simply leaves the agent to
/// self-register via its boot block, exactly as in v0.6.0.
pub fn register_watch_target_http(
    broker_url: &str,
    agent_id: &str,
    worktree_path: &std::path::Path,
    cli: &str,
) -> Result<(), PawError> {
    let body = serde_json::json!({
        "agent_id": agent_id,
        "worktree_path": worktree_path.to_string_lossy(),
        "cli": cli,
    })
    .to_string();

    let addr = broker_url.strip_prefix("http://").unwrap_or(broker_url);
    let socket_addr = if let Ok(a) = addr.parse() {
        a
    } else {
        use std::net::ToSocketAddrs;
        addr.to_socket_addrs()
            .map_err(|e| PawError::SessionError(format!("invalid broker address {addr}: {e}")))?
            .next()
            .ok_or_else(|| {
                PawError::SessionError(format!("broker address {addr} resolved to no addrs"))
            })?
    };

    let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500))
        .map_err(|e| PawError::SessionError(format!("failed to connect to broker: {e}")))?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();

    let request = format!(
        "POST /watch HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| PawError::SessionError(format!("failed to write broker request: {e}")))?;

    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    if !(response.starts_with("HTTP/1.1 202") || response.starts_with("HTTP/1.0 202")) {
        return Err(PawError::SessionError(format!(
            "broker rejected watch registration: {}",
            response.lines().next().unwrap_or("<empty>")
        )));
    }
    Ok(())
}

/// Fetches the broker's full message log over HTTP via `GET /log`.
///
/// Returns the parsed `BrokerMessage` entries in chronological order
/// (oldest first). Useful for any code that needs to read broker state
/// from outside the dashboard process.
///
/// Errors are returned; the caller decides whether to fail or fall back
/// to an empty log.
pub fn fetch_log_over_http(broker_url: &str) -> Result<Vec<BrokerMessage>, PawError> {
    let addr = broker_url.strip_prefix("http://").unwrap_or(broker_url);
    let socket_addr = if let Ok(a) = addr.parse() {
        a
    } else {
        use std::net::ToSocketAddrs;
        addr.to_socket_addrs()
            .map_err(|e| PawError::SessionError(format!("invalid broker address {addr}: {e}")))?
            .next()
            .ok_or_else(|| {
                PawError::SessionError(format!("broker address {addr} resolved to no addrs"))
            })?
    };

    let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500))
        .map_err(|e| PawError::SessionError(format!("failed to connect to broker: {e}")))?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();

    let request = format!(
        "GET /log HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| PawError::SessionError(format!("failed to write broker request: {e}")))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| PawError::SessionError(format!("failed to read broker response: {e}")))?;

    if !(response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200")) {
        return Err(PawError::SessionError(format!(
            "broker /log returned non-200: {}",
            response.lines().next().unwrap_or("<empty>")
        )));
    }

    // Split off headers — the JSON body follows the first blank line.
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b)
        .ok_or_else(|| {
            PawError::SessionError("broker /log response missing body separator".to_string())
        })?;

    let parsed: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| PawError::SessionError(format!("broker /log returned invalid JSON: {e}")))?;

    let entries = parsed
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            PawError::SessionError("broker /log response missing entries array".to_string())
        })?;

    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        if let Some(msg_value) = entry.get("message")
            && let Ok(msg) = serde_json::from_value::<BrokerMessage>(msg_value.clone())
        {
            out.push(msg);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        register_watch_target_http(&config.url(), "feat-hot", tmp.path(), "claude")
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
}
