//! Integration tests for the per-agent-worktree allowlist seeding
//! (`dev-command-allowlist` "Per-worktree placement for agent panes" and
//! `curl-allowlist` "Helper allowlist seeded per agent worktree").
//!
//! The tests target `seed_worktree_allowlists`, the wiring seam invoked from
//! `attach_agent` (shared by `git paw start` and `git paw add`) and from
//! `recover_session` in `src/main.rs` — the same helper-level strategy
//! `dev_allowlist_integration.rs` uses, because driving those commands
//! end-to-end requires tmux + a real CLI. Unlike that suite, every fixture
//! here is a REAL linked git worktree created with `git worktree add`, so
//! the worktree-local `info/exclude` behaviour is exercised against genuine
//! `.git`-file indirection, without touching tmux or any live session.

use std::path::{Path, PathBuf};
use std::process::Command;

use git_paw::config::CommonDevAllowlistConfig;
use git_paw::supervisor::dev_allowlist::{DEV_ALLOWLIST_PRESET, RUST_STACK_PRESET};
use git_paw::supervisor::worktree_allowlist::seed_worktree_allowlists;
use tempfile::TempDir;

/// Runs a git command in `dir`, asserting success, and returns stdout.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Initialises a repo with one commit and a TRACKED `.gitignore` (ignoring
/// `.git-paw/`, as `git paw init` does), mirroring a consumer repo.
fn init_repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-b", "main"]);
    git(dir, &["config", "user.email", "test@test.com"]);
    git(dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("README.md"), "# test").unwrap();
    std::fs::write(dir.join(".gitignore"), ".git-paw/\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "initial"]);
}

/// Creates a linked worktree for a new branch at `path`.
fn add_worktree(repo: &Path, branch: &str, path: &Path) {
    git(
        repo,
        &["worktree", "add", "-b", branch, path.to_str().unwrap()],
    );
}

fn read_array(path: &Path) -> Vec<String> {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    v.get("allowed_bash_prefixes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn dev_cfg(enabled: bool, stacks: &[&str], extra: &[&str]) -> CommonDevAllowlistConfig {
    CommonDevAllowlistConfig {
        enabled,
        stacks: stacks.iter().map(ToString::to_string).collect(),
        extra: extra.iter().map(ToString::to_string).collect(),
    }
}

fn settings_path(worktree: &Path) -> PathBuf {
    worktree.join(".claude").join("settings.json")
}

/// 2.1 / scenario "Start seeds every agent worktree": seeding each worktree
/// the way `cmd_supervisor`'s per-branch loop does lands the universal
/// preset, the selected stack patterns, and the helper-path prefixes in
/// every worktree's own `.claude/settings.json`.
#[test]
fn start_seeds_every_worktree_with_patterns_and_helper_prefixes() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);
    let wt_a = sandbox.path().join("repo-feat-a");
    let wt_b = sandbox.path().join("repo-feat-b");
    add_worktree(&repo, "feat-a", &wt_a);
    add_worktree(&repo, "feat-b", &wt_b);

    let cfg = dev_cfg(true, &["rust"], &[]);
    for wt in [&wt_a, &wt_b] {
        let failures = seed_worktree_allowlists(wt, true, false, Some(&cfg));
        assert!(failures.is_empty(), "unexpected failures: {failures:?}");
    }

    for wt in [&wt_a, &wt_b] {
        let entries = read_array(&settings_path(wt));
        for pat in DEV_ALLOWLIST_PRESET {
            assert!(
                entries.iter().any(|e| e == pat),
                "worktree {} missing universal pattern {pat:?}: {entries:?}",
                wt.display()
            );
        }
        for pat in RUST_STACK_PRESET {
            assert!(
                entries.iter().any(|e| e == pat),
                "worktree {} missing rust pattern {pat:?}: {entries:?}",
                wt.display()
            );
        }
        assert!(
            entries.iter().any(|e| e == ".git-paw/scripts/broker.sh"),
            "worktree {} missing broker helper grant: {entries:?}",
            wt.display()
        );
        assert!(
            entries
                .iter()
                .any(|e| e == "bash .git-paw/scripts/broker.sh"),
            "worktree {} missing `bash <helper>` grant: {entries:?}",
            wt.display()
        );
    }
}

/// 2.1 / scenario "Add seeds the new worktree": a worktree attached after
/// the session started (the `git paw add` path — same `attach_agent`
/// pipeline) carries the merged patterns too.
#[test]
fn add_seeds_the_new_worktree() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);
    let wt_new = sandbox.path().join("repo-feat-new");
    add_worktree(&repo, "feat-new", &wt_new);

    let cfg = dev_cfg(true, &[], &["just check"]);
    let failures = seed_worktree_allowlists(&wt_new, true, false, Some(&cfg));
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");

    let entries = read_array(&settings_path(&wt_new));
    assert!(entries.iter().any(|e| e == "git status"));
    assert!(entries.iter().any(|e| e == "just check"));
    assert!(entries.iter().any(|e| e == ".git-paw/scripts/broker.sh"));
}

/// 2.1 / scenario "Recovery re-seeds restored worktrees": a second seeding
/// pass over an already-seeded worktree picks up preset updates (a stack
/// added since the first seed) without disturbing what is already there.
#[test]
fn recovery_reseeds_restored_worktrees_picking_up_preset_updates() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);
    let wt = sandbox.path().join("repo-feat-r");
    add_worktree(&repo, "feat-r", &wt);

    // First seed: no stacks selected.
    let old = dev_cfg(true, &[], &[]);
    assert!(seed_worktree_allowlists(&wt, true, false, Some(&old)).is_empty());
    let before = read_array(&settings_path(&wt));
    assert!(!before.iter().any(|e| e == "cargo test"));

    // Recovery re-seed after the repo opted into the rust stack.
    let updated = dev_cfg(true, &["rust"], &[]);
    assert!(seed_worktree_allowlists(&wt, true, false, Some(&updated)).is_empty());

    let after = read_array(&settings_path(&wt));
    assert!(
        after.iter().any(|e| e == "cargo test"),
        "recovery re-seed must propagate preset updates: {after:?}"
    );
    for e in &before {
        assert!(
            after.iter().any(|a| a == e),
            "re-seed dropped pre-existing entry {e:?}"
        );
    }
    assert_eq!(
        after.iter().filter(|e| *e == "git status").count(),
        1,
        "re-seed must not duplicate entries: {after:?}"
    );
}

/// 2.2 / scenario "Existing worktree settings entries are preserved": a
/// custom entry the user (or agent) added by hand survives the seeding, a
/// pre-existing duplicate of a preset entry stays single, and unrelated
/// top-level fields are untouched.
#[test]
fn merge_preserves_preexisting_custom_entries_and_dedups() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);
    let wt = sandbox.path().join("repo-feat-m");
    add_worktree(&repo, "feat-m", &wt);

    let settings = settings_path(&wt);
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(
        &settings,
        r#"{"custom_field":"kept","allowed_bash_prefixes":["my-tool --serve","git diff",".git-paw/scripts/broker.sh"]}"#,
    )
    .unwrap();

    let cfg = dev_cfg(true, &["rust"], &[]);
    let failures = seed_worktree_allowlists(&wt, true, false, Some(&cfg));
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");

    let raw = std::fs::read_to_string(&settings).unwrap();
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        v.get("custom_field").and_then(|x| x.as_str()),
        Some("kept"),
        "unrelated top-level fields must be preserved"
    );

    let entries = read_array(&settings);
    assert!(entries.iter().any(|e| e == "my-tool --serve"));
    assert_eq!(
        entries.iter().filter(|e| *e == "git diff").count(),
        1,
        "dedup must hold for preset entries: {entries:?}"
    );
    assert_eq!(
        entries
            .iter()
            .filter(|e| *e == ".git-paw/scripts/broker.sh")
            .count(),
        1,
        "dedup must hold for helper grants: {entries:?}"
    );
    assert!(entries.iter().any(|e| e == "cargo test"));
}

/// 2.3 / scenario "Disabled feature writes nothing" (dev side): with
/// `common_dev_allowlist` disabled, no dev pattern lands — while the
/// broker-gated helper grants still do.
#[test]
fn dev_feature_disabled_seeds_no_dev_patterns() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);
    let wt = sandbox.path().join("repo-feat-d");
    add_worktree(&repo, "feat-d", &wt);

    let disabled = dev_cfg(false, &["rust"], &["just check"]);
    let failures = seed_worktree_allowlists(&wt, true, false, Some(&disabled));
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");

    let entries = read_array(&settings_path(&wt));
    assert!(entries.iter().any(|e| e == ".git-paw/scripts/broker.sh"));
    for pat in DEV_ALLOWLIST_PRESET {
        assert!(
            !entries.iter().any(|e| e == pat),
            "disabled dev feature must not seed {pat:?}: {entries:?}"
        );
    }
    assert!(!entries.iter().any(|e| e == "just check"));
}

/// 2.3 / scenario "Broker disabled seeds no broker prefix": with the broker
/// off, the worktree settings gain no helper grant from this seeder — while
/// the dev patterns still land.
#[test]
fn broker_disabled_seeds_no_broker_prefix() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);
    let wt = sandbox.path().join("repo-feat-nb");
    add_worktree(&repo, "feat-nb", &wt);

    let cfg = dev_cfg(true, &[], &[]);
    let failures = seed_worktree_allowlists(&wt, false, false, Some(&cfg));
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");

    let entries = read_array(&settings_path(&wt));
    assert!(
        !entries.iter().any(|e| e.contains("broker.sh")),
        "broker-off must not seed the broker grant: {entries:?}"
    );
    assert!(
        !entries.iter().any(|e| e.contains("sweep.sh")),
        "broker-off must not seed the sweep grant: {entries:?}"
    );
    assert!(entries.iter().any(|e| e == "git status"));
}

/// 2.3 (both gates off): the seeder writes no settings file at all.
#[test]
fn fully_disabled_writes_no_settings_file() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);
    let wt = sandbox.path().join("repo-feat-off");
    add_worktree(&repo, "feat-off", &wt);

    let disabled = dev_cfg(false, &[], &[]);
    let failures = seed_worktree_allowlists(&wt, false, false, Some(&disabled));
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");
    assert!(
        !wt.join(".claude").exists(),
        "disabled seeder must not create .claude/"
    );
}

/// 2.4 / scenario "Seeded file cannot be committed by the agent": after
/// seeding, `git status` in the worktree reports no `.claude/` entry, a
/// blanket `git add .` stages nothing from `.claude/`, and the repo's
/// tracked `.gitignore` is not modified.
#[test]
fn seeded_worktree_git_status_shows_no_claude_and_gitignore_untouched() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);
    let wt = sandbox.path().join("repo-feat-x");
    add_worktree(&repo, "feat-x", &wt);
    let gitignore_before = std::fs::read_to_string(repo.join(".gitignore")).unwrap();

    let cfg = dev_cfg(true, &["rust"], &[]);
    let failures = seed_worktree_allowlists(&wt, true, false, Some(&cfg));
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");
    assert!(settings_path(&wt).exists());

    let status = git(&wt, &["status", "--porcelain"]);
    assert!(
        !status.contains(".claude"),
        "seeded .claude/ must be excluded from git status; got: {status:?}"
    );

    git(&wt, &["add", "."]);
    let staged = git(&wt, &["diff", "--cached", "--name-only"]);
    assert!(
        !staged.contains(".claude"),
        "`git add .` must stage nothing from .claude/; got: {staged:?}"
    );

    // The worktree-local exclude did the job — no tracked .gitignore edited.
    let gitignore_after = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert_eq!(
        gitignore_before, gitignore_after,
        "tracked .gitignore must not be modified"
    );
    assert!(
        !std::fs::read_to_string(wt.join(".gitignore"))
            .unwrap()
            .contains(".claude"),
        "the worktree's checked-out .gitignore must not carry the exclusion"
    );
}

/// 2.5: the seeding + worktree-local exclusion work for both supported
/// placements — embedded (`<repo>/.git-paw/worktrees/<name>`, the child
/// default) and sibling (`<parent>/<repo>-<branch>`).
#[test]
fn works_for_embedded_and_sibling_worktree_placements() {
    let sandbox = TempDir::new().unwrap();
    let repo = sandbox.path().join("repo");
    init_repo(&repo);

    let embedded = repo.join(".git-paw").join("worktrees").join("feat-embed");
    let sibling = sandbox.path().join("repo-feat-sib");
    add_worktree(&repo, "feat-embed", &embedded);
    add_worktree(&repo, "feat-sib", &sibling);

    let cfg = dev_cfg(true, &[], &[]);
    for wt in [&embedded, &sibling] {
        let failures = seed_worktree_allowlists(wt, true, false, Some(&cfg));
        assert!(
            failures.is_empty(),
            "unexpected failures for {}: {failures:?}",
            wt.display()
        );

        let entries = read_array(&settings_path(wt));
        assert!(entries.iter().any(|e| e == "git status"));
        assert!(entries.iter().any(|e| e == ".git-paw/scripts/broker.sh"));

        let status = git(wt, &["status", "--porcelain"]);
        assert!(
            !status.contains(".claude"),
            "{}: .claude/ must be excluded; got: {status:?}",
            wt.display()
        );
    }
}
