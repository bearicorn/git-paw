//! Broker runtime-hardening regression tests (`broker-runtime-hardening`).
//!
//! These cover two hazards that only manifest on a real multi-threaded runtime,
//! which the rest of the broker suite cannot see (its tests are single-threaded,
//! router-`oneshot`, and build state with `role_gating = None`):
//!
//! * **H1** — the blocking `git` read the opsx role-gating guard performs for a
//!   `committed` artifact must not run on a tokio worker thread, or a publish
//!   burst saturates the workers and stalls every other endpoint.
//! * **H2** — a panic while a `BrokerState` guard is held poisons the lock; the
//!   broker must keep serving instead of panicking on every later acquisition.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use git_paw::broker::messages::BrokerMessage;
use git_paw::broker::{BrokerState, server};
use git_paw::config::RoleGatingMode;
use git_paw::opsx::RoleGatingContext;

/// How many `committed` artifacts the H1 burst publishes. Each one trips the
/// guard's blocking `git` read, so the burst carries far more blocking work
/// than the runtime has worker threads.
const BURST: usize = 120;

/// Upper bound on `GET /status` while the burst is in flight. Generous by
/// design — the discriminating assertion is that `/status` answers *while* the
/// burst runs, not that it answers quickly.
const STATUS_BOUND: Duration = Duration::from_secs(5);

/// The commit-message shape the role-gating guard classifies as archive
/// activity.
const CANONICAL_ARCHIVE_MSG: &str =
    "chore(specs): archive feat-archiver; sync deltas to main specs";

/// Creates a one-commit git repo standing in for an agent worktree, so the
/// guard's `git log -1` read has something real to run against.
fn init_repo_with_commit(dir: &Path, message: &str, files: &[&str]) {
    let run = |args: &[&str]| {
        let out = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?} failed");
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

/// A `POST /publish` request carrying an `agent.artifact { status: "committed" }`
/// whose `modified_files` touch the `OpenSpec` tree, so the guard runs past its
/// cheap pre-filter and into the blocking `git` read.
fn committed_artifact_request(agent_id: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/publish")
        .header("content-type", "application/json")
        .body(Body::from(format!(
            r#"{{"type":"agent.artifact","agent_id":"{agent_id}","payload":{{"status":"committed","exports":[],"modified_files":["openspec/changes/{agent_id}/tasks.md"]}}}}"#
        )))
        .expect("request builds")
}

fn status_request() -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri("/status")
        .body(Body::empty())
        .expect("request builds")
}

fn role_gating_state(mode: RoleGatingMode, roster: Vec<(String, PathBuf)>) -> Arc<BrokerState> {
    Arc::new(BrokerState::new(None).with_role_gating(RoleGatingContext {
        mode,
        engine_is_openspec: true,
        roster,
    }))
}

/// The first `opsx-role-gating` feedback text addressed to `target`, if any.
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

/// Poisons the broker state lock by panicking while a write guard is held —
/// the exact hazard H2 recovers from. The panic message is expected noise in
/// the test output.
fn poison_state_lock(state: &Arc<BrokerState>) {
    let poisoner = Arc::clone(state);
    let outcome = std::thread::spawn(move || {
        let _guard = poisoner.write();
        panic!("deliberate panic while holding the broker state write guard");
    })
    .join();
    assert!(outcome.is_err(), "the poisoning thread must have panicked");
}

// ---------------------------------------------------------------------------
// H1 — blocking `git` must not run on the async worker threads
// ---------------------------------------------------------------------------

/// Scenario: *A publish burst does not stall other HTTP endpoints.*
///
/// Two worker threads, a burst of `committed` artifacts that each trip the
/// guard's blocking `git` read, and a concurrent `GET /status`. When the
/// blocking work runs on the workers, `/status` cannot be polled until the
/// burst has almost entirely drained, so it finishes alongside the burst.
/// When the work is offloaded, `/status` is served while the burst is still
/// running.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn publish_burst_with_role_gating_does_not_stall_status() {
    let repo = TempDir::new().expect("tempdir");
    init_repo_with_commit(
        repo.path(),
        "feat(burst): an ordinary non-archive commit",
        &["openspec/changes/feat-burst/tasks.md"],
    );

    let state = role_gating_state(
        RoleGatingMode::Warn,
        vec![("feat-burst".to_string(), repo.path().to_path_buf())],
    );
    let app = server::router(state);

    let started = Instant::now();
    let burst: Vec<_> = (0..BURST)
        .map(|_| {
            let app = app.clone();
            tokio::spawn(async move { app.oneshot(committed_artifact_request("feat-burst")).await })
        })
        .collect();
    let status = {
        let app = app.clone();
        tokio::spawn(async move { app.oneshot(status_request()).await })
    };

    let status_resp = tokio::time::timeout(STATUS_BOUND, status)
        .await
        .expect("GET /status must answer while a committed-artifact burst is in flight")
        .expect("the /status task must not panic")
        .expect("the /status request must not error");
    let status_elapsed = started.elapsed();
    assert_eq!(status_resp.status(), StatusCode::OK);

    for handle in burst {
        let resp = handle
            .await
            .expect("publish task must not panic")
            .expect("publish request must not error");
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }
    let burst_elapsed = started.elapsed();

    assert!(
        status_elapsed * 2 < burst_elapsed,
        "GET /status answered after {status_elapsed:?} of the burst's {burst_elapsed:?} — it \
         waited for the role-gating git reads instead of being served while they ran"
    );
}

/// Scenario: *Guard behavior is unchanged by the offload.*
///
/// Published over the real `POST /publish` path (the handler the offload
/// changes), a coding agent's archive commit must still produce the guard's
/// feedback to the violator plus, in `block` mode, the revert request routed
/// to the supervisor.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn role_gating_guard_output_survives_the_http_publish_path() {
    let repo = TempDir::new().expect("tempdir");
    init_repo_with_commit(
        repo.path(),
        CANONICAL_ARCHIVE_MSG,
        &["openspec/changes/feat-archiver/tasks.md"],
    );

    let state = role_gating_state(
        RoleGatingMode::Block,
        vec![
            ("feat-archiver".to_string(), repo.path().to_path_buf()),
            (
                "supervisor".to_string(),
                PathBuf::from("/nonexistent-supervisor"),
            ),
        ],
    );

    let resp = server::router(Arc::clone(&state))
        .oneshot(committed_artifact_request("feat-archiver"))
        .await
        .expect("publish request must not error");
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let violator_feedback = role_feedback_text(&state, "feat-archiver")
        .expect("the guard must still warn the committing agent");
    assert!(
        violator_feedback.contains("opsx-role-gating: detected archive activity"),
        "unexpected warning text: {violator_feedback}"
    );

    let revert_request = role_feedback_text(&state, "supervisor")
        .expect("block mode must still route a revert request to the supervisor");
    assert!(
        revert_request.contains("git revert"),
        "unexpected revert text: {revert_request}"
    );

    assert!(
        state.read().message_log.iter().any(|(_, _, m)| {
            matches!(m, BrokerMessage::Learning { payload }
                if payload.category == "permission_pattern")
        }),
        "the guard must still record the permission_pattern learning"
    );
}

// ---------------------------------------------------------------------------
// H2 — a poisoned state lock is recovered, not fatal
// ---------------------------------------------------------------------------

/// Scenario: *A request after a lock-poisoning panic is still served.*
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_after_a_lock_poisoning_panic_is_still_served() {
    let state = Arc::new(BrokerState::new(None));
    poison_state_lock(&state);

    let resp = server::router(state)
        .oneshot(status_request())
        .await
        .expect("the broker must still answer after the lock was poisoned");
    assert_eq!(resp.status(), StatusCode::OK);
}
