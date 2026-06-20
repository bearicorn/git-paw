//! Agent inventory + target validation for the supervisor `/agents` and
//! `/tell` routing commands.
//!
//! The inventory is composed from two sources (design D1):
//! - broker `GET /status` — `branch_id`, `status`, `last_seen`, `cli`;
//! - `tmux list-panes` with `pane_current_path` — the live
//!   `branch_id → pane_index` mapping (v0.5.0 doctrine: never assume pane
//!   index ordering; resolve via the worktree path).
//!
//! The join, mode detection, target validation, and the freshness cache are
//! all factored as library functions (design D6) so the v0.6.0 consumer (the
//! `/tell` skill) and future consumers — notably the v1.0.0 MCP write tools'
//! `publish_agent_feedback` — share one inventory + validation shape rather
//! than re-implementing it.

use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::error::PawError;

/// Best-effort detected interaction mode of an agent's CLI pane.
///
/// Detection is heuristic (design D1 / D3): when the pane title and recent
/// capture give no clear signal the mode is [`Mode::Unknown`], and `/tell`
/// treats `unknown` (and `interactive`) as requiring the safe
/// `agent.feedback` delivery path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The CLI is in accept-edits / auto-accept mode — safe for `send-keys`.
    AcceptEdits,
    /// The CLI is in an interactive prompt-per-action mode.
    Interactive,
    /// No clear signal; consumers fall back to the safe delivery mode.
    Unknown,
}

impl Mode {
    /// The kebab-case label used in the rendered inventory and learnings.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AcceptEdits => "accept-edits",
            Self::Interactive => "interactive",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One agent's inventory record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentEntry {
    /// Agent identifier as registered with the broker (slug form, e.g.
    /// `feat-auth`). `/tell` accepts either the slug or the slash form.
    pub branch_id: String,
    /// Status label from the broker (`"working"`, `"done"`, `"blocked"`, …).
    pub status: String,
    /// Seconds since the agent was last seen, per the broker.
    pub last_seen_seconds: u64,
    /// CLI name running in the pane (`"claude"`, …), when known.
    pub cli: Option<String>,
    /// Best-effort detected interaction mode.
    pub mode: Mode,
    /// tmux pane index resolved via `pane_current_path`, when matched.
    pub pane_index: Option<usize>,
}

/// A point-in-time inventory snapshot.
#[derive(Debug, Clone)]
pub struct AgentInventory {
    /// One entry per agent the broker knows about, plus the supervisor row.
    pub entries: Vec<AgentEntry>,
    /// When this snapshot was built — used by [`InventoryCache`] freshness.
    pub refreshed_at: Instant,
}

impl AgentInventory {
    /// Looks up an entry by target identifier, matching either the slug
    /// (`feat-auth`) or slash form (`feat/auth`).
    #[must_use]
    pub fn find(&self, target_id: &str) -> Option<&AgentEntry> {
        let needle = normalize_id(target_id);
        self.entries
            .iter()
            .find(|e| normalize_id(&e.branch_id) == needle)
    }

    /// The candidate target identifiers (every agent except the supervisor),
    /// sorted for deterministic rendering.
    #[must_use]
    pub fn candidate_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.branch_id != "supervisor")
            .map(|e| e.branch_id.clone())
            .collect();
        ids.sort();
        ids
    }
}

/// Normalises an agent identifier for comparison: trims surrounding
/// whitespace and treats the slash form (`feat/auth`) and slug form
/// (`feat-auth`) as equivalent.
fn normalize_id(id: &str) -> String {
    id.trim().replace('/', "-")
}

/// One agent row parsed from the broker `/status` JSON.
///
/// Mirrors the public fields of `broker::AgentStatusEntry`; only the fields
/// the inventory needs are deserialised. Kept `pub(crate)` so the join and
/// its fixtures-based unit tests can construct rows directly.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StatusAgent {
    /// Agent identifier (slug form).
    pub agent_id: String,
    /// Status label.
    #[serde(default)]
    pub status: String,
    /// Seconds since last seen.
    #[serde(default)]
    pub last_seen_seconds: u64,
    /// CLI name; the `/status` endpoint emits an empty string when unknown,
    /// which the join maps to `None`.
    #[serde(default)]
    pub cli: String,
}

#[derive(Debug, Deserialize)]
struct StatusBody {
    #[serde(default)]
    agents: Vec<StatusAgent>,
}

/// Parses the broker `GET /status` JSON body into agent rows.
///
/// # Errors
/// Returns [`PawError::SessionError`] when the body is not valid `/status`
/// JSON.
pub fn parse_status_agents(json: &str) -> Result<Vec<StatusAgent>, PawError> {
    let body: StatusBody = serde_json::from_str(json)
        .map_err(|e| PawError::SessionError(format!("broker /status parse error: {e}")))?;
    Ok(body.agents)
}

/// Parses `tmux list-panes -F '#{pane_index} #{pane_current_path}'` output
/// into `(pane_index, current_path)` pairs.
///
/// Lines that do not start with a numeric pane index are skipped. The path is
/// the remainder of the line (paths never contain the separating space at the
/// front, and may themselves contain spaces).
#[must_use]
pub fn parse_pane_paths(output: &str) -> Vec<(usize, String)> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim_end();
            let (idx, path) = line.split_once(' ')?;
            let idx: usize = idx.trim().parse().ok()?;
            Some((idx, path.to_string()))
        })
        .collect()
}

/// Resolves the pane index for `agent_id` from the `pane_current_path`
/// mapping, per the v0.5.0 doctrine (match on the worktree path, never on
/// index ordering).
///
/// The supervisor occupies pane 0 by construction; every other agent's
/// worktree basename ends in `-<agent_id>` (e.g. `myproj-feat-auth` for
/// `feat-auth`). The `-` prefix on the suffix prevents `feat-a` from matching
/// `…-feat-api`.
#[must_use]
pub fn match_pane(agent_id: &str, pane_paths: &[(usize, String)]) -> Option<usize> {
    if agent_id == "supervisor" {
        return Some(0);
    }
    let suffix = format!("-{agent_id}");
    pane_paths.iter().find_map(|(idx, path)| {
        let base = path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(path);
        (base == agent_id || base.ends_with(&suffix)).then_some(*idx)
    })
}

/// Joins broker status rows with the tmux pane mapping and per-pane detected
/// modes into inventory entries.
///
/// `modes` maps a pane index to its detected [`Mode`]; an agent whose pane is
/// absent from the map (or unmatched) reports [`Mode::Unknown`].
#[must_use]
pub fn join_inventory<S: std::hash::BuildHasher>(
    agents: Vec<StatusAgent>,
    pane_paths: &[(usize, String)],
    modes: &HashMap<usize, Mode, S>,
) -> Vec<AgentEntry> {
    agents
        .into_iter()
        .map(|a| {
            let pane_index = match_pane(&a.agent_id, pane_paths);
            let mode = pane_index
                .and_then(|idx| modes.get(&idx).copied())
                .unwrap_or(Mode::Unknown);
            let cli = if a.cli.trim().is_empty() {
                None
            } else {
                Some(a.cli)
            };
            AgentEntry {
                branch_id: a.agent_id,
                status: a.status,
                last_seen_seconds: a.last_seen_seconds,
                cli,
                mode,
                pane_index,
            }
        })
        .collect()
}

/// Best-effort mode detection from an agent pane's title and recent capture.
///
/// Heuristic (design D1): an explicit accept-edits / bypass-permissions
/// footer marks [`Mode::AcceptEdits`]; a visible interactive prompt marks
/// [`Mode::Interactive`]; anything else is [`Mode::Unknown`]. The signal set
/// is illustrative, not exhaustive — when in doubt the result is `Unknown`
/// and consumers fall back to the safe delivery mode.
#[must_use]
pub fn detect_mode(pane_title: &str, capture: &str) -> Mode {
    let hay = format!("{pane_title}\n{capture}").to_lowercase();
    if hay.contains("accept edits")
        || hay.contains("accept-edits")
        || hay.contains("bypass permissions")
    {
        Mode::AcceptEdits
    } else if hay.contains("? for shortcuts")
        || hay.contains("do you want to proceed")
        || hay.contains("do you want to allow")
        || hay.contains("(y/n)")
        || hay.contains("[y/n]")
        || hay.contains("❯ 1. yes")
    {
        Mode::Interactive
    } else {
        Mode::Unknown
    }
}

/// Rejection returned by [`validate_target`] for an unknown target.
///
/// This is the documented, stable error shape (design D6): every consumer —
/// `/tell` today, the v1.0.0 MCP `publish_agent_feedback` later — surfaces the
/// same `{ target, candidates }` rejection so unknown targets are refused
/// consistently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// The named target is not in the inventory; `candidates` lists the
    /// available agent identifiers.
    UnknownTarget {
        /// The rejected target identifier as typed by the user.
        target: String,
        /// The available agent identifiers (supervisor excluded), sorted.
        candidates: Vec<String>,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTarget { target, candidates } => {
                write!(
                    f,
                    "unknown target `{target}`; available agents: {}",
                    candidates.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validates a `/tell` target against the inventory.
///
/// Returns the matching [`AgentEntry`] (matching either slug or slash form),
/// or a [`ValidationError::UnknownTarget`] carrying the candidate list so the
/// caller can echo the available agents back to the user. This is the shared
/// helper of design D6 — public so consumers outside the supervisor module
/// can reuse the same validation semantics.
///
/// # Errors
/// Returns [`ValidationError::UnknownTarget`] when no entry matches.
pub fn validate_target<'a>(
    inventory: &'a AgentInventory,
    target_id: &str,
) -> Result<&'a AgentEntry, ValidationError> {
    inventory
        .find(target_id)
        .ok_or_else(|| ValidationError::UnknownTarget {
            target: target_id.trim().to_string(),
            candidates: inventory.candidate_ids(),
        })
}

/// In-memory inventory cache with a freshness window (design D2).
///
/// Owned by the supervisor's sweep loop: the sweep stores a fresh snapshot at
/// its cadence, and `/tell` / `/agents` reuse the cached snapshot while it is
/// younger than `max_age`, rebuilding on demand only when stale. There is no
/// on-disk cache — a supervisor restart produces a fresh inventory.
#[derive(Debug)]
pub struct InventoryCache {
    snapshot: Option<AgentInventory>,
    max_age: Duration,
}

impl InventoryCache {
    /// Creates an empty cache with the given freshness window.
    #[must_use]
    pub fn new(max_age: Duration) -> Self {
        Self {
            snapshot: None,
            max_age,
        }
    }

    /// Creates an empty cache with a freshness window in seconds (the
    /// `[supervisor.tell] inventory_max_age_seconds` config value).
    #[must_use]
    pub fn from_seconds(seconds: u64) -> Self {
        Self::new(Duration::from_secs(seconds))
    }

    /// The configured freshness window.
    #[must_use]
    pub fn max_age(&self) -> Duration {
        self.max_age
    }

    /// The cached snapshot, if any.
    #[must_use]
    pub fn snapshot(&self) -> Option<&AgentInventory> {
        self.snapshot.as_ref()
    }

    /// Whether the cache holds a snapshot still within `max_age` as of `now`.
    #[must_use]
    pub fn is_fresh_at(&self, now: Instant) -> bool {
        self.snapshot
            .as_ref()
            .is_some_and(|s| now.duration_since(s.refreshed_at) < self.max_age)
    }

    /// Stores a freshly-built snapshot (called by the sweep loop).
    pub fn store(&mut self, snapshot: AgentInventory) {
        self.snapshot = Some(snapshot);
    }

    /// Returns the cached snapshot when fresh as of `now`; otherwise calls
    /// `refresh` to rebuild it, stores the result, and returns it.
    ///
    /// `refresh` is invoked at most once per stale lookup, so rapid
    /// consecutive `/agents` within the freshness window trigger only one
    /// rebuild (the broker is not re-polled while the snapshot is fresh).
    ///
    /// # Errors
    /// Propagates any error from `refresh`; the previous snapshot (if any) is
    /// left untouched on failure.
    ///
    /// # Panics
    /// Does not panic in practice: when the cache is stale `refresh` populates
    /// the snapshot before it is unwrapped, so the `Some` invariant holds.
    pub fn get_or_refresh<F, E>(&mut self, now: Instant, refresh: F) -> Result<&AgentInventory, E>
    where
        F: FnOnce() -> Result<AgentInventory, E>,
    {
        if !self.is_fresh_at(now) {
            let snapshot = refresh()?;
            self.snapshot = Some(snapshot);
        }
        Ok(self
            .snapshot
            .as_ref()
            .expect("snapshot present after refresh"))
    }
}

/// Builds an inventory snapshot from the live broker and tmux session.
///
/// Polls broker `GET /status`, lists the session's panes with their
/// `pane_current_path`, detects each pane's mode, and joins them into
/// [`AgentEntry`] rows. The snapshot is stamped with the current [`Instant`]
/// for cache freshness.
///
/// # Errors
/// Returns [`PawError`] when the broker is unreachable, returns a non-200
/// status, or emits unparseable `/status` JSON. tmux failures degrade
/// gracefully: a pane listing that fails yields an empty mapping (all
/// `pane_index` become `None`) rather than aborting the inventory.
pub fn build_inventory(broker_url: &str, tmux_session: &str) -> Result<AgentInventory, PawError> {
    let body = fetch_status_body(broker_url)?;
    let agents = parse_status_agents(&body)?;
    let pane_output = list_pane_paths(tmux_session).unwrap_or_default();
    let pane_paths = parse_pane_paths(&pane_output);
    let mut modes = HashMap::new();
    for (idx, _) in &pane_paths {
        modes.insert(*idx, detect_pane_mode(tmux_session, *idx));
    }
    let entries = join_inventory(agents, &pane_paths, &modes);
    Ok(AgentInventory {
        entries,
        refreshed_at: Instant::now(),
    })
}

/// Fetches and parses the broker `GET /status` agent list over HTTP.
///
/// Used by callers outside the dashboard process (e.g. the MCP server) that
/// want a live per-agent snapshot. Returns an error when the broker is
/// unreachable so the caller can degrade gracefully.
pub fn fetch_status_agents_over_http(broker_url: &str) -> Result<Vec<StatusAgent>, PawError> {
    parse_status_agents(&fetch_status_body(broker_url)?)
}

/// Fetches the raw `GET /status` JSON body over a minimal HTTP/1.1 request.
fn fetch_status_body(broker_url: &str) -> Result<String, PawError> {
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

    let request = format!("GET /status HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|e| PawError::SessionError(format!("failed to write status request: {e}")))?;

    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);

    if !(response.starts_with("HTTP/1.1 200") || response.starts_with("HTTP/1.0 200")) {
        return Err(PawError::SessionError(format!(
            "broker /status returned non-200: {}",
            response.lines().next().unwrap_or("<empty>")
        )));
    }

    let body_start = response
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .ok_or_else(|| PawError::SessionError("malformed broker /status response".to_string()))?;
    Ok(response[body_start..].to_string())
}

/// Runs `tmux list-panes` for the session, formatting each line as
/// `<pane_index> <pane_current_path>`.
fn list_pane_paths(session: &str) -> Result<String, PawError> {
    let output = std::process::Command::new("tmux")
        .args([
            "list-panes",
            "-t",
            &format!("{session}:0"),
            "-F",
            "#{pane_index} #{pane_current_path}",
        ])
        .output()
        .map_err(|e| PawError::SessionError(format!("tmux list-panes failed: {e}")))?;
    if !output.status.success() {
        return Err(PawError::SessionError(format!(
            "tmux list-panes exited with {}",
            output.status
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Captures a pane's title and recent content and detects its mode.
fn detect_pane_mode(session: &str, pane_index: usize) -> Mode {
    let title = std::process::Command::new("tmux")
        .args([
            "display-message",
            "-t",
            &format!("{session}:0.{pane_index}"),
            "-p",
            "#{pane_title}",
        ])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let capture =
        crate::supervisor::permission_prompt::capture_pane(session, pane_index).unwrap_or_default();
    detect_mode(&title, &capture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const STATUS_JSON: &str = r#"{
        "git_paw": true,
        "version": "0.6.0",
        "uptime_seconds": 42,
        "agents": [
            {"agent_id": "feat-auth", "cli": "claude", "status": "working", "last_seen_seconds": 3, "summary": ""},
            {"agent_id": "feat-api", "cli": "", "status": "blocked", "last_seen_seconds": 90, "summary": ""},
            {"agent_id": "supervisor", "cli": "claude", "status": "working", "last_seen_seconds": 1, "summary": ""}
        ]
    }"#;

    fn fixture_inventory() -> AgentInventory {
        let agents = parse_status_agents(STATUS_JSON).unwrap();
        // Non-sequential pane mapping: feat-api on a lower index than feat-auth.
        let panes = parse_pane_paths(
            "0 /home/user/myproj\n1 /home/user/myproj-feat-api\n2 /home/user/myproj-feat-auth\n",
        );
        let mut modes = HashMap::new();
        modes.insert(2usize, Mode::AcceptEdits);
        let entries = join_inventory(agents, &panes, &modes);
        AgentInventory {
            entries,
            refreshed_at: Instant::now(),
        }
    }

    #[test]
    fn parse_status_agents_reads_all_rows() {
        let agents = parse_status_agents(STATUS_JSON).unwrap();
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[0].agent_id, "feat-auth");
        assert_eq!(agents[0].cli, "claude");
        assert_eq!(agents[1].last_seen_seconds, 90);
    }

    #[test]
    fn parse_pane_paths_handles_spaces_and_skips_garbage() {
        let panes =
            parse_pane_paths("0 /home/user/my proj\n1 /home/user/wt-feat-x\nnot-a-pane line\n");
        assert_eq!(panes.len(), 2);
        assert_eq!(panes[0], (0, "/home/user/my proj".to_string()));
        assert_eq!(panes[1], (1, "/home/user/wt-feat-x".to_string()));
    }

    #[test]
    fn pane_index_is_path_resolved_not_ordered() {
        let inv = fixture_inventory();
        let api = inv.find("feat-api").unwrap();
        let auth = inv.find("feat-auth").unwrap();
        // Resolution is by worktree path, NOT alphabetical / registration order:
        // feat-api landed on pane 1, feat-auth on pane 2.
        assert_eq!(api.pane_index, Some(1));
        assert_eq!(auth.pane_index, Some(2));
    }

    #[test]
    fn match_pane_does_not_partial_match_prefix() {
        let panes = parse_pane_paths("1 /home/user/proj-feat-api\n");
        // `feat-a` must NOT match `…-feat-api`.
        assert_eq!(match_pane("feat-a", &panes), None);
        assert_eq!(match_pane("feat-api", &panes), Some(1));
    }

    #[test]
    fn supervisor_resolves_to_pane_zero() {
        let inv = fixture_inventory();
        let sup = inv.find("supervisor").unwrap();
        assert_eq!(sup.pane_index, Some(0));
    }

    #[test]
    fn empty_cli_maps_to_none() {
        let inv = fixture_inventory();
        assert_eq!(
            inv.find("feat-auth").unwrap().cli.as_deref(),
            Some("claude")
        );
        assert_eq!(inv.find("feat-api").unwrap().cli, None);
    }

    #[test]
    fn agent_removed_mid_grid_drops_pane_index() {
        // feat-api's pane was removed (middle-grid remove); its broker row
        // lingers until the next sweep but no pane matches → pane_index None.
        let agents = parse_status_agents(STATUS_JSON).unwrap();
        let panes = parse_pane_paths("0 /home/user/myproj\n2 /home/user/myproj-feat-auth\n");
        let entries = join_inventory(agents, &panes, &HashMap::new());
        let inv = AgentInventory {
            entries,
            refreshed_at: Instant::now(),
        };
        assert_eq!(inv.find("feat-api").unwrap().pane_index, None);
        assert_eq!(inv.find("feat-auth").unwrap().pane_index, Some(2));
    }

    #[test]
    fn detect_mode_accept_edits() {
        assert_eq!(
            detect_mode("", "⏵⏵ accept edits on (shift+tab to cycle)"),
            Mode::AcceptEdits
        );
        assert_eq!(
            detect_mode("claude — bypass permissions", ""),
            Mode::AcceptEdits
        );
    }

    #[test]
    fn detect_mode_interactive_prompt() {
        assert_eq!(
            detect_mode("", "Do you want to proceed?\n❯ 1. Yes"),
            Mode::Interactive
        );
    }

    #[test]
    fn detect_mode_unknown_when_no_signal() {
        assert_eq!(
            detect_mode("", "Boondoggling… (esc to interrupt)"),
            Mode::Unknown
        );
    }

    #[test]
    fn unknown_mode_signals_join_to_unknown() {
        let inv = fixture_inventory();
        // feat-api's pane had no detected mode in the modes map → Unknown.
        assert_eq!(inv.find("feat-api").unwrap().mode, Mode::Unknown);
        assert_eq!(inv.find("feat-auth").unwrap().mode, Mode::AcceptEdits);
    }

    #[test]
    fn validate_target_accepts_slug_and_slash_form() {
        let inv = fixture_inventory();
        assert!(validate_target(&inv, "feat-auth").is_ok());
        // slash form resolves to the same slug entry
        assert_eq!(
            validate_target(&inv, "feat/auth").unwrap().branch_id,
            "feat-auth"
        );
    }

    #[test]
    fn validate_target_unknown_returns_candidate_list() {
        let inv = fixture_inventory();
        let err = validate_target(&inv, "feat/ghost").unwrap_err();
        match err {
            ValidationError::UnknownTarget { target, candidates } => {
                assert_eq!(target, "feat/ghost");
                // supervisor excluded; sorted.
                assert_eq!(
                    candidates,
                    vec!["feat-api".to_string(), "feat-auth".to_string()]
                );
            }
        }
    }

    #[test]
    fn validation_error_display_lists_candidates() {
        let err = ValidationError::UnknownTarget {
            target: "feat/ghost".to_string(),
            candidates: vec!["feat/a".to_string(), "feat/b".to_string()],
        };
        let msg = err.to_string();
        assert!(msg.contains("feat/ghost"));
        assert!(msg.contains("feat/a, feat/b"), "got: {msg}");
    }

    // --- InventoryCache (design D2) ---

    fn snapshot_now() -> AgentInventory {
        AgentInventory {
            entries: Vec::new(),
            refreshed_at: Instant::now(),
        }
    }

    #[test]
    fn cache_starts_empty_and_not_fresh() {
        let cache = InventoryCache::from_seconds(60);
        assert!(cache.snapshot().is_none());
        assert!(!cache.is_fresh_at(Instant::now()));
    }

    #[test]
    fn rapid_lookups_within_window_refresh_once() {
        let calls = Cell::new(0u32);
        let mut cache = InventoryCache::from_seconds(60);
        let refresh = || {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(snapshot_now())
        };
        // Two consecutive lookups within the freshness window.
        cache.get_or_refresh(Instant::now(), refresh).unwrap();
        let refresh2 = || {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(snapshot_now())
        };
        cache.get_or_refresh(Instant::now(), refresh2).unwrap();
        assert_eq!(calls.get(), 1, "fresh cache must not re-poll the broker");
    }

    #[test]
    fn stale_snapshot_triggers_refresh() {
        let mut cache = InventoryCache::from_seconds(60);
        // Seed a snapshot timestamped 2min in the past → stale at 60s max-age.
        let stale = AgentInventory {
            entries: Vec::new(),
            refreshed_at: Instant::now()
                .checked_sub(Duration::from_mins(2))
                .expect("instant in range"),
        };
        cache.store(stale);
        assert!(!cache.is_fresh_at(Instant::now()));

        let calls = Cell::new(0u32);
        cache
            .get_or_refresh(Instant::now(), || {
                calls.set(calls.get() + 1);
                Ok::<_, ()>(snapshot_now())
            })
            .unwrap();
        assert_eq!(calls.get(), 1, "stale cache must rebuild");
        assert!(cache.is_fresh_at(Instant::now()));
    }

    // --- build_inventory end-to-end against a fake broker (IO orchestration) ---

    /// Spawns a one-shot HTTP server that answers a single `GET /status` with
    /// `body`, and returns its `http://addr` URL.
    fn spawn_status_server(body: &'static str) -> String {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn build_inventory_against_fake_broker_no_tmux() {
        let url = spawn_status_server(STATUS_JSON);
        // The tmux session does not exist, so list-panes fails and every
        // pane_index degrades to None (except the supervisor's pane-0 rule).
        let inv = build_inventory(&url, "paw-nonexistent-xyz-123").expect("inventory builds");
        assert_eq!(inv.entries.len(), 3);
        assert_eq!(inv.find("feat-auth").unwrap().pane_index, None);
        assert_eq!(inv.find("feat-auth").unwrap().mode, Mode::Unknown);
        assert_eq!(inv.find("supervisor").unwrap().pane_index, Some(0));
    }

    #[test]
    fn build_inventory_unreachable_broker_errors() {
        // Port 1 on loopback refuses immediately → connect error propagates.
        assert!(build_inventory("http://127.0.0.1:1", "x").is_err());
    }

    #[test]
    fn parse_status_agents_rejects_garbage() {
        assert!(parse_status_agents("not json at all").is_err());
    }

    #[test]
    fn detect_pane_mode_helper_on_dead_session_is_unknown() {
        // capture + title both fail for a nonexistent session/pane → Unknown.
        assert_eq!(
            detect_pane_mode("paw-nonexistent-xyz-123", 9),
            Mode::Unknown
        );
    }
}
