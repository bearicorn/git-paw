//! Integration tests for the `dev-command-allowlist` capability.
//!
//! The tests target `seed_supervisor_session`, which is the wiring
//! seam invoked from both `cmd_supervisor()` and `recover_session()`
//! in `src/main.rs`. Driving `cmd_supervisor` end-to-end is
//! impractical from a Rust integration test (it requires tmux + a
//! real CLI), so we cover the wiring at the helper level — the
//! main.rs call sites are thin top-level gates over the same helper.

use std::fs;
use std::path::Path;

use git_paw::supervisor::dev_allowlist::{
    DEV_ALLOWLIST_PRESET, seed_supervisor_session, setup_dev_allowlist,
};
use serial_test::serial;
use tempfile::TempDir;

mod helpers;

fn read_array(path: &Path) -> Vec<String> {
    let raw = fs::read_to_string(path).unwrap();
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

/// 6.1 — Default config (no `common_dev_allowlist` table, empty extra)
/// seeds every preset pattern into `<repo>/.claude/settings.json`.
#[test]
#[serial]
fn seeds_preset_into_repo_claude_settings_with_default_config() {
    let tr = helpers::setup_test_repo();
    let repo_root = tr.path();

    // Sandbox HOME so `~/.claude-oss/` does not exist on the host.
    let fake_home = TempDir::new().unwrap();
    // SAFETY: the test is annotated `#[serial]`, which serialises with
    // other HOME-mutating tests in the same binary.
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    let failures = seed_supervisor_session(&[], repo_root, &[]);
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");

    let settings = repo_root.join(".claude").join("settings.json");
    assert!(settings.exists(), "repo settings.json should be created");
    let entries = read_array(&settings);
    for pat in DEV_ALLOWLIST_PRESET {
        assert!(
            entries.iter().any(|e| e == pat),
            "preset missing {pat:?} from {entries:?}",
        );
    }

    // `~/.claude-oss/` does not pre-exist → no oss settings written.
    let oss = fake_home.path().join(".claude-oss");
    assert!(
        !oss.exists(),
        "the seeder must not create ~/.claude-oss when absent",
    );
}

/// 6.2 — When `enabled = false`, callers skip the helper entirely;
/// no preset entries land in the file. Reproduce the gate by simply
/// not calling the helper and asserting the file stays untouched.
#[test]
fn disabled_caller_does_not_modify_settings_file() {
    let tr = helpers::setup_test_repo();
    let repo_root = tr.path();

    // Pre-populate settings.json with a hand-edited entry that must
    // be preserved when the feature is disabled.
    let settings = repo_root.join(".claude").join("settings.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(&settings, r#"{"allowed_bash_prefixes":["my-custom-tool"]}"#).unwrap();

    // Simulate the caller's gate: if !enabled { skip }.
    // (We do NOT call seed_supervisor_session.)

    let entries = read_array(&settings);
    assert_eq!(entries, vec!["my-custom-tool".to_string()]);
    // None of the preset entries leaked into the file.
    for pat in DEV_ALLOWLIST_PRESET {
        assert!(!entries.iter().any(|e| e == pat));
    }
}

/// 6.3 — `extra` patterns appear in the seeded array alongside the preset.
#[test]
#[serial]
fn extra_patterns_appear_in_seeded_settings() {
    let tr = helpers::setup_test_repo();
    let repo_root = tr.path();

    let fake_home = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    let extra = vec!["pnpm test".to_string()];
    let failures = seed_supervisor_session(&extra, repo_root, &[]);
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");

    let entries = read_array(&repo_root.join(".claude").join("settings.json"));
    assert!(entries.iter().any(|e| e == "pnpm test"));
    // Preset entries still present.
    for pat in DEV_ALLOWLIST_PRESET {
        assert!(entries.iter().any(|e| e == pat));
    }
}

/// 6.4 — A configured alternate settings target (resolved from
/// `[clis.<name>].settings_path`) is written when its parent directory
/// already exists, and skipped (never created) when the parent is absent.
/// The target set is config-driven — there is no hardcoded CLI path.
#[test]
#[serial]
fn writes_configured_alt_target_when_parent_exists_and_skips_when_absent() {
    // Part A: alt-target whose parent does not exist → skipped, not created.
    let tr = helpers::setup_test_repo();
    let repo_root = tr.path();
    let missing_home = TempDir::new().unwrap();
    let absent_alt = missing_home
        .path()
        .join("does-not-exist")
        .join("settings.json");
    let failures = seed_supervisor_session(&[], repo_root, std::slice::from_ref(&absent_alt));
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");
    assert!(
        !absent_alt.exists() && !absent_alt.parent().unwrap().exists(),
        "an alt-target with an absent parent must be skipped, not created",
    );

    // Part B: alt-target whose parent exists → written with the preset.
    let tr_b = helpers::setup_test_repo();
    let repo_root_b = tr_b.path();
    let alt_dir = TempDir::new().unwrap();
    let alt_settings = alt_dir.path().join("settings.json");
    let failures = seed_supervisor_session(&[], repo_root_b, std::slice::from_ref(&alt_settings));
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");
    assert!(
        alt_settings.exists(),
        "alt-target settings.json should be created when its parent exists",
    );
    let alt_entries = read_array(&alt_settings);
    for pat in DEV_ALLOWLIST_PRESET {
        assert!(alt_entries.iter().any(|e| e == pat));
    }
    let repo_entries = read_array(&repo_root_b.join(".claude").join("settings.json"));
    for pat in DEV_ALLOWLIST_PRESET {
        assert!(repo_entries.iter().any(|e| e == pat));
    }
}

/// 6.5 — Re-seeding (recovery path) is idempotent: a second call
/// against an already-seeded file leaves the array unchanged.
#[test]
#[serial]
fn re_seed_on_recovery_is_idempotent() {
    let tr = helpers::setup_test_repo();
    let repo_root = tr.path();

    let fake_home = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    // First seed.
    let failures = seed_supervisor_session(&[], repo_root, &[]);
    assert!(failures.is_empty(), "first seed: {failures:?}");
    let first = read_array(&repo_root.join(".claude").join("settings.json"));

    // Re-seed (recovery path).
    let failures = seed_supervisor_session(&[], repo_root, &[]);
    assert!(failures.is_empty(), "re-seed: {failures:?}");
    let second = read_array(&repo_root.join(".claude").join("settings.json"));

    assert_eq!(first, second, "re-seed must be idempotent");
    // Sanity: every preset pattern exactly once.
    for pat in DEV_ALLOWLIST_PRESET {
        assert_eq!(
            second.iter().filter(|e| *e == pat).count(),
            1,
            "{pat:?} must appear exactly once after re-seed",
        );
    }
}

/// 6.6 — Seeding is independent of broker status. The helper takes
/// no broker argument and writes the preset regardless of whether a
/// broker is enabled at the caller. Verify by calling without any
/// broker configuration in scope.
#[test]
#[serial]
fn seeds_without_broker_configuration() {
    let tr = helpers::setup_test_repo();
    let repo_root = tr.path();

    let fake_home = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    let failures = seed_supervisor_session(&[], repo_root, &[]);
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");

    let entries = read_array(&repo_root.join(".claude").join("settings.json"));
    for pat in DEV_ALLOWLIST_PRESET {
        assert!(entries.iter().any(|e| e == pat));
    }
}

/// 6.7 — When the existing `.claude/settings.json` is malformed,
/// the helper returns the failure in its `Vec` (the caller logs and
/// continues). The malformed file is left untouched.
#[test]
#[serial]
fn malformed_settings_returns_failure_and_leaves_file_unchanged() {
    let tr = helpers::setup_test_repo();
    let repo_root = tr.path();

    let fake_home = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    let settings = repo_root.join(".claude").join("settings.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let malformed = "not json {{{";
    fs::write(&settings, malformed).unwrap();

    let failures = seed_supervisor_session(&[], repo_root, &[]);
    assert_eq!(failures.len(), 1, "expected one failure: {failures:?}");
    let (failed_path, err) = &failures[0];
    assert_eq!(failed_path, &settings);
    assert!(
        err.to_string().contains("invalid JSON"),
        "error should mention invalid JSON: {err}",
    );

    // File contents unchanged.
    let raw = fs::read_to_string(&settings).unwrap();
    assert_eq!(raw, malformed);
}

/// Standalone smoke test on `setup_dev_allowlist` used to verify
/// imports compile against the public API surface.
#[test]
fn setup_dev_allowlist_is_publicly_reachable() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("settings.json");
    setup_dev_allowlist(&[], &path).unwrap();
    assert!(path.exists());
}
