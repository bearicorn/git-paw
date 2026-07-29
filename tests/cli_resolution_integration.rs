//! Behavioral integration tests for the CLI resolution chain.
//!
//! Guards the deterministic (non-prompting) levels of the per-branch
//! resolve-CLI chain (`interactive::resolve_cli_for_specs`) and the
//! custom-CLI-over-detected merge (`detect::detect_clis`) end-to-end, by
//! driving the production binary and asserting the OBSERVABLE surface:
//!
//! - the `--dry-run` "session plan (from specs)" line per branch
//!   (`  <branch> → <cli> (../<wt_dir>)`), which prints the CLI each branch
//!   resolved to; and
//! - the `list-clis` table (`NAME  SOURCE  PATH`), which prints the resolved
//!   source of each available CLI.
//!
//! Only the deterministic chain levels are exercised here — Priority 1
//! (`--cli`), Priority 2 (per-spec `paw_cli`), and Priority 3
//! (`default_spec_cli`) — because they resolve WITHOUT an interactive prompt
//! and so run correctly in a non-TTY `--dry-run`. The picker levels
//! (Priority 4 `default_cli` pre-select, Priority 5 full picker) are PTY-only
//! and covered by the `cli-interaction-e2e` matrix; a non-TTY run must never
//! reach them, so every scenario below is constructed to resolve fully.
//!
//! All `--dry-run` scenarios stop before any tmux session or worktree is
//! created; `list-clis` never touches tmux. The tests are therefore
//! socket-safe.

use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Runs `git` in `repo` and asserts success.
fn git(repo: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

/// Writes a `.git-paw/config.toml` with the `OpenSpec` backend, an optional
/// `default_spec_cli`, and three custom CLIs (`alpha`, `beta`, `gamma`) all
/// backed by the real `echo` binary so detection succeeds without depending on
/// any AI CLI being installed. The three distinct `binary_name`s give the
/// resolution chain more than one candidate to choose between.
fn write_specs_config(repo: &Path, default_spec_cli: Option<&str>) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");

    let mut cfg = String::new();
    if let Some(d) = default_spec_cli {
        let _ = writeln!(cfg, "default_spec_cli = \"{d}\"");
    }
    cfg.push_str("\n[specs]\ntype = \"openspec\"\ndir = \"specs\"\n");
    for name in ["alpha", "beta", "gamma"] {
        let _ = write!(cfg, "\n[clis.{name}]\ncommand = \"echo\"\n");
    }
    fs::write(paw.join("config.toml"), cfg).expect("write config");
}

/// Writes and commits an `OpenSpec` change at `<repo>/specs/<id>/tasks.md`.
///
/// `paw_cli` optionally seeds a `paw_cli:` frontmatter field so the resolved
/// `SpecEntry.cli` (Priority 2) is set for that branch.
fn write_committed_spec(repo: &Path, id: &str, paw_cli: Option<&str>) {
    let dir = repo.join("specs").join(id);
    fs::create_dir_all(&dir).expect("create change dir");
    let body = match paw_cli {
        Some(c) => format!("---\npaw_cli: {c}\n---\nImplement {id}.\n"),
        None => format!("Implement {id}.\n"),
    };
    fs::write(dir.join("tasks.md"), body).expect("write tasks.md");
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", "add spec"]);
}

/// Creates an executable stub named `name` in `dir` (a fake detectable CLI).
fn make_fake_bin(dir: &Path, name: &str) {
    let p = dir.join(name);
    fs::write(&p, "#!/bin/sh\n").expect("write fake binary");
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod fake binary");
}

/// Extracts the dry-run plan line for `spec/<id>` and asserts it resolved to
/// `cli`. The plan format is `  <branch> → <cli> (../<wt_dir>)`; the
/// `→ <cli> (` fragment ties the branch to its resolved CLI unambiguously
/// (the worktree dir is derived from the branch, never the CLI name).
fn assert_branch_resolves(stdout: &str, id: &str, cli: &str) {
    let needle = format!("spec/{id} \u{2192} {cli} (");
    assert!(
        stdout.contains(&needle),
        "expected branch `spec/{id}` to resolve to `{cli}` (looking for `{needle}`);\ngot plan:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// Priority 1: --cli overrides everything (per-spec paw_cli AND default_spec_cli)
// ---------------------------------------------------------------------------

/// `--cli` (Priority 1) beats a per-spec `paw_cli` (Priority 2) AND a config
/// `default_spec_cli` (Priority 3): every branch resolves to the flag value.
#[test]
fn cli_flag_overrides_paw_cli_and_default_spec_cli_for_all_branches() {
    let tr = setup_test_repo();
    write_specs_config(tr.path(), Some("gamma"));
    write_committed_spec(tr.path(), "auth", Some("beta")); // has paw_cli
    write_committed_spec(tr.path(), "api", None); // would fall to default_spec_cli

    let out = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-all-specs", "--cli", "alpha", "--dry-run"])
        .output()
        .expect("run start --from-all-specs --cli alpha --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    // --cli alpha wins over paw_cli=beta and default_spec_cli=gamma for both.
    assert_branch_resolves(&stdout, "auth", "alpha");
    assert_branch_resolves(&stdout, "api", "alpha");
    assert!(
        !stdout.contains("\u{2192} beta ") && !stdout.contains("\u{2192} gamma "),
        "no branch should resolve to the overridden CLIs; got:\n{stdout}"
    );
}

// ---------------------------------------------------------------------------
// Priority 2: per-spec paw_cli wins over config default_spec_cli
// ---------------------------------------------------------------------------

/// With no `--cli`, a branch whose spec carries `paw_cli` (Priority 2) uses it,
/// while a branch without one falls to `default_spec_cli` (Priority 3) — a
/// single mixed launch resolves both without prompting.
#[test]
fn paw_cli_wins_over_default_spec_cli_and_fills_rest() {
    let tr = setup_test_repo();
    write_specs_config(tr.path(), Some("gamma"));
    write_committed_spec(tr.path(), "auth", Some("beta")); // paw_cli → beta
    write_committed_spec(tr.path(), "api", None); // default_spec_cli → gamma

    let out = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-all-specs", "--dry-run"])
        .output()
        .expect("run start --from-all-specs --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed (fully resolved, no prompt); stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_branch_resolves(&stdout, "auth", "beta"); // paw_cli beats config
    assert_branch_resolves(&stdout, "api", "gamma"); // default_spec_cli fills
}

// ---------------------------------------------------------------------------
// Priority 3: default_spec_cli fills all specs lacking paw_cli, no prompt
// ---------------------------------------------------------------------------

/// With no `--cli` and no spec carrying `paw_cli`, `default_spec_cli`
/// (Priority 3) resolves every branch with NO interactive prompt. Success in a
/// non-TTY `--dry-run` is itself the proof of "no prompt": had the chain fallen
/// through to the picker (Priority 4/5) it would have failed trying to enter
/// raw mode on a non-TTY rather than exiting 0 with a full plan.
#[test]
fn default_spec_cli_fills_all_branches_without_prompt() {
    let tr = setup_test_repo();
    write_specs_config(tr.path(), Some("gamma"));
    write_committed_spec(tr.path(), "auth", None);
    write_committed_spec(tr.path(), "api", None);
    write_committed_spec(tr.path(), "db", None);

    let out = cmd()
        .current_dir(tr.path())
        .args(["start", "--from-all-specs", "--dry-run"])
        .output()
        .expect("run start --from-all-specs --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed without prompting; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for id in ["auth", "api", "db"] {
        assert_branch_resolves(&stdout, id, "gamma");
    }
}

// ---------------------------------------------------------------------------
// Custom CLI overrides a DETECTED CLI of the same binary_name (list-clis)
// ---------------------------------------------------------------------------

/// A `[clis.<name>]` custom entry whose name collides with an auto-detected CLI
/// on PATH takes precedence: `list-clis` shows exactly one row for that name,
/// its SOURCE is `custom` (not `detected`), and its PATH is the custom command's
/// resolved binary (`echo`) — not the detected stub — proving the custom
/// definition won the merge.
#[test]
fn custom_cli_overrides_detected_cli_of_same_binary_name_in_list_clis() {
    let tr = setup_test_repo();

    // Custom `[clis.claude]` → the real `echo` binary, colliding with the
    // known/detectable `claude` name.
    let paw = tr.path().join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(
        paw.join("config.toml"),
        "[clis.claude]\ncommand = \"echo\"\ndisplay_name = \"Custom Claude\"\n",
    )
    .expect("write config");

    // A fake detectable `claude` on PATH; without the override this is what
    // detection would surface (source=detected, path=<stub>/claude).
    let fake_bin_dir = tempfile::TempDir::new().expect("fake bin dir");
    make_fake_bin(fake_bin_dir.path(), "claude");

    let inherited = std::env::var("PATH").unwrap_or_default();
    let path = format!("{}:{inherited}", fake_bin_dir.path().display());

    let out = cmd()
        .current_dir(tr.path())
        .env("PATH", path)
        .arg("list-clis")
        .output()
        .expect("run list-clis");

    assert!(
        out.status.success(),
        "list-clis should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);

    // Match the row whose NAME column is exactly `claude` — not a prefix, so a
    // sibling custom CLI like `claude-oss` (which a real global config may
    // define) is not mistaken for it.
    let claude_rows: Vec<&str> = stdout
        .lines()
        .filter(|l| l.split_whitespace().next() == Some("claude"))
        .collect();
    assert_eq!(
        claude_rows.len(),
        1,
        "custom must override detected → exactly one `claude` row; got:\n{stdout}"
    );
    let row = claude_rows[0];
    assert!(
        row.contains("custom"),
        "`claude` must resolve with source=custom (override won), not detected; got: {row}"
    );
    assert!(
        row.contains("echo") && !row.trim_end().ends_with("/claude"),
        "`claude` row PATH must be the custom command's binary (echo), not the detected stub; got: {row}"
    );
}
