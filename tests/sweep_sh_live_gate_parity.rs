//! Lockstep tests for the live-prompt gate: the bundled `sweep.sh` marker
//! detection must agree with the Rust gate
//! (`git_paw::supervisor::auto_approve::is_live_prompt`) on shared fixtures
//! (capability `automatic-approval`, scenario "Detection agrees across
//! auto-approver and sweep helper", change `approve-send-gate-hardening`).
//!
//! The shell half is driven through `sweep.sh classify`, whose first gate is
//! the same structural liveness check `approve` uses (`run_classifier`): a
//! non-live capture prints `no-op (not live)`, anything else means the gate
//! saw a live prompt. The suite needs no tmux and no broker — `classify`
//! reads the capture from stdin.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command as StdCommand, Stdio};

use git_paw::supervisor::auto_approve::is_live_prompt;
use tempfile::TempDir;

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Copies the bundled sweep.sh asset into `<repo>/.git-paw/scripts/`.
fn install_sweep(repo: &Path) -> std::path::PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh");
    let dst_dir = repo.join(".git-paw/scripts");
    fs::create_dir_all(&dst_dir).expect("mk scripts dir");
    let dst = dst_dir.join("sweep.sh");
    fs::copy(&src, &dst).expect("copy sweep.sh");
    dst
}

/// Runs the shell half of the live-prompt gate on `capture`: pipes it into
/// `sweep.sh classify` and returns whether the helper saw a LIVE prompt.
fn sweep_sees_live(repo: &Path, sweep: &Path, capture: &str) -> bool {
    let mut child = StdCommand::new("bash")
        .arg(sweep)
        .arg("classify")
        .current_dir(repo)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sweep.sh classify");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(capture.as_bytes())
        .expect("write capture");
    let out = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&out.stdout);
    !stdout.contains("no-op (not live)")
}

/// The shared fixture set: `(name, capture, expected_live)`. Every entry is
/// evaluated by BOTH the Rust gate and the sweep.sh mirror, and both must
/// agree with the expectation.
fn fixtures() -> Vec<(&'static str, &'static str, bool)> {
    vec![
        (
            "multi-option block with a don't-ask-again option, inline (esc) only",
            "╭──────────────────────────────────────────────────────────────╮\n\
             │ Bash command                                                 │\n\
             │   cargo test --workspace                                     │\n\
             │   Run the full test suite                                    │\n\
             │ Do you want to proceed?                                      │\n\
             │ ❯ 1. Yes                                                     │\n\
             │   2. Yes, and don't ask again for cargo test in this project │\n\
             │   3. No, and tell Claude what to do differently (esc)        │\n\
             ╰──────────────────────────────────────────────────────────────╯",
            true,
        ),
        (
            "multi-option block with an explicit Esc to cancel footer",
            "Do you want to proceed?\n❯ 1. Yes\n  2. Yes, and don't ask again for: git status\n  3. No\n  Esc to cancel",
            true,
        ),
        (
            "single prompt with the question at the tail",
            "Bash command\n  git status\nDo you want to proceed?",
            true,
        ),
        (
            "prose narration mentioning a safe command",
            "I might run cargo test soon.\nHere is some narration about the plan.\n$ ls -la\ndone.",
            false,
        ),
        (
            "numbered list in prose without any prompt marker",
            "Here is my plan:\n1. run the tests\n2. commit the work\n3. stand by for verification",
            false,
        ),
        (
            "prompt scrolled out of the tail by later output",
            "Do you want to proceed?\n  Esc to cancel\noutput line 1\noutput line 2\noutput line 3\noutput line 4\noutput line 5",
            false,
        ),
    ]
}

/// Task 4.3: `bash -n` on the edited helper — the bundled script must always
/// parse (guards the embedded-Python heredoc quote-tracking landmine).
#[test]
fn sweep_sh_parses_under_bash_n() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh");
    let out = StdCommand::new("bash")
        .arg("-n")
        .arg(&src)
        .output()
        .expect("run bash -n");
    assert!(
        out.status.success(),
        "bash -n must accept sweep.sh: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Spec scenario "Detection agrees across auto-approver and sweep helper":
/// the Rust gate and the sweep.sh mirror return the same liveness verdict on
/// every shared fixture, and that verdict matches the expectation.
#[test]
fn rust_gate_and_sweep_helper_agree_on_shared_fixtures() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());

    for (name, capture, expected_live) in fixtures() {
        let rust_live = is_live_prompt(capture);
        let sweep_live = sweep_sees_live(repo.path(), &sweep, capture);
        assert_eq!(
            rust_live, expected_live,
            "Rust gate disagrees with expectation on fixture: {name}"
        );
        assert_eq!(
            sweep_live, expected_live,
            "sweep.sh disagrees with expectation on fixture: {name}"
        );
    }
}
