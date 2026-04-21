//! Hook injection integration test.
//!
//! Verifies that running a real `git commit` in a worktree where
//! `install_git_hooks` has been invoked publishes an `agent.artifact`
//! message to the broker on the URL written into the per-worktree marker.

use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{Duration, Instant};

use serial_test::serial;

mod helpers;
use helpers::*;

/// Atomic counter to ensure each test gets a unique broker port.
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Starts a broker on a unique free port and returns the handle + URL.
fn spawn_test_broker() -> (git_paw::broker::BrokerHandle, String) {
    #[allow(clippy::cast_possible_truncation)]
    let base = 22_000 + (std::process::id() as u16 % 5000);
    let offset = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut port = base + offset;
    let mut attempts = 0;
    loop {
        let config = git_paw::config::BrokerConfig {
            enabled: true,
            port,
            bind: "127.0.0.1".to_string(),
        };
        match git_paw::broker::start_broker(
            &config,
            git_paw::broker::BrokerState::new(None),
            Vec::new(),
        ) {
            Ok(handle) => return (handle, config.url()),
            Err(_) if attempts < 10 => {
                port = port.wrapping_add(100);
                attempts += 1;
            }
            Err(e) => panic!("failed to start test broker after retries: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// C14: real `git commit` triggers `agent.artifact`
// ---------------------------------------------------------------------------

/// Installs hooks into the test repo, makes a commit, and confirms the broker
/// receives an `agent.artifact` message naming the modified file.
#[test]
#[serial]
fn git_commit_publishes_agent_artifact_to_broker() {
    // Skip the test gracefully if curl is not on PATH — the post-commit
    // dispatcher uses curl to publish, so without it the hook is a no-op.
    if which::which("curl").is_err() {
        eprintln!("skipping: curl not available on PATH");
        return;
    }

    let tr = setup_test_repo();
    let agent_id = "feat-hook-test";

    let (handle, url) = spawn_test_broker();

    // Install the post-commit dispatcher hook + per-worktree marker.
    git_paw::agents::install_git_hooks(tr.path(), &url, agent_id).expect("install hooks");

    // Sanity: the hook file and marker file must exist after install.
    let hook = tr.path().join(".git").join("hooks").join("post-commit");
    assert!(hook.exists(), "post-commit hook should exist at {hook:?}");
    let marker = tr.path().join(".git").join("paw-agent-id");
    assert!(marker.exists(), "marker file should exist at {marker:?}");

    // Make a real commit so post-commit fires. This is the *second* commit
    // (after the one created by setup_test_repo), so the hook's
    // `git diff HEAD~1 --name-only` resolves to a real changeset and
    // populates `modified_files`.
    std::fs::write(tr.path().join("hello.txt"), "hello world\n").expect("write file");
    let add = StdCommand::new("git")
        .current_dir(tr.path())
        .args(["add", "hello.txt"])
        .status()
        .expect("git add");
    assert!(add.success(), "git add must succeed");

    // Pass GIT_DIR explicitly: the dispatcher hook gates on `[ -n "$GIT_DIR" ]`
    // and does not propagate from CWD discovery in every git invocation flow.
    // Production wires this through tmux's `set-environment`, which gives the
    // hook the same guarantee.
    let git_dir = tr.path().join(".git");
    let commit = StdCommand::new("git")
        .current_dir(tr.path())
        .env("GIT_DIR", &git_dir)
        .args(["commit", "-m", "add hello"])
        .output()
        .expect("git commit");
    assert!(
        commit.status.success(),
        "git commit must succeed; stderr: {}",
        String::from_utf8_lossy(&commit.stderr)
    );

    // Poll the broker's message log for the artifact message. `agent.artifact`
    // is broadcast to every *other* agent's inbox (not the sender's), so we
    // assert presence in the global message log, which captures all published
    // messages regardless of routing.
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut found_artifact: Option<git_paw::broker::messages::BrokerMessage> = None;
    while Instant::now() < deadline {
        let inner = handle.state.read();
        for (_seq, _ts, msg) in &inner.message_log {
            if let git_paw::broker::messages::BrokerMessage::Artifact { agent_id: id, .. } = msg
                && id == agent_id
            {
                found_artifact = Some(msg.clone());
                break;
            }
        }
        drop(inner);
        if found_artifact.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let msg = found_artifact
        .expect("broker should have received an agent.artifact for the test agent within 5s");

    match msg {
        git_paw::broker::messages::BrokerMessage::Artifact {
            agent_id: id,
            payload,
        } => {
            assert_eq!(id, agent_id, "artifact must be tagged with our agent_id");
            assert_eq!(
                payload.status, "committed",
                "post-commit hook reports status=committed"
            );
            assert!(
                payload.modified_files.iter().any(|f| f == "hello.txt"),
                "modified_files should contain 'hello.txt', got: {:?}",
                payload.modified_files
            );
        }
        other => panic!("expected Artifact, got: {other:?}"),
    }

    drop(handle);
}
