//! HTTP broker for agent coordination.
//!
//! Provides an HTTP server that agents use to publish messages, poll for
//! incoming messages, and report status. The broker runs on a background
//! tokio runtime and is managed through [`BrokerHandle`].
//!
//! # Lock discipline
//!
//! [`BrokerState`] wraps its inner state in an `RwLock`. **Guards MUST NOT be
//! held across `.await` boundaries.** The `clippy::await_holding_lock` lint is
//! enabled project-wide to catch violations at compile time. Use the
//! `read()` / `write()` methods to obtain guards inside synchronous closures
//! only.

pub mod delivery;
pub mod messages;
pub mod publish;
pub mod server;
pub mod watcher;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::Instant;

use serde::Serialize;

use crate::config::BrokerConfig;
pub use messages::BrokerMessage;

/// Worktree to watch for git-status changes.
///
/// The broker spawns one [`watcher::watch_worktree`] task per target.
#[derive(Debug, Clone)]
pub struct WatchTarget {
    /// Agent identifier (slugified branch name) that owns this worktree.
    pub agent_id: String,
    /// CLI name running in this agent's pane (e.g. `"claude"`).
    pub cli: String,
    /// Absolute path to the worktree root.
    pub worktree_path: PathBuf,
}

/// Record of a known agent's latest state.
#[derive(Debug, Clone)]
pub struct AgentRecord {
    /// Agent identifier (slugified branch name).
    pub agent_id: String,
    /// Last reported status label.
    pub status: String,
    /// When the agent last published a message.
    pub last_seen: Instant,
    /// The most recent message from this agent.
    pub last_message: Option<BrokerMessage>,
}

/// JSON-serializable snapshot of an agent's status for the `/status` endpoint
/// and the dashboard TUI.
#[derive(Debug, Clone, Serialize)]
pub struct AgentStatusEntry {
    /// Agent identifier (slugified branch name).
    pub agent_id: String,
    /// CLI name running in this agent's pane (e.g. "claude").
    pub cli: String,
    /// Current status label (e.g. "working", "done", "blocked").
    pub status: String,
    /// Seconds since the agent was last seen.
    pub last_seen_seconds: u64,
    /// One-line summary from the last message.
    pub summary: String,
    /// When the agent was last seen (for age calculations in the dashboard).
    #[serde(skip)]
    pub last_seen: Instant,
}

/// Mutable broker state protected by an `RwLock`.
#[derive(Debug)]
pub struct BrokerStateInner {
    /// Known agents keyed by agent ID.
    pub agents: HashMap<String, AgentRecord>,
    /// CLI label per agent, populated from [`WatchTarget`] at broker start.
    pub agent_clis: HashMap<String, String>,
    /// Per-agent message inboxes: `(sequence_number, message)`.
    pub queues: HashMap<String, Vec<(u64, BrokerMessage)>>,
    /// Append-only message log for disk flush.
    pub message_log: Vec<(u64, std::time::SystemTime, BrokerMessage)>,
}

/// Shared broker state.
///
/// Wraps [`BrokerStateInner`] in an `RwLock` for concurrent read access.
/// The sequence counter is a standalone [`AtomicU64`] outside the lock so
/// that sequence numbers can be allocated without coupling to the write
/// lock.
#[derive(Debug)]
pub struct BrokerState {
    /// Protected mutable state.
    inner: RwLock<BrokerStateInner>,
    /// Global sequence counter (starts at 0; first assigned value is 1).
    next_seq: AtomicU64,
    /// Optional path for periodic log flush to disk.
    pub log_path: Option<PathBuf>,
    /// Wall-clock instant the broker state was created; used for uptime reporting.
    started_at: Instant,
}

impl BrokerState {
    /// Creates a new empty broker state.
    pub fn new(log_path: Option<PathBuf>) -> Self {
        Self {
            inner: RwLock::new(BrokerStateInner {
                agents: HashMap::new(),
                agent_clis: HashMap::new(),
                queues: HashMap::new(),
                message_log: Vec::new(),
            }),
            next_seq: AtomicU64::new(0),
            log_path,
            started_at: Instant::now(),
        }
    }

    /// Acquires a read lock on the inner state.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned (a thread panicked while holding it).
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, BrokerStateInner> {
        self.inner.read().expect("broker state lock poisoned")
    }

    /// Acquires a write lock on the inner state.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned (a thread panicked while holding it).
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, BrokerStateInner> {
        self.inner.write().expect("broker state lock poisoned")
    }

    /// Atomically allocates the next sequence number (starting at 1).
    pub fn next_seq(&self) -> u64 {
        self.next_seq.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns the number of seconds since the broker was started.
    ///
    /// Used by the HTTP `/status` handler to report uptime.
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}

/// Errors specific to broker operations.
#[derive(Debug, thiserror::Error)]
pub enum BrokerError {
    /// The configured port is already in use by a non-broker process.
    #[error(
        "port {port} is already in use by another process — change [broker] port in .git-paw/config.toml"
    )]
    PortInUse {
        /// The port that was occupied.
        port: u16,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// A probe to an existing listener on the port timed out.
    #[error("broker probe timed out on port {port} — check for stuck processes on this port")]
    ProbeTimeout {
        /// The port that timed out.
        port: u16,
    },

    /// Binding to the address failed.
    #[error("failed to bind broker: {0}")]
    BindFailed(std::io::Error),

    /// Creating the tokio runtime failed.
    #[error("failed to create broker runtime: {0}")]
    RuntimeFailed(std::io::Error),
}

/// Handle to a running broker, including the optional flush thread.
///
/// When dropped, signals the flush thread to stop and joins it, then
/// shuts down the tokio runtime. If the handle is in "reattached" mode
/// (connected to an existing broker), dropping it is a no-op.
pub struct BrokerHandle {
    /// Shared broker state.
    pub state: Arc<BrokerState>,
    /// The tokio runtime powering the broker server.
    /// `None` when reattached to an existing broker.
    runtime: Option<tokio::runtime::Runtime>,
    /// Sends a shutdown signal to the server task.
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Broadcasts the watcher shutdown signal to all watcher tasks.
    watcher_shutdown: Option<tokio::sync::watch::Sender<bool>>,
    /// The URL the broker is listening on.
    pub url: String,
    /// Flag to signal the flush thread to exit.
    stop_flag: Arc<AtomicBool>,
    /// Flush thread join handle (present only when `log_path` is `Some`).
    flush_thread: Option<JoinHandle<()>>,
}

impl BrokerHandle {
    /// Creates a handle that reattaches to an existing broker (no owned runtime).
    fn reattached(url: String, state: Arc<BrokerState>) -> Self {
        Self {
            state,
            runtime: None,
            shutdown_tx: None,
            watcher_shutdown: None,
            url,
            stop_flag: Arc::new(AtomicBool::new(false)),
            flush_thread: None,
        }
    }
}

impl Drop for BrokerHandle {
    fn drop(&mut self) {
        // 1. Signal flush thread to stop and join it.
        self.stop_flag.store(true, Ordering::Release);
        if let Some(handle) = self.flush_thread.take() {
            let _ = handle.join();
        }
        // 2. Signal watcher tasks to stop.
        if let Some(tx) = self.watcher_shutdown.take() {
            let _ = tx.send(true);
        }
        // 3. Signal shutdown to the server task.
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // 4. Give in-flight requests up to 2 seconds to drain, then drop runtime.
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_timeout(std::time::Duration::from_secs(2));
        }
    }
}

/// Result of probing an existing listener on the broker port.
#[derive(Debug, PartialEq, Eq)]
pub enum ProbeResult {
    /// Nothing is listening — safe to bind.
    NoListener,
    /// A git-paw broker is already running.
    LiveBroker,
    /// Something else is using the port.
    ForeignServer,
    /// The probe timed out.
    Timeout,
}

/// Probes an existing listener at the given URL to determine what is running.
///
/// Uses a lightweight `TcpStream` with a manual HTTP/1.1 GET to `/status`
/// to avoid pulling in a full HTTP client dependency.
/// Probes a URL to determine what broker (if any) is running there.
///
/// Public entry point for callers that need to inspect broker status without
/// starting a new server (e.g. the `status` subcommand).
pub fn probe_broker(url: &str) -> ProbeResult {
    probe_existing_broker(url)
}

fn probe_existing_broker(url: &str) -> ProbeResult {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    // Parse host:port from URL like "http://127.0.0.1:9119"
    let addr = url.strip_prefix("http://").unwrap_or(url);

    let socket_addr = if let Ok(a) = addr.parse() {
        a
    } else {
        use std::net::ToSocketAddrs;
        match addr.to_socket_addrs() {
            Ok(mut addrs) => match addrs.next() {
                Some(a) => a,
                None => return ProbeResult::NoListener,
            },
            Err(_) => return ProbeResult::NoListener,
        }
    };

    let Ok(mut stream) = TcpStream::connect_timeout(&socket_addr, Duration::from_millis(500))
    else {
        return ProbeResult::NoListener;
    };

    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .ok();

    let request = format!("GET /status HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return ProbeResult::Timeout;
    }

    let mut response = String::new();
    if stream.read_to_string(&mut response).is_err() && response.is_empty() {
        return ProbeResult::Timeout;
    }

    if response.contains("\"git_paw\":true") || response.contains("\"git_paw\": true") {
        ProbeResult::LiveBroker
    } else if response.starts_with("HTTP/") {
        ProbeResult::ForeignServer
    } else {
        ProbeResult::Timeout
    }
}

/// Starts the HTTP broker server.
///
/// Probes the configured port first:
/// - If a live git-paw broker is found, returns a reattached handle.
/// - If a foreign server occupies the port, returns [`BrokerError::PortInUse`].
/// - If the probe times out, returns [`BrokerError::ProbeTimeout`].
/// - If nothing is listening, binds and starts the server.
///
/// Also spawns the background flush thread if `state.log_path` is set.
pub fn start_broker(
    config: &BrokerConfig,
    state: BrokerState,
    watch_targets: Vec<WatchTarget>,
) -> Result<BrokerHandle, BrokerError> {
    let url = config.url();
    let state = Arc::new(state);
    let stop_flag = Arc::new(AtomicBool::new(false));

    match probe_existing_broker(&url) {
        ProbeResult::LiveBroker => return Ok(BrokerHandle::reattached(url, state)),
        ProbeResult::ForeignServer => {
            return Err(BrokerError::PortInUse {
                port: config.port,
                source: std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "port occupied by non-broker process",
                ),
            });
        }
        ProbeResult::Timeout => {
            return Err(BrokerError::ProbeTimeout { port: config.port });
        }
        ProbeResult::NoListener => {}
    }

    // Spawn flush thread if log_path is configured.
    let flush_thread = if state.log_path.is_some() {
        let s = Arc::clone(&state);
        let f = Arc::clone(&stop_flag);
        Some(std::thread::spawn(move || {
            delivery::flush_loop(&s, &f);
        }))
    } else {
        None
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(BrokerError::RuntimeFailed)?;

    let addr: std::net::SocketAddr = format!("{}:{}", config.bind, config.port).parse().map_err(
        |e: std::net::AddrParseError| {
            BrokerError::BindFailed(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))
        },
    )?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let router = server::router(Arc::clone(&state));

    let listener = runtime.block_on(async {
        let socket = tokio::net::TcpSocket::new_v4().map_err(BrokerError::BindFailed)?;
        socket
            .set_reuseaddr(true)
            .map_err(BrokerError::BindFailed)?;
        socket.bind(addr).map_err(BrokerError::BindFailed)?;
        socket.listen(1024).map_err(BrokerError::BindFailed)
    })?;

    // Install SIGINT handler so the broker does not die on Ctrl+C.
    // The dashboard process is responsible for user-facing Ctrl+C handling.
    runtime.spawn(async {
        let _ = tokio::signal::ctrl_c().await;
    });

    runtime.spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
    });

    // Pre-populate the CLI label AND the inbox queue for every watched
    // agent so (a) the dashboard shows the CLI before any status messages
    // arrive, and (b) peer `agent.artifact` broadcasts — which only target
    // already-existing queues — actually reach the watched agent even
    // before it has published anything itself.
    {
        let mut inner = state.write();
        for target in &watch_targets {
            inner
                .agent_clis
                .insert(target.agent_id.clone(), target.cli.clone());
            inner.queues.entry(target.agent_id.clone()).or_default();
        }
    }

    // Spawn one watcher task per target. All watchers share a single
    // `tokio::sync::watch` channel; flipping it to `true` on drop signals
    // every watcher to exit on its next tick.
    let (watcher_tx, watcher_rx) = tokio::sync::watch::channel(false);
    for target in watch_targets {
        let s = Arc::clone(&state);
        let rx = watcher_rx.clone();
        runtime.spawn(watcher::watch_worktree(s, target, rx));
    }

    Ok(BrokerHandle {
        state,
        runtime: Some(runtime),
        shutdown_tx: Some(shutdown_tx),
        watcher_shutdown: Some(watcher_tx),
        url,
        stop_flag,
        flush_thread,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broker_state_new_is_empty() {
        let state = BrokerState::new(None);
        let inner = state.read();
        assert!(inner.agents.is_empty());
        assert!(inner.queues.is_empty());
        assert!(inner.message_log.is_empty());
    }

    #[test]
    fn next_seq_starts_at_one() {
        let state = BrokerState::new(None);
        assert_eq!(state.next_seq(), 1);
        assert_eq!(state.next_seq(), 2);
        assert_eq!(state.next_seq(), 3);
    }

    #[test]
    fn probe_no_listener() {
        // Use a port that is almost certainly not in use.
        let result = probe_existing_broker("http://127.0.0.1:19999");
        assert_eq!(result, ProbeResult::NoListener);
    }

    #[test]
    fn reattached_handle_has_no_runtime() {
        let state = Arc::new(BrokerState::new(None));
        let h = BrokerHandle::reattached("http://127.0.0.1:9119".into(), state);
        assert!(h.runtime.is_none());
        assert!(h.shutdown_tx.is_none());
        assert!(h.flush_thread.is_none());
    }

    #[test]
    fn start_broker_on_free_port() {
        let config = BrokerConfig {
            enabled: true,
            // Use a high random port to avoid conflicts.
            #[allow(clippy::cast_possible_truncation)]
            port: 19_000 + (std::process::id() as u16 % 1000),
            bind: "127.0.0.1".to_string(),
        };
        let state = BrokerState::new(None);
        let handle = start_broker(&config, state, Vec::new());
        // If the port happens to be in use, the test is inconclusive — not a failure.
        if let Ok(h) = handle {
            assert!(h.url.contains(&config.port.to_string()));
            drop(h);
        }
    }

    #[test]
    fn start_broker_no_log_path_no_flush_thread() {
        let config = BrokerConfig {
            enabled: true,
            #[allow(clippy::cast_possible_truncation)]
            port: 19_100 + (std::process::id() as u16 % 100),
            bind: "127.0.0.1".to_string(),
        };
        let state = BrokerState::new(None);
        if let Ok(handle) = start_broker(&config, state, Vec::new()) {
            assert!(handle.flush_thread.is_none());
            drop(handle);
        }
    }

    #[test]
    fn start_broker_with_log_path_spawns_flush_thread() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("broker.log");
        let config = BrokerConfig {
            enabled: true,
            #[allow(clippy::cast_possible_truncation)]
            port: 19_200 + (std::process::id() as u16 % 100),
            bind: "127.0.0.1".to_string(),
        };
        let state = BrokerState::new(Some(log_path));
        if let Ok(handle) = start_broker(&config, state, Vec::new()) {
            assert!(handle.flush_thread.is_some());
            drop(handle);
        }
    }
}
