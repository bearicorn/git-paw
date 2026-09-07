//! Behavioral integration tests for GP-10: `--supervisor --specs` must
//! resolve each spec branch's CLI through the exact same 5-level priority
//! chain the non-supervisor `--specs` path uses
//! (`interactive::resolve_cli_for_specs`), rather than a separate
//! `--cli > default_cli > [supervisor].cli` fallback that silently ignores
//! `default_spec_cli` and any per-spec `paw_cli` override.
//!
//! Mirrors `cli_resolution_integration.rs`'s `--dry-run` approach: the plan
//! line printed for each branch (`  <branch> → <cli> (../<wt_dir>)`) proves
//! what CLI a real launch would use, without touching tmux or creating a
//! worktree.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn git(repo: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

/// Writes a `.git-paw/config.toml` wiring `[supervisor]` (so `--supervisor`
/// does not need an interactive opt-in prompt) with `default_cli` /
/// `[supervisor].cli` deliberately set to `delta` — distinct from
/// `default_spec_cli` (`gamma`) — so a plan resolving to `delta` for a
/// spec lacking `paw_cli` would prove the bug (the old `agent_cli` fallback
/// bypassing `default_spec_cli`), while resolving to `gamma` proves the fix.
fn write_supervisor_specs_config(repo: &Path, default_spec_cli: Option<&str>) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");

    let mut cfg = String::new();
    cfg.push_str("default_cli = \"delta\"\n");
    if let Some(d) = default_spec_cli {
        let _ = writeln!(cfg, "default_spec_cli = \"{d}\"");
    }
    cfg.push_str("\n[specs]\ntype = \"openspec\"\ndir = \"specs\"\n");
    cfg.push_str("\n[supervisor]\nenabled = true\ncli = \"delta\"\n");
    for name in ["alpha", "beta", "gamma", "delta"] {
        let _ = write!(cfg, "\n[clis.{name}]\ncommand = \"echo\"\n");
    }
    fs::write(paw.join("config.toml"), cfg).expect("write config");
}

/// Writes and commits an `OpenSpec` change at `<repo>/specs/<id>/tasks.md`,
/// optionally seeding a `paw_cli:` frontmatter field (Priority 2).
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

/// Extracts the dry-run plan line for `spec/<id>` and asserts it resolved to
/// `cli` — see `cli_resolution_integration.rs::assert_branch_resolves`.
fn assert_branch_resolves(stdout: &str, id: &str, cli: &str) {
    let needle = format!("spec/{id} \u{2192} {cli} (");
    assert!(
        stdout.contains(&needle),
        "expected branch `spec/{id}` to resolve to `{cli}` (looking for `{needle}`);\ngot plan:\n{stdout}"
    );
}

/// The core GP-10 regression scenario (`cli-resolution` spec: "`default_spec_cli`
/// is honoured under --supervisor --specs"): with no `--cli` flag and no
/// per-spec `paw_cli`, every branch resolves to `default_spec_cli` — not the
/// session-wide `agent_cli` fallback (`default_cli`/`[supervisor].cli`,
/// `delta` here).
#[test]
fn supervisor_specs_honours_default_spec_cli_not_the_agent_cli_fallback() {
    let tr = setup_test_repo();
    write_supervisor_specs_config(tr.path(), Some("gamma"));
    write_committed_spec(tr.path(), "auth", None);
    write_committed_spec(tr.path(), "api", None);

    let out = cmd()
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--from-all-specs", "--dry-run"])
        .output()
        .expect("run start --supervisor --from-all-specs --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_branch_resolves(&stdout, "auth", "gamma");
    assert_branch_resolves(&stdout, "api", "gamma");
    assert!(
        !stdout.contains("\u{2192} delta ("),
        "no branch should resolve to the agent_cli/[supervisor].cli fallback \
         (delta) when default_spec_cli is configured; got:\n{stdout}"
    );
}

/// Priority parity: a per-spec `paw_cli` still wins over `default_spec_cli`
/// under `--supervisor --specs`, identical to the non-supervisor path.
#[test]
fn supervisor_specs_paw_cli_still_wins_over_default_spec_cli() {
    let tr = setup_test_repo();
    write_supervisor_specs_config(tr.path(), Some("gamma"));
    write_committed_spec(tr.path(), "auth", Some("beta"));
    write_committed_spec(tr.path(), "api", None);

    let out = cmd()
        .current_dir(tr.path())
        .args(["start", "--supervisor", "--from-all-specs", "--dry-run"])
        .output()
        .expect("run start --supervisor --from-all-specs --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_branch_resolves(&stdout, "auth", "beta"); // paw_cli beats config
    assert_branch_resolves(&stdout, "api", "gamma"); // default_spec_cli fills
}

/// Priority parity: `--cli` still overrides everything under
/// `--supervisor --specs`, including a per-spec `paw_cli` and
/// `default_spec_cli`.
#[test]
fn supervisor_specs_cli_flag_still_overrides_everything() {
    let tr = setup_test_repo();
    write_supervisor_specs_config(tr.path(), Some("gamma"));
    write_committed_spec(tr.path(), "auth", Some("beta"));
    write_committed_spec(tr.path(), "api", None);

    let out = cmd()
        .current_dir(tr.path())
        .args([
            "start",
            "--supervisor",
            "--from-all-specs",
            "--cli",
            "alpha",
            "--dry-run",
        ])
        .output()
        .expect("run start --supervisor --from-all-specs --cli alpha --dry-run");

    assert!(
        out.status.success(),
        "dry-run should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_branch_resolves(&stdout, "auth", "alpha");
    assert_branch_resolves(&stdout, "api", "alpha");
}
