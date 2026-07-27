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
//! slices add the `start` / `start --from-specs` / destructive-confirmation
//! rows; keep the matrix `#[serial]`-friendly and poll-based per the PTY
//! harness contract.

use assert_cmd::Command;

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
    let supervisor_enabled = cfg.supervisor.map(|s| s.enabled).unwrap_or(false);
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
