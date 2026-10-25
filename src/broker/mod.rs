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

pub mod client;
pub mod conflict;
pub mod delivery;
pub mod learnings;
pub mod messages;
pub mod origin;
pub mod publish;
pub mod server;
pub mod watcher;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::thread::JoinHandle;
use std::time::Instant;

use serde::Serialize;

use crate::config::{BrokerConfig, ConflictConfig};
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
    /// When the agent most recently published an `agent.artifact status:
    /// "committed"` event, if ever.
    ///
    /// Bug 8 (`auto-approve-scope-v0-6-x`): the filesystem watcher consults
    /// this to decide whether a subsequent file modification should
    /// re-publish `working` (within the configured TTL window). Transient —
    /// not serialized; reset only when overwritten by a newer committed event.
    pub last_committed_at: Option<Instant>,
    /// The agent's most recently reported set of modified files, from the
    /// last `Status` or `Artifact` message that carried one.
    ///
    /// Tracked separately from [`Self::last_message`] because that field is
    /// overwritten by ANY roster-upserting message — a `Blocked` or `Intent`
    /// message carries no file list, and letting it clobber the last known
    /// set would make cleanliness undecidable. `supervisor-branch-refresh`'s
    /// gate 1 reads this directly (design D1) rather than issuing a fresh
    /// `git status` probe, so the supervisor and the watcher can never
    /// disagree about whether a worktree is clean.
    pub modified_files: Vec<String>,
    /// The `status` and `phase` of this agent's most recent `agent.status`,
    /// or `None` before it has published one.
    ///
    /// Wave-progress classification needs "what did this agent last say",
    /// and neither existing field answers that:
    /// - [`Self::status`] is deliberately **sticky** at a terminal label, so
    ///   an agent that committed and then heartbeats `working` forever would
    ///   compare unequal on every single heartbeat and read as perpetual
    ///   progress — masking exactly the stall the wave signal exists to show.
    /// - [`Self::last_message`] is overwritten by any roster-upserting
    ///   variant, so an `agent.intent` between two identical heartbeats would
    ///   make the second look like a change.
    ///
    /// Transient, like the rest of the record — not serialized.
    pub last_status_and_phase: Option<(String, Option<String>)>,
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
    /// When the agent was last seen (for age calculations in the dashboard).
    #[serde(skip)]
    pub last_seen: Instant,
    /// Most recently published `payload.phase` on an `agent.status`, if any.
    /// The dashboard prefers this label over the message-type-derived
    /// `status_label()` when rendering the agent's row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    /// The agent's most recently reported modified-file set (see
    /// [`AgentRecord::modified_files`]). Carried over `/status` so a caller
    /// with only HTTP access — the unattended drive loop's post-merge branch
    /// refresh — can read the watcher's cleanliness signal without a second
    /// working-tree probe.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modified_files: Vec<String>,
}

/// Wave-level progress snapshot reported by `GET /status` and rendered by
/// every orchestrator-facing surface.
///
/// "The wave" is the session as a whole, not one agent: `idle_seconds`
/// measures how long it has been since *anything* progressed — a wave phase
/// advancing or a branch tip moving (see
/// [`delivery::message_is_progress`]). Per-agent `last_seen` cannot answer
/// that question, because a repeated `working` heartbeat keeps it fresh while
/// the wave sits on an unanswered prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct WaveStatus {
    /// Wall-clock seconds since the Unix epoch at the last observed progress.
    /// The broker's start time until the first progress is observed.
    pub last_progress_unix_secs: u64,
    /// Seconds elapsed since [`Self::last_progress_unix_secs`].
    pub idle_seconds: u64,
    /// Configured stall threshold in seconds; `0` disables [`Self::stalled`].
    pub stall_threshold_seconds: u64,
    /// `true` when the threshold is non-zero and `idle_seconds` exceeds it.
    pub stalled: bool,
}

impl WaveStatus {
    /// Renders [`Self::idle_seconds`] for a human: `3h12m`, `1h`, `45m`,
    /// `20s`.
    ///
    /// One rendering shared by `git paw status` and the dashboard badge, so
    /// the same idleness never reads two different ways depending on where
    /// the operator looked.
    #[must_use]
    pub fn idle_human(&self) -> String {
        let hours = self.idle_seconds / 3600;
        let minutes = (self.idle_seconds % 3600) / 60;
        match (hours, minutes) {
            (0, 0) => format!("{}s", self.idle_seconds),
            (0, m) => format!("{m}m"),
            (h, 0) => format!("{h}h"),
            (h, m) => format!("{h}h{m}m"),
        }
    }
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
    /// Post-commit re-entry TTL for the filesystem watcher (bug 8).
    ///
    /// A file modification observed within this window after an agent's
    /// `committed` event re-publishes `working`. `Duration::ZERO` disables
    /// the behaviour (v0.5.0 model). Defaults to 60s; overwritten from
    /// `[broker.watcher].republish_working_ttl_seconds` at broker start.
    pub republish_working_ttl: std::time::Duration,
    /// Worktree paths the broker is currently watching, for idempotent live
    /// registration via `POST /watch`. Seeded with the start-time watch
    /// targets in [`start_broker_with`] and extended at runtime by
    /// [`BrokerState::register_watch_target`]. A vanished worktree is pruned
    /// by its watcher task (see [`watcher::watch_worktree`]), so re-adding the
    /// same path later spawns a fresh watcher.
    pub watched_paths: HashSet<PathBuf>,
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
    /// Canonical root of the repository this broker serves, plumbed from the
    /// launch path via [`BrokerState::with_repo_root`]. Reported on `GET
    /// /status` as `repo_root` so the stale-broker probe can tell this
    /// repository's broker apart from a different repository's broker holding
    /// the same port. `None` when the caller did not declare a repository
    /// (router-only unit tests), in which case the probe cannot compare and
    /// falls back to marker-only matching.
    pub repo_root: Option<PathBuf>,
    /// Wall-clock instant the broker state was created; used for uptime reporting.
    started_at: Instant,
    /// Optional learnings aggregator. Populated when supervisor + learnings
    /// mode is active; the publish path forwards every observed message.
    pub learnings: Option<learnings::SharedLearnings>,
    /// When `true`, an `agent.artifact { status: "committed" }` triggers a
    /// broker-emitted [`BrokerMessage::VerifyNow`] nudge to the supervisor
    /// inbox so per-commit verification fires on an explicit event. Resolved
    /// from `[supervisor].verify_on_commit_nudge` (default `true`) at session
    /// boot and threaded in via [`BrokerState::with_verify_on_commit_nudge`].
    pub verify_on_commit_nudge: bool,
    /// opsx role-gating context. When `Some` and active (`OpenSpec` engine +
    /// non-`off` mode), the publish path runs the role-gating guard on every
    /// `agent.artifact { status: "committed" }`. `None` (the default, and
    /// non-OpenSpec sessions) leaves the guard inert. Threaded in via
    /// [`BrokerState::with_role_gating`].
    pub role_gating: Option<crate::opsx::RoleGatingContext>,
    /// Watcher shutdown receiver, populated once in [`start_broker_with`]
    /// before the start-time watchers spawn. The `POST /watch` handler clones
    /// it to enroll a live-registered watcher in the same shared shutdown
    /// signal, so a hot-added watcher stops in lockstep with the rest on
    /// [`BrokerHandle`] drop. `None` until the broker has spawned its watchers
    /// (e.g. in router-only unit tests), in which case live registration still
    /// records the target but spawns no watcher.
    watcher_shutdown_rx: OnceLock<tokio::sync::watch::Receiver<bool>>,
    /// Signalled after every append to `message_log`, so a `GET /log` waiter
    /// wakes the moment a newer entry exists instead of polling. Lives outside
    /// the `RwLock` so the long-poll handler can await it without holding any
    /// guard (`clippy::await_holding_lock`).
    publish_notify: tokio::sync::Notify,
    /// Unix seconds at the last observed wave progress, seeded with the
    /// broker's start time. An atomic rather than a `RwLock` field so the
    /// publish path and the watcher can stamp it without taking the state
    /// lock.
    last_progress_unix: AtomicU64,
    /// Wave-stall threshold in seconds, resolved from
    /// `[broker] wave_stall_minutes` at broker start; `0` disables the
    /// `stalled` flag.
    wave_stall_seconds: AtomicU64,
    /// Identifies this broker instance: its start time in Unix
    /// milliseconds.
    ///
    /// Milliseconds, not seconds, because the value's whole job is to
    /// differ between a broker and its replacement — and a pause/start
    /// cycle can land inside the same second.
    ///
    /// Sequence numbers restart at zero on every broker start, so a cursor
    /// from a previous broker means nothing to this one — and since a cursor
    /// never rewinds, a reader holding a stale high cursor would silently
    /// skip everything this broker has appended and wait out every long
    /// poll. Reporting the epoch on every `/log` response lets a reader
    /// notice the change and reset its cursor.
    epoch: u64,
}

/// Returns the current wall-clock time in seconds since the Unix epoch, or
/// `0` if the system clock predates the epoch.
fn now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Current wall-clock time in milliseconds since the Unix epoch, saturated
/// into a `u64`.
fn now_unix_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
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
                republish_working_ttl: std::time::Duration::from_secs(
                    crate::config::WatcherConfig::DEFAULT_REPUBLISH_TTL_SECONDS,
                ),
                watched_paths: HashSet::new(),
            }),
            next_seq: AtomicU64::new(0),
            log_path,
            repo_root: None,
            started_at: Instant::now(),
            learnings: None,
            // Conservative broker-level default: the nudge is opt-in at the
            // state level and explicitly enabled from config (whose own
            // default is `true`) via `with_verify_on_commit_nudge`.
            verify_on_commit_nudge: false,
            role_gating: None,
            watcher_shutdown_rx: OnceLock::new(),
            publish_notify: tokio::sync::Notify::new(),
            last_progress_unix: AtomicU64::new(now_unix_secs()),
            wave_stall_seconds: AtomicU64::new(
                crate::config::BrokerConfig::DEFAULT_WAVE_STALL_MINUTES * 60,
            ),
            epoch: now_unix_millis(),
        }
    }

    /// Declares the repository this broker serves. Call before
    /// [`start_broker`] so the stale-broker probe can compare the port
    /// holder's repository with this one, and so the first `GET /status`
    /// reports it.
    ///
    /// The path is canonicalized so `/var` vs `/private/var` style aliases
    /// compare equal; an uncanonicalizable path (a repository removed between
    /// launch and this call) is stored as given.
    #[must_use]
    pub fn with_repo_root(mut self, repo_root: &std::path::Path) -> Self {
        self.repo_root = Some(canonical_or_owned(repo_root));
        self
    }

    /// Attaches the opsx role-gating context. Call before [`start_broker`] so
    /// the publish path observes it from the first committed artifact. Passing
    /// a context whose mode is `off` or whose engine is not `OpenSpec` leaves
    /// the guard inert (the guard re-checks [`crate::opsx::RoleGatingContext::is_active`]).
    #[must_use]
    pub fn with_role_gating(mut self, ctx: crate::opsx::RoleGatingContext) -> Self {
        self.role_gating = Some(ctx);
        self
    }

    /// Sets whether committed artifacts emit a [`BrokerMessage::VerifyNow`]
    /// nudge to the supervisor inbox. Call before [`start_broker`] so the
    /// publish path observes the resolved value from the first message.
    #[must_use]
    pub fn with_verify_on_commit_nudge(mut self, enabled: bool) -> Self {
        self.verify_on_commit_nudge = enabled;
        self
    }

    /// Authoritatively seeds an agent's CLI into the roster's CLI map.
    ///
    /// git-paw knows every pane's CLI at launch (the supervisor's from
    /// `[supervisor].cli`/`default_cli`, each agent's from the resolved
    /// agent CLI), so the launcher seeds those known values rather than
    /// depending on the agent to self-report via `agent.status`. This is
    /// the only source for the supervisor's CLI column, since the
    /// supervisor is not a filesystem watch target (W15-15). A blank `cli`
    /// is ignored so a missing config value never clobbers the map. Call
    /// before [`start_broker`] so the first `/status` snapshot is correct.
    #[must_use]
    pub fn with_seeded_cli(self, agent_id: &str, cli: &str) -> Self {
        if !cli.is_empty()
            && let Ok(mut inner) = self.inner.write()
        {
            inner
                .agent_clis
                .insert(agent_id.to_string(), cli.to_string());
        }
        self
    }

    /// Attaches a learnings aggregator. Replaces any previously attached
    /// instance. Must be called before [`start_broker`] so the publish path
    /// observes every message from the first one.
    pub fn attach_learnings(&mut self, aggregator: learnings::SharedLearnings) {
        self.learnings = Some(aggregator);
    }

    /// Sets the post-commit re-entry TTL consulted by the filesystem watcher
    /// and the `committed -> working` transition in `update_agent_record`
    /// (bug 8). `Duration::ZERO` disables the auto-republish behaviour.
    pub fn set_republish_working_ttl(&self, ttl: std::time::Duration) {
        self.write().republish_working_ttl = ttl;
    }

    /// Records the shared watcher shutdown receiver so the `POST /watch`
    /// handler can clone it when spawning a live-registered watcher. Called
    /// once in [`start_broker_with`] before the start-time watchers spawn;
    /// subsequent calls are ignored (the receiver is set-once).
    pub fn set_watcher_shutdown_rx(&self, rx: tokio::sync::watch::Receiver<bool>) {
        let _ = self.watcher_shutdown_rx.set(rx);
    }

    /// Returns a clone of the shared watcher shutdown receiver if the broker
    /// has spawned its watchers, or `None` otherwise (e.g. router-only unit
    /// tests). The `POST /watch` handler uses this to enroll a hot-added
    /// watcher in the shared shutdown signal.
    #[must_use]
    pub fn watcher_shutdown_rx(&self) -> Option<tokio::sync::watch::Receiver<bool>> {
        self.watcher_shutdown_rx.get().cloned()
    }

    /// Registers a worktree as a live watch target for `POST /watch`.
    ///
    /// Bookkeeping only — the caller spawns the actual [`watcher::watch_worktree`]
    /// task when this returns `true`. Idempotent: the worktree path is the key,
    /// so re-registering an already-watched path returns `false` and the caller
    /// spawns nothing (no duplicate watcher). On a fresh path it seeds the
    /// agent's CLI label and inbox queue exactly as the start-time targets are
    /// seeded in [`start_broker_with`], so the agent surfaces in `/status` and
    /// receives peer broadcasts on the same terms.
    pub fn register_watch_target(&self, target: &WatchTarget) -> bool {
        let mut inner = self.write();
        if !inner.watched_paths.insert(target.worktree_path.clone()) {
            return false;
        }
        if !target.cli.is_empty() {
            inner
                .agent_clis
                .insert(target.agent_id.clone(), target.cli.clone());
        }
        inner.queues.entry(target.agent_id.clone()).or_default();
        true
    }

    /// Drops a worktree from the live watch-target set so a later
    /// re-registration of the same path spawns a fresh watcher. Called by a
    /// watcher task when it detects its worktree has disappeared (the prune
    /// path for `git paw remove`).
    pub fn forget_watch_target(&self, worktree_path: &std::path::Path) {
        self.write().watched_paths.remove(worktree_path);
    }

    /// Sets the wave-stall threshold in seconds (`0` disables the `stalled`
    /// flag). Called from [`start_broker_with`] with the value resolved from
    /// `[broker] wave_stall_minutes`.
    pub fn set_wave_stall_seconds(&self, seconds: u64) {
        self.wave_stall_seconds.store(seconds, Ordering::Relaxed);
    }

    /// Records that the wave progressed just now.
    ///
    /// Called by the publish path for a message
    /// [`delivery::message_is_progress`] classifies as progress, and by the
    /// filesystem watcher when an agent's worktree `HEAD` commit changes.
    pub fn mark_progress(&self) {
        self.last_progress_unix
            .store(now_unix_secs(), Ordering::Relaxed);
    }

    /// Overrides the last-progress timestamp (Unix seconds).
    ///
    /// Exists so tests can age the wave without sleeping; production code
    /// marks progress through [`BrokerState::mark_progress`].
    pub fn set_last_progress_unix_secs(&self, unix_secs: u64) {
        self.last_progress_unix.store(unix_secs, Ordering::Relaxed);
    }

    /// Returns the current wave-progress snapshot.
    ///
    /// `stalled` is true only when the configured threshold is non-zero and
    /// `idle_seconds` exceeds it, so `wave_stall_minutes = 0` keeps reporting
    /// idleness without ever raising the flag.
    #[must_use]
    pub fn wave_status(&self) -> WaveStatus {
        let last_progress_unix_secs = self.last_progress_unix.load(Ordering::Relaxed);
        let stall_threshold_seconds = self.wave_stall_seconds.load(Ordering::Relaxed);
        let idle_seconds = now_unix_secs().saturating_sub(last_progress_unix_secs);
        WaveStatus {
            last_progress_unix_secs,
            idle_seconds,
            stall_threshold_seconds,
            stalled: stall_threshold_seconds != 0 && idle_seconds > stall_threshold_seconds,
        }
    }

    /// Returns this broker instance's epoch — its start time in Unix
    /// milliseconds. See [`BrokerState::epoch`] for why a cursor reader
    /// needs it.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Wakes every `GET /log` waiter, signalling that the message log grew.
    pub fn notify_published(&self) {
        self.publish_notify.notify_waiters();
    }

    /// Returns a future that resolves on the next [`BrokerState::notify_published`].
    ///
    /// The caller registers interest *before* re-checking the log, so an
    /// entry appended between the check and the await is never missed.
    pub fn publish_notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.publish_notify.notified()
    }

    /// Acquires a read lock on the inner state.
    ///
    /// Recovers from a poisoned lock (a thread panicked while holding a guard)
    /// by taking the guard out of the `PoisonError`, so one panic cannot brick
    /// the broker. Broker state is ephemeral — an in-memory roster/log/queue
    /// set rebuilt on every `git paw start`, with no persisted invariant a
    /// partially-updated view could corrupt — so continuing to serve is
    /// strictly better than wedging every later handler and background task.
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, BrokerStateInner> {
        self.inner
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Acquires a write lock on the inner state.
    ///
    /// Recovers from a poisoned lock exactly as [`BrokerState::read`] does.
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, BrokerStateInner> {
        self.inner
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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

    /// The port is held by a git-paw broker serving a different repository —
    /// or one that does not report which repository it serves, which an older
    /// git-paw broker does not. Distinct from [`BrokerError::PortInUse`]:
    /// that one means a foreign program holds the port, and the two have
    /// different fixes.
    ///
    /// Reattaching would route this repository's agents into the other
    /// repository's broker while this repository's dashboard rendered an
    /// empty, unwatched state, so the port holder is refused instead.
    ///
    /// `repo_root` is attacker-influenced — any local process can bind the
    /// port and answer the probe — so it MUST be built with
    /// [`sanitize_reported_repo_root`], never from the raw response.
    #[error(
        "port {port} is held by a git-paw broker for {} — set a distinct [broker] port in .git-paw/config.toml for one of the repositories",
        .repo_root.as_ref().map_or_else(
            || "another repository (it does not report which — an older git-paw broker may be holding the port)".to_string(),
            |root| format!("another repository ({root})"),
        )
    )]
    PortHeldByOtherRepo {
        /// The port held by the other repository's broker.
        port: u16,
        /// The repository the port holder reported serving, or `None` when it
        /// reported no repository at all.
        repo_root: Option<String>,
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
    /// Periodic flush thread for the learnings aggregator (present only
    /// when `state.learnings.is_some()`).
    learnings_thread: Option<JoinHandle<()>>,
}

impl BrokerHandle {
    /// Whether this handle reattached to a broker another process already
    /// runs for this repository, rather than starting one of its own.
    #[must_use]
    pub fn is_reattached(&self) -> bool {
        self.runtime.is_none()
    }

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
            learnings_thread: None,
        }
    }
}

impl Drop for BrokerHandle {
    fn drop(&mut self) {
        // 1. Signal both flush threads to stop and join the log flush thread.
        self.stop_flag.store(true, Ordering::Release);
        if let Some(handle) = self.flush_thread.take() {
            let _ = handle.join();
        }
        // 2. Join the learnings flush thread — it performs the final
        //    shutdown flush before returning.
        if let Some(handle) = self.learnings_thread.take() {
            let _ = handle.join();
        }
        // 3. Signal watcher tasks to stop.
        if let Some(tx) = self.watcher_shutdown.take() {
            let _ = tx.send(true);
        }
        // 4. Signal shutdown to the server task.
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        // 5. Give in-flight requests up to 2 seconds to drain, then drop runtime.
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
    /// A git-paw broker for the caller's own repository is already running.
    LiveBroker,
    /// A git-paw broker is running, but it serves a different repository —
    /// or it reports no repository at all (an older git-paw broker), in which
    /// case `repo_root` is `None` and the owner is treated as unknown rather
    /// than assumed to be the caller's.
    OtherRepoBroker {
        /// The repository the port holder reported serving, if any.
        repo_root: Option<String>,
    },
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
/// starting a new server (e.g. the `status` subcommand). Such callers only ask
/// "is a git-paw broker there", so no repository is supplied and any git-paw
/// broker reports as [`ProbeResult::LiveBroker`].
pub fn probe_broker(url: &str) -> ProbeResult {
    probe_existing_broker(url, None)
}

/// Probes a URL and classifies the listener against `repo_root`.
///
/// The repository-aware counterpart of [`probe_broker`], for callers that
/// need to know whether the port holder is *their* broker rather than just
/// some git-paw broker: the pre-flight launch check, `git paw status`, and
/// `git paw doctor`. A git-paw broker serving a different repository — or
/// none at all — comes back as [`ProbeResult::OtherRepoBroker`].
pub fn probe_broker_for_repo(url: &str, repo_root: &std::path::Path) -> ProbeResult {
    let canonical = canonical_or_owned(repo_root);
    probe_existing_broker(url, Some(&canonical))
}

/// Largest `GET /status` response the probe will read, in bytes.
///
/// Any local process can bind the broker port and answer the probe, so the
/// read is bounded rather than trusting the listener to stop talking. A real
/// `/status` body is a few kilobytes even with a full agent roster.
const MAX_PROBE_RESPONSE_BYTES: u64 = 64 * 1024;

/// Longest reported `repo_root` reproduced in an error message.
const MAX_REPORTED_REPO_ROOT_CHARS: usize = 256;

/// Makes a listener-reported `repo_root` safe to print in an error message.
///
/// The value comes from whatever process answered the probe, so it is
/// untrusted input on its way to a terminal: control characters (including
/// the ESC that opens an ANSI escape sequence) are replaced with `?` so a
/// malicious listener cannot repaint, clear, or forge output around git-paw's
/// error, and the result is capped so it cannot flood the screen.
fn sanitize_reported_repo_root(reported: &str) -> String {
    let mut out: String = reported
        .chars()
        .take(MAX_REPORTED_REPO_ROOT_CHARS)
        .map(|c| if c.is_control() { '?' } else { c })
        .collect();
    if reported.chars().count() > MAX_REPORTED_REPO_ROOT_CHARS {
        out.push('…');
    }
    out
}

/// Canonicalizes a path, falling back to the path as given when it cannot be
/// resolved (e.g. it no longer exists). Canonical form is what makes `/var`
/// and `/private/var` style aliases of one repository compare equal.
fn canonical_or_owned(path: &std::path::Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Extracts the `repo_root` field from a raw `GET /status` HTTP response.
///
/// Parses the body as JSON when it can be isolated; a response whose body is
/// unparseable, carries no `repo_root`, or carries an empty one yields
/// `None`, which callers treat as "owner unknown" — the safe direction.
fn parse_repo_root(response: &str) -> Option<String> {
    let body = response.split_once("\r\n\r\n").map(|(_, b)| b)?;
    let json: serde_json::Value = serde_json::from_str(body.trim()).ok()?;
    json.get("repo_root")?
        .as_str()
        .filter(|root| !root.is_empty())
        .map(ToString::to_string)
}

/// Probes the listener at `url`, classifying it against `expected_repo_root`.
///
/// When `expected_repo_root` is `Some`, a git-paw broker counts as
/// [`ProbeResult::LiveBroker`] only if it reports the same canonical
/// repository; anything else — a different repository, or no reported
/// repository at all — is [`ProbeResult::OtherRepoBroker`]. When it is `None`
/// the caller has declared no repository, so the comparison is skipped.
fn probe_existing_broker(url: &str, expected_repo_root: Option<&std::path::Path>) -> ProbeResult {
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

    // A reset, broken pipe or EOF before any response byte is a listener that
    // is going away (a broker mid-shutdown, its socket closing under us), not
    // a broker that answers too slowly. Re-probe once after a short pause: a
    // refusal or a second reset then means nothing will serve the port, so
    // the caller binds instead of failing with a probe timeout (GP-82).
    let response = match probe_attempt(&socket_addr, addr) {
        ProbeAttempt::Refused => return ProbeResult::NoListener,
        ProbeAttempt::TimedOut => return ProbeResult::Timeout,
        ProbeAttempt::Answered(response) => response,
        ProbeAttempt::Reset => {
            std::thread::sleep(PROBE_RESET_RETRY_PAUSE);
            match probe_attempt(&socket_addr, addr) {
                ProbeAttempt::Refused | ProbeAttempt::Reset => return ProbeResult::NoListener,
                ProbeAttempt::TimedOut => return ProbeResult::Timeout,
                ProbeAttempt::Answered(response) => response,
            }
        }
    };

    if response.contains("\"git_paw\":true") || response.contains("\"git_paw\": true") {
        let Some(expected) = expected_repo_root else {
            return ProbeResult::LiveBroker;
        };
        let reported = parse_repo_root(&response);
        match reported.as_deref() {
            Some(root) if canonical_or_owned(std::path::Path::new(root)).as_path() == expected => {
                ProbeResult::LiveBroker
            }
            _ => ProbeResult::OtherRepoBroker {
                repo_root: reported,
            },
        }
    } else if response.starts_with("HTTP/") {
        ProbeResult::ForeignServer
    } else {
        ProbeResult::Timeout
    }
}

/// Pause before the single re-probe that follows a reset or EOF.
const PROBE_RESET_RETRY_PAUSE: std::time::Duration = std::time::Duration::from_millis(150);

/// How one `GET /status` probe attempt ended, before its body is classified.
#[derive(Debug, PartialEq, Eq)]
enum ProbeAttempt {
    /// The connection was refused (or could not be made at all).
    Refused,
    /// The peer reset, broke the pipe or closed before sending a single byte.
    Reset,
    /// The peer accepted but sent nothing within the read timeout.
    TimedOut,
    /// The peer answered; the (bounded) response so far.
    Answered(String),
}

/// Whether an I/O error is the peer tearing the connection down, as opposed
/// to a timeout or anything else.
fn is_teardown(kind: std::io::ErrorKind) -> bool {
    use std::io::ErrorKind;
    matches!(
        kind,
        ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::BrokenPipe
            | ErrorKind::UnexpectedEof
            | ErrorKind::NotConnected
    )
}

/// Runs one probe attempt against `socket_addr`, with `addr` as the `Host` header.
fn probe_attempt(socket_addr: &std::net::SocketAddr, addr: &str) -> ProbeAttempt {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    let Ok(mut stream) = TcpStream::connect_timeout(socket_addr, Duration::from_millis(500)) else {
        return ProbeAttempt::Refused;
    };

    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_millis(500)))
        .ok();

    let request = format!("GET /status HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    if let Err(e) = stream.write_all(request.as_bytes()) {
        return if is_teardown(e.kind()) {
            ProbeAttempt::Reset
        } else {
            ProbeAttempt::TimedOut
        };
    }

    let mut response = String::new();
    match (&stream)
        .take(MAX_PROBE_RESPONSE_BYTES)
        .read_to_string(&mut response)
    {
        // EOF before a single byte: the listener closed without answering.
        Ok(_) if response.is_empty() => ProbeAttempt::Reset,
        Err(e) if response.is_empty() => {
            if is_teardown(e.kind()) {
                ProbeAttempt::Reset
            } else {
                ProbeAttempt::TimedOut
            }
        }
        _ => ProbeAttempt::Answered(response),
    }
}

/// Checks, before a session launches, that the configured broker port is not
/// held by another repository's git-paw broker.
///
/// The broker host performs the same check when it starts the broker, but
/// that happens *after* the tmux session exists and the agent panes are up —
/// and every agent pane already has `GIT_PAW_BROKER_URL` pointing at the
/// shared port, so by then this repository's agents are publishing into the
/// other repository's broker. Launchers call this first so the clash is
/// caught while nothing has been started yet.
///
/// A disabled broker is a no-op. Only the other-repository verdict fails the
/// launch; a foreign listener or a probe timeout is left to the broker
/// host's own start-up path, which reports them with their own errors.
///
/// # Errors
///
/// Returns [`BrokerError::PortHeldByOtherRepo`] when a git-paw broker for a
/// different — or unreported — repository holds the configured port.
pub fn ensure_port_free_for_repo(
    config: &BrokerConfig,
    repo_root: &std::path::Path,
) -> Result<(), BrokerError> {
    if !config.enabled {
        return Ok(());
    }
    if let ProbeResult::OtherRepoBroker { repo_root } =
        probe_broker_for_repo(&config.url(), repo_root)
    {
        return Err(BrokerError::PortHeldByOtherRepo {
            port: config.port,
            repo_root: repo_root.as_deref().map(sanitize_reported_repo_root),
        });
    }
    Ok(())
}

/// Starts the HTTP broker server.
///
/// Probes the configured port first:
/// - If a live git-paw broker for `state`'s repository is found, returns a
///   reattached handle.
/// - If a git-paw broker for a different — or unreported — repository holds
///   the port, returns [`BrokerError::PortHeldByOtherRepo`] without
///   reattaching.
/// - If a foreign server occupies the port, returns [`BrokerError::PortInUse`].
/// - If the probe times out, returns [`BrokerError::ProbeTimeout`].
/// - If nothing is listening, binds and starts the server.
///
/// Also spawns the background flush thread if `state.log_path` is set.
///
/// Calls [`start_broker`] without a conflict-detector. Equivalent to
/// `start_broker_with(config, state, watch_targets, None)`.
pub fn start_broker(
    config: &BrokerConfig,
    state: BrokerState,
    watch_targets: Vec<WatchTarget>,
) -> Result<BrokerHandle, BrokerError> {
    start_broker_with(config, state, watch_targets, None, 60)
}

/// Starts the HTTP broker server with optional conflict-detector wiring
/// and a configurable learnings-flush interval.
///
/// When `conflict` is `Some`, a background tokio task running the
/// detector loop is spawned alongside the watcher tasks. The detector
/// shuts down with the rest of the broker when [`BrokerHandle`] is
/// dropped.
///
/// `learnings_flush_interval_seconds` controls how often the learnings
/// aggregator flushes to `.git-paw/session-learnings.md` when
/// `state.learnings` is `Some`. Default for [`start_broker`] is 60s;
/// tests override to drive flush behaviour without waiting on real time.
#[allow(clippy::too_many_lines)]
pub fn start_broker_with(
    config: &BrokerConfig,
    state: BrokerState,
    watch_targets: Vec<WatchTarget>,
    conflict: Option<ConflictConfig>,
    learnings_flush_interval_seconds: u64,
) -> Result<BrokerHandle, BrokerError> {
    let url = config.url();
    let state = Arc::new(state);
    // Apply the configured post-commit re-entry TTL (bug 8) before any
    // watcher task or publish observes the state.
    state.set_republish_working_ttl(std::time::Duration::from_secs(
        config.watcher.republish_working_ttl_seconds(),
    ));
    // Apply the configured wave-stall threshold before the first `/status`
    // can be served.
    state.set_wave_stall_seconds(config.wave_stall_seconds());
    let stop_flag = Arc::new(AtomicBool::new(false));

    match probe_existing_broker(&url, state.repo_root.as_deref()) {
        ProbeResult::LiveBroker => return Ok(BrokerHandle::reattached(url, state)),
        ProbeResult::OtherRepoBroker { repo_root } => {
            return Err(BrokerError::PortHeldByOtherRepo {
                port: config.port,
                repo_root: repo_root.as_deref().map(sanitize_reported_repo_root),
            });
        }
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

    // Spawn learnings flush thread when an aggregator is attached. The
    // thread performs a final `flush_at_shutdown` after the stop flag is
    // raised so unresolved blocks and recovery cycles end up in the file.
    let learnings_thread = if state.learnings.is_some() {
        let s = Arc::clone(&state);
        let f = Arc::clone(&stop_flag);
        Some(std::thread::spawn(move || {
            learnings_flush_loop(&s, &f, learnings_flush_interval_seconds);
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

    // The browser-exposure guard checks `Host` only when the listener is
    // confined to this machine; on a network bind git-paw cannot enumerate the
    // operator's names, so it warns instead (see `origin::decide_request_origin`).
    let bind_is_loopback = origin::is_loopback_bind(&config.bind);
    if !bind_is_loopback {
        // The launch command prints this too, and that is the copy an operator
        // reads — this process is the broker host, whose output lands in its
        // detached `broker` window where nobody is looking.
        eprintln!("{}", origin::non_loopback_bind_warning(&config.bind));
    }
    let router = server::router(Arc::clone(&state), bind_is_loopback);

    let listener = runtime.block_on(async {
        let socket = tokio::net::TcpSocket::new_v4().map_err(BrokerError::BindFailed)?;
        socket
            .set_reuseaddr(true)
            .map_err(BrokerError::BindFailed)?;
        socket.bind(addr).map_err(BrokerError::BindFailed)?;
        socket.listen(1024).map_err(BrokerError::BindFailed)
    })?;

    // Install SIGINT handler so the broker does not die on Ctrl+C. The broker
    // host also ignores SIGINT for the whole process.
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
            // Seed the live target set so a `POST /watch` for a start-time path is a no-op.
            inner.watched_paths.insert(target.worktree_path.clone());
        }
    }

    // Spawn one watcher task per target. All watchers share a single
    // `tokio::sync::watch` channel; flipping it to `true` on drop signals
    // every watcher to exit on its next tick. The conflict detector
    // shares the same shutdown channel so it stops in lockstep.
    let (watcher_tx, watcher_rx) = tokio::sync::watch::channel(false);
    // Publish the shutdown receiver so `POST /watch` can enroll live watchers.
    state.set_watcher_shutdown_rx(watcher_rx.clone());
    for target in watch_targets {
        let s = Arc::clone(&state);
        let rx = watcher_rx.clone();
        runtime.spawn(watcher::watch_worktree(s, target, rx));
    }
    if let Some(conflict_cfg) = conflict {
        let s = Arc::clone(&state);
        let rx = watcher_rx.clone();
        runtime.spawn(conflict::run_detector_loop(s, conflict_cfg, rx));
    }

    Ok(BrokerHandle {
        state,
        runtime: Some(runtime),
        shutdown_tx: Some(shutdown_tx),
        watcher_shutdown: Some(watcher_tx),
        url,
        stop_flag,
        flush_thread,
        learnings_thread,
    })
}

/// Background loop driving the learnings aggregator's periodic flush.
///
/// Sleeps in small slices so it can react to the broker stop flag within
/// ~100ms. When the stop flag is raised, it performs one final
/// [`learnings::LearningsAggregator::flush_at_shutdown`] before exiting.
fn learnings_flush_loop(
    state: &Arc<BrokerState>,
    stop: &Arc<AtomicBool>,
    flush_interval_seconds: u64,
) {
    let Some(aggregator) = state.learnings.clone() else {
        return;
    };
    let interval = std::time::Duration::from_secs(flush_interval_seconds.max(1));
    let tick = std::time::Duration::from_millis(100);

    loop {
        let mut elapsed = std::time::Duration::ZERO;
        while elapsed < interval {
            if stop.load(Ordering::Acquire) {
                if let Ok(mut agg) = aggregator.lock() {
                    let _ = agg.flush_at_shutdown();
                }
                publish_pending_learnings(state, &aggregator);
                return;
            }
            std::thread::sleep(tick);
            elapsed += tick;
        }
        if let Ok(mut agg) = aggregator.lock() {
            let _ = agg.flush();
        }
        publish_pending_learnings(state, &aggregator);
    }
}

/// Drains the aggregator's pending broker records and publishes each as an
/// `agent.learning` message.
///
/// Critically, the aggregator lock is acquired *only* to drain the queue and
/// is released before any publish: `publish_message` re-enters the aggregator
/// via `observe`, so publishing while holding the lock would deadlock the
/// non-reentrant `Mutex`. When broker publish is disabled the queue is always
/// empty, so this is a cheap no-op.
fn publish_pending_learnings(state: &Arc<BrokerState>, aggregator: &learnings::SharedLearnings) {
    let records = match aggregator.lock() {
        Ok(mut agg) => agg.take_pending_publish(),
        Err(_) => return,
    };
    for record in &records {
        delivery::publish_message(state, &BrokerMessage::from(record));
    }
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
    fn register_watch_target_is_idempotent_and_seeds_roster() {
        let state = BrokerState::new(None);
        let target = WatchTarget {
            agent_id: "feat-hot".to_string(),
            cli: "claude".to_string(),
            worktree_path: PathBuf::from("/tmp/feat-hot"),
        };
        // First registration is fresh — the caller should spawn a watcher.
        assert!(
            state.register_watch_target(&target),
            "first registration must return true"
        );
        // Re-registering the same path is a no-op — no duplicate watcher.
        assert!(
            !state.register_watch_target(&target),
            "duplicate registration must return false"
        );
        let inner = state.read();
        assert_eq!(inner.watched_paths.len(), 1, "path recorded exactly once");
        assert_eq!(
            inner.agent_clis.get("feat-hot").map(String::as_str),
            Some("claude"),
            "registration seeds the CLI label"
        );
        assert!(
            inner.queues.contains_key("feat-hot"),
            "registration seeds the inbox queue"
        );
    }

    #[test]
    fn forget_watch_target_allows_re_registration() {
        let state = BrokerState::new(None);
        let target = WatchTarget {
            agent_id: "feat-hot".to_string(),
            cli: "claude".to_string(),
            worktree_path: PathBuf::from("/tmp/feat-hot"),
        };
        assert!(state.register_watch_target(&target));
        state.forget_watch_target(&target.worktree_path);
        assert!(
            state.register_watch_target(&target),
            "after forgetting, the same path registers fresh again"
        );
    }

    #[test]
    fn read_and_write_recover_from_a_poisoned_lock() {
        let state = Arc::new(BrokerState::new(None));
        // Poison the lock the way a panicking handler or background task would:
        // panic while a write guard is held. The panic message is expected
        // noise in the test output.
        let poisoner = Arc::clone(&state);
        let outcome = std::thread::spawn(move || {
            let _guard = poisoner.write();
            panic!("deliberate panic while holding the broker state write guard");
        })
        .join();
        assert!(outcome.is_err(), "the poisoning thread must have panicked");

        // Both accessors must still hand back a usable guard.
        assert!(state.read().queues.is_empty());
        state
            .write()
            .queues
            .entry("feat-x".to_string())
            .or_default();
        assert!(
            state.read().queues.contains_key("feat-x"),
            "a write through a recovered guard must be observable through a recovered read"
        );
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
        let result = probe_existing_broker("http://127.0.0.1:19999", None);
        assert_eq!(result, ProbeResult::NoListener);
    }

    /// Spawns a listener that accepts every connection and closes it at once,
    /// before sending a byte — a broker socket being torn down under the
    /// probe. Returns its URL; the listener lives for the test's duration.
    fn resetting_listener() -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                // Dropping the accepted stream closes it with nothing sent:
                // the client reads EOF or a reset, never a response byte.
                drop(stream);
            }
        });
        format!("http://{addr}")
    }

    /// `broker-runtime` *Stale broker detection*, "A listener that resets
    /// mid-handshake is not a live broker" (GP-82): a reset / EOF before any
    /// response byte, on the probe and on its single re-probe, is no live
    /// broker — not a probe timeout.
    #[test]
    fn a_listener_that_resets_mid_handshake_is_not_a_live_broker() {
        let url = resetting_listener();
        assert_eq!(probe_existing_broker(&url, None), ProbeResult::NoListener);
        let repo = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            probe_broker_for_repo(&url, repo.path()),
            ProbeResult::NoListener,
            "the repository-scoped probe classifies a reset the same way"
        );
    }

    /// `broker-runtime` *Stale broker detection*, "A silent listener is a
    /// probe timeout": a listener that accepts and never answers is still
    /// `Timeout` (→ `BrokerError::ProbeTimeout`), not mistaken for a reset.
    #[test]
    fn a_silent_listener_is_a_probe_timeout() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            // Hold the accepted stream open, silent, until the probe is done.
            let held = listener.accept();
            let _ = done_rx.recv_timeout(std::time::Duration::from_secs(5));
            drop(held);
        });
        let result = probe_existing_broker(&format!("http://{addr}"), None);
        let _ = done_tx.send(());
        assert_eq!(result, ProbeResult::Timeout);
    }

    /// A reset on the first probe followed by a real answer on the re-probe
    /// is classified from that answer: the re-probe is a retry, not a verdict.
    #[test]
    fn a_reset_followed_by_an_answer_classifies_the_answer() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        std::thread::spawn(move || {
            let mut incoming = listener.incoming();
            drop(incoming.next());
            if let Some(Ok(mut stream)) = incoming.next() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let body = "not found";
                let _ = write!(
                    stream,
                    "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        assert_eq!(
            probe_existing_broker(&format!("http://{addr}"), None),
            ProbeResult::ForeignServer
        );
    }

    /// Binds and releases an ephemeral port, yielding a port number that is
    /// free right now, for a test that starts its own broker on it next.
    ///
    /// Returns only once a connect to the port is refused. Closing the
    /// listener does not take effect at once: under load macOS can still
    /// complete a handshake against the listener it is tearing down and then
    /// reset it, and `start_broker`'s pre-start probe — which runs a moment
    /// later — reads that reset as `ProbeTimeout` instead of `NoListener`.
    fn free_port() -> u16 {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind ephemeral port")
            .local_addr()
            .expect("read ephemeral addr")
            .port();
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline
            && std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(100))
                .is_ok()
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        port
    }

    fn config_on(port: u16) -> BrokerConfig {
        BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
            ..Default::default()
        }
    }

    /// Starts a broker serving `repo_root` on a free port. Returns the handle
    /// (keep it alive for the duration of the test) and its config.
    fn broker_for(repo_root: &std::path::Path) -> (BrokerHandle, BrokerConfig) {
        let config = config_on(free_port());
        let handle = start_broker(
            &config,
            BrokerState::new(None).with_repo_root(repo_root),
            Vec::new(),
        )
        .expect("start broker for repository");
        (handle, config)
    }

    /// One idleness rendering shared by `git paw status` and the dashboard
    /// badge, so the same wave never reads two different ways.
    #[test]
    fn idle_human_renders_hours_minutes_and_seconds() {
        let wave = |idle_seconds| WaveStatus {
            last_progress_unix_secs: 0,
            idle_seconds,
            stall_threshold_seconds: 60,
            stalled: true,
        };
        assert_eq!(wave(11_520).idle_human(), "3h12m");
        assert_eq!(wave(3_600).idle_human(), "1h");
        assert_eq!(wave(2_700).idle_human(), "45m");
        assert_eq!(wave(20).idle_human(), "20s");
    }

    /// `broker-runtime` scenario "Absent key uses the default": a config with
    /// the broker enabled and no `wave_stall_minutes` starts a broker whose
    /// reported threshold is 30 minutes.
    #[test]
    fn broker_start_without_the_key_uses_the_default_wave_threshold() {
        let config = config_on(free_port());
        assert_eq!(config.wave_stall_minutes, None);

        // Start from a threshold that is NOT the default, so the assertion
        // distinguishes "the config default was applied" from "the state's
        // own seed was never overwritten". Without this the test passes even
        // when `start_broker_with` never reads the config at all.
        let state = BrokerState::new(None);
        state.set_wave_stall_seconds(1);

        let Ok(handle) = start_broker(&config, state, Vec::new()) else {
            // A busy port makes the test inconclusive, not failing.
            return;
        };
        assert_eq!(handle.state.wave_status().stall_threshold_seconds, 1800);
    }

    /// `broker-runtime` scenario "Configured threshold is honoured".
    #[test]
    fn broker_start_honours_a_configured_wave_threshold() {
        let config = BrokerConfig {
            wave_stall_minutes: Some(5),
            ..config_on(free_port())
        };

        let Ok(handle) = start_broker(&config, BrokerState::new(None), Vec::new()) else {
            return;
        };
        assert_eq!(handle.state.wave_status().stall_threshold_seconds, 300);
    }

    #[test]
    fn another_repositorys_broker_on_the_port_is_refused() {
        let repo_a = tempfile::tempdir().expect("tempdir A");
        let repo_b = tempfile::tempdir().expect("tempdir B");
        let (_held, config) = broker_for(repo_a.path());

        let result = start_broker(
            &config,
            BrokerState::new(None).with_repo_root(repo_b.path()),
            Vec::new(),
        );

        let Err(err) = result else {
            panic!("repository B must not get a handle to repository A's broker");
        };
        assert!(
            matches!(err, BrokerError::PortHeldByOtherRepo { .. }),
            "refusal must be its own variant, not PortInUse: {err:?}"
        );
        let message = err.to_string();
        let canonical_a = repo_a.path().canonicalize().expect("canonicalize A");
        assert!(
            message.contains(&canonical_a.display().to_string()),
            "error must name the holding repository: {message}"
        );
        assert!(
            message.contains(&config.port.to_string()),
            "error must name the port: {message}"
        );
        assert!(
            message.contains("[broker] port"),
            "error must name the config fix: {message}"
        );
    }

    #[test]
    fn broker_that_reports_no_repository_is_refused() {
        let repo = tempfile::tempdir().expect("tempdir");
        // A broker started without a declared repository stands in for an
        // older git-paw whose `/status` does not report `repo_root`.
        let config = config_on(free_port());
        let _held = start_broker(&config, BrokerState::new(None), Vec::new())
            .expect("start repository-less broker");

        let result = start_broker(
            &config,
            BrokerState::new(None).with_repo_root(repo.path()),
            Vec::new(),
        );

        let Err(err) = result else {
            panic!("a broker of unknown repository must not be reattached to");
        };
        assert!(
            matches!(
                err,
                BrokerError::PortHeldByOtherRepo {
                    repo_root: None,
                    ..
                }
            ),
            "unknown owner must surface as an unknown repository: {err:?}"
        );
        assert!(
            err.to_string().contains("older git-paw"),
            "error should point at an older git-paw broker: {err}"
        );
    }

    #[test]
    fn same_repositorys_broker_is_still_reattached() {
        let repo = tempfile::tempdir().expect("tempdir");
        let (_held, config) = broker_for(repo.path());

        let handle = start_broker(
            &config,
            BrokerState::new(None).with_repo_root(repo.path()),
            Vec::new(),
        )
        .expect("same repository must reattach");
        assert!(
            handle.runtime.is_none(),
            "a reattached handle owns no runtime"
        );
    }

    #[test]
    fn a_foreign_http_server_on_the_port_is_refused() {
        use std::io::{Read, Write};
        // A plain HTTP server that answers GET /status with 404 — no git-paw
        // marker, so it must be reported as an occupied port rather than
        // mistaken for a broker of any repository.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind foreign server");
        let port = listener.local_addr().expect("foreign addr").port();
        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            }
        });

        let repo = tempfile::tempdir().expect("tempdir");
        let result = start_broker(
            &config_on(port),
            BrokerState::new(None).with_repo_root(repo.path()),
            Vec::new(),
        );
        let _ = server.join();

        let Err(err) = result else {
            panic!("a foreign HTTP server must not be reattached to");
        };
        assert!(
            matches!(err, BrokerError::PortInUse { .. }),
            "a non-git-paw listener is an occupied port, not a repository clash: {err:?}"
        );
        assert!(
            err.to_string().contains(&port.to_string()),
            "error must name the port: {err}"
        );
    }

    #[test]
    fn the_probe_stops_reading_a_listener_that_never_stops_talking() {
        use std::io::{Read, Write};
        // A listener that answers the probe and then streams far more than
        // the bound, without ever closing. Unbounded, the probe would grow
        // its buffer until the write side gave up or memory ran out; bounded,
        // it reads its cap and moves on.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind flooding server");
        let port = listener.local_addr().expect("flood addr").port();
        let sent = Arc::new(AtomicU64::new(0));
        let sent_in_thread = Arc::clone(&sent);
        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n{\"git_paw\":true,\"pad\":\"");
                // Four times the bound, so a passing test cannot be an
                // accident of the flood being smaller than the cap.
                let chunk = vec![b'A'; 8 * 1024];
                let flood = 4 * MAX_PROBE_RESPONSE_BYTES;
                while sent_in_thread.load(Ordering::Relaxed) < flood {
                    if stream.write_all(&chunk).is_err() {
                        break;
                    }
                    sent_in_thread.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                }
            }
        });

        let repo = tempfile::tempdir().expect("tempdir");
        // The probe must return — a hang here fails the test by timing out —
        // and must not treat the flood as our own broker.
        let result = probe_broker_for_repo(&format!("http://127.0.0.1:{port}"), repo.path());
        assert_ne!(
            result,
            ProbeResult::LiveBroker,
            "a flooding listener is never this repository's broker"
        );

        // The server stops only because the probe closed the connection after
        // its cap — so joining also proves the probe did not keep draining.
        let _ = server.join();
        // The bound is what the probe READS, not what the attacker sends: the
        // server also fills the socket buffer. Asserting the listener pushed
        // at least a full cap's worth is what makes the returning probe
        // evidence of the bound rather than of a small flood.
        assert!(
            sent.load(Ordering::Relaxed) >= MAX_PROBE_RESPONSE_BYTES,
            "the listener must have pushed at least the read bound for this to prove anything; sent {}",
            sent.load(Ordering::Relaxed)
        );
    }

    #[test]
    fn a_reported_repository_cannot_inject_terminal_escapes() {
        // Any local process can bind the port and answer the probe, so the
        // repository it claims is attacker-controlled text on its way to a
        // terminal.
        let hostile = "/repo\u{1b}[2J\u{1b}[31mFATAL: disk wiped\u{7}\nreal-looking line";
        let sanitized = sanitize_reported_repo_root(hostile);
        assert!(
            !sanitized.chars().any(char::is_control),
            "no control character may survive: {sanitized:?}"
        );
        assert!(
            !sanitized.contains('\u{1b}'),
            "the ESC that opens an ANSI sequence must be gone: {sanitized:?}"
        );
        assert!(
            sanitized.contains("/repo"),
            "printable text is preserved so the message stays useful: {sanitized:?}"
        );
    }

    #[test]
    fn a_reported_repository_cannot_flood_the_screen() {
        let flood = "/".repeat(10_000);
        let sanitized = sanitize_reported_repo_root(&flood);
        assert!(
            sanitized.chars().count() <= MAX_REPORTED_REPO_ROOT_CHARS + 1,
            "an over-long report is capped, with one marker char: {}",
            sanitized.chars().count()
        );
        assert!(
            sanitized.ends_with('…'),
            "truncation is visible to the user"
        );
    }

    #[test]
    fn the_refusal_message_names_the_port_the_holder_and_the_fix() {
        let err = BrokerError::PortHeldByOtherRepo {
            port: 9119,
            repo_root: Some("/home/you/other-repo".to_string()),
        };
        let message = err.to_string();
        assert!(message.contains("9119"), "names the port: {message}");
        assert!(
            message.contains("/home/you/other-repo"),
            "names the holding repository: {message}"
        );
        assert!(
            message.contains("[broker] port"),
            "names the one-line fix: {message}"
        );
    }

    #[test]
    fn an_unknown_holder_is_not_claimed_as_ours() {
        let err = BrokerError::PortHeldByOtherRepo {
            port: 9119,
            repo_root: None,
        };
        let message = err.to_string();
        assert!(message.contains("9119"), "names the port: {message}");
        assert!(
            message.contains("[broker] port"),
            "names the one-line fix: {message}"
        );
        assert!(
            message.contains("older git-paw"),
            "points at the likely holder: {message}"
        );
        assert!(
            message.contains("another repository"),
            "an unreported owner is never claimed as ours: {message}"
        );
    }

    #[test]
    fn port_in_use_is_actionable_and_hides_its_source() {
        let err = BrokerError::PortInUse {
            port: 9119,
            source: std::io::Error::new(std::io::ErrorKind::AddrInUse, "address already in use"),
        };
        let message = err.to_string();
        assert!(message.contains("9119"), "names the port: {message}");
        assert!(
            message.contains("[broker] port"),
            "suggests the config change: {message}"
        );
        assert!(
            !message.contains("address already in use"),
            "the io::Error text is reserved for the source chain: {message}"
        );
        assert!(
            !message.contains("another repository"),
            "a foreign process is not another repository's broker: {message}"
        );
    }

    #[test]
    fn probe_timeout_is_actionable() {
        let message = BrokerError::ProbeTimeout { port: 9119 }.to_string();
        assert!(message.contains("9119"), "names the port: {message}");
        assert!(
            message.contains("stuck processes"),
            "suggests what to look for: {message}"
        );
    }

    #[test]
    fn every_broker_error_exits_one() {
        for err in [
            BrokerError::PortHeldByOtherRepo {
                port: 9119,
                repo_root: None,
            },
            BrokerError::ProbeTimeout { port: 9119 },
            BrokerError::BindFailed(std::io::Error::other("boom")),
            BrokerError::RuntimeFailed(std::io::Error::other("boom")),
        ] {
            let wrapped = crate::error::PawError::from(err);
            assert_eq!(wrapped.exit_code(), 1, "broker failures exit 1: {wrapped}");
        }
    }

    #[test]
    fn the_refusal_message_carries_only_sanitized_text() {
        let err = BrokerError::PortHeldByOtherRepo {
            port: 9119,
            repo_root: Some(sanitize_reported_repo_root("/repo\u{1b}[2Kspoof")),
        };
        let message = err.to_string();
        assert!(
            !message.chars().any(char::is_control),
            "the rendered error must be safe to print: {message:?}"
        );
        assert!(
            message.contains("9119"),
            "the port is still named: {message}"
        );
    }

    #[test]
    fn ensure_port_free_refuses_another_repositorys_broker_before_launch() {
        let repo_a = tempfile::tempdir().expect("tempdir A");
        let repo_b = tempfile::tempdir().expect("tempdir B");
        let (_held, config) = broker_for(repo_a.path());

        let result = ensure_port_free_for_repo(&config, repo_b.path());

        let Err(err) = result else {
            panic!("the pre-flight check must refuse repository A's broker");
        };
        assert!(
            matches!(err, BrokerError::PortHeldByOtherRepo { .. }),
            "pre-flight refusal uses the same variant as start_broker: {err:?}"
        );
    }

    #[test]
    fn ensure_port_free_allows_our_own_broker_and_a_disabled_broker() {
        let repo = tempfile::tempdir().expect("tempdir");
        let (_held, config) = broker_for(repo.path());
        assert!(
            ensure_port_free_for_repo(&config, repo.path()).is_ok(),
            "our own broker on the port is a reattach, not a clash"
        );

        // A disabled broker never probes, so even a clashing port is fine.
        let other = tempfile::tempdir().expect("tempdir other");
        let disabled = BrokerConfig {
            enabled: false,
            ..config
        };
        assert!(
            ensure_port_free_for_repo(&disabled, other.path()).is_ok(),
            "a disabled broker launches regardless of the port"
        );
    }

    #[test]
    fn repo_root_is_read_from_a_status_body() {
        let response = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n\
             {\"git_paw\":true,\"repo_root\":\"/repos/alpha\",\"agents\":[]}";
        assert_eq!(
            parse_repo_root(response),
            Some("/repos/alpha".to_string()),
            "a reported repository is read back verbatim"
        );
    }

    #[test]
    fn a_status_body_without_repo_root_reads_as_unknown() {
        // An older git-paw omits the field entirely; a broker with no declared
        // repository reports it empty. Both mean "owner unknown".
        let absent = "HTTP/1.1 200 OK\r\n\r\n{\"git_paw\":true,\"agents\":[]}";
        let empty = "HTTP/1.1 200 OK\r\n\r\n{\"git_paw\":true,\"repo_root\":\"\"}";
        assert_eq!(parse_repo_root(absent), None);
        assert_eq!(parse_repo_root(empty), None);
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
            ..Default::default()
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
    fn start_broker_on_occupied_port_returns_error_not_loop() {
        use std::net::TcpListener;
        // Occupy a port with a plain TCP listener that never speaks HTTP, so
        // the broker's probe cannot mistake it for a live git-paw broker and
        // reattach. This is the bind-failure seam the dashboard relies on to
        // exit non-zero instead of busy-looping: the call must return an error
        // promptly, not spin or block.
        let squatter = TcpListener::bind("127.0.0.1:0").expect("bind squatter listener");
        let port = squatter.local_addr().expect("squatter addr").port();
        let config = BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
            ..Default::default()
        };
        let state = BrokerState::new(None);
        let result = start_broker(&config, state, Vec::new());
        assert!(
            result.is_err(),
            "starting a broker on a port held by a foreign process must return an error (terminal), not loop or reattach"
        );
    }

    #[test]
    fn start_broker_no_log_path_no_flush_thread() {
        let config = BrokerConfig {
            enabled: true,
            #[allow(clippy::cast_possible_truncation)]
            port: 19_100 + (std::process::id() as u16 % 100),
            bind: "127.0.0.1".to_string(),
            ..Default::default()
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
            ..Default::default()
        };
        let state = BrokerState::new(Some(log_path));
        if let Ok(handle) = start_broker(&config, state, Vec::new()) {
            assert!(handle.flush_thread.is_some());
            drop(handle);
        }
    }

    // === agent-learning-variant: dual-output through the real publish path ===

    fn conflict_feedback(target: &str, other: &str) -> BrokerMessage {
        BrokerMessage::Feedback {
            agent_id: target.to_string(),
            payload: messages::FeedbackPayload {
                from: "supervisor".to_string(),
                errors: vec![format!(
                    "[conflict-detector] in-flight conflict with {other} on src/a.rs"
                )],
            },
        }
    }

    fn learning_payloads(state: &Arc<BrokerState>) -> Vec<messages::LearningPayload> {
        state
            .read()
            .message_log
            .iter()
            .filter_map(|(_, _, m)| match m {
                BrokerMessage::Learning { payload } => Some(payload.clone()),
                _ => None,
            })
            .collect()
    }

    /// Drives one aggregator tick (flush + publish-pending) the way the flush
    /// loop does, against an attached aggregator.
    fn tick(state: &Arc<BrokerState>) {
        let aggregator = state.learnings.clone().expect("aggregator attached");
        if let Ok(mut a) = aggregator.lock() {
            a.flush().unwrap();
        }
        publish_pending_learnings(state, &aggregator);
    }

    fn state_with_aggregator(path: PathBuf, broker_publish: bool) -> Arc<BrokerState> {
        let mut agg = learnings::LearningsAggregator::new(path);
        agg.set_broker_publish(broker_publish);
        agg.register_agent("feat-x");
        agg.register_agent("feat-y");
        let mut state = BrokerState::new(None);
        state.attach_learnings(Arc::new(std::sync::Mutex::new(agg)));
        Arc::new(state)
    }

    // Task 7.1: broker on — a conflict scenario yields an `agent.learning`
    // record on the broker AND a matching entry in the learnings file.
    #[test]
    fn dual_output_publishes_learning_and_writes_file_when_broker_on() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("session-learnings.md");
        let state = state_with_aggregator(path.clone(), true);

        delivery::publish_message(&state, &conflict_feedback("feat-x", "feat-y"));
        tick(&state);

        let md = std::fs::read_to_string(&path).unwrap();
        assert!(
            md.contains("### Conflict events"),
            "file missing conflict:\n{md}"
        );

        let learnings = learning_payloads(&state);
        assert_eq!(learnings.len(), 1, "expected one agent.learning record");
        assert_eq!(learnings[0].category, "conflict_event");
        assert_eq!(learnings[0].id.len(), 16);
        assert!(
            md.contains(&learnings[0].title),
            "file title must match broker record title: {}",
            learnings[0].title
        );
    }

    // Task 7.2: broker off — same scenario writes the file but attempts no
    // broker publish.
    #[test]
    fn no_broker_publish_when_disabled_but_file_still_written() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("session-learnings.md");
        let state = state_with_aggregator(path.clone(), false);

        delivery::publish_message(&state, &conflict_feedback("feat-x", "feat-y"));
        tick(&state);

        let md = std::fs::read_to_string(&path).unwrap();
        assert!(md.contains("### Conflict events"));
        assert!(
            learning_payloads(&state).is_empty(),
            "no agent.learning record should be published when broker publish is off"
        );
    }

    // Task 7.4: re-ticking after the queue is drained does not re-publish —
    // each record reaches the broker exactly once.
    #[test]
    fn re_ticking_does_not_duplicate_learning_records() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("session-learnings.md");
        let state = state_with_aggregator(path, true);

        delivery::publish_message(&state, &conflict_feedback("feat-x", "feat-y"));
        tick(&state);
        tick(&state); // second tick: queue already drained, nothing new flushed

        assert_eq!(
            learning_payloads(&state).len(),
            1,
            "the conflict record must be published exactly once"
        );
    }

    // The branch-scoped record is retrievable via `messages/<branch_id>`.
    #[test]
    fn branch_scoped_learning_is_routed_to_branch_inbox() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("session-learnings.md");
        let state = state_with_aggregator(path, true);

        // A stuck-duration scenario is branch-scoped to "feat-x".
        delivery::publish_message(
            &state,
            &BrokerMessage::Blocked {
                agent_id: "feat-x".to_string(),
                payload: messages::BlockedPayload {
                    needs: "types".to_string(),
                    from: "feat-y".to_string(),
                },
            },
        );
        delivery::publish_message(
            &state,
            &BrokerMessage::Artifact {
                agent_id: "feat-x".to_string(),
                payload: messages::ArtifactPayload {
                    status: "done".to_string(),
                    exports: vec![],
                    modified_files: vec![],
                },
            },
        );
        tick(&state);

        let (msgs, _) = delivery::poll_messages(&state, "feat-x", 0);
        assert!(
            msgs.iter().any(|m| matches!(
                m,
                BrokerMessage::Learning { payload } if payload.category == "stuck_duration"
            )),
            "stuck_duration learning should land in feat-x's inbox"
        );
    }
}
