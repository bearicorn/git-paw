//! Delegation tests for the `classify` subcommand of the bundled
//! `<repo>/.git-paw/scripts/sweep.sh` (capability `approval-command-safety`,
//! scenario "sweep.sh classifies by delegating to the Rust classifier").
//!
//! The helper no longer implements the classification rules, so there is no
//! second implementation to assert parity against. What these tests pin is the
//! delegation itself:
//!
//! - the rendered decision faithfully reproduces the verdict `git paw
//!   __classify` prints for the same capture (safe / danger / unknown + the
//!   option index);
//! - with no `git paw` reachable the helper fails CLOSED — it escalates rather
//!   than approving, which a helper still classifying locally could not do;
//! - the liveness gate stays in the helper and short-circuits before any
//!   delegation;
//! - the shipped script carries no parallel classifier (single-source guard).
//!
//! The classification RULES themselves are guarded where they now live once:
//! the `auto_approve` / `drive` unit tables and `tests/classify_subcommand.rs`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn init_git_repo(dir: &Path) {
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "test@test.com"][..],
        &["config", "user.name", "Test"][..],
    ] {
        let st = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git");
        assert!(st.success());
    }
    fs::write(dir.join("README.md"), "# test").expect("write readme");
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["add", "."])
        .status();
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["commit", "-q", "-m", "initial"])
        .status();
}

struct Fixture {
    _tmp: TempDir,
    sweep: PathBuf,
    root: PathBuf,
}

/// Temp repo with `git paw init` run and the `rust` stack declared, so the
/// composed whitelist carries the toolchain verbs the rows below need.
fn setup() -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    let init_out = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("git paw init");
    assert!(init_out.status.success(), "git paw init must succeed");
    let sweep = tmp.path().join(".git-paw/scripts/sweep.sh");
    assert!(sweep.exists(), "init must write sweep.sh");
    let config = tmp.path().join(".git-paw/config.toml");
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(&config)
        .expect("open config.toml");
    writeln!(
        f,
        "\n[supervisor.common_dev_allowlist]\nstacks = [\"rust\"]"
    )
    .expect("declare rust stack");
    let root = tmp.path().to_path_buf();
    Fixture {
        _tmp: tmp,
        sweep,
        root,
    }
}

/// Directory holding the freshly built binary, prepended to a child's `PATH`
/// so `git paw __classify` resolves to THIS build.
fn built_bin_dir() -> PathBuf {
    assert_cmd::cargo::cargo_bin("git-paw")
        .parent()
        .expect("built binary has a parent dir")
        .to_path_buf()
}

/// The ambient `PATH` with every directory holding a `git-paw` executable
/// removed, so `git paw` cannot resolve at all. Keeps `bash` / `git` / `grep` /
/// `tail` reachable, so only the delegation target goes missing.
fn path_without_git_paw() -> String {
    let path = std::env::var("PATH").unwrap_or_default();
    path.split(':')
        .filter(|dir| !dir.is_empty() && !Path::new(dir).join("git-paw").exists())
        .collect::<Vec<_>>()
        .join(":")
}

/// Whether the delegation target is reachable from the helper's `PATH`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Delegate {
    Reachable,
    Missing,
}

/// Pipes `capture` into `sweep.sh classify` and returns its printed decision.
fn classify(fx: &Fixture, capture: &str, root_arg: Option<&str>, delegate: Delegate) -> String {
    let path = match delegate {
        Delegate::Reachable => format!(
            "{}:{}",
            built_bin_dir().display(),
            std::env::var("PATH").unwrap_or_default()
        ),
        Delegate::Missing => path_without_git_paw(),
    };
    let mut c = StdCommand::new("bash");
    c.arg(&fx.sweep).arg("classify");
    if let Some(r) = root_arg {
        c.arg(r);
    }
    let mut child = c
        .current_dir(&fx.root)
        .env("PATH", path)
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
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Runs `git paw __classify` on the same capture and returns its raw
/// `<class> <option>` verdict — the value the helper must render.
fn raw_verdict(fx: &Fixture, capture: &str, root_arg: Option<&str>) -> String {
    let mut c = StdCommand::new(assert_cmd::cargo::cargo_bin("git-paw"));
    c.arg("__classify");
    if let Some(r) = root_arg {
        c.arg("--worktree-root").arg(r);
    } else {
        c.arg("--worktree-root").arg(&fx.root);
    }
    let mut child = c
        .current_dir(&fx.root)
        .env_remove("TMUX")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git paw __classify");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(capture.as_bytes())
        .expect("write capture");
    let out = child.wait_with_output().expect("wait");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The decision line the helper must print for a raw `<class> <option>` verdict.
fn rendered(verdict: &str) -> String {
    let mut parts = verdict.split_whitespace();
    let class = parts.next().unwrap_or_default();
    let option = parts.next().unwrap_or_default();
    match class {
        "safe" => format!("approve option={option} (safe)"),
        "danger" => "escalate (danger)".to_string(),
        _ => "escalate (unknown)".to_string(),
    }
}

fn two_option(cmd: &str) -> String {
    format!("Bash command\n  {cmd}\nDo you want to proceed?\nEsc to cancel")
}

fn three_option(cmd: &str) -> String {
    format!(
        "Bash command\n  {cmd}\nDo you want to proceed?\n\
         1. Yes\n2. Yes, and don't ask again for: {cmd}\n3. No\nEsc to cancel"
    )
}

/// Spec scenario "sweep.sh classifies by delegating to the Rust classifier":
/// for every shape of verdict the helper prints exactly what `git paw
/// __classify` decided for the same capture — including the option index.
#[test]
#[serial]
fn classify_renders_the_delegated_verdict() {
    let fx = setup();
    let root = fx.root.to_string_lossy().to_string();

    let rows: Vec<(&str, String, Option<&str>, &str)> = vec![
        (
            "danger command",
            two_option("git push --force origin main"),
            None,
            "escalate (danger)",
        ),
        (
            "safe stack command takes the durable option",
            three_option("cargo fmt --check"),
            None,
            "approve option=2 (safe)",
        ),
        (
            "safe read-mostly verb on a 2-option prompt",
            two_option("grep -rn foo src/"),
            None,
            "approve option=1 (safe)",
        ),
        (
            "unknown command",
            two_option("someprog --do-thing"),
            None,
            "escalate (unknown)",
        ),
        (
            "worktree-confined commit",
            two_option("git commit -m wip"),
            Some(root.as_str()),
            "approve option=1 (safe)",
        ),
        (
            "scratch delete",
            two_option("rm -rf /tmp/paw-build-1"),
            None,
            "approve option=1 (safe)",
        ),
    ];

    for (name, capture, root_arg, expected) in rows {
        let out = classify(&fx, &capture, root_arg, Delegate::Reachable);
        assert_eq!(out, expected, "unexpected decision for row: {name}");
        // …and it is the delegate's verdict, rendered — not an independent one.
        let verdict = raw_verdict(&fx, &capture, root_arg);
        assert_eq!(
            out,
            rendered(&verdict),
            "helper must render __classify's verdict ({verdict:?}) for row: {name}"
        );
    }
}

/// Fail-closed direction (task 2.2): with no `git paw` reachable the helper
/// produces NO approval. A helper that still classified locally would print
/// `approve …` for this whitelisted capture, so this is also the behavioural
/// proof that classification is delegated rather than duplicated.
#[test]
#[serial]
fn missing_delegate_fails_closed_and_never_approves() {
    let fx = setup();
    let out = classify(
        &fx,
        &three_option("cargo fmt --check"),
        None,
        Delegate::Missing,
    );
    assert!(
        !out.contains("approve"),
        "an unreachable classifier must never approve, got: {out}"
    );
    assert!(
        out.contains("escalate"),
        "an unreachable classifier must escalate, got: {out}"
    );

    // Sanity: the same capture DOES approve once the delegate is reachable, so
    // the escalation above is the missing delegate and not a broken fixture.
    let out = classify(
        &fx,
        &three_option("cargo fmt --check"),
        None,
        Delegate::Reachable,
    );
    assert_eq!(
        out, "approve option=2 (safe)",
        "reachable delegate approves"
    );
}

/// The liveness gate stays in the helper and runs BEFORE any delegation: a
/// non-live capture is a no-op even with no `git paw` reachable.
#[test]
#[serial]
fn non_live_capture_is_a_noop_without_delegating() {
    let fx = setup();
    for delegate in [Delegate::Reachable, Delegate::Missing] {
        let out = classify(
            &fx,
            "I might run cargo test later\njust narration",
            None,
            delegate,
        );
        assert_eq!(
            out, "no-op (not live)",
            "non-live capture must be a no-op regardless of the delegate"
        );
    }
}

/// Single-source guard: the shipped helper delegates to `git paw __classify`
/// and carries no parallel classifier — none of the retired mirror functions or
/// verb/preset arrays survive in the asset.
#[test]
fn sweep_sh_carries_no_parallel_classifier() {
    let src = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/scripts/sweep.sh"
    ))
    .expect("read assets/scripts/sweep.sh");

    assert!(
        src.contains("git paw \"${classify_args[@]}\""),
        "sweep.sh must obtain its verdict from `git paw __classify`"
    );
    assert!(
        src.contains("__classify"),
        "sweep.sh must reference the __classify subcommand"
    );

    // The retired mirror functions (proposal §Why) and the verb/preset arrays
    // they matched against must be gone — their presence would mean a second
    // classifier is back.
    for retired in [
        "normalize_command",
        "is_dangerous",
        "protected_violation",
        "classified_safe",
        "detect_shape",
        "is_arbitrary",
        "select_option",
        "READ_MOSTLY",
        "EXPLICIT_SAFE",
        "DEV_UNIVERSAL",
        "STACK_RUST",
        "STACK_NODE",
        "STACK_PYTHON",
        "STACK_GO",
    ] {
        assert!(
            !src.contains(retired),
            "sweep.sh must not re-implement the classifier; found {retired:?}"
        );
    }
}
