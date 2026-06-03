//! Common dev-command allowlist seeding for the supervisor.
//!
//! Implements the `dev-command-allowlist` capability of the
//! `common-dev-allowlist-preset` change: write a curated preset of
//! prefix patterns (`cargo build`, `git commit`, `just`, `mdbook
//! build`, `openspec validate`, `find`, `grep`, `sed -n`, ...) into
//! `.claude/settings.json::allowed_bash_prefixes` so the supervisor
//! does not hand-approve every dev-loop command variant.
//!
//! The preset is hard-coded in [`DEV_ALLOWLIST_PRESET`] (one source
//! of truth, reviewable in PRs). Users extend it through
//! `[supervisor.common_dev_allowlist] extra = [...]` in the repo
//! config. The merge semantics are identical to
//! [`crate::supervisor::curl_allowlist`]: existing entries are
//! preserved, missing entries are appended, no duplicates are
//! written, and the parent directory is created when missing.

use std::path::Path;

use crate::error::PawError;

/// Common dev-loop prefix patterns seeded into Claude's
/// `allowed_bash_prefixes` on supervisor start.
///
/// Inclusion rubric (see `design.md` D3): observed as a repeated
/// prompt source in the v0.5.0 dogfood; bounded side-effects (no
/// arbitrary network or arbitrary code execution); aligns with
/// CLAUDE.md's git-safety protocol. Destructive operations
/// (`cargo install`, `cargo run`, `git rebase`, `git reset`,
/// `git checkout`, `git push --force`, write-mode `sed`, and
/// non-cargo package managers) are intentionally excluded.
///
/// The constant is the single source of truth — no other location
/// in the codebase may hard-code these patterns.
pub const DEV_ALLOWLIST_PRESET: &[&str] = &[
    // Cargo (read + build + test, no install/run/bench).
    "cargo build",
    "cargo test",
    "cargo clippy",
    "cargo fmt",
    "cargo check",
    "cargo tree",
    "cargo deny",
    "cargo update",
    // Git read-only.
    "git status",
    "git log",
    "git diff",
    "git show",
    "git fetch",
    // Git write (non-destructive; rebase/reset/checkout excluded).
    "git commit",
    "git push",
    "git pull",
    "git merge",
    "git stash",
    "git add",
    "git restore",
    "git rm",
    // Just (any recipe).
    "just",
    // mdBook.
    "mdbook build",
    // OpenSpec.
    "openspec validate",
    "openspec new",
    "openspec archive",
    "openspec list",
    "openspec status",
    "openspec instructions",
    // Search (read-only; `sed -n` is the read-only invocation).
    "find",
    "grep",
    "sed -n",
];

/// Returns the effective ordered preset list with `extra` patterns
/// appended after the built-in preset.
///
/// Entries already present in [`DEV_ALLOWLIST_PRESET`] are skipped
/// when found in `extra` so the caller does not produce duplicates
/// before reaching the file-merge step. The preset slice is returned
/// in declaration order; `extra` entries follow in their input order.
#[must_use]
pub fn effective_patterns(extra: &[String]) -> Vec<String> {
    let mut out: Vec<String> = DEV_ALLOWLIST_PRESET
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    for entry in extra {
        if !out.iter().any(|existing| existing == entry) {
            out.push(entry.clone());
        }
    }
    out
}

/// Merges the dev-allowlist preset + `extra` patterns into the JSON
/// file at `settings_path`.
///
/// Behaviour mirrors [`crate::supervisor::curl_allowlist::setup_curl_allowlist`]:
///
/// - When `settings_path` does not exist, a fresh JSON object is
///   created with `allowed_bash_prefixes` set to
///   [`effective_patterns`] applied to `extra`.
/// - When the file exists with valid JSON, existing fields are
///   preserved unchanged and missing entries are appended to the
///   `allowed_bash_prefixes` array.
/// - When the file exists but is not a JSON object (or
///   `allowed_bash_prefixes` is not an array), an error is returned
///   and the file is left unchanged.
/// - Parent directories are created when missing.
/// - The function never panics.
///
/// # Errors
///
/// Returns [`PawError::ConfigError`] when the file cannot be read,
/// contains invalid JSON, has a non-object top level, has a
/// non-array `allowed_bash_prefixes`, or cannot be written back.
pub fn setup_dev_allowlist(extra: &[String], settings_path: &Path) -> Result<(), PawError> {
    let new_entries = effective_patterns(extra);

    let mut value: serde_json::Value = if settings_path.exists() {
        let raw = std::fs::read_to_string(settings_path).map_err(|e| {
            PawError::ConfigError(format!("failed to read {}: {e}", settings_path.display()))
        })?;
        if raw.trim().is_empty() {
            serde_json::Value::Object(serde_json::Map::new())
        } else {
            serde_json::from_str(&raw).map_err(|e| {
                PawError::ConfigError(format!("{}: invalid JSON: {e}", settings_path.display()))
            })?
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let obj = value.as_object_mut().ok_or_else(|| {
        PawError::ConfigError(format!(
            "{}: top-level value must be a JSON object",
            settings_path.display()
        ))
    })?;

    let entry = obj
        .entry("allowed_bash_prefixes".to_string())
        .or_insert_with(|| serde_json::Value::Array(Vec::new()));

    let array = entry.as_array_mut().ok_or_else(|| {
        PawError::ConfigError(format!(
            "{}: allowed_bash_prefixes must be an array",
            settings_path.display()
        ))
    })?;

    for new_entry in new_entries {
        let already_present = array
            .iter()
            .any(|v| v.as_str().is_some_and(|s| s == new_entry));
        if !already_present {
            array.push(serde_json::Value::String(new_entry));
        }
    }

    if let Some(parent) = settings_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            PawError::ConfigError(format!("failed to create {}: {e}", parent.display()))
        })?;
    }

    let serialized = serde_json::to_string_pretty(&value).map_err(|e| {
        PawError::ConfigError(format!(
            "failed to serialize {}: {e}",
            settings_path.display()
        ))
    })?;
    std::fs::write(settings_path, serialized).map_err(|e| {
        PawError::ConfigError(format!("failed to write {}: {e}", settings_path.display()))
    })?;
    Ok(())
}

/// Seeds the dev allowlist into every Claude settings target a
/// supervisor session needs.
///
/// Targets:
///
/// - `<repo>/.claude/settings.json` — always written (its parent
///   `<repo>/.claude/` is created if absent).
/// - each path in `alt_settings` — a configured alternate settings
///   file (resolved from `[clis.<name>].settings_path`). These are
///   written only when their parent directory already exists; a
///   target whose parent is absent is skipped, never created. The
///   target set is config-driven — there is no hardcoded CLI name or
///   path.
///
/// Each target is processed independently and failures are reported
/// individually via the returned [`Vec`]; callers (e.g.
/// `cmd_supervisor`) treat the per-target result as non-fatal and
/// log warnings to stderr while continuing session start. Returns
/// the empty vec on full success.
pub fn seed_supervisor_session(
    extra: &[String],
    repo_root: &Path,
    alt_settings: &[std::path::PathBuf],
) -> Vec<(std::path::PathBuf, PawError)> {
    let mut failures = Vec::new();

    let repo_settings = repo_root.join(".claude").join("settings.json");
    if let Err(e) = setup_dev_allowlist(extra, &repo_settings) {
        failures.push((repo_settings, e));
    }

    for target in alt_settings {
        // Defence-in-depth: skip a target whose parent directory does not
        // exist so the seeder never creates an alternate config dir (the
        // caller's resolver already filters on this, but `setup_dev_allowlist`
        // would otherwise `create_dir_all` the parent).
        if target.parent().is_some_and(std::path::Path::is_dir)
            && let Err(e) = setup_dev_allowlist(extra, target)
        {
            failures.push((target.clone(), e));
        }
    }

    failures
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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

    #[test]
    fn writes_preset_when_file_absent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        setup_dev_allowlist(&[], &path).unwrap();
        let entries = read_array(&path);
        for pat in DEV_ALLOWLIST_PRESET {
            assert!(
                entries.iter().any(|e| e == pat),
                "missing preset pattern {pat:?} in {entries:?}",
            );
        }
    }

    #[test]
    fn merges_with_existing_user_entries() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"some_custom_field":"value","allowed_bash_prefixes":["my-tool","some-other"]}"#,
        )
        .unwrap();
        setup_dev_allowlist(&[], &path).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            v.get("some_custom_field").and_then(|x| x.as_str()),
            Some("value"),
            "must preserve unrelated top-level fields",
        );
        let entries = read_array(&path);
        assert!(entries.iter().any(|e| e == "my-tool"));
        assert!(entries.iter().any(|e| e == "some-other"));
        for pat in DEV_ALLOWLIST_PRESET {
            assert!(entries.iter().any(|e| e == pat), "missing {pat}");
        }
    }

    #[test]
    fn does_not_duplicate_existing_preset_entries() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(
            &path,
            r#"{"allowed_bash_prefixes":["cargo build","git push"]}"#,
        )
        .unwrap();
        setup_dev_allowlist(&[], &path).unwrap();
        let entries = read_array(&path);
        assert_eq!(entries.iter().filter(|e| *e == "cargo build").count(), 1);
        assert_eq!(entries.iter().filter(|e| *e == "git push").count(), 1);
    }

    #[test]
    fn appends_extra_patterns_after_preset() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        let extra = vec!["pnpm test".to_string(), "deno fmt".to_string()];
        setup_dev_allowlist(&extra, &path).unwrap();
        let entries = read_array(&path);
        assert!(entries.iter().any(|e| e == "pnpm test"));
        assert!(entries.iter().any(|e| e == "deno fmt"));
        let pnpm_idx = entries.iter().position(|e| e == "pnpm test").unwrap();
        let last_preset_idx = entries
            .iter()
            .rposition(|e| DEV_ALLOWLIST_PRESET.contains(&e.as_str()))
            .unwrap();
        assert!(
            pnpm_idx > last_preset_idx,
            "extra entries must follow the preset; entries: {entries:?}",
        );
    }

    #[test]
    fn extra_entries_not_validated() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        let extra = vec!["this is nonsense $$".to_string()];
        setup_dev_allowlist(&extra, &path).unwrap();
        let entries = read_array(&path);
        assert!(entries.iter().any(|e| e == "this is nonsense $$"));
    }

    #[test]
    fn extra_duplicates_preset_entry_not_added_twice() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        let extra = vec!["cargo build".to_string()];
        setup_dev_allowlist(&extra, &path).unwrap();
        let entries = read_array(&path);
        assert_eq!(
            entries.iter().filter(|e| *e == "cargo build").count(),
            1,
            "cargo build appears more than once: {entries:?}",
        );
    }

    #[test]
    fn invalid_json_returns_error_not_panic() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, "not json {{{").unwrap();
        let err = setup_dev_allowlist(&[], &path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("invalid JSON"), "got: {msg}");
        // File left unchanged.
        let raw = std::fs::read_to_string(&path).unwrap();
        assert_eq!(raw, "not json {{{");
    }

    #[test]
    fn creates_parent_directory_when_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".claude").join("settings.json");
        assert!(!path.parent().unwrap().exists());
        setup_dev_allowlist(&[], &path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn preset_constant_contains_all_required_patterns_and_no_excluded_ones() {
        let required = [
            "cargo build",
            "cargo test",
            "cargo clippy",
            "cargo fmt",
            "cargo check",
            "cargo tree",
            "cargo deny",
            "cargo update",
            "git status",
            "git log",
            "git diff",
            "git show",
            "git fetch",
            "git commit",
            "git push",
            "git pull",
            "git merge",
            "git stash",
            "git add",
            "git restore",
            "git rm",
            "just",
            "mdbook build",
            "openspec validate",
            "openspec new",
            "openspec archive",
            "openspec list",
            "openspec status",
            "openspec instructions",
            "find",
            "grep",
            "sed -n",
        ];
        for r in required {
            assert!(
                DEV_ALLOWLIST_PRESET.contains(&r),
                "preset missing required pattern: {r}",
            );
        }

        let excluded = [
            "cargo install",
            "cargo run",
            "cargo bench",
            "git rebase",
            "git reset",
            "git checkout",
            "git branch -D",
            "git push --force",
            "git push -f",
            "sed",
            "npm",
            "pnpm",
            "yarn",
            "deno",
            "bun",
            "uv",
            "pip",
            "pipx",
            "gem",
        ];
        for e in excluded {
            assert!(
                !DEV_ALLOWLIST_PRESET.contains(&e),
                "preset must not contain excluded pattern: {e}",
            );
        }
    }

    #[test]
    fn effective_patterns_orders_preset_before_extra() {
        let extra = vec!["pnpm test".to_string()];
        let out = effective_patterns(&extra);
        let pnpm_idx = out.iter().position(|s| s == "pnpm test").unwrap();
        let cargo_idx = out.iter().position(|s| s == "cargo build").unwrap();
        assert!(
            cargo_idx < pnpm_idx,
            "preset entries must precede extra: cargo@{cargo_idx} vs pnpm@{pnpm_idx}",
        );
    }

    #[test]
    fn effective_patterns_deduplicates_extra_against_preset() {
        let extra = vec!["cargo build".to_string()];
        let out = effective_patterns(&extra);
        assert_eq!(out.iter().filter(|s| *s == "cargo build").count(), 1);
    }

    #[test]
    fn rejects_top_level_array() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, "[]").unwrap();
        let err = setup_dev_allowlist(&[], &path).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("must be a JSON object"), "got: {msg}");
    }
}
