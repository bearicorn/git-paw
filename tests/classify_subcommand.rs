//! Behavioral tests for the hidden `git paw __classify` subcommand — the IPC
//! seam that makes the safe-command classifier single-source (capability
//! `approval-command-safety`, requirement "Safe-command classification is
//! exposed via a hidden __classify subcommand").
//!
//! Each test drives the real binary in a `tempfile` git repo, pipes a scripted
//! pane capture on stdin, and asserts the printed verdict. Every row is ALSO
//! classified in-process through `classify_prompt` — the function the in-tool
//! auto-approver consumes — over the same resolved inputs, so the two cannot
//! disagree.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;

use assert_cmd::Command;
use git_paw::supervisor::auto_approve::ProtectedPaths;
use git_paw::supervisor::drive::{PromptVerdict, classify_prompt};
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        let st = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
        assert!(st.status.success(), "git {args:?} failed");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "# test").expect("write readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// A temp repo with `git paw init` run and the `rust` stack declared, so the
/// composed whitelist carries the toolchain verbs a classifier row needs.
struct Fixture {
    _tmp: TempDir,
    root: PathBuf,
}

fn setup() -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    let out = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("git paw init");
    assert!(out.status.success(), "git paw init must succeed");
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
    Fixture { _tmp: tmp, root }
}

/// Runs `git paw __classify` in the fixture repo with `capture` on stdin and
/// returns `(stdout, exit_ok)`.
fn classify_via_cli(
    fx: &Fixture,
    capture: &str,
    worktree_root: Option<&Path>,
    resolve_option: bool,
) -> (String, bool) {
    let bin = assert_cmd::cargo::cargo_bin("git-paw");
    let mut c = StdCommand::new(bin);
    c.arg("__classify");
    if let Some(root) = worktree_root {
        c.arg("--worktree-root").arg(root);
    }
    if resolve_option {
        c.arg("--resolve-option");
    }
    let mut child = c
        .current_dir(&fx.root)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
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
    (
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        out.status.success(),
    )
}

/// Classifies `capture` in-process the way the in-tool auto-approver does — the
/// reference verdict every CLI row is compared against.
///
/// The classifier inputs are built the way the **unattended drive loop** builds
/// them in `src/commands/supervisor.rs` (`cmd_supervisor`'s
/// `run_unattended_drive_loop`), deliberately including the
/// `AutoApproveConfig::resolved()` step that `cmd_classify` skips. That skip is
/// safe today only because `resolved()` touches neither `safe_commands` nor
/// `approve_worktree_writes`; pinning the reference to the real drive-loop
/// construction — rather than mirroring `cmd_classify`'s choices — is what makes
/// this test FAIL if `resolved()` ever gains a field that does. A reference that
/// skipped it too would silently agree with a divergence.
fn classify_in_process(fx: &Fixture, capture: &str, worktree_root: Option<&Path>) -> PromptVerdict {
    let config = git_paw::config::load_config(&fx.root, None).expect("load fixture config");
    let supervisor = config.supervisor.clone().unwrap_or_default();
    let auto_approve = supervisor
        .auto_approve
        .clone()
        .unwrap_or_default()
        .resolved();
    classify_prompt(
        capture,
        &auto_approve.effective_whitelist(&supervisor.common_dev_allowlist),
        worktree_root,
        auto_approve.approve_worktree_writes(),
        &ProtectedPaths::derive(&config, Some(&fx.root)),
    )
}

/// A capture whose command slice is `cmd`, wrapped in a 2-option prompt.
fn two_option(cmd: &str) -> String {
    format!("Bash command\n  {cmd}\nDo you want to proceed?\nEsc to cancel")
}

/// A capture whose command slice is `cmd`, wrapped in a 3-option prompt that
/// offers the permanent broad grant.
fn three_option(cmd: &str) -> String {
    format!(
        "Bash command\n  {cmd}\nDo you want to proceed?\n\
         1. Yes\n2. Yes, and don't ask again for: {cmd}\n3. No\nEsc to cancel"
    )
}

/// Spec scenarios "__classify emits the classifier's verdict for a danger
/// command", "__classify emits a safe verdict with the option index", and the
/// unknown class: the printed `<class> <option>` matches the expectation AND
/// the in-process classifier's verdict for the same capture.
#[test]
#[allow(clippy::too_many_lines)]
fn classify_matrix_matches_the_in_tool_classifier() {
    let fx = setup();
    let root = fx.root.clone();

    let rows: Vec<(&str, String, Option<&Path>, &str, u8)> = vec![
        (
            "danger command escalates",
            two_option("git push --force origin main"),
            None,
            "danger",
            1,
        ),
        (
            // The broad-grant rule is unchanged by this de-duplication: a
            // read-mostly leading verb still resolves the durable option even
            // when the verdict escalates (only the auto-approver's danger-first
            // precedence keeps it from ever being sent unattended).
            "danger command keeps the read-mostly durable-option rule",
            three_option("git push --force origin main"),
            None,
            "danger",
            2,
        ),
        (
            "arbitrary-code runner never resolves the durable grant",
            three_option("python3 -c import os"),
            None,
            "unknown",
            1,
        ),
        (
            "safe stack command takes the durable option",
            three_option("cargo test --lib"),
            None,
            "safe",
            2,
        ),
        (
            "safe read-mostly verb on a 2-option prompt",
            two_option("grep -rn foo src/"),
            None,
            "safe",
            1,
        ),
        (
            "unknown command escalates",
            two_option("someprog --do-thing"),
            None,
            "unknown",
            1,
        ),
        (
            "worktree-confined commit is safe",
            two_option("git commit -m wip"),
            Some(root.as_path()),
            "safe",
            1,
        ),
        (
            "scratch delete is safe",
            two_option("rm -rf /tmp/paw-build-1"),
            None,
            "safe",
            1,
        ),
        (
            "exit-probe wrapper does not rescue a danger command",
            two_option("git push --force origin main; echo $?"),
            None,
            "danger",
            1,
        ),
        (
            // GP-02a parity: the leading-assignment/env/nohup normalization
            // the classifier applies must agree between the CLI seam and the
            // in-tool classifier.
            "leading assignment prefix normalizes to the bare safe command",
            three_option("TMPDIR=/tmp/x cargo test --lib"),
            None,
            "safe",
            2,
        ),
        (
            "leading env wrapper normalizes to the bare safe command",
            two_option("env FOO=bar cargo build"),
            None,
            "safe",
            1,
        ),
        (
            "leading nohup wrapper does not rescue a danger command",
            two_option("nohup git push --force origin main"),
            None,
            "danger",
            1,
        ),
        (
            // GP-02b parity: a bundled helper-script invocation is safe.
            "managed helper-script invocation is safe",
            two_option(".git-paw/scripts/broker.sh --agent feat-x status booting"),
            None,
            "safe",
            1,
        ),
        (
            "managed helper-script chained with a danger op still escalates",
            two_option(".git-paw/scripts/sweep.sh snapshot && rm -rf /"),
            None,
            "danger",
            1,
        ),
        (
            // GP-04b parity: a write under `.git/` escalates even though
            // `echo` is a read-mostly verb (which still keeps the read-mostly
            // durable-option rule on the escalating verdict, exactly like the
            // "danger command keeps the read-mostly durable-option rule" row
            // above — only the auto-approver's danger-first precedence keeps
            // it from ever being sent unattended).
            "write under .git/ escalates as danger",
            three_option("echo '.git-paw/' >> .git/info/exclude"),
            Some(root.as_path()),
            "danger",
            2,
        ),
    ];

    for (name, capture, worktree_root, expected_class, expected_option) in rows {
        let (stdout, ok) = classify_via_cli(&fx, &capture, worktree_root, false);
        assert!(ok, "__classify must exit 0 for row: {name}");
        assert_eq!(
            stdout,
            format!("{expected_class} {expected_option}"),
            "unexpected verdict for row: {name}"
        );

        // Parity: the same capture through the in-tool classifier.
        let reference = classify_in_process(&fx, &capture, worktree_root);
        assert_eq!(
            reference.label(),
            expected_class,
            "in-tool classifier disagrees on class for row: {name}"
        );
        if let PromptVerdict::Safe { option_index, .. } = reference {
            assert_eq!(
                option_index, expected_option,
                "in-tool classifier disagrees on option index for row: {name}"
            );
        }
    }
}

/// Spec scenario "__classify emits a safe verdict with the option index",
/// `--resolve-option` half: the flag prints only the index the in-tool
/// auto-approver would select.
#[test]
fn resolve_option_prints_only_the_option_index() {
    let fx = setup();
    let (stdout, ok) = classify_via_cli(&fx, &three_option("cargo test --lib"), None, true);
    assert!(ok, "__classify --resolve-option must exit 0");
    assert_eq!(stdout, "2", "durable grant index expected, got: {stdout}");

    let (stdout, ok) = classify_via_cli(&fx, &two_option("cargo test --lib"), None, true);
    assert!(ok, "__classify --resolve-option must exit 0");
    assert_eq!(stdout, "1", "2-option prompt selects Yes, got: {stdout}");
}

/// Recursively collects `(relative path, byte length)` for every file under
/// `dir`, sorted — the observable file-state snapshot the read-only assertion
/// compares.
fn file_snapshot(dir: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push(path),
                Ok(t) if t.is_file() => {
                    let rel = path
                        .strip_prefix(dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    let len = entry.metadata().map(|m| m.len()).unwrap_or_default();
                    out.push((rel, len));
                }
                _ => {}
            }
        }
    }
    out.sort();
    out
}

/// Spec scenario "__classify is read-only": it emits a verdict on stdout and
/// leaves no observable side effect — no file in the repo is created, removed,
/// or resized, and it needs neither a tmux session (`TMUX` is unset, so no
/// keystroke can be dispatched) nor a reachable broker.
#[test]
fn classify_is_read_only() {
    let fx = setup();
    let before = file_snapshot(&fx.root);

    let (stdout, ok) = classify_via_cli(&fx, &three_option("cargo test --lib"), None, false);
    assert!(ok, "__classify must succeed without tmux or a broker");
    assert_eq!(
        stdout, "safe 2",
        "verdict expected on stdout, got: {stdout}"
    );

    let after = file_snapshot(&fx.root);
    assert_eq!(
        before, after,
        "__classify must not create, remove, or modify any file"
    );
}

/// Spec scenario "__classify is hidden from help": the real binary's root
/// `--help` never advertises the internal subcommand, yet invoking it works.
#[test]
fn classify_is_hidden_from_root_help() {
    let out = cmd()
        .arg("--help")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("git paw --help");
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(
        !help.contains("__classify"),
        "root --help must not surface __classify; got: {help}"
    );
}
