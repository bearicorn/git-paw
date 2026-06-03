//! Post-commit role-gating guard for the `OpenSpec` spec engine.
//!
//! When a worktree commit lands, the broker runs this guard against the
//! `agent.artifact { status: "committed" }` event. The guard:
//!
//! 1. fast-fails unless the commit touched `openspec/changes/` or
//!    `openspec/specs/` (cheap pre-filter before the heavier heuristic);
//! 2. classifies the commit as archive activity via [`classify_commit`]
//!    (commit-message match OR diff-shape signal);
//! 3. attributes the commit to an agent via [`resolve_agent_id`]
//!    (`"supervisor"` is the only non-violating role);
//! 4. publishes feedback / learning per the configured
//!    [`RoleGatingMode`].
//!
//! The detection heuristic is deliberately conservative — a false positive
//! on the supervisor's own archive is cleared by the attribution check
//! (step 3), and the warning text names the trigger so the user can spot
//! a genuine false positive at a glance. See the change's `design.md` D1–D7.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

use regex::Regex;

use crate::broker::learnings::{CATEGORY_PERMISSION_PATTERN, LearningRecord};
use crate::broker::messages::{ArtifactPayload, BrokerMessage, FeedbackPayload};
use crate::broker::{BrokerState, delivery};
use crate::config::RoleGatingMode;

/// The agent id that owns verification and archival. Any other id committing
/// archive activity is a role-gating violation. Established by the v0.5.0
/// supervisor-as-pane work.
pub const SUPERVISOR_AGENT_ID: &str = "supervisor";

/// The `from` / sender label stamped on guard-published messages so the user
/// (and the offending agent's LLM) can see the warning came from the guard.
pub const ROLE_GUARD_SENDER: &str = "opsx-role-gating";

/// Result of classifying a commit against the archive-activity heuristic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    /// The commit resembles an archive operation. `reason` names the signal(s)
    /// that triggered classification (for diagnosability — D7).
    Archive {
        /// Human-readable explanation of which signal(s) fired, including the
        /// matched message subject and/or detected path.
        reason: String,
    },
    /// The commit does not resemble an archive operation.
    NotArchive,
}

/// The changed-path view of a commit's diff used by [`classify_commit`].
///
/// Built from the `agent.artifact` payload's `modified_files` (which the
/// post-commit hook derives from `git diff HEAD~1 --name-only`), so the
/// archive-destination and main-spec paths appear without an extra git read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitDiff {
    /// Paths touched by the commit (added, modified, and rename destinations).
    pub touched_paths: Vec<String>,
}

impl CommitDiff {
    /// Builds a [`CommitDiff`] from a slice of changed paths.
    #[must_use]
    pub fn from_paths(paths: &[String]) -> Self {
        Self {
            touched_paths: paths.to_vec(),
        }
    }
}

/// Attribution of a commit to the role that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentAttribution {
    /// The supervisor (`agent_id == "supervisor"`). Never a violation.
    Supervisor,
    /// A coding agent, named by its `agent_id`.
    Coding(String),
    /// The worktree could not be resolved to a session agent. Treated as a
    /// violation (conservative default — D2).
    Unknown,
}

impl AgentAttribution {
    /// Whether this attribution counts as a role-gating violation.
    ///
    /// Everything except [`Self::Supervisor`] is a violation — including
    /// [`Self::Unknown`], per the conservative default.
    #[must_use]
    pub fn is_violation(&self) -> bool {
        !matches!(self, AgentAttribution::Supervisor)
    }
}

/// Per-session role-gating context threaded into the broker state.
///
/// Built at broker start from the resolved spec engine, the configured mode,
/// and the session's worktree roster (each coding agent's
/// `agent_id -> worktree_path`, plus `("supervisor", repo_root)`).
#[derive(Debug, Clone)]
pub struct RoleGatingContext {
    /// The configured enforcement mode.
    pub mode: RoleGatingMode,
    /// Whether the session's resolved spec engine is `OpenSpec`. The guard is
    /// inert when this is `false`.
    pub engine_is_openspec: bool,
    /// `(agent_id, worktree_path)` pairs for every committing role, including
    /// the supervisor mapped to the repo root.
    pub roster: Vec<(String, PathBuf)>,
}

impl RoleGatingContext {
    /// Returns the worktree path registered for `agent_id`, if any.
    #[must_use]
    pub fn worktree_for(&self, agent_id: &str) -> Option<&Path> {
        self.roster
            .iter()
            .find(|(id, _)| id == agent_id)
            .map(|(_, p)| p.as_path())
    }

    /// Whether the guard should run at all (active mode + `OpenSpec` engine).
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.engine_is_openspec && self.mode != RoleGatingMode::Off
    }
}

/// The canonical archive commit-message pattern (D1 signal 1): the convention
/// emitted by the v0.5.0+ release/archive procedure.
fn archive_message_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^chore\(specs\): archive [a-z0-9-]+; sync deltas to main specs$")
            .expect("archive message regex compiles")
    })
}

/// Matches a path that moves a change into `openspec/changes/archive/<name>/`
/// (D1 signal 2, archive-move half).
fn archive_move_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"openspec/changes/archive/[^/]+/").expect("archive move regex compiles")
    })
}

/// Matches an addition/update to a main spec `openspec/specs/<capability>/spec.md`
/// (D1 signal 2, deltas-merged half).
fn spec_addition_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"openspec/specs/[^/]+/spec\.md$").expect("spec addition regex compiles")
    })
}

/// Whether a path touches the `OpenSpec` change/spec trees — the cheap
/// pre-filter that gates the heavier heuristic (task 5.3).
#[must_use]
pub fn touches_openspec(path: &str) -> bool {
    path.contains("openspec/changes/") || path.contains("openspec/specs/")
}

/// Classifies a commit as archive activity or not.
///
/// A commit is archive activity when EITHER the commit message's subject line
/// matches the canonical archive pattern OR the diff moves files into
/// `openspec/changes/archive/<name>/` and/or adds/updates a main spec under
/// `openspec/specs/<capability>/spec.md`. The returned `reason` names every
/// signal that fired (including the matched subject and/or detected path) so
/// the user can diagnose a false positive.
#[must_use]
pub fn classify_commit(commit_message: &str, diff: &CommitDiff) -> Classification {
    let mut reasons: Vec<String> = Vec::new();

    let subject = commit_message.lines().next().unwrap_or("").trim();
    if !subject.is_empty() && archive_message_re().is_match(subject) {
        reasons.push(format!(
            "commit message matched the archive heuristic (\"{subject}\")"
        ));
    }

    if let Some(path) = diff
        .touched_paths
        .iter()
        .find(|p| archive_move_re().is_match(p))
    {
        reasons.push(format!(
            "diff moved files into openspec/changes/archive/ ({path})"
        ));
    }

    if let Some(path) = diff
        .touched_paths
        .iter()
        .find(|p| spec_addition_re().is_match(p))
    {
        reasons.push(format!("diff added/updated a main spec ({path})"));
    }

    if reasons.is_empty() {
        Classification::NotArchive
    } else {
        Classification::Archive {
            reason: reasons.join("; "),
        }
    }
}

/// Resolves the committing role for `worktree` against the session roster.
///
/// The roster is `(agent_id, worktree_path)` pairs. A worktree matching the
/// supervisor's entry resolves to [`AgentAttribution::Supervisor`]; any other
/// match resolves to [`AgentAttribution::Coding`]; an unmatched worktree
/// resolves to [`AgentAttribution::Unknown`] (treated as a violation).
#[must_use]
pub fn resolve_agent_id(worktree: &Path, roster: &[(String, PathBuf)]) -> AgentAttribution {
    for (agent_id, path) in roster {
        if paths_match(worktree, path) {
            return if agent_id == SUPERVISOR_AGENT_ID {
                AgentAttribution::Supervisor
            } else {
                AgentAttribution::Coding(agent_id.clone())
            };
        }
    }
    AgentAttribution::Unknown
}

/// Path equality that tolerates symlinked roots (e.g. macOS `/var` ->
/// `/private/var`) by falling back to canonicalisation. Direct equality is
/// tried first so non-existent test paths still compare correctly.
fn paths_match(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    matches!((a.canonicalize(), b.canonicalize()), (Ok(ca), Ok(cb)) if ca == cb)
}

/// Builds the diagnosable warning text published to the offending agent (D7,
/// task 6.4): short SHA + `agent_id` + trigger reason.
#[must_use]
pub fn warning_text(short_sha: &str, agent_id: &str, reason: &str) -> String {
    format!(
        "opsx-role-gating: detected archive activity on commit {short_sha} by agent {agent_id} \
         (not the supervisor).\n  Reason: {reason}.\n  `/opsx:verify` and `/opsx:archive` are \
         supervisor-only — the supervisor verifies and archives changes after merge. Do not run \
         them (or `openspec archive`) from a coding-agent worktree."
    )
}

/// Builds the revert-request text published to the supervisor in `block` mode.
#[must_use]
pub fn revert_request_text(short_sha: &str, agent_id: &str, reason: &str) -> String {
    format!(
        "opsx-role-gating (block mode): coding agent {agent_id} committed an OpenSpec archive \
         ({short_sha}) — this is supervisor-only. Per your merge-orchestration revert flow, \
         confirm with the user (unless `[supervisor] auto_revert = true`), then run \
         `git revert {short_sha}` and send the agent an `agent.feedback` explaining the revert. \
         Trigger: {reason}."
    )
}

/// Runs the role-gating guard for one `agent.artifact { status: "committed" }`
/// event. Called from [`crate::broker::delivery::publish_message`] after the
/// write lock is released, mirroring the verify-on-commit nudge path.
///
/// No-ops unless the context is active (`OpenSpec` engine + non-`off` mode) and
/// the commit both touches the `OpenSpec` trees and classifies as archive
/// activity by a non-supervisor agent.
pub fn run_guard(
    state: &Arc<BrokerState>,
    agent_id: &str,
    payload: &ArtifactPayload,
    ctx: &RoleGatingContext,
) {
    if !ctx.is_active() {
        return;
    }

    // Fast-fail before the heavier heuristic (task 5.3).
    if !payload.modified_files.iter().any(|p| touches_openspec(p)) {
        return;
    }

    // Resolve the committing worktree so we can read the commit message + SHA.
    // An unresolved worktree leaves the SHA/message blank but still classifies
    // from the diff shape (the modified-files list) — the conservative path.
    let worktree = ctx.worktree_for(agent_id).map(Path::to_path_buf);
    let (short_sha, message) = worktree
        .as_deref()
        .and_then(head_commit_info)
        .unwrap_or_else(|| ("unknown".to_string(), String::new()));

    let diff = CommitDiff::from_paths(&payload.modified_files);
    let reason = match classify_commit(&message, &diff) {
        Classification::Archive { reason } => reason,
        Classification::NotArchive => return,
    };

    let attribution = match worktree.as_deref() {
        Some(wt) => resolve_agent_id(wt, &ctx.roster),
        None => AgentAttribution::Unknown,
    };
    if !attribution.is_violation() {
        // The supervisor's own archive trips the heuristic but clears here.
        return;
    }

    // Warn-mode actions: feedback to the violator + a permission_pattern
    // learning (always performed for warn AND block).
    publish_warn(state, agent_id, &short_sha, &reason);

    // Block-mode adds a revert request routed to the supervisor.
    if ctx.mode == RoleGatingMode::Block {
        let revert = revert_request_text(&short_sha, agent_id, &reason);
        delivery::publish_message(
            state,
            &BrokerMessage::Feedback {
                agent_id: SUPERVISOR_AGENT_ID.to_string(),
                payload: FeedbackPayload {
                    from: ROLE_GUARD_SENDER.to_string(),
                    errors: vec![revert],
                },
            },
        );
    }
}

/// Publishes the warn-mode outputs: an `agent.feedback` to the violator and an
/// `agent.learning` with category `permission_pattern`.
fn publish_warn(state: &Arc<BrokerState>, violator: &str, short_sha: &str, reason: &str) {
    let warning = warning_text(short_sha, violator, reason);
    delivery::publish_message(
        state,
        &BrokerMessage::Feedback {
            agent_id: violator.to_string(),
            payload: FeedbackPayload {
                from: ROLE_GUARD_SENDER.to_string(),
                errors: vec![warning.clone()],
            },
        },
    );

    let record = LearningRecord {
        category: CATEGORY_PERMISSION_PATTERN.to_string(),
        agent_id: violator.to_string(),
        // Cross-cutting permission pattern → not branch-scoped (routes to the
        // supervisor inbox + the learnings file the user reads).
        branch_id: None,
        title: format!("opsx role-gating violation: {violator} ran an archive ({short_sha})"),
        body: serde_json::json!({
            "rule": "opsx-role-gating",
            "agent_id": violator,
            "commit": short_sha,
            "reason": reason,
        }),
        timestamp: SystemTime::now(),
    };
    delivery::publish_message(state, &BrokerMessage::from(&record));
}

/// Best-effort read of `(short_sha, full_message)` for the worktree's `HEAD`
/// commit. Returns `None` on any git failure (the caller then treats the SHA
/// as `"unknown"` and classifies from the diff shape alone).
fn head_commit_info(worktree: &Path) -> Option<(String, String)> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["log", "-1", "--pretty=format:%h%n%B"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.splitn(2, '\n');
    let short_sha = parts.next()?.trim().to_string();
    let message = parts.next().unwrap_or("").to_string();
    Some((short_sha, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff(paths: &[&str]) -> CommitDiff {
        CommitDiff {
            touched_paths: paths.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    // --- classify_commit (task 3.5) ---

    #[test]
    fn canonical_archive_message_is_classified() {
        let result = classify_commit(
            "chore(specs): archive opsx-role-gating; sync deltas to main specs",
            &CommitDiff::default(),
        );
        match result {
            Classification::Archive { reason } => {
                assert!(reason.contains("commit message matched"), "got: {reason}");
                assert!(reason.contains("opsx-role-gating"), "got: {reason}");
            }
            Classification::NotArchive => panic!("expected Archive"),
        }
    }

    #[test]
    fn archive_message_with_body_matches_on_subject_line() {
        let msg =
            "chore(specs): archive add-auth; sync deltas to main specs\n\nLonger body text here.";
        assert!(matches!(
            classify_commit(msg, &CommitDiff::default()),
            Classification::Archive { .. }
        ));
    }

    #[test]
    fn non_canonical_message_with_archive_move_diff_is_classified() {
        let result = classify_commit(
            "chore: tidy up changes",
            &diff(&[
                "openspec/changes/feat-x/proposal.md",
                "openspec/changes/archive/feat-x/proposal.md",
            ]),
        );
        match result {
            Classification::Archive { reason } => {
                assert!(
                    reason.contains("openspec/changes/archive/"),
                    "got: {reason}"
                );
                assert!(!reason.contains("commit message matched"), "got: {reason}");
            }
            Classification::NotArchive => panic!("expected Archive via diff shape"),
        }
    }

    #[test]
    fn spec_addition_diff_is_classified() {
        let result = classify_commit(
            "docs: sync",
            &diff(&["openspec/specs/some-capability/spec.md"]),
        );
        match result {
            Classification::Archive { reason } => {
                assert!(reason.contains("main spec"), "got: {reason}");
                assert!(reason.contains("some-capability/spec.md"), "got: {reason}");
            }
            Classification::NotArchive => panic!("expected Archive via spec addition"),
        }
    }

    #[test]
    fn both_signals_combine_into_one_reason() {
        let result = classify_commit(
            "chore(specs): archive feat-x; sync deltas to main specs",
            &diff(&[
                "openspec/changes/archive/feat-x/tasks.md",
                "openspec/specs/feat-x/spec.md",
            ]),
        );
        match result {
            Classification::Archive { reason } => {
                assert!(reason.contains("commit message matched"), "got: {reason}");
                assert!(
                    reason.contains("openspec/changes/archive/"),
                    "got: {reason}"
                );
                assert!(reason.contains("main spec"), "got: {reason}");
                assert!(
                    reason.contains(';'),
                    "combined reason joins signals: {reason}"
                );
            }
            Classification::NotArchive => panic!("expected Archive"),
        }
    }

    #[test]
    fn neither_signal_is_not_archive() {
        let result = classify_commit(
            "feat(broker): add a new endpoint",
            &diff(&["src/broker/server.rs", "openspec/changes/feat-x/tasks.md"]),
        );
        assert_eq!(result, Classification::NotArchive);
    }

    #[test]
    fn archive_word_in_a_normal_message_does_not_match() {
        // Only the exact canonical shape matches; a stray "archive" mention
        // (without the diff shape) is NotArchive.
        let result = classify_commit(
            "feat: archive old logs to cold storage",
            &CommitDiff::default(),
        );
        assert_eq!(result, Classification::NotArchive);
    }

    // --- resolve_agent_id (task 4.3) ---

    fn roster() -> Vec<(String, PathBuf)> {
        vec![
            ("supervisor".to_string(), PathBuf::from("/repo")),
            (
                "feat-x".to_string(),
                PathBuf::from("/repo/.worktrees/feat-x"),
            ),
            (
                "feat-y".to_string(),
                PathBuf::from("/repo/.worktrees/feat-y"),
            ),
        ]
    }

    #[test]
    fn resolve_supervisor_worktree() {
        let r = roster();
        assert_eq!(
            resolve_agent_id(Path::new("/repo"), &r),
            AgentAttribution::Supervisor
        );
        assert!(!AgentAttribution::Supervisor.is_violation());
    }

    #[test]
    fn resolve_coding_worktree() {
        let r = roster();
        assert_eq!(
            resolve_agent_id(Path::new("/repo/.worktrees/feat-x"), &r),
            AgentAttribution::Coding("feat-x".to_string())
        );
        assert!(AgentAttribution::Coding("feat-x".to_string()).is_violation());
    }

    #[test]
    fn resolve_unknown_worktree_is_violation() {
        let r = roster();
        assert_eq!(
            resolve_agent_id(Path::new("/somewhere/else"), &r),
            AgentAttribution::Unknown
        );
        assert!(AgentAttribution::Unknown.is_violation());
    }

    // --- diagnosable warning text (Diagnosable warning text requirement) ---

    #[test]
    fn warning_text_names_sha_agent_and_reason() {
        let text = warning_text(
            "abc1234",
            "feat-x",
            "commit message matched the archive heuristic (\"chore(specs): archive feat-x; sync deltas to main specs\")",
        );
        assert!(text.contains("abc1234"));
        assert!(text.contains("feat-x"));
        assert!(text.contains("commit message matched"));
        assert!(text.contains("/opsx:archive"));
    }

    #[test]
    fn revert_request_text_addresses_supervisor_revert_flow() {
        let text = revert_request_text(
            "abc1234",
            "feat-x",
            "diff moved files into openspec/changes/archive/ (x)",
        );
        assert!(text.contains("git revert abc1234"));
        assert!(text.contains("auto_revert"));
        assert!(text.contains("feat-x"));
    }

    // --- context activation ---

    #[test]
    fn context_inactive_when_off_or_non_openspec() {
        let base = RoleGatingContext {
            mode: RoleGatingMode::Warn,
            engine_is_openspec: true,
            roster: vec![],
        };
        assert!(base.is_active());
        assert!(
            !RoleGatingContext {
                mode: RoleGatingMode::Off,
                ..base.clone()
            }
            .is_active()
        );
        assert!(
            !RoleGatingContext {
                engine_is_openspec: false,
                ..base
            }
            .is_active()
        );
    }

    #[test]
    fn touches_openspec_pre_filter() {
        assert!(touches_openspec("openspec/changes/feat-x/tasks.md"));
        assert!(touches_openspec("openspec/specs/cap/spec.md"));
        assert!(!touches_openspec("src/main.rs"));
    }
}
