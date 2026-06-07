//! Integration tests for `worktree-branch-guard` (Bug 4 of
//! session-bugfixes-v0-6-x).
//!
//! Drives the real hooks installed by `install_git_hooks` against throwaway
//! git repos (never `setup_test_repo`, so no live-session guard and no tmux):
//! the pre-commit guard blocks a commit whose HEAD branch differs from the
//! worktree's expected branch when strict, allows it when strict is off, and
//! the post-commit dispatcher publishes `agent.feedback` + `agent.learning`
//! on a mismatch. Also asserts the coordination skill teaches the discipline.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

/// Init a committed git repo on a known branch.
fn init_repo(dir: &Path, branch: &str) {
    let run = |args: &[&str]| {
        let st = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git");
        assert!(st.status.success(), "git {args:?} failed");
    };
    run(&["init", "-q", "-b", branch]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("seed"), "seed").expect("seed");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Stage a new file and attempt a commit; returns the commit `Output`.
fn attempt_commit(dir: &Path, file: &str) -> std::process::Output {
    fs::write(dir.join(file), "change").expect("write change");
    StdCommand::new("git")
        .current_dir(dir)
        .args(["add", "."])
        .output()
        .expect("git add");
    StdCommand::new("git")
        .current_dir(dir)
        .args(["commit", "-m", "work"])
        .output()
        .expect("git commit")
}

/// A localhost stub that records every POST body it receives.
fn spawn_recording_stub() -> (u16, Arc<Mutex<Vec<String>>>, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().unwrap().port();
    let bodies = Arc::new(Mutex::new(Vec::<String>::new()));
    let stop = Arc::new(AtomicBool::new(false));
    let (b, s) = (bodies.clone(), stop.clone());
    thread::spawn(move || {
        listener.set_nonblocking(true).expect("nonblocking");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !s.load(Ordering::SeqCst) && Instant::now() < deadline {
            match listener.accept() {
                Ok((mut sock, _)) => {
                    let _ = sock.set_read_timeout(Some(Duration::from_millis(500)));
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 2048];
                    if let Ok(n) = sock.read(&mut chunk) {
                        buf.extend_from_slice(&chunk[..n]);
                    }
                    let req = String::from_utf8_lossy(&buf).to_string();
                    if let Some(idx) = req.find("\r\n\r\n") {
                        b.lock().unwrap().push(req[idx + 4..].to_string());
                    }
                    let _ = sock.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    );
                    let _ = sock.flush();
                }
                Err(_) => thread::sleep(Duration::from_millis(25)),
            }
        }
    });
    (port, bodies, stop)
}

const UNREACHABLE: &str = "http://127.0.0.1:1";

#[test]
fn pre_commit_blocks_when_branch_mismatches_and_strict() {
    let repo = tempfile::TempDir::new().unwrap();
    init_repo(repo.path(), "main");
    // Worktree expected to be on feat/foo, but HEAD is main → mismatch.
    git_paw::agents::install_git_hooks(repo.path(), UNREACHABLE, "feat-foo", "feat/foo", true)
        .expect("install hooks");

    let out = attempt_commit(repo.path(), "a.txt");
    assert!(
        !out.status.success(),
        "strict guard must block the cross-branch commit"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("branch guard"),
        "error should name the branch guard; got:\n{stderr}"
    );
}

#[test]
fn pre_commit_allows_when_branch_matches() {
    let repo = tempfile::TempDir::new().unwrap();
    init_repo(repo.path(), "main");
    // Expected branch equals the actual HEAD branch → no block.
    git_paw::agents::install_git_hooks(repo.path(), UNREACHABLE, "main", "main", true)
        .expect("install hooks");

    let out = attempt_commit(repo.path(), "a.txt");
    assert!(
        out.status.success(),
        "matching-branch commit must pass the guard; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn pre_commit_allows_mismatch_when_strict_disabled() {
    let repo = tempfile::TempDir::new().unwrap();
    init_repo(repo.path(), "main");
    // Mismatch (HEAD=main, expected=feat/foo) but strict OFF → commit allowed.
    git_paw::agents::install_git_hooks(repo.path(), UNREACHABLE, "feat-foo", "feat/foo", false)
        .expect("install hooks");

    let out = attempt_commit(repo.path(), "a.txt");
    assert!(
        out.status.success(),
        "strict_branch_guard=false must let the commit through; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn post_commit_publishes_feedback_and_learning_on_mismatch() {
    let (port, bodies, stop) = spawn_recording_stub();
    let broker = format!("http://127.0.0.1:{port}");

    let repo = tempfile::TempDir::new().unwrap();
    init_repo(repo.path(), "main");
    // Mismatch + strict OFF so the commit lands and the post-commit hook runs
    // its detection (detection without enforcement).
    git_paw::agents::install_git_hooks(repo.path(), &broker, "feat-foo", "feat/foo", false)
        .expect("install hooks");

    let out = attempt_commit(repo.path(), "a.txt");
    assert!(
        out.status.success(),
        "commit should land (strict off); stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Give the hook's curls a moment to reach the stub.
    let deadline = Instant::now() + Duration::from_secs(5);
    let (mut saw_feedback, mut saw_learning) = (false, false);
    while Instant::now() < deadline && !(saw_feedback && saw_learning) {
        {
            let got = bodies.lock().unwrap();
            saw_feedback = got.iter().any(|b| b.contains("\"agent.feedback\""));
            saw_learning = got
                .iter()
                .any(|b| b.contains("\"agent.learning\"") && b.contains("permission_pattern"));
        }
        thread::sleep(Duration::from_millis(100));
    }
    stop.store(true, Ordering::SeqCst);

    assert!(
        saw_feedback,
        "post-commit must publish agent.feedback on mismatch"
    );
    assert!(
        saw_learning,
        "post-commit must publish agent.learning(category=permission_pattern) on mismatch"
    );
}

#[test]
fn install_git_hooks_is_idempotent_no_duplicate_blocks() {
    const MARKER: &str = "# >>> git-paw managed hook >>>";
    // Re-running install_git_hooks against an already-installed repo must not
    // duplicate the git-paw managed hook block in any hook file.
    let repo = tempfile::TempDir::new().unwrap();
    init_repo(repo.path(), "feat/foo");
    let install = || {
        git_paw::agents::install_git_hooks(repo.path(), UNREACHABLE, "feat-foo", "feat/foo", true)
            .expect("install hooks");
    };
    install();
    install(); // second run — must be idempotent

    for hook in ["pre-commit", "post-commit", "pre-push"] {
        let path = repo.path().join(".git/hooks").join(hook);
        let body = fs::read_to_string(&path).unwrap_or_else(|_| panic!("read {hook}"));
        let count = body.matches(MARKER).count();
        assert_eq!(
            count, 1,
            "{hook} must contain exactly one git-paw managed block after re-install, found {count}"
        );
    }
}

#[test]
fn coordination_skill_teaches_stay_inside_your_worktree() {
    let skill = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/agent-skills/coordination.md");
    let body = fs::read_to_string(skill).expect("read coordination.md");
    assert!(
        body.contains("### Stay inside your worktree"),
        "coordination skill must include the 'Stay inside your worktree' section"
    );
    assert!(
        body.contains("relative paths"),
        "the section must teach relative-paths-only discipline"
    );
    assert!(
        body.contains("pre-commit"),
        "the section must reference the pre-commit branch guard as enforcement"
    );
}
