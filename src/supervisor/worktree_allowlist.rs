//! Per-agent-worktree allowlist seeding.
//!
//! Implements the per-worktree placement requirements of the
//! `dev-command-allowlist` and `curl-allowlist` capabilities: a
//! claude-format CLI resolves its PROJECT settings from its working
//! directory, and each coding agent's cwd is its own worktree — so the
//! repo-root `.claude/settings.json` seeded by
//! [`crate::supervisor::dev_allowlist::seed_supervisor_session`] and
//! [`crate::supervisor::curl_allowlist::setup_curl_allowlist`] never
//! applies inside an agent pane. This module merges the same two gated
//! content sources into `<worktree>/.claude/settings.json` at the same
//! events that provision the helper scripts themselves (agent attach via
//! `git paw start` / `git paw add`, and session recovery):
//!
//! - the helper-path prefixes (`broker.sh` / `sweep.sh` when the broker is
//!   enabled; `docs-fetch.sh` when a docs base URL is configured);
//! - the resolved dev-command patterns (universal preset + named stacks +
//!   `extra`) when `[supervisor.common_dev_allowlist]` is enabled.
//!
//! Merge semantics are identical to the repo-root targets (preserve
//! existing entries, dedup, non-fatal per-target failures reported to the
//! caller as warnings).
//!
//! The seeded `.claude/` directory is excluded from version control via the
//! WORKTREE-LOCAL ignore mechanism (the worktree's own `info/exclude`,
//! resolved through `git rev-parse --git-path info/exclude`) — never by
//! editing any tracked `.gitignore` — so an agent's `git add .` can never
//! commit the seeded file. Exclusion failures degrade gracefully: the
//! grants still work, only the hygiene is reported as a warning.

use std::path::{Path, PathBuf};

use crate::config::CommonDevAllowlistConfig;
use crate::error::PawError;
use crate::supervisor::{curl_allowlist, dev_allowlist};

/// Seeds the per-worktree allowlists into `<worktree>/.claude/settings.json`.
///
/// Content sources, each independently gated:
///
/// - `broker_enabled` — the `broker.sh` / `sweep.sh` helper-path prefixes
///   ([`curl_allowlist::broker_prefixes`] / [`curl_allowlist::sweep_prefixes`]).
/// - `docs_fetch_configured` — the `docs-fetch.sh` helper-path prefixes
///   ([`curl_allowlist::docs_fetch_prefixes`]); mirrors the gate that
///   provisions the script itself.
/// - `dev_allowlist` — the resolved dev-command patterns (universal preset +
///   named stacks + `extra`) when `Some` and `enabled`. `None` for sessions
///   without a supervisor config in play (bare mode recovery).
///
/// When every source is gated off, nothing is written — no settings file and
/// no exclude entry. Otherwise the worktree-local `info/exclude` is updated
/// first (so the untracked `.claude/` is never even transiently visible to
/// `git status`), then the gated entries are merged into the settings file,
/// creating `<worktree>/.claude/` as needed.
///
/// Failures are per-step and non-fatal: each is returned as a
/// `(path, error)` pair for the caller to report as a warning, mirroring
/// [`dev_allowlist::seed_supervisor_session`]. Returns the empty vec on
/// full success.
#[must_use]
pub fn seed_worktree_allowlists(
    worktree_root: &Path,
    broker_enabled: bool,
    docs_fetch_configured: bool,
    dev_allowlist: Option<&CommonDevAllowlistConfig>,
) -> Vec<(PathBuf, PawError)> {
    let mut failures = Vec::new();

    let mut helper_entries: Vec<String> = Vec::new();
    if broker_enabled {
        helper_entries.extend(curl_allowlist::broker_prefixes());
        helper_entries.extend(curl_allowlist::sweep_prefixes());
    }
    if docs_fetch_configured {
        helper_entries.extend(curl_allowlist::docs_fetch_prefixes());
    }
    let dev = dev_allowlist.filter(|cfg| cfg.enabled);

    // Every content source gated off — this seeder writes nothing.
    if helper_entries.is_empty() && dev.is_none() {
        return failures;
    }

    // Exclude before write: the broker's filesystem watcher polls
    // `git status` roughly every 2 seconds, so the ignore entry must land
    // before the settings file makes the worktree dirty.
    if let Err(e) = ensure_claude_dir_excluded(worktree_root) {
        failures.push((worktree_root.to_path_buf(), e));
    }

    let settings = worktree_root.join(".claude").join("settings.json");
    if !helper_entries.is_empty()
        && let Err(e) = curl_allowlist::merge_allowlist_entries(&settings, &helper_entries)
    {
        failures.push((settings.clone(), e));
    }
    if let Some(cfg) = dev
        && let Err(e) = dev_allowlist::setup_dev_allowlist(&cfg.stacks, &cfg.extra, &settings)
    {
        failures.push((settings, e));
    }

    failures
}

/// Ensures `.claude/` is ignored via the WORKTREE-LOCAL exclude file.
///
/// Resolves the worktree's own exclude file with
/// `git rev-parse --git-path info/exclude` (for a linked worktree this is
/// `<repo>/.git/worktrees/<name>/info/exclude`; for a main checkout,
/// `<repo>/.git/info/exclude`), creates its parent directory when missing,
/// and appends a `.claude/` line unless one is already present — idempotent
/// across re-seeding. No tracked `.gitignore` is ever touched.
///
/// # Errors
///
/// Returns [`PawError::WorktreeError`] when the exclude path cannot be
/// resolved (git failure) or the file cannot be read or written. Callers
/// treat this as a non-fatal warning: the seeded grants still work, only
/// the version-control hygiene degrades.
pub fn ensure_claude_dir_excluded(worktree_root: &Path) -> Result<(), PawError> {
    let exclude = git_path_info_exclude(worktree_root)?;

    let existing = if exclude.exists() {
        std::fs::read_to_string(&exclude).map_err(|e| {
            PawError::WorktreeError(format!("failed to read {}: {e}", exclude.display()))
        })?
    } else {
        String::new()
    };

    if existing.lines().any(|line| line.trim() == ".claude/") {
        return Ok(());
    }

    if let Some(parent) = exclude.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            PawError::WorktreeError(format!("failed to create {}: {e}", parent.display()))
        })?;
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(".claude/\n");
    std::fs::write(&exclude, updated).map_err(|e| {
        PawError::WorktreeError(format!("failed to write {}: {e}", exclude.display()))
    })?;
    Ok(())
}

/// Resolves the worktree's own `info/exclude` path via
/// `git rev-parse --git-path info/exclude` run inside `worktree_root`.
///
/// `--git-path` already accounts for linked-worktree indirection (it answers
/// under `.git/worktrees/<name>/` for a linked worktree) and may print a
/// path relative to the worktree, so a relative answer is joined onto
/// `worktree_root`.
fn git_path_info_exclude(worktree_root: &Path) -> Result<PathBuf, PawError> {
    let output = std::process::Command::new("git")
        .current_dir(worktree_root)
        .args(["rev-parse", "--git-path", "info/exclude"])
        .output()
        .map_err(|e| {
            PawError::WorktreeError(format!(
                "failed to run git rev-parse --git-path info/exclude in '{}': {e}",
                worktree_root.display()
            ))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::WorktreeError(format!(
            "git rev-parse --git-path info/exclude failed in '{}': {stderr}",
            worktree_root.display()
        )));
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = PathBuf::from(&raw);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(worktree_root.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn dev_cfg(enabled: bool, stacks: &[&str], extra: &[&str]) -> CommonDevAllowlistConfig {
        CommonDevAllowlistConfig {
            enabled,
            stacks: stacks.iter().map(ToString::to_string).collect(),
            extra: extra.iter().map(ToString::to_string).collect(),
        }
    }

    fn init_repo(dir: &Path) {
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "t@t.t"],
            vec!["config", "user.name", "T"],
        ] {
            let out = Command::new("git")
                .current_dir(dir)
                .args(&args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?} failed");
        }
        std::fs::write(dir.join("README.md"), "# t").unwrap();
        for args in [vec!["add", "."], vec!["commit", "-m", "init"]] {
            let out = Command::new("git")
                .current_dir(dir)
                .args(&args)
                .output()
                .unwrap();
            assert!(out.status.success(), "git {args:?} failed");
        }
    }

    fn read_array(path: &Path) -> Vec<String> {
        let raw = std::fs::read_to_string(path).unwrap();
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

    /// Both gated sources ride together: one seeding event lands the helper
    /// prefixes AND the dev patterns in the worktree settings (design D4).
    #[test]
    fn seeds_helper_prefixes_and_dev_patterns_together() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        let cfg = dev_cfg(true, &["rust"], &[]);
        let failures = seed_worktree_allowlists(tmp.path(), true, false, Some(&cfg));
        assert!(failures.is_empty(), "unexpected failures: {failures:?}");

        let entries = read_array(&tmp.path().join(".claude").join("settings.json"));
        assert!(entries.iter().any(|e| e == ".git-paw/scripts/broker.sh"));
        assert!(entries.iter().any(|e| e == ".git-paw/scripts/sweep.sh"));
        assert!(entries.iter().any(|e| e == "git status"));
        assert!(entries.iter().any(|e| e == "cargo test"));
        // Docs not configured — its helper prefix must not ride along.
        assert!(
            !entries
                .iter()
                .any(|e| e == ".git-paw/scripts/docs-fetch.sh")
        );
    }

    /// Broker disabled: no broker/sweep prefix from this seeder; the
    /// dev patterns still land (independent gates).
    #[test]
    fn broker_disabled_seeds_no_broker_prefix() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        let cfg = dev_cfg(true, &[], &[]);
        let failures = seed_worktree_allowlists(tmp.path(), false, false, Some(&cfg));
        assert!(failures.is_empty(), "unexpected failures: {failures:?}");

        let entries = read_array(&tmp.path().join(".claude").join("settings.json"));
        assert!(!entries.iter().any(|e| e.contains("broker.sh")));
        assert!(!entries.iter().any(|e| e.contains("sweep.sh")));
        assert!(entries.iter().any(|e| e == "git status"));
    }

    /// Docs base URL configured: the docs-fetch helper prefix is seeded even
    /// with the broker off (mirrors the script-provisioning gate).
    #[test]
    fn docs_configured_seeds_docs_fetch_prefix() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        let failures = seed_worktree_allowlists(tmp.path(), false, true, None);
        assert!(failures.is_empty(), "unexpected failures: {failures:?}");

        let entries = read_array(&tmp.path().join(".claude").join("settings.json"));
        assert!(
            entries
                .iter()
                .any(|e| e == ".git-paw/scripts/docs-fetch.sh")
        );
        assert!(!entries.iter().any(|e| e.contains("broker.sh")));
    }

    /// Every source gated off — the seeder writes nothing at all.
    #[test]
    fn fully_gated_off_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        let disabled = dev_cfg(false, &["rust"], &[]);
        let failures = seed_worktree_allowlists(tmp.path(), false, false, Some(&disabled));
        assert!(failures.is_empty(), "unexpected failures: {failures:?}");
        assert!(
            !tmp.path().join(".claude").exists(),
            "disabled seeder must not create .claude/"
        );
        let exclude = tmp.path().join(".git").join("info").join("exclude");
        let content = std::fs::read_to_string(exclude).unwrap_or_default();
        assert!(
            !content.contains(".claude/"),
            "disabled seeder must not touch info/exclude"
        );
    }

    /// The exclude append is idempotent — re-seeding never duplicates the
    /// `.claude/` line.
    #[test]
    fn exclude_entry_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        ensure_claude_dir_excluded(tmp.path()).unwrap();
        ensure_claude_dir_excluded(tmp.path()).unwrap();
        let exclude = tmp.path().join(".git").join("info").join("exclude");
        let content = std::fs::read_to_string(exclude).unwrap();
        assert_eq!(
            content.lines().filter(|l| l.trim() == ".claude/").count(),
            1,
            "exactly one .claude/ line expected: {content:?}"
        );
    }

    /// A pre-existing exclude file without a trailing newline is appended
    /// to on its own line, not glued to the last entry.
    #[test]
    fn exclude_append_preserves_existing_lines() {
        let tmp = TempDir::new().unwrap();
        init_repo(tmp.path());
        let exclude = tmp.path().join(".git").join("info").join("exclude");
        std::fs::create_dir_all(exclude.parent().unwrap()).unwrap();
        std::fs::write(&exclude, "custom-entry").unwrap();
        ensure_claude_dir_excluded(tmp.path()).unwrap();
        let content = std::fs::read_to_string(&exclude).unwrap();
        assert!(content.lines().any(|l| l == "custom-entry"));
        assert!(content.lines().any(|l| l == ".claude/"));
    }

    /// A directory that is not a git worktree reports a non-fatal failure
    /// (warn-and-continue at the call sites) — and still seeds the grants.
    #[test]
    fn non_repo_reports_exclude_failure_but_still_seeds() {
        let tmp = TempDir::new().unwrap();
        let cfg = dev_cfg(true, &[], &[]);
        let failures = seed_worktree_allowlists(tmp.path(), true, false, Some(&cfg));
        assert_eq!(failures.len(), 1, "exclude failure expected: {failures:?}");
        let entries = read_array(&tmp.path().join(".claude").join("settings.json"));
        assert!(entries.iter().any(|e| e == ".git-paw/scripts/broker.sh"));
        assert!(entries.iter().any(|e| e == "git status"));
    }
}
