//! Git-status polling watcher.
//!
//! Each `WatchTarget` spawns one async task that runs `git status --porcelain`
//! in the worktree every [`POLL_INTERVAL`]. When the set of reported paths
//! differs from the previous tick, the watcher publishes `agent.status` with
//! the current paths in `modified_files`.
//!
//! The watcher inherits git's exclusion rules (`.gitignore`, `.git/` internals)
//! instead of maintaining a hand-rolled filter list.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::messages::{BrokerMessage, StatusPayload};
use super::{BrokerState, WatchTarget, delivery};

/// Interval between `git status --porcelain` polls.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Decides whether the watcher should publish `working` for an observed
/// git-status change, given the agent's current broker status, the time since
/// its last `committed` event (if any), and the configured post-commit
/// re-entry TTL (bug 8).
///
/// - When the agent is **not** in the `committed` state, the watcher publishes
///   `working` exactly as in v0.5.0.
/// - When the agent **is** `committed`:
///   - `ttl == 0` suppresses the publish (committed stays terminal — the
///     v0.5.0 opt-out model).
///   - otherwise the publish fires only when the elapsed time since the
///     committed event is within `ttl`; past the window the agent is
///     considered settled and the watcher suppresses the publish.
#[must_use]
pub fn should_republish_working(
    status: &str,
    since_committed: Option<Duration>,
    ttl: Duration,
) -> bool {
    if status != "committed" {
        return true;
    }
    if ttl.is_zero() {
        return false;
    }
    match since_committed {
        Some(elapsed) => elapsed <= ttl,
        None => false,
    }
}

/// Parses `git status --porcelain` output into a sorted, deduplicated list of paths.
///
/// Each porcelain line looks like `XY PATH` or `XY PATH1 -> PATH2` for renames.
/// For renames, both the source and destination paths are reported.
fn parse_porcelain(stdout: &str) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for line in stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        // Skip the two-character status prefix and the separating space.
        let rest = &line[3..];
        if let Some((from, to)) = rest.split_once(" -> ") {
            paths.push(from.trim().to_string());
            paths.push(to.trim().to_string());
        } else {
            paths.push(rest.trim().to_string());
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

/// Runs `git status --porcelain` in `worktree` and returns the parsed path list.
///
/// Returns `None` when git is unavailable or the command fails — callers treat
/// that as "no change detected this tick" and retry on the next interval.
async fn run_git_status(worktree: &Path) -> Option<Vec<String>> {
    let output = tokio::process::Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(worktree)
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(parse_porcelain(&stdout))
}

/// Watches a single worktree, publishing `agent.status` when git-status output changes.
///
/// The task runs until the broker's shutdown signal fires. Each iteration waits
/// [`POLL_INTERVAL`] and then checks `git status --porcelain`. If the result
/// differs from the previous tick, it publishes a status message.
pub async fn watch_worktree(
    state: Arc<BrokerState>,
    target: WatchTarget,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut previous: Option<Vec<String>> = None;
    let mut ticker = tokio::time::interval(POLL_INTERVAL);
    // Skip the immediate first tick so we wait one interval before the first poll.
    ticker.tick().await;
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    break;
                }
            }
        }

        let Some(current) = run_git_status(&target.worktree_path).await else {
            continue;
        };

        if previous.as_ref() == Some(&current) {
            continue;
        }

        // Skip the very first baseline when the worktree is clean. We only
        // want to announce the agent once it has actual dirty state; otherwise
        // a quiet worktree would publish an empty status on startup with no
        // useful information.
        if previous.is_none() && current.is_empty() {
            previous = Some(current);
            continue;
        }

        // Bug 8: gate the `working` publish against the post-commit re-entry
        // TTL. Read the agent's status, time-since-committed, and the
        // configured TTL under a short read lock (never held across an await).
        let (status, since_committed, ttl) = {
            let inner = state.read();
            let ttl = inner.republish_working_ttl;
            let rec = inner.agents.get(&target.agent_id);
            let status = rec.map(|r| r.status.clone()).unwrap_or_default();
            let since = rec.and_then(|r| r.last_committed_at).map(|t| t.elapsed());
            (status, since, ttl)
        };
        if !should_republish_working(&status, since_committed, ttl) {
            // Agent is settled at `committed`; absorb the change as the new
            // baseline without re-publishing `working`.
            previous = Some(current);
            continue;
        }

        let msg = BrokerMessage::Status {
            agent_id: target.agent_id.clone(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: current.clone(),
                message: None,
                ..Default::default()
            },
        };
        delivery::publish_message(&state, &msg);
        previous = Some(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === Bug 8: post-commit re-entry decision (should_republish_working) ===

    #[test]
    fn non_committed_status_always_publishes() {
        // Normal operation: a working agent publishes regardless of TTL.
        assert!(should_republish_working(
            "working",
            None,
            Duration::from_secs(45)
        ));
        assert!(should_republish_working("idle", None, Duration::ZERO));
    }

    #[test]
    fn committed_within_ttl_republishes() {
        assert!(should_republish_working(
            "committed",
            Some(Duration::from_secs(10)),
            Duration::from_secs(45)
        ));
    }

    #[test]
    fn committed_past_ttl_does_not_republish() {
        assert!(!should_republish_working(
            "committed",
            Some(Duration::from_secs(290)),
            Duration::from_secs(45)
        ));
    }

    #[test]
    fn committed_with_zero_ttl_does_not_republish() {
        // Opt-out: TTL=0 keeps committed terminal even within "0 seconds".
        assert!(!should_republish_working(
            "committed",
            Some(Duration::from_secs(0)),
            Duration::ZERO
        ));
    }

    #[test]
    fn committed_without_timestamp_does_not_republish() {
        assert!(!should_republish_working(
            "committed",
            None,
            Duration::from_secs(45)
        ));
    }

    #[test]
    fn parse_porcelain_handles_modified_and_untracked() {
        let input = " M src/main.rs\n?? new_file.txt\nM  src/lib.rs\n";
        let parsed = parse_porcelain(input);
        assert_eq!(
            parsed,
            vec![
                "new_file.txt".to_string(),
                "src/lib.rs".to_string(),
                "src/main.rs".to_string(),
            ]
        );
    }

    #[test]
    fn parse_porcelain_handles_renames() {
        let input = "R  old.rs -> new.rs\n";
        let parsed = parse_porcelain(input);
        assert_eq!(parsed, vec!["new.rs".to_string(), "old.rs".to_string()]);
    }

    #[test]
    fn parse_porcelain_empty_is_empty_vec() {
        assert!(parse_porcelain("").is_empty());
    }

    #[test]
    fn parse_porcelain_dedupes() {
        let input = " M a.rs\n M a.rs\n";
        let parsed = parse_porcelain(input);
        assert_eq!(parsed, vec!["a.rs".to_string()]);
    }

    fn init_test_repo(dir: &std::path::Path) {
        use std::process::Command;
        let run = |args: &[&str]| {
            Command::new("git")
                .args(args)
                .current_dir(dir)
                .output()
                .expect("git command failed");
        };
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "test"]);
        run(&["commit", "--allow-empty", "-m", "root", "-q"]);
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn run_git_status_detects_new_file() {
        let tmp = tempfile::tempdir().unwrap();
        init_test_repo(tmp.path());
        std::fs::write(tmp.path().join("hello.txt"), "hi").unwrap();
        let result = run_git_status(tmp.path()).await.unwrap();
        assert!(
            result.iter().any(|p| p == "hello.txt"),
            "expected hello.txt in {result:?}"
        );
    }

    /// Spec scenario: multiple writes within the TTL republish `working`
    /// exactly once (the 2s poll coalesces a burst into one tick).
    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn watch_worktree_burst_republishes_working_once() {
        use crate::broker::BrokerState;
        use crate::broker::messages::{ArtifactPayload, BrokerMessage};

        let tmp = tempfile::tempdir().unwrap();
        init_test_repo(tmp.path());

        let state = Arc::new(BrokerState::new(None));
        super::delivery::publish_message(
            &state,
            &BrokerMessage::Artifact {
                agent_id: "feat-b".to_string(),
                payload: ArtifactPayload {
                    status: "committed".to_string(),
                    exports: vec![],
                    modified_files: vec![],
                },
            },
        );

        let (tx, rx) = tokio::sync::watch::channel(false);
        let target = WatchTarget {
            agent_id: "feat-b".to_string(),
            cli: "claude".to_string(),
            worktree_path: tmp.path().to_path_buf(),
        };
        let handle = tokio::spawn(watch_worktree(Arc::clone(&state), target, rx));

        // Burst: ten files written well within one poll interval.
        tokio::time::sleep(Duration::from_millis(300)).await;
        for i in 0..10 {
            std::fs::write(tmp.path().join(format!("f{i}.rs")), "x").unwrap();
        }

        tokio::time::sleep(POLL_INTERVAL + Duration::from_millis(800)).await;

        let working_count = {
            let inner = state.read();
            inner
                .message_log
                .iter()
                .filter(|(_, _, m)| {
                    matches!(m, BrokerMessage::Status { agent_id, payload }
                        if agent_id == "feat-b" && payload.status == "working")
                })
                .count()
        };
        assert_eq!(
            working_count, 1,
            "a burst of writes within one poll interval must republish working exactly once"
        );

        let _ = tx.send(true);
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn run_git_status_respects_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        init_test_repo(tmp.path());
        std::fs::write(tmp.path().join(".gitignore"), "target/\n").unwrap();
        std::fs::create_dir(tmp.path().join("target")).unwrap();
        std::fs::write(tmp.path().join("target").join("build.o"), "x").unwrap();
        let result = run_git_status(tmp.path()).await.unwrap();
        assert!(
            !result.iter().any(|p| p.starts_with("target/")),
            "target/ should be filtered by gitignore, got {result:?}"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn watch_worktree_publishes_on_change() {
        use crate::broker::BrokerState;
        let tmp = tempfile::tempdir().unwrap();
        init_test_repo(tmp.path());

        let state = Arc::new(BrokerState::new(None));
        let (tx, rx) = tokio::sync::watch::channel(false);
        let target = WatchTarget {
            agent_id: "feat-x".to_string(),
            cli: "claude".to_string(),
            worktree_path: tmp.path().to_path_buf(),
        };
        let state_clone = Arc::clone(&state);
        let handle = tokio::spawn(watch_worktree(state_clone, target, rx));

        // Give the first tick a chance, then create a file.
        tokio::time::sleep(Duration::from_millis(300)).await;
        std::fs::write(tmp.path().join("change.txt"), "hello").unwrap();

        // Wait long enough for at least two poll intervals.
        tokio::time::sleep(POLL_INTERVAL + Duration::from_millis(800)).await;

        let msg = {
            let inner = state.read();
            let record = inner
                .agents
                .get("feat-x")
                .expect("watcher should register the agent");
            record
                .last_message
                .clone()
                .expect("watcher should publish a message")
        };
        match msg {
            BrokerMessage::Status { agent_id, payload } => {
                assert_eq!(agent_id, "feat-x");
                assert!(payload.modified_files.iter().any(|p| p == "change.txt"));
            }
            other => panic!("expected Status message, got {other:?}"),
        }

        let _ = tx.send(true);
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
    }

    /// Spec scenario (task 5.5): an agent commits, the broker records
    /// `committed`, then the agent keeps editing — the watcher re-publishes
    /// `working`, so the record transitions `committed` -> `working`.
    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn watch_worktree_reenters_working_after_commit() {
        use crate::broker::BrokerState;
        use crate::broker::messages::{ArtifactPayload, BrokerMessage};

        let tmp = tempfile::tempdir().unwrap();
        init_test_repo(tmp.path());

        let state = Arc::new(BrokerState::new(None));
        // Record a committed artifact so the agent is in the committed state
        // with a fresh last_committed_at (default TTL is 60s).
        super::delivery::publish_message(
            &state,
            &BrokerMessage::Artifact {
                agent_id: "feat-x".to_string(),
                payload: ArtifactPayload {
                    status: "committed".to_string(),
                    exports: vec![],
                    modified_files: vec![],
                },
            },
        );
        assert_eq!(state.read().agents["feat-x"].status, "committed");

        let (tx, rx) = tokio::sync::watch::channel(false);
        let target = WatchTarget {
            agent_id: "feat-x".to_string(),
            cli: "claude".to_string(),
            worktree_path: tmp.path().to_path_buf(),
        };
        let handle = tokio::spawn(watch_worktree(Arc::clone(&state), target, rx));

        // Agent keeps editing after the commit.
        tokio::time::sleep(Duration::from_millis(300)).await;
        std::fs::write(tmp.path().join("more_work.rs"), "fn extra() {}").unwrap();

        tokio::time::sleep(POLL_INTERVAL + Duration::from_millis(800)).await;

        assert_eq!(
            state.read().agents["feat-x"].status,
            "working",
            "watcher must re-enter working after a post-commit edit within TTL"
        );

        let _ = tx.send(true);
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
    }

    /// With TTL=0 the post-commit edit must NOT re-publish `working` — the
    /// dashboard keeps showing `committed` (v0.5.0 opt-out).
    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn watch_worktree_does_not_reenter_when_ttl_zero() {
        use crate::broker::BrokerState;
        use crate::broker::messages::{ArtifactPayload, BrokerMessage};

        let tmp = tempfile::tempdir().unwrap();
        init_test_repo(tmp.path());

        let state = Arc::new(BrokerState::new(None));
        state.set_republish_working_ttl(Duration::ZERO);
        super::delivery::publish_message(
            &state,
            &BrokerMessage::Artifact {
                agent_id: "feat-z".to_string(),
                payload: ArtifactPayload {
                    status: "committed".to_string(),
                    exports: vec![],
                    modified_files: vec![],
                },
            },
        );

        let (tx, rx) = tokio::sync::watch::channel(false);
        let target = WatchTarget {
            agent_id: "feat-z".to_string(),
            cli: "claude".to_string(),
            worktree_path: tmp.path().to_path_buf(),
        };
        let handle = tokio::spawn(watch_worktree(Arc::clone(&state), target, rx));

        tokio::time::sleep(Duration::from_millis(300)).await;
        std::fs::write(tmp.path().join("more_work.rs"), "fn extra() {}").unwrap();
        tokio::time::sleep(POLL_INTERVAL + Duration::from_millis(800)).await;

        assert_eq!(
            state.read().agents["feat-z"].status,
            "committed",
            "with TTL=0 the watcher must not re-enter working after commit"
        );

        let _ = tx.send(true);
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
    }

    #[tokio::test(flavor = "current_thread")]
    #[serial_test::serial]
    async fn watch_worktree_does_not_publish_when_unchanged() {
        use crate::broker::BrokerState;
        let tmp = tempfile::tempdir().unwrap();
        init_test_repo(tmp.path());

        let state = Arc::new(BrokerState::new(None));
        let (tx, rx) = tokio::sync::watch::channel(false);
        let target = WatchTarget {
            agent_id: "feat-y".to_string(),
            cli: "claude".to_string(),
            worktree_path: tmp.path().to_path_buf(),
        };
        let state_clone = Arc::clone(&state);
        let handle = tokio::spawn(watch_worktree(state_clone, target, rx));

        // Let several ticks elapse with no changes.
        tokio::time::sleep(POLL_INTERVAL * 2 + Duration::from_millis(200)).await;

        let has_entry = {
            let inner = state.read();
            inner.agents.contains_key("feat-y")
        };
        assert!(
            !has_entry,
            "no publish expected when git status is unchanged"
        );

        let _ = tx.send(true);
        let _ = tokio::time::timeout(Duration::from_secs(3), handle).await;
    }
}
