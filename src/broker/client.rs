//! The broker's HTTP client: every request a process other than the broker
//! host makes to the broker.
//!
//! The broker runs in its own host process (`git paw __broker`), so the
//! dashboard, the launcher, `git paw add`, `git paw status`, the drive loop and
//! the MCP server all reach it over loopback HTTP. [`BrokerClient`] is the one
//! place those requests are made: a minimal HTTP/1.1 exchange over a
//! `TcpStream` (no HTTP client dependency), with bounded connect, write and
//! read timeouts so a slow or wedged broker can never freeze a caller.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use super::messages::BrokerMessage;
use super::publish::{BrokerLogEntry, BrokerLogPage};
use super::{AgentStatusEntry, ProbeResult, WaveStatus};
use crate::error::PawError;

/// How long a connection attempt may take before the broker counts as
/// unreachable.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(500);

/// How long a request may wait for the broker's answer (beyond any long-poll
/// wait the caller asked for).
const IO_TIMEOUT: Duration = Duration::from_secs(2);

/// A client bound to one broker URL (`http://<host>:<port>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerClient {
    url: String,
}

/// One agent row of a `GET /status` answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct StatusAgentRow {
    /// Agent identifier (slugified branch name).
    pub agent_id: String,
    /// CLI label; empty when the broker does not know it.
    #[serde(default)]
    pub cli: String,
    /// Current status label.
    #[serde(default)]
    pub status: String,
    /// Seconds since the agent was last seen.
    #[serde(default)]
    pub last_seen_seconds: u64,
    /// Most recently published phase, if any.
    #[serde(default)]
    pub phase: Option<String>,
    /// The agent's most recently reported modified files.
    #[serde(default)]
    pub modified_files: Vec<String>,
}

impl StatusAgentRow {
    /// The row as the dashboard's in-memory snapshot entry, aged from `now`.
    #[must_use]
    pub fn into_entry(self, now: std::time::Instant) -> AgentStatusEntry {
        let last_seen = now
            .checked_sub(Duration::from_secs(self.last_seen_seconds))
            .unwrap_or(now);
        AgentStatusEntry {
            agent_id: self.agent_id,
            cli: self.cli,
            status: self.status,
            last_seen_seconds: self.last_seen_seconds,
            last_seen,
            phase: self.phase,
            modified_files: self.modified_files,
        }
    }
}

/// A parsed `GET /status` answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BrokerStatus {
    /// The git-paw broker marker.
    #[serde(default)]
    pub git_paw: bool,
    /// Seconds since this broker host started. A value lower than one seen
    /// earlier means the host restarted.
    #[serde(default)]
    pub uptime_seconds: u64,
    /// The repository the broker serves; empty when it declares none.
    #[serde(default)]
    pub repo_root: String,
    /// Every agent the broker knows.
    #[serde(default)]
    pub agents: Vec<StatusAgentRow>,
    /// Wave progress; absent from a broker predating the field.
    #[serde(default)]
    pub wave: Option<WaveStatus>,
}

impl BrokerClient {
    /// A client for the broker at `url`.
    #[must_use]
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }

    /// The broker URL this client talks to.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// `GET /status`, parsed.
    ///
    /// # Errors
    /// Returns [`PawError::SessionError`] when the broker cannot be reached,
    /// answers non-200, or answers something that is not `/status` JSON.
    pub fn status(&self) -> Result<BrokerStatus, PawError> {
        let body = self.status_body()?;
        serde_json::from_str(&body)
            .map_err(|e| PawError::SessionError(format!("broker /status parse error: {e}")))
    }

    /// `GET /status`, as the raw JSON body.
    ///
    /// # Errors
    /// Returns [`PawError::SessionError`] when the broker cannot be reached or
    /// answers non-200.
    pub fn status_body(&self) -> Result<String, PawError> {
        let (code, body) = self.request("GET", "/status", None, Duration::ZERO)?;
        if code != 200 {
            return Err(PawError::SessionError(format!(
                "broker /status returned non-200: {code}"
            )));
        }
        Ok(body)
    }

    /// `GET /log?since=<since>[&wait=<secs>]`: entries newer than `since`,
    /// oldest first, with the serving broker's epoch.
    ///
    /// `wait` long-polls: the broker holds the request until an entry arrives
    /// or the wait elapses, and the read timeout is extended past it.
    ///
    /// # Errors
    /// Returns [`PawError::SessionError`] when the broker cannot be reached,
    /// answers non-200, or answers something that is not `/log` JSON.
    pub fn log_page(&self, since: u64, wait: Option<Duration>) -> Result<BrokerLogPage, PawError> {
        let query = match wait {
            Some(w) => format!("/log?since={since}&wait={}", w.as_secs()),
            None => format!("/log?since={since}"),
        };
        let (code, body) = self.request("GET", &query, None, wait.unwrap_or(Duration::ZERO))?;
        if code != 200 {
            return Err(PawError::SessionError(format!(
                "broker /log returned non-200: {code}"
            )));
        }
        parse_log_page(&body)
    }

    /// `POST /publish` of one message.
    ///
    /// # Errors
    /// Returns [`PawError::SessionError`] when the broker cannot be reached or
    /// does not accept the message.
    pub fn publish(&self, msg: &BrokerMessage) -> Result<(), PawError> {
        let body = serde_json::to_string(msg).map_err(|e| {
            PawError::SessionError(format!("failed to serialize broker message: {e}"))
        })?;
        let (code, _) = self.request("POST", "/publish", Some(&body), Duration::ZERO)?;
        if code != 202 {
            return Err(PawError::SessionError(format!(
                "broker rejected publish: {code}"
            )));
        }
        Ok(())
    }

    /// `POST /watch`: registers an agent's worktree with the broker, with the
    /// spec it was launched from when there is one.
    ///
    /// # Errors
    /// Returns [`PawError::SessionError`] when the broker cannot be reached or
    /// rejects the registration.
    pub fn watch(
        &self,
        agent_id: &str,
        worktree_path: &Path,
        cli: &str,
        spec_id: Option<&str>,
    ) -> Result<(), PawError> {
        let mut body = serde_json::json!({
            "agent_id": agent_id,
            "worktree_path": worktree_path.to_string_lossy(),
            "cli": cli,
        });
        if let (Some(spec_id), Some(map)) = (spec_id, body.as_object_mut()) {
            map.insert("spec_id".to_string(), serde_json::json!(spec_id));
        }
        let (code, _) = self.request("POST", "/watch", Some(&body.to_string()), Duration::ZERO)?;
        if code != 202 {
            return Err(PawError::SessionError(format!(
                "broker rejected watch registration: {code}"
            )));
        }
        Ok(())
    }

    /// Classifies whatever listens on this client's URL: this repository's
    /// broker, another repository's, a foreign server, nothing, or a probe
    /// that timed out. With `repo_root` `None`, any git-paw broker counts as
    /// live.
    #[must_use]
    pub fn probe(&self, repo_root: Option<&Path>) -> ProbeResult {
        match repo_root {
            Some(root) => super::probe_broker_for_repo(&self.url, root),
            None => super::probe_broker(&self.url),
        }
    }

    /// Resolves the URL's `host:port` to a socket address.
    fn socket_addr(&self) -> Result<(SocketAddr, &str), PawError> {
        let addr = self.url.strip_prefix("http://").unwrap_or(&self.url);
        if let Ok(a) = addr.parse() {
            return Ok((a, addr));
        }
        let resolved = addr
            .to_socket_addrs()
            .map_err(|e| PawError::SessionError(format!("invalid broker address {addr}: {e}")))?
            .next()
            .ok_or_else(|| {
                PawError::SessionError(format!("broker address {addr} resolved to no addrs"))
            })?;
        Ok((resolved, addr))
    }

    /// One HTTP/1.1 exchange: returns the status code and the body.
    ///
    /// `extra_wait` extends the read timeout for a long-poll the broker is
    /// entitled to hold.
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
        extra_wait: Duration,
    ) -> Result<(u16, String), PawError> {
        let (socket_addr, addr) = self.socket_addr()?;
        let mut stream = TcpStream::connect_timeout(&socket_addr, CONNECT_TIMEOUT)
            .map_err(|e| PawError::SessionError(format!("failed to connect to broker: {e}")))?;
        stream.set_write_timeout(Some(IO_TIMEOUT)).ok();

        let request = match body {
            Some(body) => format!(
                "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
            None => format!(
                "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
            ),
        };
        stream
            .write_all(request.as_bytes())
            .map_err(|e| PawError::SessionError(format!("failed to write broker request: {e}")))?;

        let response = read_bounded(&mut stream, IO_TIMEOUT + extra_wait)?;
        parse_response(&response)
    }
}

/// Largest broker answer a client reads, in bytes. Anything on loopback can
/// hold the broker port, so a response is untrusted input: the read stops
/// here rather than growing without bound. A full `/log` of a long session
/// is a few megabytes.
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Reads a whole response within `budget` in total and `MAX_RESPONSE_BYTES`
/// in size.
///
/// The socket read timeout bounds each `read`, not the exchange: a peer that
/// trickles a byte at a time would otherwise hold the caller indefinitely.
/// A read error after some bytes (the peer closing abruptly) ends the answer;
/// a read error with nothing read is a failure.
fn read_bounded(stream: &mut TcpStream, budget: Duration) -> Result<String, PawError> {
    let deadline = std::time::Instant::now() + budget;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            return Err(PawError::SessionError(
                "broker response did not complete in time".to_string(),
            ));
        }
        stream.set_read_timeout(Some(remaining)).ok();
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.len() > MAX_RESPONSE_BYTES {
                    return Err(PawError::SessionError(
                        "broker response exceeded the size limit".to_string(),
                    ));
                }
            }
            Err(e) if bytes.is_empty() => {
                return Err(PawError::SessionError(format!(
                    "failed to read broker response: {e}"
                )));
            }
            Err(_) => break,
        }
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Splits a raw HTTP/1.x response into its status code and body.
fn parse_response(response: &str) -> Result<(u16, String), PawError> {
    let code = response
        .strip_prefix("HTTP/1.1 ")
        .or_else(|| response.strip_prefix("HTTP/1.0 "))
        .and_then(|rest| rest.get(..3))
        .and_then(|c| c.parse::<u16>().ok())
        .ok_or_else(|| {
            PawError::SessionError(format!(
                "malformed broker response: {}",
                response.lines().next().unwrap_or("<empty>")
            ))
        })?;
    let body = response
        .split_once("\r\n\r\n")
        .map_or_else(String::new, |(_, b)| b.to_string());
    Ok((code, body))
}

/// Parses a `GET /log` JSON body.
fn parse_log_page(body: &str) -> Result<BrokerLogPage, PawError> {
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
            out.push(BrokerLogEntry {
                seq: entry
                    .get("seq")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                timestamp_unix_secs: entry
                    .get("timestamp_unix_secs")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                message: msg,
            });
        }
    }
    Ok(BrokerLogPage {
        entries: out,
        // A broker predating the field reports no epoch. `0` is then constant
        // across its responses, so a reader never sees a change and keeps its
        // cursor — the pre-epoch behaviour.
        epoch: parsed
            .get("epoch")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::{BrokerState, start_broker};
    use crate::config::BrokerConfig;
    use std::net::TcpListener;

    /// Starts a real broker on a free loopback port for `repo`.
    fn live_broker(repo: &Path) -> (crate::broker::BrokerHandle, BrokerClient) {
        for _ in 0..10 {
            let port = TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            let config = BrokerConfig {
                enabled: true,
                port,
                ..Default::default()
            };
            if let Ok(handle) = start_broker(
                &config,
                BrokerState::new(None).with_repo_root(repo),
                Vec::new(),
            ) {
                let client = BrokerClient::new(format!("http://127.0.0.1:{port}"));
                return (handle, client);
            }
        }
        panic!("could not start a test broker");
    }

    #[test]
    fn status_reports_the_marker_repository_and_uptime() {
        let repo = tempfile::tempdir().unwrap();
        let (_handle, client) = live_broker(repo.path());
        let status = client.status().unwrap();
        assert!(status.git_paw);
        assert_eq!(
            Path::new(&status.repo_root),
            repo.path().canonicalize().unwrap()
        );
        assert!(status.wave.is_some());
    }

    #[test]
    fn a_published_message_reaches_the_log_and_the_cursor_filters_it() {
        let repo = tempfile::tempdir().unwrap();
        let (_handle, client) = live_broker(repo.path());
        let msg = crate::broker::publish::build_status_message("feat-x", "working", None, None);
        client.publish(&msg).unwrap();

        let page = client.log_page(0, None).unwrap();
        assert_eq!(page.entries.len(), 1);
        let seq = page.entries[0].seq;
        assert!(
            client.log_page(seq, None).unwrap().entries.is_empty(),
            "entries at or below the cursor are not returned again"
        );
    }

    #[test]
    fn a_watch_registration_is_accepted_with_or_without_a_spec_id() {
        let repo = tempfile::tempdir().unwrap();
        let (_handle, client) = live_broker(repo.path());
        let wt_x = tempfile::tempdir().unwrap();
        let wt_y = tempfile::tempdir().unwrap();
        client
            .watch("feat-x", wt_x.path(), "claude", Some("003-user-list"))
            .unwrap();
        client.watch("feat-y", wt_y.path(), "claude", None).unwrap();
    }

    #[test]
    fn an_unreachable_broker_is_an_error_not_a_hang() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let client = BrokerClient::new(format!("http://127.0.0.1:{port}"));
        let started = std::time::Instant::now();
        assert!(client.status().is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_broker_that_stops_answering_times_out_within_the_bound() {
        // A listener that accepts and never answers: the request must give up
        // within the bounded read timeout rather than freeze the caller.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            std::thread::sleep(IO_TIMEOUT + Duration::from_secs(1));
            drop(stream);
        });
        let client = BrokerClient::new(format!("http://{addr}"));
        let started = std::time::Instant::now();
        assert!(client.status().is_err());
        assert!(started.elapsed() < IO_TIMEOUT + Duration::from_millis(900));
        server.join().unwrap();
    }

    #[test]
    fn a_broker_that_trickles_its_answer_is_cut_off_at_the_overall_deadline() {
        // A peer that keeps sending one byte at a time never trips the
        // per-read timeout; the exchange as a whole must still end.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_server = std::sync::Arc::clone(&stop);
        let server = std::thread::spawn(move || {
            use std::io::Write as _;
            let (mut stream, _) = listener.accept().unwrap();
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n");
            while !stop_server.load(std::sync::atomic::Ordering::Relaxed) {
                if stream.write_all(b" ").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        });
        let client = BrokerClient::new(format!("http://{addr}"));
        let started = std::time::Instant::now();
        assert!(client.status().is_err());
        assert!(started.elapsed() < IO_TIMEOUT + Duration::from_secs(1));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        server.join().unwrap();
    }

    #[test]
    fn probe_classifies_this_repositorys_broker_as_live() {
        let repo = tempfile::tempdir().unwrap();
        let (_handle, client) = live_broker(repo.path());
        assert_eq!(client.probe(Some(repo.path())), ProbeResult::LiveBroker);
        let other = tempfile::tempdir().unwrap();
        assert!(matches!(
            client.probe(Some(other.path())),
            ProbeResult::OtherRepoBroker { .. }
        ));
    }

    #[test]
    fn a_status_row_ages_from_its_last_seen_seconds() {
        let now = std::time::Instant::now();
        let row = StatusAgentRow {
            agent_id: "feat-x".to_string(),
            cli: "claude".to_string(),
            status: "working".to_string(),
            last_seen_seconds: 30,
            phase: None,
            modified_files: Vec::new(),
        };
        let entry = row.into_entry(now);
        assert_eq!(
            now.saturating_duration_since(entry.last_seen),
            Duration::from_secs(30)
        );
    }
}
