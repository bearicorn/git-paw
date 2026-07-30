//! `cli-interaction-e2e` — the interactive prompt-gating matrix.
//!
//! Asserts which prompts are SHOWN vs BYPASSED as a function of config + flags +
//! TTY. TTY "shown" rows are driven via the shared PTY harness
//! (`helpers::pty`, in `init_interactive_specs.rs` and future slices here);
//! non-TTY "bypassed" rows are driven via `assert_cmd` — piped stdin is a
//! non-TTY, so the interactive prompts are skipped deterministically (no tmux,
//! no flake).
//!
//! This slice covers the `git paw init` family's non-TTY bypass rows. Later
//! slices add the `start` / `--from-all-specs` (+ bare `--specs`) /
//! destructive-confirmation rows; keep the matrix `#[serial]`-friendly and
//! poll-based per the PTY harness contract. The deprecated `--from-specs` alias
//! (removal at v1.0.0) is deliberately NOT exercised.

use assert_cmd::Command;

mod helpers;
use helpers::{TmuxTestEnv, pty, setup_test_repo};
use serial_test::serial;
use std::fmt::Write as _;
use std::time::Duration;

/// init, non-TTY: the supervisor `Confirm` and the spec-system `Select` are
/// bypassed. The written config has no *active* `[specs]` section (the base
/// template's commented example only), and supervisor is not enabled.
#[test]
fn init_non_tty_bypasses_prompts_and_writes_commented_template() {
    let repo = tempfile::TempDir::new().expect("tempdir");
    // `init` does not require a repo, but keep the fixture realistic.
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status();

    // assert_cmd pipes stdin by default → the child sees a non-TTY.
    Command::cargo_bin("git-paw")
        .expect("binary")
        .arg("init")
        .current_dir(repo.path())
        .assert()
        .success();

    let config_path = repo.path().join(".git-paw").join("config.toml");
    let content = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| panic!("non-TTY init must still write config: {e}"));
    let cfg: git_paw::config::PawConfig =
        toml::from_str(&content).unwrap_or_else(|e| panic!("parse config: {e}\n{content}"));

    assert!(
        cfg.specs.is_none(),
        "non-TTY init must leave [specs] commented (no active section chosen):\n{content}"
    );
    let supervisor_enabled = cfg.supervisor.is_some_and(|s| s.enabled);
    assert!(
        !supervisor_enabled,
        "non-TTY init must not enable supervisor (Confirm bypassed):\n{content}"
    );
}

/// init on an already-initialised repo is idempotent in non-TTY mode: it
/// succeeds without prompting (the migrate-supervisor `Confirm` is a TTY-only
/// row, exercised in the PTY slice).
#[test]
fn init_non_tty_is_idempotent() {
    let repo = tempfile::TempDir::new().expect("tempdir");
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status();

    for _ in 0..2 {
        Command::cargo_bin("git-paw")
            .expect("binary")
            .arg("init")
            .current_dir(repo.path())
            .assert()
            .success();
    }

    let config_path = repo.path().join(".git-paw").join("config.toml");
    assert!(
        config_path.is_file(),
        "config should exist after idempotent non-TTY init"
    );
}

// ---------------------------------------------------------------------------
// start family — BYPASS rows (deterministic via `--dry-run`, which resolves
// selection and prints the plan without launching tmux). The TTY "shown"
// picker rows are driven via the PTY harness in a later slice.
// ---------------------------------------------------------------------------

/// Writes a minimal `.git-paw/config.toml` with a fake `echo` CLI so detection
/// succeeds. When `supervisor` is `Some`, seeds a `[supervisor]` section with
/// that `enabled` value plus an `echo` CLI and a trivial test command.
fn write_echo_config(repo: &std::path::Path, supervisor: Option<bool>) {
    let paw = repo.join(".git-paw");
    std::fs::create_dir_all(&paw).expect("create .git-paw");
    let mut cfg = String::from(
        "default_cli = \"echo\"\n\n[clis.echo]\ncommand = \"echo\"\ndisplay_name = \"Echo\"\n",
    );
    if let Some(enabled) = supervisor {
        let _ = write!(
            cfg,
            "\n[supervisor]\nenabled = {enabled}\ncli = \"echo\"\ntest_command = \"true\"\nagent_approval = \"manual\"\n"
        );
    }
    std::fs::write(paw.join("config.toml"), cfg).expect("write config");
}

/// start with all selection flags given: the branch picker, mode picker, and
/// CLI picker are all BYPASSED. `--dry-run` prints the plan (both branches
/// present, no prompt, no launch).
#[test]
fn start_all_flags_bypass_all_pickers() {
    let tr = setup_test_repo();
    write_echo_config(tr.path(), None);

    let out = Command::cargo_bin("git-paw")
        .expect("binary")
        .current_dir(tr.path())
        .args([
            "start",
            "--branches",
            "feat/a,feat/b",
            "--cli",
            "echo",
            "--dry-run",
        ])
        .output()
        .expect("run start --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Dry run"),
        "expected a dry-run plan; got:\n{stdout}"
    );
    assert!(
        stdout.contains("feat/a") && stdout.contains("feat/b"),
        "both --branches must appear in the plan (branch picker bypassed); got:\n{stdout}"
    );
}

/// start `--supervisor`: supervisor mode is entered because the explicit flag
/// short-circuits the resolution chain (no "Start in supervisor mode?" Confirm),
/// even though the config's `[supervisor] enabled = false` would not route there.
#[test]
fn start_supervisor_flag_enters_supervisor_mode() {
    let tr = setup_test_repo();
    write_echo_config(tr.path(), Some(false));

    let out = Command::cargo_bin("git-paw")
        .expect("binary")
        .current_dir(tr.path())
        .args([
            "start",
            "--branches",
            "feat/a",
            "--cli",
            "echo",
            "--supervisor",
            "--dry-run",
        ])
        .output()
        .expect("run start --supervisor --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Supervisor:"),
        "--supervisor must enter supervisor mode (flag short-circuits the confirm); got:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// start family — SHOWN rows (TTY, driven via the PTY harness). We assert the
// picker RENDERS (the "shown" gate) inside a `--dry-run` run, then tear down —
// driving the fuzzy multi-select / env-dependent CLI list to completion is a
// separate, flakier concern. `#[serial]` + tmux-availability-gated + socket-
// isolated, per the harness contract.
// ---------------------------------------------------------------------------

fn git(repo: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .expect("git");
    assert!(status.success(), "git {args:?} failed");
}

/// start with NO selection flags in a TTY: the branch picker is SHOWN.
#[test]
#[serial]
fn start_no_flags_shows_branch_picker() {
    if !pty::tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tmux_env = TmuxTestEnv::new();
    let _proc = tmux_env.apply_to_process();

    let tr = setup_test_repo();
    write_echo_config(tr.path(), None);
    git(tr.path(), &["branch", "feat/a"]);
    git(tr.path(), &["branch", "feat/b"]);

    let session = pty::unique_session_name("paw-matrix-branchpicker");
    pty::create_detached_session(&session);
    let bin = env!("CARGO_BIN_EXE_git-paw");
    let cmd = format!("cd '{}' && '{bin}' start --dry-run", tr.path().display());
    pty::send_keys(&session, &[&cmd, "Enter"]);

    // No --branches → the branch picker renders.
    pty::wait_for_pane(&session, "Select branches", Duration::from_secs(10));
    pty::kill_session(&session);
}

/// start `--branches` but NO `--cli` in a TTY: the branch picker is BYPASSED,
/// and the CLI-assignment mode picker is SHOWN (because `--cli` is absent).
#[test]
#[serial]
fn start_branches_without_cli_shows_mode_picker() {
    if !pty::tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tmux_env = TmuxTestEnv::new();
    let _proc = tmux_env.apply_to_process();

    let tr = setup_test_repo();
    write_echo_config(tr.path(), None);

    let session = pty::unique_session_name("paw-matrix-modepicker");
    pty::create_detached_session(&session);
    let bin = env!("CARGO_BIN_EXE_git-paw");
    let cmd = format!(
        "cd '{}' && '{bin}' start --branches feat/a,feat/b --dry-run",
        tr.path().display()
    );
    pty::send_keys(&session, &[&cmd, "Enter"]);

    // --branches bypasses the branch picker; --cli absent → the mode picker renders.
    pty::wait_for_pane(&session, "CLI assignment mode", Duration::from_secs(10));
    pty::kill_session(&session);
}

// ---------------------------------------------------------------------------
// destructive-confirmation gating (deterministic assert_cmd, non-TTY).
// ---------------------------------------------------------------------------

/// stop is non-destructive and renders NO confirmation prompt (the `cli-parsing`
/// spec was reconciled to match `cmd_stop` — see spec-traceability-audit). With
/// no active session it reports so and exits 0, without prompting.
#[test]
fn stop_does_not_prompt() {
    let repo = tempfile::TempDir::new().expect("tempdir");
    git(repo.path(), &["init", "-q"]);

    Command::cargo_bin("git-paw")
        .expect("binary")
        .arg("stop")
        .current_dir(repo.path())
        .assert()
        .success();
}

/// purge `--force`: the confirmation is BYPASSED; it exits 0 without prompting.
#[test]
fn purge_force_bypasses_confirmation() {
    let repo = tempfile::TempDir::new().expect("tempdir");
    git(repo.path(), &["init", "-q"]);

    Command::cargo_bin("git-paw")
        .expect("binary")
        .args(["purge", "--force"])
        .current_dir(repo.path())
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// spec-launch prompt gating — BYPASS / dispatch rows (deterministic via
// `--dry-run`; the shown side for bare `--specs` is a PTY row, see the note at
// the CLI-picker section). The deprecated `--from-specs` alias (removal at
// v1.0.0) is deliberately NOT exercised.
// ---------------------------------------------------------------------------

/// Writes an `OpenSpec` `[specs]` config (+ echo CLI) and commits one
/// `specs/<id>/tasks.md` per id so `--from-all-specs` discovers them.
fn write_openspec_specs_repo(repo: &std::path::Path, ids: &[&str]) {
    let paw = repo.join(".git-paw");
    std::fs::create_dir_all(&paw).expect("create .git-paw");
    std::fs::write(
        paw.join("config.toml"),
        "default_cli = \"echo\"\n\n[clis.echo]\ncommand = \"echo\"\ndisplay_name = \"Echo\"\n\n[specs]\ntype = \"openspec\"\ndir = \"specs\"\n",
    )
    .expect("write config");
    for id in ids {
        let d = repo.join("specs").join(id);
        std::fs::create_dir_all(&d).expect("spec dir");
        std::fs::write(d.join("tasks.md"), format!("Implement {id}.\n")).expect("tasks.md");
    }
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", "specs"]);
}

/// `--from-all-specs` bypasses the spec picker and dispatches EVERY discovered
/// spec: the `--dry-run` plan lists both spec branches, with no prompt (a
/// non-TTY run would hang on a picker if one were shown). `--cli echo` keeps
/// CLI resolution non-interactive so the run isolates the spec-picker bypass.
#[test]
fn from_all_specs_launches_every_spec_without_picker() {
    let tr = setup_test_repo();
    write_openspec_specs_repo(tr.path(), &["auth", "api"]);

    let out = Command::cargo_bin("git-paw")
        .expect("binary")
        .current_dir(tr.path())
        .args(["start", "--from-all-specs", "--cli", "echo", "--dry-run"])
        .output()
        .expect("run start --from-all-specs --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed without prompting; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("spec/auth") && stdout.contains("spec/api"),
        "both discovered specs must be dispatched (picker bypassed); got:\n{stdout}"
    );
}

/// `--from-all-specs` with NEITHER `--specs-format` NOR a `[specs]` section
/// SHALL error with explicit-only guidance, never silently auto-detect (the
/// v0.12.0 no-filesystem-detection rule).
#[test]
fn from_all_specs_unconfigured_spec_format_errors() {
    let tr = setup_test_repo();
    // echo CLI configured, but NO [specs] section and no --specs-format flag.
    write_echo_config(tr.path(), None);

    let out = Command::cargo_bin("git-paw")
        .expect("binary")
        .current_dir(tr.path())
        .args(["start", "--from-all-specs", "--dry-run"])
        .output()
        .expect("run start --from-all-specs with no specs config");

    assert!(
        !out.status.success(),
        "unconfigured spec-format must error, not silently succeed; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let stderr = String::from_utf8_lossy(&out.stderr).to_lowercase();
    assert!(
        stderr.contains("specs") || stderr.contains("specs-format"),
        "error must give explicit-only spec-config guidance; got stderr:\n{stderr}"
    );
}

/// bare `--specs` (no values) on a TTY SHOWS the spec multi-select picker. The
/// behavioral replacement for the source-grep `cli_specs_tty_proceeds_to_picker`
/// test — driven via the PTY harness: assert the picker RENDERS, then tear down
/// (driving the fuzzy multi-select to completion is a separate, flakier concern).
#[test]
#[serial]
fn bare_specs_on_tty_shows_spec_picker() {
    if !pty::tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tmux_env = TmuxTestEnv::new();
    let _proc = tmux_env.apply_to_process();

    let tr = setup_test_repo();
    write_openspec_specs_repo(tr.path(), &["auth", "api"]);

    let session = pty::unique_session_name("paw-matrix-specpicker");
    pty::create_detached_session(&session);
    let bin = env!("CARGO_BIN_EXE_git-paw");
    let cmd = format!(
        "cd '{}' && '{bin}' start --specs --dry-run",
        tr.path().display()
    );
    pty::send_keys(&session, &[&cmd, "Enter"]);

    // bare --specs → the spec picker renders (it runs during resolution, before
    // --dry-run's plan short-circuits execution).
    pty::wait_for_pane(&session, "Select specs", Duration::from_secs(10));
    pty::kill_session(&session);
}

// NOTE — the uniform / per-branch CLI-picker SHOWN rows are deliberately NOT
// driven here: reaching the CLI picker requires first passing the mode picker
// (a fuzzy drive-to-completion = flaky). They are covered instead by: the
// bypass side (`start_all_flags_bypass_all_pickers`), the resolution-chain
// short-circuit (`tests/cli_resolution_integration.rs`), and the mode-picker
// render precursor (`start_branches_without_cli_shows_mode_picker`).
