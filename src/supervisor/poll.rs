//! Single-tick orchestration for the supervisor auto-approve poll loop.
//!
//! Exposes [`poll_tick`] — given a broker state, a session name, a
//! pane-index resolver, and an [`AutoApproveConfig`], it:
//!
//! 1. Detects stalled agents via [`super::stall::detect_stalled_agents`].
//! 2. Captures each stalled agent's pane via
//!    [`super::permission_prompt::detect_permission_prompt`].
//! 3. For safe-classified prompts, dispatches `BTab Down Enter` via
//!    [`super::approve::auto_approve_pane`].
//! 4. For `Unknown` prompts, forwards a question to the dashboard inbox so
//!    the human can resolve it.
//!
//! The loop driver lives in `main.rs` (background thread spawned by
//! `cmd_supervisor`); this module keeps the per-tick logic pure and
//! testable.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::Deserialize;

use crate::broker::BrokerState;
use crate::config::AutoApproveConfig;
use crate::error::PawError;

use super::approve::{ApprovalRequest, KeyDispatcher, auto_approve_pane};
use super::auto_approve::is_safe_command;
use super::permission_prompt::{PermissionType, detect_permission_prompt};
use super::stall::detect_stalled_agents;

/// Outcome of processing a single stalled agent during a poll tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TickOutcome {
    /// No permission prompt was found in the pane.
    NoPrompt,
    /// Prompt was detected, classified safe, and approved.
    Approved {
        /// Whitelist entry that matched the captured command.
        matched_entry: String,
        /// Permission class of the approved prompt.
        kind: PermissionType,
    },
    /// Prompt was detected but did not match the whitelist; the supervisor
    /// should forward it to the dashboard.
    Forwarded {
        /// Permission class of the forwarded prompt.
        kind: PermissionType,
    },
}

/// Trait providing the pane-index for a given agent ID.
///
/// `cmd_supervisor` knows the mapping from session state; tests substitute
/// a closure-backed implementation.
pub trait PaneResolver {
    /// Returns the tmux pane index for `agent_id`, or `None` if the agent
    /// has no pane (e.g. the supervisor itself).
    fn pane_index_for(&self, agent_id: &str) -> Option<usize>;
}

impl<F> PaneResolver for F
where
    F: Fn(&str) -> Option<usize>,
{
    fn pane_index_for(&self, agent_id: &str) -> Option<usize> {
        self(agent_id)
    }
}

/// Trait providing the captured pane content for an agent.
///
/// In production this is a thin shim over [`super::permission_prompt::capture_pane`].
/// Tests inject a stub so the captured text is deterministic.
pub trait PaneInspector {
    /// Captures the pane and returns the classification, or `None` when
    /// no approval marker is present.
    fn inspect(&self, session: &str, pane_index: usize) -> Option<PermissionType>;
    /// Returns the raw captured content for whitelist matching, or empty
    /// string when capture fails.
    fn captured_text(&self, session: &str, pane_index: usize) -> String;
}

/// Production [`PaneInspector`] backed by `tmux capture-pane`.
pub struct TmuxPaneInspector;

impl PaneInspector for TmuxPaneInspector {
    fn inspect(&self, session: &str, pane_index: usize) -> Option<PermissionType> {
        detect_permission_prompt(session, pane_index)
    }
    fn captured_text(&self, session: &str, pane_index: usize) -> String {
        super::permission_prompt::capture_pane(session, pane_index).unwrap_or_default()
    }
}

/// Forwarder for unsafe prompts — abstracted so tests can record forwards.
pub trait QuestionForwarder {
    /// Forward a question to the supervisor dashboard inbox.
    ///
    /// Returns the dispatch result; failures are logged but do not abort
    /// the poll tick.
    fn forward_question(&mut self, agent_id: &str, kind: PermissionType, captured: &str);
}

/// Inputs for [`poll_tick`].
///
/// Bundled so the per-tick API is one parameter wide and clippy's
/// `too_many_arguments` lint stays happy.
pub struct PollContext<'a, R, I, D, Q>
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
{
    /// Broker state used for stall detection by [`poll_tick`].
    ///
    /// Set to `None` when calling [`tick_from_status`] from a process that
    /// does not own the broker state (e.g. the supervisor's background
    /// poll thread, which queries `/status` over HTTP instead).
    pub state: Option<&'a BrokerState>,
    /// tmux session name.
    pub session: &'a str,
    /// Auto-approve config (presets applied by [`poll_tick`]).
    pub config: &'a AutoApproveConfig,
    /// Resolves agent ID to pane index.
    pub resolver: &'a R,
    /// Inspects pane content.
    pub inspector: &'a I,
    /// Sends approval keystrokes.
    pub dispatcher: &'a mut D,
    /// Forwards unsafe prompts to the dashboard.
    pub forwarder: &'a mut Q,
    /// Optional broker URL for audit-log publishing.
    pub broker_url: Option<&'a str>,
}

/// Runs one tick of the auto-approve poll loop and returns the outcome
/// for each stalled agent (in iteration order).
pub fn poll_tick<R, I, D, Q>(ctx: &mut PollContext<'_, R, I, D, Q>) -> Vec<(String, TickOutcome)>
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
{
    let cfg = ctx.config.resolved();
    if !cfg.enabled {
        return Vec::new();
    }
    let Some(state) = ctx.state else {
        return Vec::new();
    };
    let threshold = Duration::from_secs(cfg.stall_threshold_seconds);
    let stalled = detect_stalled_agents(state, threshold);
    let whitelist = cfg.effective_whitelist();
    drive_outcomes(stalled, ctx, &cfg, &whitelist)
}

/// Subset of an agent record returned by the broker `/status` endpoint that
/// the supervisor poll loop cares about.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentStatusRow {
    /// Agent identifier (slugified branch name).
    pub agent_id: String,
    /// Status label (e.g. `"working"`, `"done"`).
    pub status: String,
    /// Seconds since the agent was last seen.
    pub last_seen_seconds: u64,
}

/// Fetches the broker `/status` endpoint and returns the agent summary.
///
/// Used by `cmd_supervisor`'s background poll thread because the broker
/// state lives in the dashboard process, not in `cmd_supervisor` itself.
/// Errors are surfaced so the caller can decide whether to retry.
pub fn fetch_status_over_http(broker_url: &str) -> Result<Vec<AgentStatusRow>, PawError> {
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

    // Find the JSON body (first `{` after the headers).
    let body_start = response
        .find("\r\n\r\n")
        .map(|i| i + 4)
        .ok_or_else(|| PawError::SessionError("malformed broker response".to_string()))?;
    let body = &response[body_start..];

    let parsed: StatusResponse = serde_json::from_str(body)
        .map_err(|e| PawError::SessionError(format!("broker /status parse error: {e}")))?;
    Ok(parsed.agents)
}

#[derive(Deserialize)]
struct StatusResponse {
    agents: Vec<AgentStatusRow>,
}

/// Returns the IDs of agents whose `status` is non-terminal and whose
/// `last_seen_seconds` is at or above `threshold_seconds`.
///
/// HTTP-friendly counterpart to [`super::stall::detect_stalled_agents`]
/// for callers that only have a `/status` snapshot (the supervisor's
/// background poll thread).
#[must_use]
pub fn stalled_from_status(rows: &[AgentStatusRow], threshold_seconds: u64) -> Vec<String> {
    rows.iter()
        .filter(|r| !super::stall::TERMINAL_STATUSES.contains(&r.status.as_str()))
        .filter(|r| r.last_seen_seconds >= threshold_seconds)
        .map(|r| r.agent_id.clone())
        .collect()
}

/// Runs one tick driven by an HTTP `/status` snapshot rather than an
/// in-process [`BrokerState`].
///
/// Mirrors [`poll_tick`] but takes pre-fetched [`AgentStatusRow`] entries
/// so the supervisor's background thread does not need access to the
/// broker's lock.
pub fn tick_from_status<R, I, D, Q>(
    rows: &[AgentStatusRow],
    ctx: &mut PollContext<'_, R, I, D, Q>,
) -> Vec<(String, TickOutcome)>
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
{
    let cfg = ctx.config.resolved();
    if !cfg.enabled {
        return Vec::new();
    }
    let stalled = stalled_from_status(rows, cfg.stall_threshold_seconds);
    let whitelist = cfg.effective_whitelist();
    drive_outcomes(stalled, ctx, &cfg, &whitelist)
}

fn drive_outcomes<R, I, D, Q>(
    stalled: Vec<String>,
    ctx: &mut PollContext<'_, R, I, D, Q>,
    cfg: &AutoApproveConfig,
    whitelist: &[String],
) -> Vec<(String, TickOutcome)>
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
{
    let mut out = Vec::with_capacity(stalled.len());
    for agent_id in stalled {
        let Some(pane_index) = ctx.resolver.pane_index_for(&agent_id) else {
            continue;
        };
        let Some(kind) = ctx.inspector.inspect(ctx.session, pane_index) else {
            out.push((agent_id, TickOutcome::NoPrompt));
            continue;
        };
        let captured = ctx.inspector.captured_text(ctx.session, pane_index);
        let matched = first_whitelist_match(&captured, whitelist);
        if let Some(entry) = matched {
            let req = ApprovalRequest {
                enabled: cfg.enabled,
                session: ctx.session,
                pane_index,
                agent_id: &agent_id,
                kind,
                matched_entry: Some(entry.as_str()),
                broker_url: ctx.broker_url,
            };
            match auto_approve_pane(ctx.dispatcher, req) {
                Ok(true) => out.push((
                    agent_id,
                    TickOutcome::Approved {
                        matched_entry: entry,
                        kind,
                    },
                )),
                _ => out.push((agent_id, TickOutcome::Forwarded { kind })),
            }
        } else {
            ctx.forwarder.forward_question(&agent_id, kind, &captured);
            out.push((agent_id, TickOutcome::Forwarded { kind }));
        }
    }
    out
}

fn first_whitelist_match(captured: &str, whitelist: &[String]) -> Option<String> {
    // Walk lines so multi-line pane captures only match the actual command
    // being prompted. Using is_safe_command per-line keeps the prefix-
    // boundary semantics intact.
    for line in captured.lines() {
        for entry in whitelist {
            if is_safe_command(line, std::slice::from_ref(entry)) {
                return Some(entry.clone());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::messages::{BrokerMessage, StatusPayload};
    use crate::broker::{AgentRecord, BrokerState};
    use crate::config::AutoApproveConfig;
    use std::cell::RefCell;
    use std::time::Instant;

    struct StubInspector {
        kind: Option<PermissionType>,
        captured: String,
    }
    impl PaneInspector for StubInspector {
        fn inspect(&self, _session: &str, _pane_index: usize) -> Option<PermissionType> {
            self.kind
        }
        fn captured_text(&self, _session: &str, _pane_index: usize) -> String {
            self.captured.clone()
        }
    }

    struct RecordingDispatcher {
        events: Vec<(String, usize, String)>,
    }
    impl KeyDispatcher for RecordingDispatcher {
        fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()> {
            self.events
                .push((session.to_string(), pane_index, key.to_string()));
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingForwarder {
        forwards: RefCell<Vec<(String, PermissionType, String)>>,
    }
    impl QuestionForwarder for RecordingForwarder {
        fn forward_question(&mut self, agent_id: &str, kind: PermissionType, captured: &str) {
            self.forwards
                .borrow_mut()
                .push((agent_id.to_string(), kind, captured.to_string()));
        }
    }

    fn insert_stalled(state: &BrokerState, id: &str, age_secs: u64) {
        let mut inner = state.write();
        inner.agents.insert(
            id.to_string(),
            AgentRecord {
                agent_id: id.to_string(),
                status: "working".to_string(),
                last_seen: Instant::now()
                    .checked_sub(Duration::from_secs(age_secs))
                    .unwrap_or_else(Instant::now),
                last_message: Some(BrokerMessage::Status {
                    agent_id: id.to_string(),
                    payload: StatusPayload {
                        status: "working".to_string(),
                        modified_files: Vec::new(),
                        message: None,
                    },
                }),
            },
        );
    }

    fn run_tick<R: PaneResolver, I: PaneInspector>(
        state: &BrokerState,
        cfg: &AutoApproveConfig,
        resolver: &R,
        inspector: &I,
    ) -> (
        Vec<(String, TickOutcome)>,
        RecordingDispatcher,
        RecordingForwarder,
    ) {
        let mut dispatcher = RecordingDispatcher { events: vec![] };
        let mut forwarder = RecordingForwarder::default();
        let out = {
            let mut ctx = PollContext {
                state: Some(state),
                session: "paw-x",
                config: cfg,
                resolver,
                inspector,
                dispatcher: &mut dispatcher,
                forwarder: &mut forwarder,
                broker_url: None,
            };
            poll_tick(&mut ctx)
        };
        (out, dispatcher, forwarder)
    }

    #[test]
    fn disabled_config_returns_empty() {
        let state = BrokerState::new(None);
        insert_stalled(&state, "stuck", 600);
        let cfg = AutoApproveConfig {
            enabled: false,
            ..AutoApproveConfig::default()
        };
        let resolver = |_id: &str| Some(1);
        let inspector = StubInspector {
            kind: Some(PermissionType::Cargo),
            captured: "cargo test".into(),
        };
        let (out, dispatcher, _) = run_tick(&state, &cfg, &resolver, &inspector);
        assert!(out.is_empty());
        assert!(dispatcher.events.is_empty());
    }

    #[test]
    fn stalled_safe_agent_is_approved() {
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-a", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |id: &str| if id == "agent-a" { Some(2) } else { None };
        let inspector = StubInspector {
            kind: Some(PermissionType::Cargo),
            captured: "cargo test --workspace".into(),
        };
        let (out, dispatcher, forwarder) = run_tick(&state, &cfg, &resolver, &inspector);
        assert_eq!(out.len(), 1);
        let (id, outcome) = &out[0];
        assert_eq!(id, "agent-a");
        match outcome {
            TickOutcome::Approved {
                matched_entry,
                kind,
            } => {
                assert_eq!(matched_entry, "cargo test");
                assert_eq!(*kind, PermissionType::Cargo);
            }
            _ => panic!("expected Approved, got {outcome:?}"),
        }
        // BTab + Down + Enter dispatched in order.
        let keys: Vec<&str> = dispatcher
            .events
            .iter()
            .map(|(_, _, k)| k.as_str())
            .collect();
        assert_eq!(keys, vec!["BTab", "Down", "Enter"]);
        assert!(forwarder.forwards.borrow().is_empty());
    }

    #[test]
    fn stalled_unsafe_agent_is_forwarded_not_approved() {
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-b", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |_id: &str| Some(3);
        let inspector = StubInspector {
            kind: Some(PermissionType::Unknown),
            captured: "rm -rf /tmp/foo\nrequires approval".into(),
        };
        let (out, dispatcher, forwarder) = run_tick(&state, &cfg, &resolver, &inspector);
        assert_eq!(out.len(), 1);
        match &out[0].1 {
            TickOutcome::Forwarded { kind } => assert_eq!(*kind, PermissionType::Unknown),
            other => panic!("expected Forwarded, got {other:?}"),
        }
        assert!(
            dispatcher.events.is_empty(),
            "no keystrokes for unsafe prompt"
        );
        let forwards = forwarder.forwards.borrow();
        assert_eq!(forwards.len(), 1);
        assert_eq!(forwards[0].0, "agent-b");
    }

    #[test]
    fn fresh_agent_is_skipped() {
        let state = BrokerState::new(None);
        insert_stalled(&state, "fresh", 0); // age 0 < 30s threshold
        let cfg = AutoApproveConfig::default();
        let resolver = |_id: &str| Some(1);
        let inspector = StubInspector {
            kind: Some(PermissionType::Cargo),
            captured: "cargo test".into(),
        };
        let (out, dispatcher, _) = run_tick(&state, &cfg, &resolver, &inspector);
        assert!(out.is_empty(), "fresh agent must not be polled");
        assert!(dispatcher.events.is_empty());
    }

    #[test]
    fn no_marker_means_no_prompt_outcome() {
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-c", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |_id: &str| Some(1);
        let inspector = StubInspector {
            kind: None,
            captured: String::new(),
        };
        let (out, dispatcher, _) = run_tick(&state, &cfg, &resolver, &inspector);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, TickOutcome::NoPrompt);
        assert!(dispatcher.events.is_empty());
    }

    // --- stalled_from_status / tick_from_status ---

    fn row(agent_id: &str, status: &str, last_seen_seconds: u64) -> AgentStatusRow {
        AgentStatusRow {
            agent_id: agent_id.to_string(),
            status: status.to_string(),
            last_seen_seconds,
        }
    }

    #[test]
    fn stalled_from_status_filters_by_threshold() {
        let rows = vec![
            row("fresh", "working", 5),
            row("stale", "working", 60),
            row("ancient", "working", 600),
        ];
        let stalled = stalled_from_status(&rows, 30);
        assert!(stalled.contains(&"stale".to_string()));
        assert!(stalled.contains(&"ancient".to_string()));
        assert!(!stalled.contains(&"fresh".to_string()));
    }

    #[test]
    fn stalled_from_status_skips_terminal() {
        let rows = vec![
            row("a", "done", 600),
            row("b", "verified", 600),
            row("c", "blocked", 600),
            row("d", "committed", 600),
            row("e", "working", 600),
        ];
        let stalled = stalled_from_status(&rows, 30);
        assert_eq!(stalled, vec!["e".to_string()]);
    }

    #[test]
    fn tick_from_status_dispatches_safe_prompt() {
        let rows = vec![row("agent-a", "working", 300)];
        let cfg = AutoApproveConfig::default();
        let resolver = |id: &str| if id == "agent-a" { Some(2) } else { None };
        let inspector = StubInspector {
            kind: Some(PermissionType::Cargo),
            captured: "cargo test --workspace".into(),
        };
        let mut dispatcher = RecordingDispatcher { events: vec![] };
        let mut forwarder = RecordingForwarder::default();
        let out = {
            let mut ctx = PollContext {
                state: None,
                session: "paw-x",
                config: &cfg,
                resolver: &resolver,
                inspector: &inspector,
                dispatcher: &mut dispatcher,
                forwarder: &mut forwarder,
                broker_url: None,
            };
            tick_from_status(&rows, &mut ctx)
        };
        assert_eq!(out.len(), 1);
        let keys: Vec<&str> = dispatcher
            .events
            .iter()
            .map(|(_, _, k)| k.as_str())
            .collect();
        assert_eq!(keys, vec!["BTab", "Down", "Enter"]);
    }
}
