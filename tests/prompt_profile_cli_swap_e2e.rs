//! A CLI swap on a kept worktree re-resolves the pane's prompt-shape profile
//! (`cli-prompt-profiles`, requirement *A surface's CLI is taken from
//! git-paw's launch record*).
//!
//! Drives the real binary on an isolated tmux socket and `HOME`: start
//! `feat/x` under `cat`, `remove --keep-worktree`, then `add --cli mycli`.
//! The launch records git-paw then holds are what the drive loop (the session
//! receipt) and the helper (the per-repo discovery roster) resolve the pane's
//! CLI from. Skips when tmux is unavailable.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use git_paw::agent_cli::CliProfile;
use git_paw::config::load_config_from;
use git_paw::supervisor::permission_prompt::has_approval_marker;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

/// A capture only `mycli`'s `approval_markers` override matches.
const MYCLI_CAPTURE: &str = "mycli is waiting for go-ahead on: make deploy";

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// A supervisor config (broker off) whose default CLI is `cat`, plus `mycli`
/// — a stub on a test `PATH` that `exec`s `cat` — carrying an
/// `approval_markers` override. Returns the `PATH` to run `git paw` with.
fn write_swap_config(repo: &Path, bin_dir: &Path) -> String {
    let stub = bin_dir.join("mycli");
    fs::write(&stub, "#!/bin/sh\nexec cat\n").expect("write stub CLI");
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("chmod stub");

    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n[supervisor]\nenabled = true\ncli = \"cat\"\n\n\
         [clis.mycli]\ncommand = \"mycli\"\n\n\
         [clis.mycli.prompt_profile]\napproval_markers = [\"waiting for go-ahead\"]\n",
    )
    .expect("write config");

    let inherited = std::env::var("PATH").unwrap_or_default();
    format!("{}:{inherited}", bin_dir.display())
}

/// Run `git paw <args>` in `repo` against the test socket and `HOME`.
fn paw(
    repo: &Path,
    home: &Path,
    tmux_env: &TmuxTestEnv,
    path_env: &str,
    args: &[&str],
) -> std::process::Output {
    let mut c = Command::cargo_bin("git-paw").expect("binary exists");
    tmux_env.apply_assert(&mut c);
    c.current_dir(repo)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .env("PATH", path_env)
        .args(args)
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run git paw")
}

fn assert_ok(out: &std::process::Output, what: &str) {
    assert!(
        out.status.success(),
        "{what} failed; stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The global session receipt under an isolated `HOME`.
fn read_receipt(home: &Path, session: &str) -> serde_json::Value {
    let dir = if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    };
    let raw = fs::read_to_string(dir.join(format!("{session}.json"))).expect("read receipt");
    serde_json::from_str(&raw).expect("parse receipt")
}

/// The per-repo discovery roster the helper reads.
fn read_discovery(repo: &Path, session: &str) -> serde_json::Value {
    let path = repo.join(format!(".git-paw/sessions/{session}.json"));
    let raw = fs::read_to_string(path).expect("read discovery");
    serde_json::from_str(&raw).expect("parse discovery")
}

fn kill(tmux_env: &TmuxTestEnv, session: &str) {
    let mut c = StdCommand::new("tmux");
    tmux_env.apply(&mut c);
    let _ = c.args(["kill-session", "-t", session]).status();
}

/// Spec scenario "A CLI swap on a kept worktree re-resolves the profile".
#[test]
#[serial]
fn a_cli_swap_on_a_kept_worktree_re_resolves_the_profile() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let tr = setup_test_repo();
    let repo = tr.path();
    let bin = TempDir::new().expect("bin");
    let path_env = write_swap_config(repo, bin.path());
    let home = TempDir::new().expect("home");
    let tmux_env = tmux_test_env();

    let started = paw(
        repo,
        home.path(),
        &tmux_env,
        &path_env,
        &["start", "--supervisor", "--branches", "feat/x"],
    );
    assert_ok(&started, "start");
    let session = String::from_utf8_lossy(&started.stdout)
        .lines()
        .find(|l| l.contains("tmux attach -t"))
        .and_then(|l| l.split_whitespace().last())
        .expect("session name in start stdout")
        .to_string();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_ok(
            &paw(
                repo,
                home.path(),
                &tmux_env,
                &path_env,
                &["remove", "feat/x", "--keep-worktree"],
            ),
            "remove --keep-worktree",
        );
        assert_ok(
            &paw(
                repo,
                home.path(),
                &tmux_env,
                &path_env,
                &["add", "feat/x", "--cli", "mycli"],
            ),
            "add --cli mycli",
        );

        let clis = load_config_from(&repo.join("no-global.toml"), repo)
            .expect("config loads")
            .clis;

        // The drive loop reads the pane's CLI from the session receipt.
        let receipt = read_receipt(home.path(), &session);
        let loop_cli = receipt["worktrees"]
            .as_array()
            .expect("worktrees")
            .iter()
            .find(|w| w["branch"] == "feat/x")
            .and_then(|w| w["cli"].as_str())
            .expect("feat/x recorded with a CLI")
            .to_string();
        assert_eq!(loop_cli, "mycli", "receipt: {receipt}");
        let loop_profile = CliProfile::resolve(&loop_cli, &clis)
            .compiled_prompt_profile()
            .expect("compiles");
        assert!(
            has_approval_marker(MYCLI_CAPTURE, &loop_profile),
            "the loop reads the swapped pane with mycli's override"
        );

        // The helper reads the pane's CLI from the discovery roster and
        // resolves it through `__prompt-profile --cli`.
        let discovery = read_discovery(repo, &session);
        let helper_cli = discovery["agents"]
            .as_array()
            .expect("agents")
            .iter()
            .find(|a| a["branch_id"] == "feat-x")
            .and_then(|a| a["cli"].as_str())
            .expect("feat-x listed with a CLI")
            .to_string();
        assert_eq!(helper_cli, "mycli", "discovery: {discovery}");
        let field = Command::cargo_bin("git-paw")
            .expect("binary exists")
            .current_dir(repo)
            .env("HOME", home.path())
            .args([
                "__prompt-profile",
                "--cli",
                &helper_cli,
                "--field",
                "approval_markers",
            ])
            .output()
            .expect("run __prompt-profile");
        assert!(field.status.success());
        assert_eq!(
            String::from_utf8_lossy(&field.stdout).trim(),
            "waiting for go-ahead",
            "the helper resolves mycli's override"
        );
    }));
    kill(&tmux_env, &session);
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
