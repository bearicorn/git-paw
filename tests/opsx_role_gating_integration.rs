//! Integration tests for the `opsx-role-gating` post-commit guard.
//!
//! Drives the real broker publish path (`delivery::publish_message`) against
//! an in-process `BrokerState` carrying a `RoleGatingContext`, over throwaway
//! git repos that stand in for agent worktrees. Each test publishes an
//! `agent.artifact { status: "committed" }` and inspects the broker message
//! log for the guard's feedback / learning output.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::Arc;

use git_paw::broker::messages::{ArtifactPayload, BrokerMessage};
use git_paw::broker::{BrokerState, delivery};
use git_paw::config::RoleGatingMode;
use git_paw::opsx::RoleGatingContext;

/// Create a one-commit git repo whose HEAD message and changed files are the
/// given fixture. Returns the repo path's owner via the caller's `TempDir`.
fn init_repo_with_commit(dir: &Path, message: &str, files: &[&str]) {
    let run = |args: &[&str]| {
        let st = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git runs");
        assert!(st.status.success(), "git {args:?} failed");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    for f in files {
        let p = dir.join(f);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(&p, "x").expect("write");
    }
    run(&["add", "."]);
    run(&["commit", "-q", "-m", message]);
}

fn artifact(agent_id: &str, modified_files: &[&str]) -> BrokerMessage {
    BrokerMessage::Artifact {
        agent_id: agent_id.to_string(),
        payload: ArtifactPayload {
            status: "committed".to_string(),
            exports: vec![],
            modified_files: modified_files.iter().map(|s| (*s).to_string()).collect(),
        },
    }
}

fn state_with(ctx: RoleGatingContext) -> Arc<BrokerState> {
    Arc::new(BrokerState::new(None).with_role_gating(ctx))
}

/// True if the log contains an opsx-role-gating feedback addressed to `target`.
fn has_role_feedback(state: &Arc<BrokerState>, target: &str) -> bool {
    state.read().message_log.iter().any(|(_, _, m)| {
        matches!(m, BrokerMessage::Feedback { agent_id, payload }
            if agent_id == target && payload.from == "opsx-role-gating")
    })
}

/// The first opsx-role-gating feedback text addressed to `target`, if any.
fn role_feedback_text(state: &Arc<BrokerState>, target: &str) -> Option<String> {
    state
        .read()
        .message_log
        .iter()
        .find_map(|(_, _, m)| match m {
            BrokerMessage::Feedback { agent_id, payload }
                if agent_id == target && payload.from == "opsx-role-gating" =>
            {
                payload.errors.first().cloned()
            }
            _ => None,
        })
}

fn has_permission_learning(state: &Arc<BrokerState>) -> bool {
    state.read().message_log.iter().any(|(_, _, m)| {
        matches!(m, BrokerMessage::Learning { payload }
            if payload.category == "permission_pattern")
    })
}

const CANONICAL_MSG: &str = "chore(specs): archive feat-x; sync deltas to main specs";

/// 10.1 — warn mode: a coding-agent archive produces feedback + learning.
#[test]
fn e2e_warn_mode_produces_feedback_and_learning() {
    let wt = tempfile::TempDir::new().unwrap();
    init_repo_with_commit(
        wt.path(),
        CANONICAL_MSG,
        &["openspec/changes/feat-x/tasks.md"],
    );

    let state = state_with(RoleGatingContext {
        mode: RoleGatingMode::Warn,
        engine_is_openspec: true,
        roster: vec![
            ("feat-x".to_string(), wt.path().to_path_buf()),
            (
                "supervisor".to_string(),
                PathBuf::from("/nonexistent-supervisor"),
            ),
        ],
    });

    delivery::publish_message(
        &state,
        &artifact("feat-x", &["openspec/changes/feat-x/tasks.md"]),
    );

    assert!(
        has_role_feedback(&state, "feat-x"),
        "violator gets feedback"
    );
    assert!(
        has_permission_learning(&state),
        "permission_pattern learning recorded"
    );
    assert!(
        !has_role_feedback(&state, "supervisor"),
        "warn mode does not signal a revert"
    );

    // Diagnosable text: short SHA + agent_id + message-match reason.
    let text = role_feedback_text(&state, "feat-x").expect("feedback present");
    assert!(text.contains("feat-x"), "names the agent: {text}");
    assert!(
        text.contains("commit message matched"),
        "names the trigger: {text}"
    );
    assert!(
        !text.contains("unknown"),
        "resolved worktree → real SHA: {text}"
    );
}

/// 10.2 — block mode: feedback to the violator AND a revert request to the
/// supervisor.
#[test]
fn e2e_block_mode_signals_supervisor_revert() {
    let wt = tempfile::TempDir::new().unwrap();
    init_repo_with_commit(
        wt.path(),
        CANONICAL_MSG,
        &["openspec/changes/feat-x/tasks.md"],
    );

    let state = state_with(RoleGatingContext {
        mode: RoleGatingMode::Block,
        engine_is_openspec: true,
        roster: vec![
            ("feat-x".to_string(), wt.path().to_path_buf()),
            (
                "supervisor".to_string(),
                PathBuf::from("/nonexistent-supervisor"),
            ),
        ],
    });

    delivery::publish_message(
        &state,
        &artifact("feat-x", &["openspec/changes/feat-x/tasks.md"]),
    );

    assert!(
        has_role_feedback(&state, "feat-x"),
        "violator still gets feedback"
    );
    assert!(has_permission_learning(&state), "learning still recorded");
    assert!(
        has_role_feedback(&state, "supervisor"),
        "supervisor gets a revert request"
    );
    let revert = role_feedback_text(&state, "supervisor").expect("supervisor feedback");
    assert!(
        revert.contains("git revert"),
        "revert request teaches git revert: {revert}"
    );
}

/// 10.3 — off mode: no broker traffic from the guard.
#[test]
fn e2e_off_mode_is_silent() {
    let wt = tempfile::TempDir::new().unwrap();
    init_repo_with_commit(
        wt.path(),
        CANONICAL_MSG,
        &["openspec/changes/feat-x/tasks.md"],
    );

    let state = state_with(RoleGatingContext {
        mode: RoleGatingMode::Off,
        engine_is_openspec: true,
        roster: vec![("feat-x".to_string(), wt.path().to_path_buf())],
    });

    delivery::publish_message(
        &state,
        &artifact("feat-x", &["openspec/changes/feat-x/tasks.md"]),
    );

    assert!(
        !has_role_feedback(&state, "feat-x"),
        "no feedback in off mode"
    );
    assert!(!has_permission_learning(&state), "no learning in off mode");
}

/// 10.4 — supervisor archive: no guard firing (attribution clears it).
#[test]
fn e2e_supervisor_archive_does_not_fire() {
    let repo = tempfile::TempDir::new().unwrap();
    init_repo_with_commit(
        repo.path(),
        CANONICAL_MSG,
        &["openspec/changes/feat-x/tasks.md"],
    );

    let state = state_with(RoleGatingContext {
        mode: RoleGatingMode::Block,
        engine_is_openspec: true,
        roster: vec![("supervisor".to_string(), repo.path().to_path_buf())],
    });

    // The supervisor publishes the committed artifact under its own agent_id.
    delivery::publish_message(
        &state,
        &artifact("supervisor", &["openspec/changes/feat-x/tasks.md"]),
    );

    assert!(
        !has_role_feedback(&state, "supervisor"),
        "supervisor's own archive is not a violation"
    );
    assert!(
        !has_permission_learning(&state),
        "no learning for supervisor archive"
    );
}

/// 10.5 — diff-shape detection: a non-canonical message but archive-shaped diff
/// still fires.
#[test]
fn e2e_diff_shape_fires_without_canonical_message() {
    let wt = tempfile::TempDir::new().unwrap();
    init_repo_with_commit(
        wt.path(),
        "chore: tidy things up",
        &[
            "openspec/changes/archive/feat-x/proposal.md",
            "openspec/specs/some-cap/spec.md",
        ],
    );

    let state = state_with(RoleGatingContext {
        mode: RoleGatingMode::Warn,
        engine_is_openspec: true,
        roster: vec![("feat-x".to_string(), wt.path().to_path_buf())],
    });

    delivery::publish_message(
        &state,
        &artifact(
            "feat-x",
            &[
                "openspec/changes/archive/feat-x/proposal.md",
                "openspec/specs/some-cap/spec.md",
            ],
        ),
    );

    assert!(
        has_role_feedback(&state, "feat-x"),
        "diff-shape commit fires the guard"
    );
    let text = role_feedback_text(&state, "feat-x").expect("feedback present");
    assert!(
        text.contains("openspec/changes/archive/") || text.contains("main spec"),
        "names the diff-shape signal: {text}"
    );
}

/// 10.6 — unresolvable worktree: treated as a violation (conservative default).
#[test]
fn e2e_unresolvable_worktree_is_a_violation() {
    let state = state_with(RoleGatingContext {
        mode: RoleGatingMode::Warn,
        engine_is_openspec: true,
        // Roster does NOT contain the committing agent.
        roster: vec![("supervisor".to_string(), PathBuf::from("/nonexistent"))],
    });

    delivery::publish_message(
        &state,
        &artifact(
            "feat-ghost",
            &["openspec/changes/archive/feat-ghost/tasks.md"],
        ),
    );

    assert!(
        has_role_feedback(&state, "feat-ghost"),
        "unresolved worktree is a violation"
    );
    assert!(
        has_permission_learning(&state),
        "learning recorded for unresolved worktree"
    );
    let text = role_feedback_text(&state, "feat-ghost").expect("feedback present");
    assert!(
        text.contains("unknown"),
        "SHA is unknown for an unresolved worktree: {text}"
    );
}

/// 1a.4 (guard half) — the guard is inert under a non-OpenSpec engine even in
/// block mode.
#[test]
fn e2e_guard_inert_under_non_openspec_engine() {
    let wt = tempfile::TempDir::new().unwrap();
    init_repo_with_commit(
        wt.path(),
        CANONICAL_MSG,
        &["openspec/changes/feat-x/tasks.md"],
    );

    let state = state_with(RoleGatingContext {
        mode: RoleGatingMode::Block,
        engine_is_openspec: false, // e.g. [specs] type = "markdown"
        roster: vec![("feat-x".to_string(), wt.path().to_path_buf())],
    });

    delivery::publish_message(
        &state,
        &artifact("feat-x", &["openspec/changes/feat-x/tasks.md"]),
    );

    assert!(
        !has_role_feedback(&state, "feat-x"),
        "guard inert under non-OpenSpec engine"
    );
    assert!(
        !has_role_feedback(&state, "supervisor"),
        "no revert request either"
    );
    assert!(
        !has_permission_learning(&state),
        "no learning under non-OpenSpec engine"
    );
}

/// A non-archive commit that touches the `OpenSpec` tree does NOT fire (the
/// fast-fail lets it through but classification clears it).
#[test]
fn e2e_non_archive_openspec_commit_does_not_fire() {
    let wt = tempfile::TempDir::new().unwrap();
    init_repo_with_commit(
        wt.path(),
        "feat(x): implement the thing",
        &["openspec/changes/feat-x/tasks.md", "src/x.rs"],
    );

    let state = state_with(RoleGatingContext {
        mode: RoleGatingMode::Warn,
        engine_is_openspec: true,
        roster: vec![("feat-x".to_string(), wt.path().to_path_buf())],
    });

    delivery::publish_message(
        &state,
        &artifact("feat-x", &["openspec/changes/feat-x/tasks.md", "src/x.rs"]),
    );

    assert!(
        !has_role_feedback(&state, "feat-x"),
        "ordinary change-dir edit is not archive activity"
    );
    assert!(
        !has_permission_learning(&state),
        "no learning for a normal commit"
    );
}
