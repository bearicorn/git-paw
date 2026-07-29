//! Integration tests for protected-path / out-of-worktree write enforcement.
//!
//! Closes the `core-memory-isolation` integration GAP and the
//! `approval-command-safety` out-of-worktree + scratch-path PARTIAL: the
//! protected-path check and command classification were only unit-tested
//! inside `src/supervisor/auto_approve.rs`; there was no integration test
//! asserting end-to-end that "a write outside the worktree / into a protected
//! path is caught, but the same write inside the worktree is allowed."
//!
//! These tests drive the REAL classification path through the crate's public
//! API — the production derivation entry point `ProtectedPaths::derive`
//! (which reads `[clis.<name>].settings_path` from config, `CLAUDE_CONFIG_DIR`
//! from the environment, and the host repo's control dirs) plus the
//! danger-class verdict functions `is_protected_path_violation`,
//! `is_worktree_file_op`, `is_scratch_rm`, and `is_dangerous`. They assert the
//! observable verdict (violation / not-a-violation, safe-by-pattern / escalate)
//! — never internal fields.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use git_paw::config::{CustomCli, PawConfig};
use git_paw::supervisor::auto_approve::{
    ProtectedPaths, is_dangerous, is_protected_path_violation, is_scratch_rm, is_worktree_file_op,
};
use tempfile::TempDir;

/// Builds a config whose `[clis.<name>].settings_path` points at `settings`,
/// standing in for an operator CLI's configuration territory. The parent of
/// this path joins the derived protected-path set.
fn config_with_cli_settings(name: &str, settings: &Path) -> PawConfig {
    let mut config = PawConfig::default();
    config.clis.insert(
        name.to_string(),
        CustomCli {
            command: name.to_string(),
            display_name: None,
            submit_delay_ms: None,
            settings_path: Some(settings.to_string_lossy().into_owned()),
            approval_args: HashMap::new(),
        },
    );
    config
}

/// Creates an embedded agent worktree under `<repo>/.git-paw/worktrees/feat-x`
/// and returns its path — the isolated territory an agent may freely write to.
fn make_embedded_worktree(repo: &Path) -> std::path::PathBuf {
    let worktree = repo.join(".git-paw").join("worktrees").join("feat-x");
    fs::create_dir_all(&worktree).unwrap();
    worktree
}

/// GAP-closer 1: a write into the operator's configuration territory —
/// either a configured CLI `settings_path` dir or the host repo's `.git-paw/`
/// control dir, both OUTSIDE the agent's worktree — is caught as a terminal
/// danger-class escalation and is never classifiable as a safe worktree op.
#[test]
fn out_of_worktree_write_into_operator_config_is_caught() {
    let op_config_dir = TempDir::new().unwrap();
    let settings = op_config_dir.path().join("settings.json");
    let config = config_with_cli_settings("myvariant", &settings);

    let repo = TempDir::new().unwrap();
    let worktree = make_embedded_worktree(repo.path());

    // The real production derivation: config-driven settings_path parent +
    // the host repo's `.claude/` and `.git-paw/` control dirs.
    let protected = ProtectedPaths::derive(&config, Some(repo.path()));

    // (a) filesystem prompt writing into the operator's configured CLI dir.
    let prompt = format!(
        "Do you want to allow this write to {}?",
        settings.to_string_lossy()
    );
    assert!(
        is_protected_path_violation(&prompt, &prompt, &protected, Some(&worktree)),
        "write into operator config territory must be a danger-class escalation"
    );
    assert!(
        !is_worktree_file_op(&prompt, &worktree, true),
        "an out-of-worktree write is never auto-approvable via the worktree rule"
    );

    // (b) shell redirect appending into the host repo's `.git-paw/` control dir.
    let repo_cfg = repo.path().join(".git-paw").join("config.toml");
    let slice = format!("echo x >> {}", repo_cfg.to_string_lossy());
    assert!(
        is_protected_path_violation(&slice, &slice, &protected, Some(&worktree)),
        "append into repo-root .git-paw/ must be a danger-class escalation"
    );
}

/// GAP-closer 2: the SAME kind of write, but targeting a path INSIDE the
/// agent's own worktree, is NOT a protected-path violation and IS classifiable
/// as a safe worktree op. The contrast proves the check is about the write's
/// LOCATION, not the verb.
#[test]
fn in_worktree_write_of_same_kind_is_allowed() {
    let op_config_dir = TempDir::new().unwrap();
    let settings = op_config_dir.path().join("settings.json");
    let config = config_with_cli_settings("myvariant", &settings);

    let repo = TempDir::new().unwrap();
    let worktree = make_embedded_worktree(repo.path());
    let protected = ProtectedPaths::derive(&config, Some(repo.path()));

    // A relative write resolving inside the worktree — the memory-isolation
    // spec's "in-worktree writes are unaffected" scenario.
    let prompt = "Do you want to allow this write to notes/memory.md?";
    assert!(
        !is_protected_path_violation(prompt, prompt, &protected, Some(&worktree)),
        "an in-worktree write must not be a protected-path violation"
    );
    assert!(
        is_worktree_file_op(prompt, &worktree, true),
        "an in-worktree write must classify as a safe worktree op"
    );

    // An absolute path inside the worktree behaves identically.
    let inner = worktree.join("src").join("lib.rs");
    let abs_prompt = format!(
        "Do you want to allow this write to {}?",
        inner.to_string_lossy()
    );
    assert!(
        !is_protected_path_violation(&abs_prompt, &abs_prompt, &protected, Some(&worktree)),
        "an absolute in-worktree write must not be a protected-path violation"
    );
    assert!(
        is_worktree_file_op(&abs_prompt, &worktree, true),
        "an absolute in-worktree write must classify safe"
    );
}

/// GAP-closer 3: the documented scratch-dir exception classifies `rm -rf`
/// safe-by-pattern even though `rm -rf` is otherwise danger-listed. A
/// non-scratch `rm -rf` still escalates (contrast proves the exception is
/// scoped to scratch locations).
#[test]
fn scratch_path_rm_rf_is_auto_approvable_exception() {
    for scratch in [
        "rm -rf /tmp/paw-build-123",
        "rm -rf /private/tmp/paw-cache",
        "rm -rf .git-paw/tmp/wave-7",
    ] {
        assert!(
            is_scratch_rm(scratch),
            "{scratch} must classify safe under the scratch-path exception"
        );
        assert!(
            !is_dangerous(scratch),
            "{scratch} must not escalate under the scratch-path exception"
        );
    }

    // Contrast: a non-scratch rm -rf is still a terminal escalation.
    let danger = "rm -rf /etc/important";
    assert!(
        is_dangerous(danger),
        "a non-scratch rm -rf must escalate as danger"
    );
    assert!(
        !is_scratch_rm(danger),
        "a non-scratch rm -rf must not be treated as a scratch delete"
    );
}

/// GAP-closer 4: the protected set is CONFIG-DRIVEN with no hardcoded CLI /
/// product names. The same operator dir is protected only when a config field
/// names it, and at repo scope only the documented `.claude/` and `.git-paw/`
/// control dirs are protected — arbitrary product-named siblings are not.
#[test]
fn protected_set_is_config_driven_with_no_hardcoded_product_names() {
    let op_dir = TempDir::new().unwrap();
    let settings = op_dir.path().join("settings.json");
    let target = settings.to_string_lossy().into_owned();

    // Config A names the CLI's settings_path → its parent joins the set.
    let config_a = config_with_cli_settings("myvariant", &settings);
    let set_a = ProtectedPaths::derive(&config_a, None);
    assert!(
        set_a.contains_dir(op_dir.path()),
        "a configured settings_path parent must join the protected set"
    );
    assert!(
        set_a.matches_target(&target, None),
        "a write into the configured territory must be protected"
    );

    // Config B (empty) → the SAME dir is NOT protected. Membership traces to
    // config, not a hardcoded product name.
    let set_b = ProtectedPaths::derive(&PawConfig::default(), None);
    assert!(
        !set_b.contains_dir(op_dir.path()),
        "with no config the operator dir must NOT be protected — set is config-driven"
    );
    assert!(
        !set_b.matches_target(&target, None),
        "with no config a write into that dir must NOT be a violation"
    );

    // Repo-scope derivation: only the documented control dirs are hardcoded.
    let repo = TempDir::new().unwrap();
    let worktree = make_embedded_worktree(repo.path());
    let set_repo = ProtectedPaths::derive(&PawConfig::default(), Some(repo.path()));

    // The documented `.git-paw/` control dir IS protected (outside the worktree).
    let repo_cfg = repo.path().join(".git-paw").join("config.toml");
    assert!(
        set_repo.matches_target(&repo_cfg.to_string_lossy(), Some(&worktree)),
        "the documented repo-root .git-paw/ control dir must be protected"
    );

    // Arbitrary product-named siblings are NOT built into the set.
    for product in [".gemini", ".codex", ".claude-oss", ".aider"] {
        let sibling = repo.path().join(product).join("settings.json");
        assert!(
            !set_repo.matches_target(&sibling.to_string_lossy(), Some(&worktree)),
            "{product} must not be protected — no hardcoded product names"
        );
    }
}
