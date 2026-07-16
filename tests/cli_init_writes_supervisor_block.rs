//! `git paw init` writes a commented-out `[supervisor]` block enumerating
//! every gate-command key.
//!
//! Covers the `supervisor-gate-templating-v0-5-x` change's
//! `supervisor-config` spec scenarios:
//!   - "git paw init writes the commented block"
//!   - "The written commented block is valid TOML when uncommented"

use assert_cmd::Command;

mod helpers;
use helpers::*;

/// Returns the `[supervisor]` keys that the commented teaching block MUST
/// enumerate. Listed in the order they appear in `generate_default_config`.
/// The trailing `agent_approval` is part of the block but is not in scope
/// for the gate-templating spec, so this list omits it. `approval` (the
/// supervisor pane's own level, supervisor-native-auto-mode) IS asserted.
const SUPERVISOR_KEYS: [&str; 10] = [
    "enabled",
    "cli",
    "test_command",
    "lint_command",
    "build_command",
    "fmt_check_command",
    "doc_build_command",
    "spec_validate_command",
    "security_audit_command",
    "approval",
];

#[test]
fn init_writes_commented_supervisor_block_with_all_gate_keys() {
    let repo = setup_test_repo();

    // Non-interactive stdin (assert_cmd pipes by default) drives the
    // init flow into the explicit opt-out branch — `[supervisor]\nenabled
    // = false\n` is appended after the commented teaching block. We assert
    // on the commented block, not the appended section.
    Command::cargo_bin("git-paw")
        .expect("binary exists")
        .current_dir(repo.path())
        .arg("init")
        .assert()
        .success();

    let config_path = repo.path().join(".git-paw").join("config.toml");
    let content = std::fs::read_to_string(&config_path).expect("config.toml written");

    assert!(
        content.contains("# [supervisor]"),
        "config.toml should contain the commented `# [supervisor]` header; got:\n{content}"
    );

    for key in SUPERVISOR_KEYS {
        let needle = format!("# {key} =");
        assert!(
            content.contains(&needle),
            "commented block should contain `{needle}`; got:\n{content}"
        );
    }
}

#[test]
fn init_commented_supervisor_block_parses_as_valid_config_when_uncommented() {
    let repo = setup_test_repo();
    Command::cargo_bin("git-paw")
        .expect("binary exists")
        .current_dir(repo.path())
        .arg("init")
        .assert()
        .success();

    let config_path = repo.path().join(".git-paw").join("config.toml");
    let content = std::fs::read_to_string(&config_path).expect("config.toml written");

    // Extract the commented `[supervisor]` block: the contiguous run of
    // `# `-prefixed lines starting at the `# [supervisor]` header. We stop
    // at the first blank line or `# [supervisor.*]` sub-section header.
    let header_pos = content
        .find("# [supervisor]")
        .expect("commented `# [supervisor]` header present");
    let after = &content[header_pos..];
    let mut uncommented = String::new();
    for line in after.lines() {
        if !line.starts_with('#') {
            break;
        }
        // Skip the optional `#` blank-comment separator.
        let trimmed = line.trim_start_matches('#').trim_start();
        if trimmed.is_empty() {
            break;
        }
        // Stop at the next commented sub-section header.
        if trimmed.starts_with("[supervisor.") {
            break;
        }
        uncommented.push_str(trimmed);
        uncommented.push('\n');
    }

    // Truncate any trailing inline `#` example annotation per-line —
    // `test_command = "just check"   # or: "cargo test"` should round-trip
    // as just the assignment.
    let cleaned: String = uncommented
        .lines()
        .map(|line| {
            // Crude `# `-comment stripper that respects quoted-string
            // boundaries — we have only simple double-quoted TOML strings
            // in this block.
            let mut out = String::with_capacity(line.len());
            let mut in_string = false;
            for c in line.chars() {
                if c == '"' {
                    in_string = !in_string;
                    out.push(c);
                } else if c == '#' && !in_string {
                    break;
                } else {
                    out.push(c);
                }
            }
            out.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Parse the uncommented block as TOML. The `[supervisor]` header
    // re-introduces the section.
    let parsed: git_paw::config::PawConfig = toml::from_str(&cleaned).unwrap_or_else(|e| {
        panic!(
            "uncommented [supervisor] block should parse as PawConfig; error:\n{e}\nblock:\n{cleaned}"
        )
    });
    let supervisor = parsed.supervisor.expect("supervisor section parses");

    assert!(
        supervisor.enabled,
        "uncommented block should set enabled = true"
    );
    assert_eq!(supervisor.cli.as_deref(), Some("claude"));
    assert_eq!(supervisor.test_command.as_deref(), Some("just check"));
    assert!(supervisor.lint_command.is_some(), "lint_command parsed");
    assert!(supervisor.build_command.is_some(), "build_command parsed");
    assert!(
        supervisor.doc_build_command.is_some(),
        "doc_build_command parsed"
    );
    assert!(
        supervisor.spec_validate_command.is_some(),
        "spec_validate_command parsed"
    );
    assert!(
        supervisor.fmt_check_command.is_some(),
        "fmt_check_command parsed"
    );
    assert!(
        supervisor.security_audit_command.is_some(),
        "security_audit_command parsed"
    );
    assert_eq!(
        supervisor.approval,
        Some(git_paw::config::ApprovalLevel::Manual),
        "uncommented approval line parses as the supervisor pane's own level"
    );

    // The spec_validate_command value SHALL still contain the literal
    // `{{CHANGE_ID}}` placeholder — that is substituted by the supervisor
    // agent at verification time, not at config load.
    assert!(
        supervisor
            .spec_validate_command
            .as_deref()
            .unwrap_or("")
            .contains("{{CHANGE_ID}}"),
        "spec_validate_command should retain the literal {{CHANGE_ID}} placeholder"
    );
}

#[test]
fn init_is_idempotent_does_not_duplicate_commented_supervisor_block() {
    let repo = setup_test_repo();
    let path = repo.path();

    Command::cargo_bin("git-paw")
        .expect("binary exists")
        .current_dir(path)
        .arg("init")
        .assert()
        .success();
    Command::cargo_bin("git-paw")
        .expect("binary exists")
        .current_dir(path)
        .arg("init")
        .assert()
        .success();

    let config_path = path.join(".git-paw").join("config.toml");
    let content = std::fs::read_to_string(&config_path).expect("config.toml written");

    let header_count = content.matches("# [supervisor]\n").count();
    assert_eq!(
        header_count, 1,
        "second `git paw init` must not duplicate the commented `# [supervisor]` block; \
         found {header_count} occurrences in:\n{content}"
    );
}
