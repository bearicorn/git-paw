//! Source-tree reads for the MCP server.
//!
//! Browses and reads the repository's working tree via git plumbing so that
//! gitignore handling and the tracked-set semantics come for free and stay
//! consistent with the git-context tools. The "working tree" is defined as
//! tracked files **plus** untracked-but-not-ignored files; gitignored paths
//! (build artifacts, secrets) are excluded throughout.
//!
//! [`read_file`] is confined to the repository root: the requested path is
//! resolved under the root, canonicalised, and verified to still lie within
//! it, so `..`/absolute/symlink escapes are refused before any file outside
//! the root is read (the same guard as [`super::docs::read_doc`]). It
//! additionally refuses gitignored paths via `git check-ignore`.
//!
//! Degradation contract (design D4): a non-git directory or a search with no
//! matches yields an empty result, never a transport error.

use std::path::Path;
use std::process::Command;

use rmcp::schemars;
use serde::Serialize;

use crate::error::PawError;

use super::resolve_under_root;

/// Maximum number of search matches returned by [`search_code`]; results
/// beyond this cap are dropped and the caller is told the result was
/// truncated, rather than returning an unbounded response.
const SEARCH_MATCH_CAP: usize = 200;

/// One match returned by [`search_code`].
#[derive(Debug, Clone, Serialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct CodeMatch {
    /// Path relative to the repository root (forward slashes, as git emits).
    pub path: String,
    /// 1-based line number of the match.
    pub line_number: u64,
    /// The matching line's content (trailing newline stripped).
    pub line: String,
}

/// Outcome of a [`read_file`] call: either the file's working-tree content, or
/// a refusal/absence carrying a human-readable reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOutcome {
    /// File content from the local working tree, or `None` when refused or
    /// absent.
    pub content: Option<String>,
    /// Human-readable note when `content` is `None` (refused traversal, a
    /// gitignored path, or a missing file). `None` when `content` is present.
    pub message: Option<String>,
}

/// Runs `git` in `repo_root`, returning stdout on success and `None` when git
/// is unavailable or exits non-zero.
fn git(repo_root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Lists the repository's working-tree files (tracked plus
/// untracked-but-not-ignored), optionally scoped to `subpath`.
///
/// Runs `git ls-files --cached --others --exclude-standard [-- <subpath>]` in
/// `repo_root`, so gitignored paths are excluded and untracked-not-ignored
/// files are included. Returns paths relative to the repository root. Yields an
/// empty list when the directory is not a git repository or git fails
/// (graceful degradation).
#[must_use]
pub fn list_files(repo_root: &Path, subpath: Option<&str>) -> Vec<String> {
    let mut args = vec!["ls-files", "--cached", "--others", "--exclude-standard"];
    if let Some(sub) = subpath {
        args.push("--");
        args.push(sub);
    }
    let Some(raw) = git(repo_root, &args) else {
        return Vec::new();
    };
    raw.lines()
        .filter(|l| !l.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// Returns true when `path` (relative to `repo_root`) is gitignored.
///
/// Uses `git check-ignore -q <path>`: exit 0 means the path is ignored, exit 1
/// means it is not, any other exit (or git unavailable) is treated as "not
/// ignored" so the confinement guard remains the primary gate.
fn is_gitignored(repo_root: &Path, path: &str) -> bool {
    Command::new("git")
        .current_dir(repo_root)
        .args(["check-ignore", "-q", "--", path])
        .output()
        .ok()
        .is_some_and(|out| out.status.success())
}

/// Reads one file from the local working tree, confined to the repository
/// root and refusing gitignored paths.
///
/// Steps, in order:
/// 1. Resolve `path` under `repo_root`, canonicalise it, and verify it still
///    lies within the canonical repository root. Any escape (`..`, an absolute
///    path, a symlink target outside the root) is **refused** — no file
///    outside the root is read.
/// 2. Refuse gitignored paths (`git check-ignore`), so secrets/build artifacts
///    are never returned even when they sit inside the root.
/// 3. Read the on-disk working-tree content (so uncommitted/branch state is
///    reflected).
///
/// Returns:
/// - refused traversal/escape → `Ok(ReadOutcome { content: None, message })`.
/// - gitignored path → `Ok(ReadOutcome { content: None, message })`.
/// - missing file → `Ok(ReadOutcome { content: None, message })`.
/// - readable file → `Ok(ReadOutcome { content: Some(..), message: None })`.
/// - present-but-unreadable (e.g. a permission error) → `Err`, so the tool
///   layer can surface the misconfiguration.
pub fn read_file(repo_root: &Path, path: &str) -> Result<ReadOutcome, PawError> {
    let refused = |reason: &str| {
        Ok(ReadOutcome {
            content: None,
            message: Some(reason.to_string()),
        })
    };

    // Confinement: the canonical repository root must exist for the guard to
    // be meaningful.
    let Ok(canonical_root) = repo_root.canonicalize() else {
        return refused("repository root could not be resolved");
    };

    let requested = resolve_under_root(repo_root, Path::new(path));
    // Canonicalise the requested path; a non-existent file (or a broken
    // traversal target) cannot be confirmed inside the root, so it is treated
    // as absent.
    let Ok(canonical) = requested.canonicalize() else {
        return refused(&format!("file not found within the repository: {path:?}"));
    };
    // Confinement check: the canonical target must stay within the canonical
    // repository root. This rejects `..`, absolute paths, and symlink escapes
    // alike.
    if !canonical.starts_with(&canonical_root) {
        return refused(&format!(
            "path {path:?} resolves outside the repository root and was refused"
        ));
    }

    // Refuse gitignored paths even when confined to the root.
    if is_gitignored(repo_root, path) {
        return refused(&format!("path {path:?} is gitignored and was refused"));
    }

    match std::fs::read_to_string(&canonical) {
        Ok(content) => Ok(ReadOutcome {
            content: Some(content),
            message: None,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            refused(&format!("file not found within the repository: {path:?}"))
        }
        Err(e) => Err(PawError::McpError(format!(
            "file {} could not be read: {e}",
            canonical.display()
        ))),
    }
}

/// Searches file contents across the repository's working tree (tracked plus
/// untracked-but-not-ignored), optionally scoped to `subpath`.
///
/// Runs `git grep -n -I --untracked -e <query> [-- <subpath>]` in `repo_root`.
/// `-I` skips binary files, `--untracked` extends the search to
/// untracked-not-ignored files. Returns matches as `{ path, line_number, line }`,
/// capped at [`SEARCH_MATCH_CAP`]; the returned flag reports whether the result
/// was truncated. `git grep` exits 1 when there are no matches — that and a
/// non-git directory both degrade to an empty list (never an error).
#[must_use]
pub fn search_code(repo_root: &Path, query: &str, subpath: Option<&str>) -> (Vec<CodeMatch>, bool) {
    let mut args = vec!["grep", "-n", "-I", "--untracked", "-e", query];
    if let Some(sub) = subpath {
        args.push("--");
        args.push(sub);
    }
    // `git grep` exits 1 on no-match (treated as empty by `git()` returning
    // None), and 0 with output on match.
    let Some(raw) = git(repo_root, &args) else {
        return (Vec::new(), false);
    };

    let mut matches = Vec::new();
    let mut truncated = false;
    for line in raw.lines() {
        // Format: "<path>:<line_number>:<content>".
        let mut parts = line.splitn(3, ':');
        let (Some(path), Some(num), Some(content)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let Ok(line_number) = num.parse::<u64>() else {
            continue;
        };
        if matches.len() >= SEARCH_MATCH_CAP {
            truncated = true;
            break;
        }
        matches.push(CodeMatch {
            path: path.to_string(),
            line_number,
            line: content.to_string(),
        });
    }
    (matches, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn init_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(dir)
                    .args(&args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        tmp
    }

    fn git_run(dir: &Path, args: &[&str]) {
        assert!(
            Command::new("git")
                .current_dir(dir)
                .args(args)
                .status()
                .unwrap()
                .success(),
            "git {args:?} failed"
        );
    }

    /// Builds a fixture repo: a tracked file, an untracked-but-not-ignored
    /// file, a `.gitignore`, and a gitignored path.
    fn fixture() -> tempfile::TempDir {
        let tmp = init_repo();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/main.rs"),
            "fn main() {\n    register_watch_target_http();\n}\n",
        )
        .unwrap();
        std::fs::write(dir.join(".gitignore"), "target/\n").unwrap();
        git_run(dir, &["add", "src/main.rs", ".gitignore"]);
        git_run(dir, &["commit", "-q", "-m", "first"]);
        // Untracked-but-not-ignored.
        std::fs::write(dir.join("notes.txt"), "loose notes\n").unwrap();
        // Gitignored path.
        std::fs::create_dir_all(dir.join("target/debug")).unwrap();
        std::fs::write(dir.join("target/debug/foo"), "build artifact\n").unwrap();
        tmp
    }

    // Scenario: list_files returns the working tree excluding gitignored paths.
    #[test]
    fn list_files_includes_tracked_and_untracked_excludes_gitignored() {
        let tmp = fixture();
        let files = list_files(tmp.path(), None);
        assert!(files.iter().any(|f| f == "src/main.rs"), "tracked listed");
        assert!(
            files.iter().any(|f| f == "notes.txt"),
            "untracked-not-ignored listed"
        );
        assert!(
            !files.iter().any(|f| f.starts_with("target/")),
            "gitignored excluded: {files:?}"
        );
    }

    // Scenario: list_files scopes to a subpath.
    #[test]
    fn list_files_scopes_to_subpath() {
        let tmp = fixture();
        let files = list_files(tmp.path(), Some("src"));
        assert_eq!(files, vec!["src/main.rs".to_string()]);
    }

    // Scenario: list_files degrades to empty when not a git repository.
    #[test]
    fn list_files_empty_when_not_a_git_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(list_files(tmp.path(), None).is_empty());
    }

    // Scenario: read_file returns a file's content from the local working tree.
    #[test]
    fn read_file_happy_path_returns_working_tree_content() {
        let tmp = fixture();
        let out = read_file(tmp.path(), "src/main.rs").unwrap();
        assert!(
            out.content
                .as_deref()
                .unwrap()
                .contains("register_watch_target_http")
        );
        assert!(out.message.is_none());
    }

    // Scenario: read_file refuses path traversal outside the repository root.
    #[test]
    fn read_file_refuses_dotdot_traversal() {
        let tmp = fixture();
        // A secret outside the repo root.
        let parent = tmp.path().parent().unwrap();
        std::fs::write(parent.join("paw-secret.txt"), "TOPSECRET").unwrap();
        let out = read_file(tmp.path(), "../paw-secret.txt").unwrap();
        assert!(out.content.is_none(), "traversal must be refused");
        assert!(out.message.is_some());
    }

    // Scenario: read_file refuses path traversal outside the repository root
    // (absolute form).
    #[test]
    fn read_file_refuses_absolute_path_outside_root() {
        let tmp = fixture();
        let parent = tmp.path().parent().unwrap();
        let secret = parent.join("paw-secret-abs.txt");
        std::fs::write(&secret, "TOPSECRET").unwrap();
        let abs = secret.to_string_lossy().into_owned();
        let out = read_file(tmp.path(), &abs).unwrap();
        assert!(out.content.is_none(), "absolute escape must be refused");
        assert!(out.message.is_some());
    }

    // Scenario: read_file refuses a gitignored path.
    #[test]
    fn read_file_refuses_gitignored_path() {
        let tmp = fixture();
        let out = read_file(tmp.path(), "target/debug/foo").unwrap();
        assert!(out.content.is_none(), "gitignored path must be refused");
        assert!(
            out.message.as_deref().unwrap().contains("gitignored"),
            "message: {:?}",
            out.message
        );
    }

    #[test]
    fn read_file_missing_file_yields_none() {
        let tmp = fixture();
        let out = read_file(tmp.path(), "src/does-not-exist.rs").unwrap();
        assert!(out.content.is_none());
        assert!(out.message.is_some());
    }

    // Scenario: search_code returns matches across the working tree.
    #[test]
    fn search_code_finds_known_string() {
        let tmp = fixture();
        let (matches, truncated) = search_code(tmp.path(), "register_watch_target_http", None);
        assert!(!truncated);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "src/main.rs");
        assert_eq!(matches[0].line_number, 2);
        assert!(matches[0].line.contains("register_watch_target_http"));
    }

    // Scenario: search_code degrades to empty when there are no matches.
    #[test]
    fn search_code_empty_when_no_match() {
        let tmp = fixture();
        let (matches, truncated) = search_code(tmp.path(), "a-string-that-appears-nowhere", None);
        assert!(matches.is_empty());
        assert!(!truncated);
    }

    #[test]
    fn search_code_empty_when_not_a_git_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let (matches, truncated) = search_code(tmp.path(), "anything", None);
        assert!(matches.is_empty());
        assert!(!truncated);
    }

    #[test]
    fn search_code_truncates_beyond_cap() {
        let tmp = init_repo();
        let dir = tmp.path();
        let mut body = String::new();
        for _ in 0..(SEARCH_MATCH_CAP + 50) {
            body.push_str("needle\n");
        }
        std::fs::write(dir.join("big.txt"), body).unwrap();
        git_run(dir, &["add", "big.txt"]);
        let (matches, truncated) = search_code(dir, "needle", None);
        assert_eq!(matches.len(), SEARCH_MATCH_CAP);
        assert!(truncated);
    }
}
