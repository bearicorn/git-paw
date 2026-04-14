//! Helpers for constructing and publishing `BrokerMessage` values to a
//! running broker over HTTP.
//!
//! The supervisor command (in `main.rs`) and the merge loop both need to
//! publish status messages to the broker running in the dashboard pane.
//! These helpers live in the library so both call sites share the same
//! wire format and error handling.

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
pub fn build_status_message(
    agent_id: &str,
    status: &str,
    message: Option<String>,
) -> BrokerMessage {
    BrokerMessage::Status {
        agent_id: agent_id.to_string(),
        payload: StatusPayload {
            status: status.to_string(),
            modified_files: Vec::new(),
            message,
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

/// Fetches the broker's full message log over HTTP via `GET /log`.
///
/// Returns the parsed `BrokerMessage` entries in chronological order
/// (oldest first). Used by `cmd_supervisor` to reconstruct broker state
/// from outside the dashboard process so it can build the dependency graph
/// for merge ordering and write a real session summary instead of an
/// empty one.
///
/// Errors are returned but the caller decides whether to fail or fall back
/// to an empty log (in which case the merge order is alphabetical and the
/// session summary contains only what came back from `MergeResults`).
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
