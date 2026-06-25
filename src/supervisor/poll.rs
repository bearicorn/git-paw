//! Single-tick orchestration for the supervisor auto-approve poll loop.
//!
//! Exposes [`poll_tick`] — given a broker state, a session name, a
//! pane-index resolver, and an [`AutoApproveConfig`], it:
//!
//! 1. Detects stalled agents via [`super::stall::detect_stalled_agents`].
//! 2. Captures each stalled agent's pane via
//!    [`super::permission_prompt::detect_permission_prompt`].
//! 3. For LIVE, safe-classified prompts, selects the option index and
//!    dispatches the approval keystrokes via [`super::approve::auto_approve_pane`].
//! 4. For danger-list matches and `Unknown` prompts, forwards a question to
//!    the supervisor so the human can resolve it.
//!
//! The loop driver lives in `main.rs` (background thread spawned by
//! `cmd_supervisor`); this module keeps the per-tick logic pure and
//! testable.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::broker::BrokerState;
use crate::config::AutoApproveConfig;
use crate::error::PawError;

use super::approve::{ApprovalRequest, KeyDispatcher, auto_approve_pane};
use super::auto_approve::{
    detect_prompt_shape, extract_command_slice, is_dangerous, is_live_prompt, is_safe_command,
    is_scratch_rm, is_worktree_file_op, is_worktree_git_op, select_option_index,
};
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

/// Trait providing the worktree root path for a given agent ID.
///
/// Used by the worktree-file-op classifier (bug 3) to resolve a captured
/// file-operation prompt's target against the agent's worktree boundary.
/// `cmd_supervisor` builds the mapping from session state; tests substitute a
/// closure-backed implementation. Returns `None` for agents without a known
/// worktree (e.g. the supervisor itself), which suppresses file-op
/// auto-approval for that agent.
pub trait WorktreeResolver {
    /// Returns the worktree root for `agent_id`, or `None` when unknown.
    fn worktree_root_for(&self, agent_id: &str) -> Option<PathBuf>;
}

impl<F> WorktreeResolver for F
where
    F: Fn(&str) -> Option<PathBuf>,
{
    fn worktree_root_for(&self, agent_id: &str) -> Option<PathBuf> {
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
pub struct PollContext<'a, R, I, D, Q, W>
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
    W: WorktreeResolver,
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
    /// Resolves agent ID to worktree root for the file-op classifier (bug 3).
    pub worktree_resolver: &'a W,
    /// Optional broker URL for audit-log publishing.
    pub broker_url: Option<&'a str>,
}

/// Runs one tick of the auto-approve poll loop and returns the outcome
/// for each stalled agent (in iteration order).
pub fn poll_tick<R, I, D, Q, W>(
    ctx: &mut PollContext<'_, R, I, D, Q, W>,
) -> Vec<(String, TickOutcome)>
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
    W: WorktreeResolver,
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
pub fn tick_from_status<R, I, D, Q, W>(
    rows: &[AgentStatusRow],
    ctx: &mut PollContext<'_, R, I, D, Q, W>,
) -> Vec<(String, TickOutcome)>
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
    W: WorktreeResolver,
{
    let cfg = ctx.config.resolved();
    if !cfg.enabled {
        return Vec::new();
    }
    let stalled = stalled_from_status(rows, cfg.stall_threshold_seconds);
    let whitelist = cfg.effective_whitelist();
    drive_outcomes(stalled, ctx, &cfg, &whitelist)
}

fn drive_outcomes<R, I, D, Q, W>(
    stalled: Vec<String>,
    ctx: &mut PollContext<'_, R, I, D, Q, W>,
    cfg: &AutoApproveConfig,
    whitelist: &[String],
) -> Vec<(String, TickOutcome)>
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
    W: WorktreeResolver,
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
        // Classify against the prompted COMMAND slice (text between the
        // `Bash command` / `Bash(` header and the confirmation question), not
        // the surrounding narration. Fall back to the whole capture when no
        // header is present.
        let slice = extract_command_slice(&captured).unwrap_or_else(|| captured.clone());

        // Live-prompt gate: act only when the footer marker `Esc to cancel`
        // is within the last ~4 non-blank lines. A non-live capture (mere
        // narration, or a prompt scrolled away) is never acted on — no
        // keystrokes and no forward — so it cannot trip a phantom approval.
        if !is_live_prompt(&captured) {
            out.push((agent_id, TickOutcome::NoPrompt));
            continue;
        }

        // Option-index selection per the prompt shape and broad-grant rule.
        let option_index = select_option_index(detect_prompt_shape(&captured), &slice);

        // Danger-first precedence: a curated danger-list match is a terminal
        // escalate that overrides any whitelist / safe-by-pattern match.
        if is_dangerous(&slice) {
            ctx.forwarder.forward_question(&agent_id, kind, &captured);
            out.push((agent_id, TickOutcome::Forwarded { kind }));
            continue;
        }

        // Scratch-path exception: an `rm -rf` whose every target is repo/OS
        // scratch classifies safe-by-pattern (the danger-list does not escalate
        // it).
        if is_scratch_rm(&slice) {
            out.push(dispatch_safe(
                ctx,
                &agent_id,
                pane_index,
                cfg.enabled,
                option_index,
                "scratch-rm",
            ));
            continue;
        }

        // Worktree-confined `git add` / `git commit` pre-approval (F2
        // keystone): an unattended agent stages and commits its own work
        // without stalling, since its cwd resolves inside its isolated
        // worktree. `git push` is NOT covered — the danger-list above already
        // escalated it.
        if let Some(root) = ctx.worktree_resolver.worktree_root_for(&agent_id)
            && is_worktree_git_op(&slice, &root)
        {
            out.push(dispatch_safe(
                ctx,
                &agent_id,
                pane_index,
                cfg.enabled,
                option_index,
                "worktree-git",
            ));
            continue;
        }

        // Shell whitelist (read-mostly verbs + configured safe commands),
        // subordinate to the danger-list above.
        if let Some(entry) = first_whitelist_match(&slice, whitelist) {
            out.push(dispatch_safe(
                ctx,
                &agent_id,
                pane_index,
                cfg.enabled,
                option_index,
                &entry,
            ));
            continue;
        }

        // Bug 3: a Claude write / edit / create prompt whose target resolves
        // inside the agent's own worktree is auto-approved when
        // `approve_worktree_writes` is enabled.
        if let Some(root) = ctx.worktree_resolver.worktree_root_for(&agent_id)
            && is_worktree_file_op(&captured, &root, cfg.approve_worktree_writes())
        {
            let req = ApprovalRequest {
                enabled: cfg.enabled,
                session: ctx.session,
                pane_index,
                agent_id: &agent_id,
                kind: PermissionType::WorktreeFileOp,
                matched_entry: Some("worktree-file-op"),
                live_prompt: true,
                option_index,
                broker_url: ctx.broker_url,
            };
            match auto_approve_pane(ctx.dispatcher, req) {
                Ok(true) => out.push((
                    agent_id,
                    TickOutcome::Approved {
                        matched_entry: "worktree-file-op".to_string(),
                        kind: PermissionType::WorktreeFileOp,
                    },
                )),
                _ => out.push((
                    agent_id,
                    TickOutcome::Forwarded {
                        kind: PermissionType::WorktreeFileOp,
                    },
                )),
            }
            continue;
        }

        ctx.forwarder.forward_question(&agent_id, kind, &captured);
        out.push((agent_id, TickOutcome::Forwarded { kind }));
    }
    out
}

/// Dispatches the approval keystrokes for a command the classifier judged
/// known-safe ([`PermissionType::SafeCommand`]) and returns the per-agent
/// outcome. Centralises the request build + dispatch shared by the
/// scratch-rm, worktree-git, and whitelist approval paths.
///
/// The caller has already passed the live-prompt gate, so `live_prompt: true`
/// is set unconditionally here; `auto_approve_pane` re-checks it as a hard
/// precondition.
fn dispatch_safe<R, I, D, Q, W>(
    ctx: &mut PollContext<'_, R, I, D, Q, W>,
    agent_id: &str,
    pane_index: usize,
    enabled: bool,
    option_index: u8,
    matched_entry: &str,
) -> (String, TickOutcome)
where
    R: PaneResolver,
    I: PaneInspector,
    D: KeyDispatcher,
    Q: QuestionForwarder,
    W: WorktreeResolver,
{
    let req = ApprovalRequest {
        enabled,
        session: ctx.session,
        pane_index,
        agent_id,
        kind: PermissionType::SafeCommand,
        matched_entry: Some(matched_entry),
        live_prompt: true,
        option_index,
        broker_url: ctx.broker_url,
    };
    match auto_approve_pane(ctx.dispatcher, req) {
        Ok(true) => (
            agent_id.to_string(),
            TickOutcome::Approved {
                matched_entry: matched_entry.to_string(),
                kind: PermissionType::SafeCommand,
            },
        ),
        _ => (
            agent_id.to_string(),
            TickOutcome::Forwarded {
                kind: PermissionType::SafeCommand,
            },
        ),
    }
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
                        ..Default::default()
                    },
                }),
                last_committed_at: None,
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
        // Default: no worktree mapping (file-op classifier inert).
        let no_worktree = |_id: &str| None::<PathBuf>;
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
                worktree_resolver: &no_worktree,
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
            captured: "cargo test --workspace\nEsc to cancel".into(),
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
                assert_eq!(*kind, PermissionType::SafeCommand);
            }
            _ => panic!("expected Approved, got {outcome:?}"),
        }
        // BTab + Down + Enter dispatched in order.
        let keys: Vec<&str> = dispatcher
            .events
            .iter()
            .map(|(_, _, k)| k.as_str())
            .collect();
        assert_eq!(keys, vec!["1", "Enter"]);
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
            captured: "rm -rf /tmp/foo\nrequires approval\nEsc to cancel".into(),
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

    /// Spec scenario "git push still escalates despite worktree confinement" /
    /// "Danger match overrides a whitelist match": `git push` is on the
    /// danger-list and forwards even though `git push` is whitelisted.
    #[test]
    fn git_push_escalates_despite_whitelist() {
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-p", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |_id: &str| Some(4);
        let inspector = StubInspector {
            kind: Some(PermissionType::Git),
            captured:
                "Bash command\n  git push origin main\nDo you want to proceed?\nEsc to cancel"
                    .into(),
        };
        let (out, dispatcher, forwarder) = run_tick(&state, &cfg, &resolver, &inspector);
        assert_eq!(out.len(), 1);
        assert!(
            matches!(out[0].1, TickOutcome::Forwarded { .. }),
            "git push must escalate, got {:?}",
            out[0].1
        );
        assert!(dispatcher.events.is_empty(), "no keystrokes for danger");
        assert_eq!(forwarder.forwards.borrow().len(), 1);
    }

    /// Spec scenario "Scratch temp delete auto-approves": an `rm -rf` of a
    /// `/tmp/paw-*` scratch dir is approved by the scratch-path exception.
    #[test]
    fn scratch_rm_is_auto_approved() {
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-s", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |_id: &str| Some(5);
        let inspector = StubInspector {
            kind: Some(PermissionType::Unknown),
            captured:
                "Bash command\n  rm -rf /tmp/paw-build-9\nDo you want to proceed?\nEsc to cancel"
                    .into(),
        };
        let (out, dispatcher, forwarder) = run_tick(&state, &cfg, &resolver, &inspector);
        assert_eq!(out.len(), 1);
        match &out[0].1 {
            TickOutcome::Approved {
                matched_entry,
                kind,
            } => {
                assert_eq!(matched_entry, "scratch-rm");
                assert_eq!(*kind, PermissionType::SafeCommand);
            }
            other => panic!("expected Approved scratch-rm, got {other:?}"),
        }
        assert!(!dispatcher.events.is_empty());
        assert!(forwarder.forwards.borrow().is_empty());
    }

    /// Spec scenario "Safe class with non-live prompt does not fire": a safe
    /// command without the `Esc to cancel` footer in the live window is not
    /// acted on — no keystrokes and no forward.
    #[test]
    fn non_live_prompt_yields_no_prompt() {
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-n", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |_id: &str| Some(1);
        let inspector = StubInspector {
            kind: Some(PermissionType::Cargo),
            captured: "I plan to run cargo test soon\njust some narration".into(),
        };
        let (out, dispatcher, forwarder) = run_tick(&state, &cfg, &resolver, &inspector);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].1,
            TickOutcome::NoPrompt,
            "non-live prompt must not fire"
        );
        assert!(dispatcher.events.is_empty(), "no keystrokes for non-live");
        assert!(forwarder.forwards.borrow().is_empty(), "no forward either");
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
            captured: "cargo test --workspace\nEsc to cancel".into(),
        };
        let no_worktree = |_id: &str| None::<PathBuf>;
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
                worktree_resolver: &no_worktree,
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
        assert_eq!(keys, vec!["1", "Enter"]);
    }

    // --- Bug 3: worktree file-op approval through the poll loop ---

    fn run_tick_with_worktree<R, I, Wt>(
        state: &BrokerState,
        cfg: &AutoApproveConfig,
        resolver: &R,
        inspector: &I,
        worktree_resolver: &Wt,
    ) -> (
        Vec<(String, TickOutcome)>,
        RecordingDispatcher,
        RecordingForwarder,
    )
    where
        R: PaneResolver,
        I: PaneInspector,
        Wt: WorktreeResolver,
    {
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
                worktree_resolver,
                broker_url: None,
            };
            poll_tick(&mut ctx)
        };
        (out, dispatcher, forwarder)
    }

    #[test]
    fn in_worktree_file_prompt_is_auto_approved() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-a", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |id: &str| if id == "agent-a" { Some(2) } else { None };
        // A file-write prompt classifies as Unknown by command class, then the
        // worktree classifier promotes it to WorktreeFileOp.
        let inspector = StubInspector {
            kind: Some(PermissionType::Unknown),
            captured: "Do you want to allow this write to Containerfile?\nEsc to cancel".into(),
        };
        let worktree = move |id: &str| {
            if id == "agent-a" {
                Some(root.clone())
            } else {
                None
            }
        };
        let (out, dispatcher, forwarder) =
            run_tick_with_worktree(&state, &cfg, &resolver, &inspector, &worktree);
        assert_eq!(out.len(), 1);
        match &out[0].1 {
            TickOutcome::Approved {
                matched_entry,
                kind,
            } => {
                assert_eq!(matched_entry, "worktree-file-op");
                assert_eq!(*kind, PermissionType::WorktreeFileOp);
            }
            other => panic!("expected Approved worktree-file-op, got {other:?}"),
        }
        let keys: Vec<&str> = dispatcher
            .events
            .iter()
            .map(|(_, _, k)| k.as_str())
            .collect();
        assert_eq!(keys, vec!["1", "Enter"]);
        assert!(forwarder.forwards.borrow().is_empty());
    }

    /// Spec scenario "Worktree git commit auto-approves" exercised through the
    /// poll loop: a worktree-confined `git commit` is approved via the
    /// dedicated worktree-git path (`matched_entry` `worktree-git`).
    #[test]
    fn worktree_git_commit_is_auto_approved() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-g", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |id: &str| if id == "agent-g" { Some(2) } else { None };
        let inspector = StubInspector {
            kind: Some(PermissionType::Git),
            captured:
                "Bash command\n  git commit -m \"feat: x\"\nDo you want to proceed?\nEsc to cancel"
                    .into(),
        };
        let worktree = move |id: &str| {
            if id == "agent-g" {
                Some(root.clone())
            } else {
                None
            }
        };
        let (out, dispatcher, forwarder) =
            run_tick_with_worktree(&state, &cfg, &resolver, &inspector, &worktree);
        assert_eq!(out.len(), 1);
        match &out[0].1 {
            TickOutcome::Approved {
                matched_entry,
                kind,
            } => {
                assert_eq!(matched_entry, "worktree-git");
                assert_eq!(*kind, PermissionType::SafeCommand);
            }
            other => panic!("expected Approved worktree-git, got {other:?}"),
        }
        assert!(!dispatcher.events.is_empty());
        assert!(forwarder.forwards.borrow().is_empty());
    }

    #[test]
    fn out_of_worktree_file_prompt_is_forwarded() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-b", 600);
        let cfg = AutoApproveConfig::default();
        let resolver = |_id: &str| Some(3);
        let inspector = StubInspector {
            kind: Some(PermissionType::Unknown),
            captured: "Do you want to allow this write to /etc/hosts?\nEsc to cancel".into(),
        };
        let worktree = move |_id: &str| Some(root.clone());
        let (out, dispatcher, forwarder) =
            run_tick_with_worktree(&state, &cfg, &resolver, &inspector, &worktree);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0].1, TickOutcome::Forwarded { .. }));
        assert!(
            dispatcher.events.is_empty(),
            "out-of-worktree prompt must not dispatch keystrokes"
        );
        assert_eq!(forwarder.forwards.borrow().len(), 1);
    }

    #[test]
    fn disabled_worktree_writes_forwards_file_prompt() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let state = BrokerState::new(None);
        insert_stalled(&state, "agent-c", 600);
        let cfg = AutoApproveConfig {
            approve_worktree_writes: Some(false),
            ..AutoApproveConfig::default()
        };
        let resolver = |_id: &str| Some(1);
        let inspector = StubInspector {
            kind: Some(PermissionType::Unknown),
            captured: "Do you want to allow this write to Containerfile?\nEsc to cancel".into(),
        };
        let worktree = move |_id: &str| Some(root.clone());
        let (out, dispatcher, _forwarder) =
            run_tick_with_worktree(&state, &cfg, &resolver, &inspector, &worktree);
        assert_eq!(out.len(), 1);
        assert!(
            matches!(out[0].1, TickOutcome::Forwarded { .. }),
            "approve_worktree_writes=false must forward, got {:?}",
            out[0].1
        );
        assert!(dispatcher.events.is_empty());
    }
}
