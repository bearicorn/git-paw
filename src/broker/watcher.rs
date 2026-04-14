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

        let msg = BrokerMessage::Status {
            agent_id: target.agent_id.clone(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: current.clone(),
                message: None,
            },
        };
        delivery::publish_message(&state, &msg);
        previous = Some(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
