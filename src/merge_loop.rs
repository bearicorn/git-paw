//! Supervisor merge loop and topological dependency ordering.
//!
//! Walks the agent set in topological merge order, merges each branch into
//! the current branch, and runs the supervisor's configured test command (if
//! any) between merges. The captured per-branch outcomes are returned for
//! [`crate::summary::write_supervisor_summary`] to render into the session
//! summary.
//!
//! Pulled out of `main.rs` so integration tests can drive the merge loop
//! without spawning the binary or depending on the supervisor CLI.

use std::collections::HashMap;
use std::path::Path;

use crate::broker::messages::BrokerMessage;
use crate::broker::publish::{build_status_message, publish_to_broker_http};
use crate::config::BrokerConfig;
use crate::error::PawError;
use crate::git;
use crate::session::Session;
use crate::summary::TestResult;

/// Target branch for the supervisor merge loop.
const MERGE_TARGET_BRANCH: &str = "main";

/// Builds a dependency graph from broker messages.
///
/// An edge `B → A` means "A was blocked on B" — so B must merge before A.
/// The returned map is keyed by the dependency (B) and lists its dependents
/// (A's). Used by [`topological_merge_order`] to pick a safe merge order.
pub fn build_dependency_graph(messages: &[(u64, BrokerMessage)]) -> HashMap<String, Vec<String>> {
    let mut graph: HashMap<String, Vec<String>> = HashMap::new();
    for (_, msg) in messages {
        if let BrokerMessage::Blocked { agent_id, payload } = msg {
            let dep = payload.from.clone();
            let dependent = agent_id.clone();
            graph.entry(dep).or_default().push(dependent);
        }
    }
    graph
}

/// Topological sort of the dependency graph. Returns a merge order where
/// agents with no dependents come first (so agents that are depended upon
/// merge before the agents that depend on them).
///
/// On cycle detection, logs a warning and falls back to returning all agents
/// in arbitrary order so the caller can still proceed.
pub fn topological_merge_order<S: std::hash::BuildHasher>(
    graph: &HashMap<String, Vec<String>, S>,
    all_agents: &[String],
) -> Vec<String> {
    // Compute in-degree of each agent (number of deps it has).
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    for agent in all_agents {
        in_degree.entry(agent.clone()).or_insert(0);
    }
    for dependents in graph.values() {
        for dependent in dependents {
            *in_degree.entry(dependent.clone()).or_insert(0) += 1;
        }
    }

    // Kahn's algorithm — seed with all zero-in-degree agents.
    let mut queue: Vec<String> = in_degree
        .iter()
        .filter_map(|(k, v)| if *v == 0 { Some(k.clone()) } else { None })
        .collect();
    queue.sort();

    let mut order = Vec::new();
    while let Some(node) = queue.pop() {
        order.push(node.clone());
        if let Some(dependents) = graph.get(&node) {
            for dep in dependents {
                if let Some(deg) = in_degree.get_mut(dep) {
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        queue.push(dep.clone());
                        queue.sort();
                    }
                }
            }
        }
    }

    if order.len() == all_agents.len() {
        order
    } else {
        let cycle_members: Vec<String> = in_degree
            .iter()
            .filter_map(|(k, v)| if *v > 0 { Some(k.clone()) } else { None })
            .collect();
        eprintln!(
            "warning: dependency cycle detected among agents {cycle_members:?}; \
             falling back to sorted merge order"
        );
        // Sort so the fallback order is deterministic regardless of the
        // caller's input ordering — otherwise tests (and operators) get
        // different results from the same cyclic graph.
        let mut fallback = all_agents.to_vec();
        fallback.sort();
        fallback
    }
}

/// Results of running the merge loop.
///
/// `merge_order` lists the branches in the order they were attempted (matches
/// `topological_merge_order`'s return). `test_results` records, per branch,
/// whether the merge + test succeeded plus the captured stdout (or a synthetic
/// "Merge failed: ..." / "No test command configured" line).
#[derive(Debug, Clone)]
pub struct MergeResults {
    /// Branches in the order they were processed.
    pub merge_order: Vec<String>,
    /// Per-branch outcome (keyed by branch name).
    pub test_results: HashMap<String, TestResult>,
}

/// Runs the configured test command via `sh -c` and captures stdout.
///
/// Returns a [`TestResult`] whose `success` reflects the shell exit status
/// and whose `output` is the captured stdout (stderr is intentionally not
/// captured here — the supervisor summary surfaces stdout to the operator).
pub fn run_test_command(repo_root: &Path, test_command: &str) -> Result<TestResult, PawError> {
    let output = std::process::Command::new("sh")
        .current_dir(repo_root)
        .arg("-c")
        .arg(test_command)
        .output()
        .map_err(|e| PawError::SessionError(format!("failed to run test command: {e}")))?;

    let success = output.status.success();
    let output_str = String::from_utf8_lossy(&output.stdout).to_string();

    Ok(TestResult {
        success,
        output: output_str,
    })
}

/// Walks the agent set in topological order, merging each branch into the
/// current branch, optionally running `test_command` between merges, and
/// returning the per-branch outcomes.
///
/// `dep_graph` is the dependency map produced by [`build_dependency_graph`]
/// from the broker's `agent.blocked` messages. An empty map (or `None`)
/// means no dependencies are known, in which case [`topological_merge_order`]
/// falls back to a sorted alphabetical merge.
///
/// Per-branch and final supervisor status messages are dispatched via
/// `publisher`. The production callsite injects a closure that POSTs to the
/// broker over HTTP; tests inject a closure that calls
/// [`crate::broker::delivery::publish_message`] against a synthetic state so
/// they can assert on the resulting broker state without a live HTTP server.
#[allow(clippy::unnecessary_wraps)]
pub fn run_merge_loop_with_publisher<S: std::hash::BuildHasher>(
    repo_root: &Path,
    session: &Session,
    test_command: Option<&String>,
    dep_graph: &HashMap<String, Vec<String>, S>,
    publisher: &dyn Fn(&BrokerMessage),
) -> Result<MergeResults, PawError> {
    let agents: Vec<String> = session.worktrees.iter().map(|w| w.branch.clone()).collect();
    let merge_order = topological_merge_order(dep_graph, &agents);

    let mut test_results: HashMap<String, TestResult> = HashMap::new();
    let mut n_ok: usize = 0;
    let mut n_fail: usize = 0;

    let _ = std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["checkout", MERGE_TARGET_BRANCH])
        .status();

    for branch in &merge_order {
        println!("Merging branch: {branch}");

        if let Err(e) = git::merge_branch(repo_root, branch) {
            eprintln!("Warning: Failed to merge branch {branch}: {e}");
            let reason = format!("merge failed: {e}");
            test_results.insert(
                branch.clone(),
                TestResult {
                    success: false,
                    output: format!("Merge failed: {e}"),
                },
            );
            publisher(&build_status_message(branch, "merge_failed", Some(reason)));
            n_fail += 1;
            continue;
        }

        // `git::merge_branch` uses `--no-commit`, so even a clean merge leaves
        // MERGE_HEAD set and blocks the next merge. Finalize by committing
        // with the default merge message; if there is nothing to commit
        // (e.g. the branch was already merged), `git commit` exits non-zero
        // and we treat that as a no-op.
        let _ = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(["commit", "--no-edit", "--allow-empty"])
            .output();

        let merged_msg = format!("merged into {MERGE_TARGET_BRANCH}");

        if let Some(cmd) = test_command {
            println!("Running test command: {cmd}");
            match run_test_command(repo_root, cmd) {
                Ok(result) => {
                    let success = result.success;
                    test_results.insert(branch.clone(), result);
                    if success {
                        println!("\u{2713} Tests passed for {branch}");
                        publisher(&build_status_message(
                            branch,
                            "merged",
                            Some(merged_msg.clone()),
                        ));
                        n_ok += 1;
                    } else {
                        println!("\u{2717} Tests failed for {branch}");
                        publisher(&build_status_message(
                            branch,
                            "merge_failed",
                            Some(format!("test command failed for {branch}")),
                        ));
                        n_fail += 1;
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Test command failed for {branch}: {e}");
                    let reason = format!("test execution failed: {e}");
                    test_results.insert(
                        branch.clone(),
                        TestResult {
                            success: false,
                            output: format!("Test execution failed: {e}"),
                        },
                    );
                    publisher(&build_status_message(branch, "merge_failed", Some(reason)));
                    n_fail += 1;
                }
            }
        } else {
            println!("\u{2713} Merged {branch} (no test command configured)");
            test_results.insert(
                branch.clone(),
                TestResult {
                    success: true,
                    output: "No test command configured".to_string(),
                },
            );
            publisher(&build_status_message(branch, "merged", Some(merged_msg)));
            n_ok += 1;
        }
    }

    publisher(&build_status_message(
        "supervisor",
        "working",
        Some(format!("merge loop done: {n_ok} merged, {n_fail} failed")),
    ));

    Ok(MergeResults {
        merge_order,
        test_results,
    })
}

/// Production wrapper around [`run_merge_loop_with_publisher`].
///
/// Publishes merge results to the broker over HTTP using `broker_config.url()`
/// when the broker is enabled; otherwise the publisher is a no-op. The
/// dependency graph is sourced from the broker's `agent.blocked` messages
/// fetched via `GET /log` so the merge order honours real cross-agent
/// dependencies. If the fetch fails, falls back to an empty graph (sorted
/// alphabetical merge) with a warning to stderr.
pub fn run_merge_loop(
    repo_root: &Path,
    session: &Session,
    test_command: Option<&String>,
    broker_config: &BrokerConfig,
) -> Result<MergeResults, PawError> {
    let dep_graph = if broker_config.enabled {
        match crate::broker::publish::fetch_log_over_http(&broker_config.url()) {
            Ok(messages) => {
                let pairs: Vec<(u64, BrokerMessage)> = messages
                    .into_iter()
                    .enumerate()
                    .map(|(i, m)| (i as u64, m))
                    .collect();
                build_dependency_graph(&pairs)
            }
            Err(e) => {
                eprintln!(
                    "warning: failed to fetch broker /log for merge dependency graph: {e}; \
                     falling back to alphabetical merge order"
                );
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    };

    let publisher: Box<dyn Fn(&BrokerMessage)> = if broker_config.enabled {
        let url = broker_config.url();
        Box::new(move |msg: &BrokerMessage| {
            if let Err(e) = publish_to_broker_http(&url, msg) {
                eprintln!("warning: failed to publish merge status to broker: {e}");
            }
        })
    } else {
        Box::new(|_msg: &BrokerMessage| {})
    };
    run_merge_loop_with_publisher(repo_root, session, test_command, &dep_graph, &*publisher)
}

#[cfg(test)]
mod tests {
    //! Behavioral tests for `run_merge_loop_with_publisher`. The publisher
    //! closure routes through `broker::delivery::publish_message` against a
    //! synthetic in-process broker state so we can assert on the resulting
    //! status records without spinning up an HTTP server.

    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::Arc;
    use std::time::SystemTime;

    use crate::broker;
    use crate::broker::delivery;
    use crate::session::{Session, SessionStatus, WorktreeEntry};

    use super::run_merge_loop_with_publisher;

    fn init_repo(dir: &std::path::Path) {
        let git = which::which("git").expect("git on PATH");
        Command::new(&git)
            .current_dir(dir)
            .args(["init", "-b", "main"])
            .output()
            .expect("git init");
        Command::new(&git)
            .current_dir(dir)
            .args(["config", "user.email", "test@test.com"])
            .output()
            .expect("git config email");
        Command::new(&git)
            .current_dir(dir)
            .args(["config", "user.name", "Test"])
            .output()
            .expect("git config name");
        std::fs::write(dir.join("README.md"), "# test\n").unwrap();
        Command::new(&git)
            .current_dir(dir)
            .args(["add", "README.md"])
            .output()
            .expect("git add");
        Command::new(&git)
            .current_dir(dir)
            .args(["commit", "-m", "init"])
            .output()
            .expect("git commit");
    }

    fn synthetic_session(branches: &[&str]) -> Session {
        Session {
            session_name: "paw-test".to_string(),
            repo_path: PathBuf::from("/tmp"),
            project_name: "test".to_string(),
            created_at: SystemTime::now(),
            status: SessionStatus::Active,
            worktrees: branches
                .iter()
                .map(|b| WorktreeEntry {
                    branch: (*b).to_string(),
                    worktree_path: PathBuf::from("/tmp"),
                    cli: "claude".to_string(),
                    branch_created: false,
                })
                .collect(),
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
        }
    }

    #[test]
    fn merge_loop_publishes_final_supervisor_status() {
        let tmp = tempfile::tempdir().expect("tempdir");
        init_repo(tmp.path());

        let state = Arc::new(broker::BrokerState::new(None));
        let publisher_state = Arc::clone(&state);
        let publisher = move |msg: &broker::messages::BrokerMessage| {
            delivery::publish_message(&publisher_state, msg);
        };

        let session = synthetic_session(&["feat-a", "feat-b"]);
        let _ =
            run_merge_loop_with_publisher(tmp.path(), &session, None, &HashMap::new(), &publisher);

        let inner = state.read();
        let supervisor = inner
            .agents
            .get("supervisor")
            .expect("supervisor record published at end of merge loop");
        assert_eq!(supervisor.status, "working");
        let last_msg = supervisor
            .last_message
            .as_ref()
            .expect("supervisor last_message recorded");
        match last_msg {
            broker::messages::BrokerMessage::Status { payload, .. } => {
                let body = payload.message.as_deref().unwrap_or("");
                assert!(
                    body.starts_with("merge loop done:"),
                    "expected 'merge loop done: ...' message, got: {body}"
                );
            }
            other => panic!("expected Status, got {other:?}"),
        }
    }

    #[test]
    fn merge_loop_publishes_merge_failed_when_test_command_fails() {
        let tmp = tempfile::tempdir().expect("tempdir");
        init_repo(tmp.path());

        let git = which::which("git").expect("git on PATH");
        Command::new(&git)
            .current_dir(tmp.path())
            .args(["checkout", "-b", "feat-broken"])
            .output()
            .expect("checkout -b feat-broken");
        std::fs::write(tmp.path().join("broken.txt"), "broken\n").unwrap();
        Command::new(&git)
            .current_dir(tmp.path())
            .args(["add", "broken.txt"])
            .output()
            .expect("git add");
        Command::new(&git)
            .current_dir(tmp.path())
            .args(["commit", "-m", "broken"])
            .output()
            .expect("git commit");
        Command::new(&git)
            .current_dir(tmp.path())
            .args(["checkout", "main"])
            .output()
            .expect("checkout main");

        let state = Arc::new(broker::BrokerState::new(None));
        let publisher_state = Arc::clone(&state);
        let publisher = move |msg: &broker::messages::BrokerMessage| {
            delivery::publish_message(&publisher_state, msg);
        };

        let session = synthetic_session(&["feat-broken"]);
        let test_cmd = "exit 1".to_string();
        let _ = run_merge_loop_with_publisher(
            tmp.path(),
            &session,
            Some(&test_cmd),
            &HashMap::new(),
            &publisher,
        );

        let inner = state.read();
        let record = inner
            .agents
            .get("feat-broken")
            .expect("feat-broken status published");
        assert_eq!(
            record.status, "merge_failed",
            "branch should publish merge_failed when test command fails",
        );

        let supervisor = inner.agents.get("supervisor").expect("supervisor row");
        let last_msg = supervisor
            .last_message
            .as_ref()
            .expect("supervisor last message recorded");
        if let broker::messages::BrokerMessage::Status { payload, .. } = last_msg {
            let body = payload.message.as_deref().unwrap_or("");
            assert!(
                body.contains("0 merged") && body.contains("1 failed"),
                "expected '0 merged, 1 failed' in body, got: {body}",
            );
        } else {
            panic!("expected Status message");
        }
    }

    #[test]
    fn merge_loop_publishes_merged_status_when_branch_exists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        init_repo(tmp.path());

        let git = which::which("git").expect("git on PATH");
        Command::new(&git)
            .current_dir(tmp.path())
            .args(["checkout", "-b", "feat-ok"])
            .output()
            .expect("checkout -b feat-ok");
        std::fs::write(tmp.path().join("feature.txt"), "feature\n").unwrap();
        Command::new(&git)
            .current_dir(tmp.path())
            .args(["add", "feature.txt"])
            .output()
            .expect("git add feature.txt");
        Command::new(&git)
            .current_dir(tmp.path())
            .args(["commit", "-m", "feat"])
            .output()
            .expect("git commit");
        Command::new(&git)
            .current_dir(tmp.path())
            .args(["checkout", "main"])
            .output()
            .expect("checkout main");

        let state = Arc::new(broker::BrokerState::new(None));
        let publisher_state = Arc::clone(&state);
        let publisher = move |msg: &broker::messages::BrokerMessage| {
            delivery::publish_message(&publisher_state, msg);
        };

        let session = synthetic_session(&["feat-ok"]);
        let _ =
            run_merge_loop_with_publisher(tmp.path(), &session, None, &HashMap::new(), &publisher);

        let inner = state.read();
        let record = inner
            .agents
            .get("feat-ok")
            .expect("feat-ok status published");
        assert_eq!(record.status, "merged");
        let last_msg = record
            .last_message
            .as_ref()
            .expect("feat-ok last message recorded");
        if let broker::messages::BrokerMessage::Status { payload, .. } = last_msg {
            assert_eq!(payload.message.as_deref(), Some("merged into main"));
        } else {
            panic!("expected Status message");
        }
    }

    #[test]
    fn topological_merge_order_cycle_fallback_is_deterministic() {
        use std::collections::HashMap;

        use super::topological_merge_order;

        // 2-cycle: a depends on b, b depends on a.
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        graph.insert("a".into(), vec!["b".into()]);
        graph.insert("b".into(), vec!["a".into()]);

        // Caller passes agents in a non-sorted order. The cycle fallback must
        // sort them so the result is deterministic regardless of input order.
        let all_agents: Vec<String> = vec!["c".into(), "a".into(), "b".into()];
        let order = topological_merge_order(&graph, &all_agents);
        assert_eq!(
            order,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );

        // Same agents in another order produce the same fallback.
        let alt: Vec<String> = vec!["b".into(), "c".into(), "a".into()];
        let order_alt = topological_merge_order(&graph, &alt);
        assert_eq!(order_alt, order);
    }
}
