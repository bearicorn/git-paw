//! Safe-command classification for the auto-approve feature.
//!
//! Filled out in Section 2 of `openspec/changes/auto-approve-patterns/tasks.md`.
//! Section 1 only needs `default_safe_commands()` so [`crate::config::AutoApproveConfig`]
//! can compute its effective whitelist.
//!
//! The `auto-approve-scope-v0-6-x` change adds a *worktree file-operation*
//! category on top of the shell-command whitelist: a Claude
//! write / edit / create permission prompt is auto-approved when the target
//! path resolves *inside* the agent's own worktree root. The boundary check
//! canonicalises the path before the `starts_with(worktree_root)` test so a
//! `..`/symlink escape cannot smuggle an out-of-worktree path past the gate.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

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

/// Compiled regex matching the lead-in of a Claude filesystem-operation
/// permission prompt and capturing the target path.
///
/// Covers the documented write / edit / create prompt shapes, e.g.
/// `"Do you want to allow this write to <path>?"` and
/// `"Do you want to make this edit to <path>?"`. The capture group runs to
/// the end of the line (minus a trailing `?`), so the path may contain
/// spaces. Matching is case-insensitive.
fn file_prompt_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:allow this write to|allow this edit to|make this edit to|write to|create file|edit file|write file)\s+(?:the file\s+)?(.+?)\s*\??\s*$",
        )
        .expect("static file-prompt regex is valid")
    })
}

/// Extracts the target path from a Claude filesystem-operation permission
/// prompt, or `None` when the prompt is not a recognised file-op prompt.
///
/// The prompt text is scanned line by line; the first line whose lead-in
/// matches a documented write / edit / create shape yields its captured
/// path. Surrounding quotes and backticks are stripped. The returned string
/// is the raw path as it appeared in the prompt — resolution against the
/// worktree root is the caller's job via [`is_path_inside_worktree`].
#[must_use]
pub fn extract_path_from_file_prompt(prompt: &str) -> Option<String> {
    let re = file_prompt_regex();
    for line in prompt.lines() {
        if let Some(caps) = re.captures(line) {
            let raw = caps.get(1)?.as_str().trim();
            let cleaned = raw
                .trim_matches(|c| c == '"' || c == '\'' || c == '`')
                .trim();
            if !cleaned.is_empty() {
                return Some(cleaned.to_string());
            }
        }
    }
    None
}

/// Resolves `target` to an absolute, symlink/`..`-collapsed path for the
/// worktree-boundary check, even when the target does not exist yet.
///
/// `canonicalize` fails for a not-yet-created file (the common case for a
/// file-create prompt), so this walks up to the deepest *existing* ancestor,
/// canonicalises that, and re-appends the non-existent suffix. Any component
/// that is `..` (a `Path::file_name` of `None`) aborts the walk and returns
/// `None`, which the caller treats as "outside the worktree" — fail-closed.
fn resolve_for_boundary(target: &Path) -> Option<PathBuf> {
    let mut suffix: Vec<OsString> = Vec::new();
    let mut cur = target.to_path_buf();
    loop {
        if let Ok(base) = cur.canonicalize() {
            let mut resolved = base;
            for comp in suffix.iter().rev() {
                resolved.push(comp);
            }
            return Some(resolved);
        }
        let file = cur.file_name()?.to_os_string();
        if !cur.pop() {
            return None;
        }
        suffix.push(file);
    }
}

/// Returns `true` when `path` (resolved against `worktree_root`) lies inside
/// the worktree.
///
/// Both sides are canonicalised before the `starts_with` comparison so a
/// `..`-relative path or a symlink that escapes the worktree fails the
/// check. A path that cannot be resolved (e.g. one containing a bare `..`
/// that walks off the filesystem) is treated as outside the worktree.
#[must_use]
pub fn is_path_inside_worktree(path: &str, worktree_root: &Path) -> bool {
    let target = Path::new(path);
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        worktree_root.join(target)
    };
    let Some(resolved) = resolve_for_boundary(&joined) else {
        return false;
    };
    let root = worktree_root
        .canonicalize()
        .unwrap_or_else(|_| worktree_root.to_path_buf());
    resolved.starts_with(&root)
}

/// Classifies a captured permission prompt as a safe worktree file operation.
///
/// Returns `true` only when (a) `approve_worktree_writes` is enabled, (b) the
/// prompt matches a documented Claude file-op shape, and (c) the extracted
/// target path resolves inside `worktree_root`. Out-of-worktree paths,
/// symlink-escape attempts, and shell-command prompts all return `false` —
/// those continue through the shell whitelist or the manual-prompt flow.
#[must_use]
pub fn is_worktree_file_op(
    prompt: &str,
    worktree_root: &Path,
    approve_worktree_writes: bool,
) -> bool {
    if !approve_worktree_writes {
        return false;
    }
    match extract_path_from_file_prompt(prompt) {
        Some(path) => is_path_inside_worktree(&path, worktree_root),
        None => false,
    }
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

    // --- Bug 3: worktree file-operation classifier ---

    use tempfile::TempDir;

    #[test]
    fn extracts_path_from_write_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to allow this write to Containerfile?"),
            Some("Containerfile".to_string())
        );
    }

    #[test]
    fn extracts_path_from_edit_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to make this edit to src/main.rs?"),
            Some("src/main.rs".to_string())
        );
    }

    #[test]
    fn extracts_absolute_path_from_prompt() {
        assert_eq!(
            extract_path_from_file_prompt("Do you want to allow this write to /etc/hosts?"),
            Some("/etc/hosts".to_string())
        );
    }

    #[test]
    fn shell_prompt_yields_no_file_path() {
        // A shell-command prompt must not be mistaken for a file op.
        assert_eq!(
            extract_path_from_file_prompt("Do you want to proceed?\ncargo build --release"),
            None
        );
    }

    /// Spec scenario: In-worktree file create is auto-approved.
    #[test]
    fn in_worktree_file_create_is_classified_safe() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // Containerfile does not exist yet — the create case.
        let prompt = "Do you want to allow this write to Containerfile?";
        assert!(is_worktree_file_op(prompt, root, true));
    }

    /// Spec scenario: Out-of-worktree file create requires manual approval.
    #[test]
    fn out_of_worktree_path_is_not_classified_safe() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let prompt = "Do you want to allow this write to /etc/hosts?";
        assert!(!is_worktree_file_op(prompt, root, true));
    }

    /// Spec scenario: Symlink/`..`-escape attempt does not bypass the boundary.
    #[test]
    fn dotdot_escape_attempt_is_rejected() {
        let tmp = TempDir::new().unwrap();
        // Nest the worktree so `../../` resolves above it but still exists.
        let root = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&root).unwrap();
        let prompt = "Do you want to allow this write to ../../escaped.txt?";
        assert!(!is_worktree_file_op(prompt, &root, true));
    }

    #[test]
    fn nested_in_worktree_path_is_classified_safe() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let prompt = "Do you want to make this edit to deep/nested/new_file.rs?";
        assert!(is_worktree_file_op(prompt, root, true));
    }

    /// Task 1.6 / spec scenario: shell auto-approve is unaffected — a shell
    /// prompt is never classified as a worktree file op.
    #[test]
    fn shell_prompt_is_not_a_worktree_file_op() {
        let tmp = TempDir::new().unwrap();
        let prompt = "Do you want to proceed?\ncargo test --workspace";
        assert!(!is_worktree_file_op(prompt, tmp.path(), true));
        // The shell whitelist still matches it.
        assert!(is_safe_command(
            "cargo test --workspace",
            &["cargo test".to_string()]
        ));
    }

    /// Task 2.3 / spec scenario: explicit `approve_worktree_writes = false`
    /// reverts a worktree-confined prompt to manual.
    #[test]
    fn disabled_flag_rejects_in_worktree_path() {
        let tmp = TempDir::new().unwrap();
        let prompt = "Do you want to allow this write to Containerfile?";
        assert!(
            !is_worktree_file_op(prompt, tmp.path(), false),
            "approve_worktree_writes=false must not classify any file op as safe"
        );
    }
}
