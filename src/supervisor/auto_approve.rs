//! Safe-command classification for the auto-approve feature.
//!
//! Filled out in Section 2 of `openspec/changes/auto-approve-patterns/tasks.md`.
//! Section 1 only needs `default_safe_commands()` so [`crate::config::AutoApproveConfig`]
//! can compute its effective whitelist.

/// Built-in whitelist of command prefixes eligible for auto-approval.
///
/// Each entry is matched against captured command text via prefix +
/// whitespace boundary semantics in [`is_safe_command`]. The list is
/// intentionally narrow; users extend it via
/// `[supervisor.auto_approve] safe_commands` in `.git-paw/config.toml`.
#[must_use]
pub fn default_safe_commands() -> &'static [&'static str] {
    &[
        "cargo fmt",
        "cargo clippy",
        "cargo test",
        "cargo build",
        "git commit",
        "git push",
        "curl http://127.0.0.1:",
    ]
}

/// Returns `true` if `captured` begins with any whitelist entry followed
/// by either end-of-string or ASCII whitespace.
///
/// Prefix matching is intentional: a single entry like `cargo test` should
/// match `cargo test --no-run --workspace` without the user having to
/// enumerate every flag combination. The whitespace boundary prevents
/// `cargotest --foo` from matching the `cargo test` prefix.
#[must_use]
pub fn is_safe_command(captured: &str, whitelist: &[String]) -> bool {
    let captured = captured.trim_start();
    whitelist.iter().any(|entry| {
        let entry = entry.as_str();
        if !captured.starts_with(entry) {
            return false;
        }
        match captured.as_bytes().get(entry.len()) {
            None => true,
            Some(b) => b.is_ascii_whitespace(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_contain_documented_classes() {
        let defaults = default_safe_commands();
        assert!(defaults.contains(&"cargo fmt"));
        assert!(defaults.contains(&"cargo clippy"));
        assert!(defaults.contains(&"cargo test"));
        assert!(defaults.contains(&"cargo build"));
        assert!(defaults.contains(&"git commit"));
        assert!(defaults.contains(&"git push"));
        assert!(defaults.contains(&"curl http://127.0.0.1:"));
    }

    #[test]
    fn prefix_match_accepts_flag_variations() {
        let whitelist = vec!["cargo test".to_string()];
        assert!(is_safe_command(
            "cargo test --no-run --workspace",
            &whitelist
        ));
        assert!(is_safe_command("cargo test", &whitelist));
    }

    #[test]
    fn prefix_match_requires_word_boundary() {
        let whitelist = vec!["cargo test".to_string()];
        assert!(
            !is_safe_command("cargotest --foo", &whitelist),
            "no whitespace boundary should fail"
        );
    }

    #[test]
    fn unknown_command_is_not_safe() {
        let whitelist: Vec<String> = default_safe_commands()
            .iter()
            .map(|s| (*s).into())
            .collect();
        assert!(!is_safe_command("rm -rf /tmp/foo", &whitelist));
    }

    #[test]
    fn config_extras_extend_whitelist() {
        let mut whitelist: Vec<String> = default_safe_commands()
            .iter()
            .map(|s| (*s).into())
            .collect();
        whitelist.push("just smoke".to_string());
        assert!(is_safe_command("just smoke -v", &whitelist));
    }

    #[test]
    fn empty_extras_keeps_defaults() {
        let whitelist: Vec<String> = default_safe_commands()
            .iter()
            .map(|s| (*s).into())
            .collect();
        assert!(is_safe_command("cargo test", &whitelist));
        assert!(is_safe_command("git commit -m hi", &whitelist));
    }

    #[test]
    fn leading_whitespace_is_tolerated() {
        let whitelist = vec!["cargo fmt".to_string()];
        assert!(is_safe_command("   cargo fmt --check", &whitelist));
    }
}
