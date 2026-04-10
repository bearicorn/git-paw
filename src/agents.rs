//! AGENTS.md generation and injection.
//!
//! Provides marker-based section injection into `AGENTS.md` files.
//! Core logic uses pure `&str → String` functions for testability,
//! with a thin I/O wrapper for file operations.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::PawError;

/// Start marker prefix used for detection (ignores trailing comment text).
const START_MARKER_PREFIX: &str = "<!-- git-paw:start";

/// Full start marker line.
const START_MARKER: &str = "<!-- git-paw:start — managed by git-paw, do not edit manually -->";

/// End marker line.
const END_MARKER: &str = "<!-- git-paw:end -->";

/// Returns `true` if `content` contains a git-paw section start marker.
pub fn has_git_paw_section(content: &str) -> bool {
    content
        .lines()
        .any(|line| line.starts_with(START_MARKER_PREFIX))
}

/// Replaces the git-paw section (start marker through end marker, inclusive)
/// with `new_section`. If the end marker is missing, replaces from the start
/// marker to EOF.
pub fn replace_git_paw_section(content: &str, new_section: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    let Some(start_idx) = lines
        .iter()
        .position(|l| l.starts_with(START_MARKER_PREFIX))
    else {
        return content.to_string();
    };

    let end_idx = lines[start_idx..]
        .iter()
        .position(|l| l.contains(END_MARKER))
        .map(|rel| start_idx + rel);

    let mut result = String::new();

    // Content before the start marker
    for line in &lines[..start_idx] {
        result.push_str(line);
        result.push('\n');
    }

    // The new section
    result.push_str(new_section);

    // Content after the end marker (if it exists)
    if let Some(end) = end_idx
        && end + 1 < lines.len()
    {
        for line in &lines[end + 1..] {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Preserve trailing newline behavior of original if we replaced to EOF
    if end_idx.is_none() && content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }

    result
}

/// Injects `section` into `content`: appends if no git-paw section exists,
/// replaces the existing one if present.
pub fn inject_into_content(content: &str, section: &str) -> String {
    if content.is_empty() {
        return section.to_string();
    }

    if has_git_paw_section(content) {
        return replace_git_paw_section(content, section);
    }

    // Append with proper spacing
    let mut result = content.to_string();
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result.push('\n');
    result.push_str(section);
    result
}

/// Reads a file (or treats a missing file as empty), injects `section`,
/// and writes the result back.
/// Per-worktree assignment context passed by the session launch flow.
pub struct WorktreeAssignment {
    /// The branch this worktree is checked out on.
    pub branch: String,
    /// The CLI name (e.g. "claude", "cursor") running in this worktree.
    pub cli: String,
    /// Optional spec content to embed in the assignment section.
    pub spec_content: Option<String>,
    /// Optional list of files this worktree owns.
    pub owned_files: Option<Vec<String>>,
    /// Optional rendered skill content to inject into the assignment section.
    pub skill_content: Option<String>,
}

/// Generates a marker-delimited assignment section for a worktree's AGENTS.md.
pub fn generate_worktree_section(assignment: &WorktreeAssignment) -> String {
    let mut section = String::new();
    section.push_str(START_MARKER);
    section.push('\n');
    section.push('\n');
    section.push_str("## git-paw Session Assignment\n");
    section.push('\n');
    let _ = writeln!(section, "- **Branch:** `{}`", assignment.branch);
    let _ = writeln!(section, "- **CLI:** {}", assignment.cli);

    if let Some(ref spec) = assignment.spec_content {
        section.push('\n');
        section.push_str("### Spec\n");
        section.push('\n');
        section.push_str(spec);
        if !spec.ends_with('\n') {
            section.push('\n');
        }
    }

    if let Some(ref files) = assignment.owned_files {
        section.push('\n');
        section.push_str("### File Ownership\n");
        section.push('\n');
        for file in files {
            let _ = writeln!(section, "- `{file}`");
        }
    }

    if let Some(ref skill) = assignment.skill_content {
        section.push('\n');
        section.push_str(skill);
        if !skill.ends_with('\n') {
            section.push('\n');
        }
    }

    section.push('\n');
    section.push_str(END_MARKER);
    section.push('\n');
    section
}

/// Reads the root repo's AGENTS.md, injects the worktree assignment section,
/// writes the result to the worktree root, and protects it from being committed.
///
/// Uses two layers of protection:
/// 1. `.git/info/exclude` — hides AGENTS.md from `git status`
/// 2. `git update-index --assume-unchanged` — prevents `git add -A` from staging it
///
/// The second layer is critical for AI agents that run `git add -A` or
/// `git add .` to commit their work — without it, the injected session
/// content would be committed to the branch.
pub fn setup_worktree_agents_md(
    repo_root: &Path,
    worktree_root: &Path,
    assignment: &WorktreeAssignment,
) -> Result<(), PawError> {
    let root_agents = repo_root.join("AGENTS.md");
    let root_content = match fs::read_to_string(&root_agents) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(PawError::AgentsMdError(format!(
                "failed to read '{}': {e}",
                root_agents.display()
            )));
        }
    };

    let section = generate_worktree_section(assignment);
    let output = inject_into_content(&root_content, &section);

    let worktree_agents = worktree_root.join("AGENTS.md");
    fs::write(&worktree_agents, &output).map_err(|e| {
        PawError::AgentsMdError(format!(
            "failed to write '{}': {e}",
            worktree_agents.display()
        ))
    })?;

    exclude_from_git(worktree_root, "AGENTS.md")?;

    // Belt-and-suspenders: mark the file as assume-unchanged so `git add -A`
    // doesn't stage it. This only works when AGENTS.md is already tracked in
    // the index (which it is for worktrees of repos that have a tracked
    // AGENTS.md). For repos without a tracked AGENTS.md, exclude_from_git
    // above is the primary protection.
    let _ = assume_unchanged(worktree_root, "AGENTS.md");

    Ok(())
}

/// Marks a file as assume-unchanged in git's index.
///
/// This prevents `git add -A`, `git add .`, and `git commit -a` from
/// staging changes to the file, even if its content has been modified.
/// Used to protect injected session content from being committed by
/// AI coding agents.
pub fn assume_unchanged(worktree_root: &Path, filename: &str) -> Result<(), PawError> {
    let output = std::process::Command::new("git")
        .current_dir(worktree_root)
        .args(["update-index", "--assume-unchanged", filename])
        .output()
        .map_err(|e| {
            PawError::AgentsMdError(format!(
                "failed to run git update-index --assume-unchanged: {e}"
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::AgentsMdError(format!(
            "git update-index --assume-unchanged failed for '{filename}': {stderr}"
        )));
    }

    Ok(())
}

/// Resolves the actual `.git` directory for a worktree.
///
/// In regular repos, `.git` is a directory. In worktrees created by
/// `git worktree add`, `.git` is a file containing `gitdir: <path>`.
fn resolve_git_dir(worktree_root: &Path) -> Result<PathBuf, PawError> {
    let dot_git = worktree_root.join(".git");
    if dot_git.is_dir() {
        return Ok(dot_git);
    }
    // Worktree: .git is a file with "gitdir: <path>"
    if dot_git.is_file() {
        let content = fs::read_to_string(&dot_git).map_err(|e| {
            PawError::AgentsMdError(format!("failed to read '{}': {e}", dot_git.display()))
        })?;
        if let Some(gitdir) = content.trim().strip_prefix("gitdir: ") {
            let path = Path::new(gitdir);
            if path.is_absolute() {
                return Ok(path.to_path_buf());
            }
            return Ok(worktree_root.join(path));
        }
    }
    // Fallback: treat as regular .git directory
    Ok(dot_git)
}

/// Adds `filename` to the worktree's `.git/info/exclude` if not already present.
pub fn exclude_from_git(worktree_root: &Path, filename: &str) -> Result<(), PawError> {
    let git_dir = resolve_git_dir(worktree_root)?;
    let git_info = git_dir.join("info");
    if !git_info.exists() {
        fs::create_dir_all(&git_info).map_err(|e| {
            PawError::AgentsMdError(format!("failed to create '{}': {e}", git_info.display()))
        })?;
    }

    let exclude_path = git_info.join("exclude");
    let content = match fs::read_to_string(&exclude_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(PawError::AgentsMdError(format!(
                "failed to read '{}': {e}",
                exclude_path.display()
            )));
        }
    };

    if content.lines().any(|line| line.trim() == filename) {
        return Ok(());
    }

    let mut new_content = content;
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str(filename);
    new_content.push('\n');

    fs::write(&exclude_path, &new_content).map_err(|e| {
        PawError::AgentsMdError(format!("failed to write '{}': {e}", exclude_path.display()))
    })
}

pub fn inject_section_into_file(path: &Path, section: &str) -> Result<(), PawError> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(PawError::AgentsMdError(format!(
                "failed to read '{}': {e}",
                path.display()
            )));
        }
    };

    let output = inject_into_content(&content, section);

    fs::write(path, &output)
        .map_err(|e| PawError::AgentsMdError(format!("failed to write '{}': {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper: generates a sample marker-delimited section for testing injection logic.
    fn sample_section() -> String {
        format!("{START_MARKER}\n## git-paw test section\n{END_MARKER}\n")
    }

    // -----------------------------------------------------------------------
    // has_git_paw_section
    // -----------------------------------------------------------------------

    #[test]
    fn has_section_returns_true_when_marker_present() {
        let content = "# My Project\n\n<!-- git-paw:start — managed by git-paw, do not edit manually -->\nstuff\n<!-- git-paw:end -->\n";
        assert!(has_git_paw_section(content));
    }

    #[test]
    fn has_section_returns_false_without_marker() {
        let content = "# My Project\n\nSome instructions.\n";
        assert!(!has_git_paw_section(content));
    }

    #[test]
    fn has_section_returns_false_for_empty() {
        assert!(!has_git_paw_section(""));
    }

    // -----------------------------------------------------------------------
    // generate_git_paw_section
    // -----------------------------------------------------------------------

    #[test]
    fn generated_section_has_markers() {
        let section = sample_section();
        assert!(section.starts_with(START_MARKER));
        assert!(section.contains(END_MARKER));
    }

    #[test]
    fn sample_section_contains_git_paw_reference() {
        let section = sample_section();
        assert!(section.contains("git-paw"));
    }

    // -----------------------------------------------------------------------
    // replace_git_paw_section
    // -----------------------------------------------------------------------

    #[test]
    fn replace_with_both_markers_preserves_surrounding() {
        let content = "# Title\n\n<!-- git-paw:start — managed by git-paw, do not edit manually -->\nold content\n<!-- git-paw:end -->\n\n## Footer\n";
        let new_section = "<!-- git-paw:start — managed by git-paw, do not edit manually -->\nnew content\n<!-- git-paw:end -->\n";
        let result = replace_git_paw_section(content, new_section);
        assert!(result.contains("# Title"));
        assert!(result.contains("new content"));
        assert!(!result.contains("old content"));
        assert!(result.contains("## Footer"));
    }

    #[test]
    fn replace_with_missing_end_marker_replaces_to_eof() {
        let content = "# Title\n\n<!-- git-paw:start — managed by git-paw, do not edit manually -->\nold content that never ends\n";
        let new_section = "<!-- git-paw:start — managed by git-paw, do not edit manually -->\nfixed\n<!-- git-paw:end -->\n";
        let result = replace_git_paw_section(content, new_section);
        assert!(result.contains("# Title"));
        assert!(result.contains("fixed"));
        assert!(!result.contains("old content"));
    }

    // -----------------------------------------------------------------------
    // inject_into_content
    // -----------------------------------------------------------------------

    #[test]
    fn inject_appends_when_no_existing_section() {
        let content = "# My Project\n\nSome info.\n";
        let section = sample_section();
        let result = inject_into_content(content, &section);
        assert!(result.starts_with("# My Project"));
        assert!(result.contains(START_MARKER));
    }

    #[test]
    fn inject_replaces_existing_section() {
        let old_section = format!("{START_MARKER}\nold\n{END_MARKER}\n");
        let content = format!("# Title\n\n{old_section}\n## Footer\n");
        let new_section = format!("{START_MARKER}\nnew\n{END_MARKER}\n");
        let result = inject_into_content(&content, &new_section);
        assert!(result.contains("new"));
        assert!(!result.contains("old"));
        assert!(result.contains("## Footer"));
    }

    #[test]
    fn inject_into_empty_content_returns_section_only() {
        let section = sample_section();
        let result = inject_into_content("", &section);
        assert_eq!(result, section);
    }

    // -----------------------------------------------------------------------
    // Spacing tests
    // -----------------------------------------------------------------------

    #[test]
    fn spacing_with_trailing_newline() {
        let content = "# Title\n";
        let section = "<!-- git-paw:start -->\n<!-- git-paw:end -->\n";
        let result = inject_into_content(content, section);
        // Should have blank line separator: "# Title\n\n<!-- git-paw..."
        assert!(result.contains("# Title\n\n<!-- git-paw:start"));
    }

    #[test]
    fn spacing_without_trailing_newline() {
        let content = "# Title";
        let section = "<!-- git-paw:start -->\n<!-- git-paw:end -->\n";
        let result = inject_into_content(content, section);
        // Should add newline + blank line: "# Title\n\n<!-- git-paw..."
        assert!(result.contains("# Title\n\n<!-- git-paw:start"));
    }

    // -----------------------------------------------------------------------
    // File I/O tests
    // -----------------------------------------------------------------------

    #[test]
    fn file_inject_appends_to_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        fs::write(&path, "# Existing\n").unwrap();

        let section = sample_section();
        inject_section_into_file(&path, &section).unwrap();

        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains("# Existing"));
        assert!(result.contains(START_MARKER));
    }

    #[test]
    fn file_inject_replaces_existing_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        let initial = format!("# Title\n\n{START_MARKER}\nold\n{END_MARKER}\n");
        fs::write(&path, &initial).unwrap();

        let new_section = sample_section();
        inject_section_into_file(&path, &new_section).unwrap();

        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains("# Title"));
        assert!(!result.contains("\nold\n"));
        assert!(result.contains("git-paw test section"));
    }

    #[test]
    fn file_inject_creates_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        assert!(!path.exists());

        let section = sample_section();
        inject_section_into_file(&path, &section).unwrap();

        let result = fs::read_to_string(&path).unwrap();
        assert!(result.contains(START_MARKER));
    }

    #[test]
    fn file_inject_readonly_returns_error() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AGENTS.md");
        fs::write(&path, "content").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();

        let section = sample_section();
        let result = inject_section_into_file(&path, &section);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("AGENTS.md error"), "got: {msg}");
        assert!(
            msg.contains("AGENTS.md"),
            "should mention file path, got: {msg}"
        );

        // Cleanup: restore permissions so tempdir can be removed
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    }

    // -----------------------------------------------------------------------
    // generate_worktree_section
    // -----------------------------------------------------------------------

    fn make_assignment(spec: Option<&str>, files: Option<Vec<&str>>) -> WorktreeAssignment {
        WorktreeAssignment {
            branch: "feat/foo".to_string(),
            cli: "claude".to_string(),
            spec_content: spec.map(ToString::to_string),
            owned_files: files.map(|v| v.into_iter().map(ToString::to_string).collect()),
            skill_content: None,
        }
    }

    fn make_assignment_with_skill(
        spec: Option<&str>,
        files: Option<Vec<&str>>,
        skill: Option<&str>,
    ) -> WorktreeAssignment {
        WorktreeAssignment {
            branch: "feat/foo".to_string(),
            cli: "claude".to_string(),
            spec_content: spec.map(ToString::to_string),
            owned_files: files.map(|v| v.into_iter().map(ToString::to_string).collect()),
            skill_content: skill.map(ToString::to_string),
        }
    }

    #[test]
    fn worktree_section_all_fields() {
        let assignment = make_assignment(
            Some("Implement the widget.\n"),
            Some(vec!["src/widget.rs", "tests/widget.rs"]),
        );
        let section = generate_worktree_section(&assignment);
        assert!(section.starts_with(START_MARKER));
        assert!(section.contains(END_MARKER));
        assert!(section.contains("`feat/foo`"));
        assert!(section.contains("claude"));
        assert!(section.contains("### Spec"));
        assert!(section.contains("Implement the widget."));
        assert!(section.contains("### File Ownership"));
        assert!(section.contains("`src/widget.rs`"));
        assert!(section.contains("`tests/widget.rs`"));
    }

    #[test]
    fn worktree_section_no_spec() {
        let assignment = make_assignment(None, Some(vec!["src/main.rs"]));
        let section = generate_worktree_section(&assignment);
        assert!(section.contains("`feat/foo`"));
        assert!(!section.contains("### Spec"));
        assert!(section.contains("### File Ownership"));
    }

    #[test]
    fn worktree_section_no_files() {
        let assignment = make_assignment(Some("Do the thing.\n"), None);
        let section = generate_worktree_section(&assignment);
        assert!(section.contains("### Spec"));
        assert!(!section.contains("### File Ownership"));
    }

    #[test]
    fn worktree_section_minimal() {
        let assignment = make_assignment(None, None);
        let section = generate_worktree_section(&assignment);
        assert!(section.starts_with(START_MARKER));
        assert!(section.contains(END_MARKER));
        assert!(section.contains("`feat/foo`"));
        assert!(section.contains("claude"));
        assert!(!section.contains("### Spec"));
        assert!(!section.contains("### File Ownership"));
    }

    // -----------------------------------------------------------------------
    // setup_worktree_agents_md
    // -----------------------------------------------------------------------

    /// Creates a real git repo in a tempdir (git init + initial commit).
    ///
    /// Resolves the absolute path to `git` once to avoid ENOENT races
    /// under heavy parallel test load on macOS.
    fn init_git_repo(dir: &Path) {
        use std::process::Command;
        let git = which::which("git").expect("git must be on PATH");
        Command::new(&git)
            .current_dir(dir)
            .args(["init"])
            .output()
            .expect("git init");
        Command::new(&git)
            .current_dir(dir)
            .args(["config", "user.email", "test@test.com"])
            .output()
            .expect("git config email");
        Command::new(&git)
            .current_dir(dir)
            .args(["config", "user.name", "Test"])
            .output()
            .expect("git config name");
        // Create and commit a file so HEAD exists
        fs::write(dir.join("README.md"), "# test\n").unwrap();
        Command::new(&git)
            .current_dir(dir)
            .args(["add", "README.md"])
            .output()
            .expect("git add");
        Command::new(&git)
            .current_dir(dir)
            .args(["commit", "-m", "init"])
            .output()
            .expect("git commit");
    }

    #[test]
    fn setup_worktree_root_exists() {
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        fs::write(repo.path().join("AGENTS.md"), "# Project Rules\n").unwrap();

        // Track AGENTS.md in the worktree's git index so assume-unchanged works
        fs::write(wt.path().join("AGENTS.md"), "# placeholder\n").unwrap();
        std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["add", "AGENTS.md"])
            .output()
            .expect("git add AGENTS.md");
        std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["commit", "-m", "add agents"])
            .output()
            .expect("git commit");

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        let result = fs::read_to_string(wt.path().join("AGENTS.md")).unwrap();
        assert!(result.contains("# Project Rules"));
        assert!(result.contains("`feat/foo`"));
        assert!(result.contains(START_MARKER));

        // Verify AGENTS.md is hidden from git status (assume-unchanged)
        let status = std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["status", "--porcelain"])
            .output()
            .expect("git status");
        let status_output = String::from_utf8_lossy(&status.stdout);
        assert!(
            !status_output.contains("AGENTS.md"),
            "AGENTS.md should not appear in git status, got: {status_output}"
        );
    }

    #[test]
    fn setup_worktree_root_missing() {
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        let result = fs::read_to_string(wt.path().join("AGENTS.md")).unwrap();
        assert!(!result.contains("# Project Rules"));
        assert!(result.contains("`feat/foo`"));
    }

    #[test]
    fn setup_worktree_replaces_root_section() {
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        let root_content =
            format!("# Rules\n\n{START_MARKER}\nold root section\n{END_MARKER}\n\n## Footer\n");
        fs::write(repo.path().join("AGENTS.md"), &root_content).unwrap();

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        let result = fs::read_to_string(wt.path().join("AGENTS.md")).unwrap();
        assert!(result.contains("# Rules"));
        assert!(result.contains("## Footer"));
        assert!(!result.contains("old root section"));
        assert!(result.contains("`feat/foo`"));
        assert_eq!(
            result.matches(START_MARKER_PREFIX).count(),
            1,
            "should have exactly one git-paw section"
        );
    }

    // -----------------------------------------------------------------------
    // setup_worktree_agents_md — write failure
    // -----------------------------------------------------------------------

    #[test]
    fn setup_worktree_write_failure_returns_agents_md_error() {
        use std::os::unix::fs::PermissionsExt;

        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());

        // Make the worktree root read-only so AGENTS.md cannot be written
        fs::set_permissions(wt.path(), fs::Permissions::from_mode(0o555)).unwrap();

        let assignment = make_assignment(None, None);
        let result = setup_worktree_agents_md(repo.path(), wt.path(), &assignment);

        // Restore permissions so tempdir cleanup can succeed
        fs::set_permissions(wt.path(), fs::Permissions::from_mode(0o755)).unwrap();

        assert!(result.is_err(), "should fail when worktree is read-only");
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("AGENTS.md error"),
            "should return AgentsMdError, got: {msg}"
        );
    }

    // -----------------------------------------------------------------------
    // exclude_from_git
    // -----------------------------------------------------------------------

    #[test]
    fn exclude_creates_file_when_missing() {
        let wt = tempfile::tempdir().unwrap();
        fs::create_dir_all(wt.path().join(".git/info")).unwrap();

        exclude_from_git(wt.path(), "AGENTS.md").unwrap();

        let content = fs::read_to_string(wt.path().join(".git/info/exclude")).unwrap();
        assert!(content.contains("AGENTS.md"));
    }

    #[test]
    fn exclude_appends_when_not_present() {
        let wt = tempfile::tempdir().unwrap();
        let info = wt.path().join(".git/info");
        fs::create_dir_all(&info).unwrap();
        fs::write(info.join("exclude"), "*.log\n").unwrap();

        exclude_from_git(wt.path(), "AGENTS.md").unwrap();

        let content = fs::read_to_string(info.join("exclude")).unwrap();
        assert!(content.contains("*.log"));
        assert!(content.contains("AGENTS.md"));
    }

    #[test]
    fn exclude_no_duplicate() {
        let wt = tempfile::tempdir().unwrap();
        let info = wt.path().join(".git/info");
        fs::create_dir_all(&info).unwrap();
        fs::write(info.join("exclude"), "AGENTS.md\n").unwrap();

        exclude_from_git(wt.path(), "AGENTS.md").unwrap();

        let content = fs::read_to_string(info.join("exclude")).unwrap();
        assert_eq!(content.matches("AGENTS.md").count(), 1);
    }

    #[test]
    fn exclude_creates_info_dir() {
        let wt = tempfile::tempdir().unwrap();
        fs::create_dir_all(wt.path().join(".git")).unwrap();
        assert!(!wt.path().join(".git/info").exists());

        exclude_from_git(wt.path(), "AGENTS.md").unwrap();

        assert!(wt.path().join(".git/info/exclude").exists());
        let content = fs::read_to_string(wt.path().join(".git/info/exclude")).unwrap();
        assert!(content.contains("AGENTS.md"));
    }

    // -----------------------------------------------------------------------
    // generate_worktree_section — skill_content
    // -----------------------------------------------------------------------

    #[test]
    fn worktree_section_all_fields_with_skill() {
        let assignment = make_assignment_with_skill(
            Some("Implement the widget.\n"),
            Some(vec!["src/widget.rs", "tests/widget.rs"]),
            Some("## Coordination\nUse the broker at ${GIT_PAW_BROKER_URL} as feat-foo.\n"),
        );
        let section = generate_worktree_section(&assignment);
        assert!(section.starts_with(START_MARKER));
        assert!(section.contains(END_MARKER));
        assert!(section.contains("`feat/foo`"));
        assert!(section.contains("claude"));
        assert!(section.contains("### Spec"));
        assert!(section.contains("Implement the widget."));
        assert!(section.contains("### File Ownership"));
        assert!(section.contains("`src/widget.rs`"));
        assert!(section.contains("## Coordination"));
        // Skill content appears after file ownership and before end marker
        let ownership_pos = section.find("### File Ownership").unwrap();
        let skill_pos = section.find("## Coordination").unwrap();
        let end_pos = section.find(END_MARKER).unwrap();
        assert!(
            ownership_pos < skill_pos,
            "skill must come after file ownership"
        );
        assert!(skill_pos < end_pos, "skill must come before end marker");
    }

    #[test]
    fn worktree_section_skill_without_spec_or_files() {
        let assignment = make_assignment_with_skill(
            None,
            None,
            Some("## Coordination\nBroker instructions here.\n"),
        );
        let section = generate_worktree_section(&assignment);
        assert!(section.contains("`feat/foo`"));
        assert!(section.contains("claude"));
        assert!(!section.contains("### Spec"));
        assert!(!section.contains("### File Ownership"));
        assert!(section.contains("## Coordination"));
        // Skill content appears after assignment and before end marker
        let assignment_pos = section.find("**CLI:**").unwrap();
        let skill_pos = section.find("## Coordination").unwrap();
        let end_pos = section.find(END_MARKER).unwrap();
        assert!(
            assignment_pos < skill_pos,
            "skill must come after assignment"
        );
        assert!(skill_pos < end_pos, "skill must come before end marker");
    }

    #[test]
    fn worktree_section_none_skill_matches_v020() {
        // With skill_content = None, output must be identical to make_assignment (no skill)
        let with_none =
            make_assignment_with_skill(Some("Do the thing.\n"), Some(vec!["src/main.rs"]), None);
        let without = make_assignment(Some("Do the thing.\n"), Some(vec!["src/main.rs"]));
        assert_eq!(
            generate_worktree_section(&with_none),
            generate_worktree_section(&without),
            "skill_content = None must produce identical output to v0.2.0"
        );
    }

    #[test]
    fn worktree_section_skill_contains_slugified_branch() {
        let assignment = WorktreeAssignment {
            branch: "feat/http-broker".to_string(),
            cli: "claude".to_string(),
            spec_content: None,
            owned_files: None,
            skill_content: Some(
                "Agent ID: feat-http-broker\nURL: ${GIT_PAW_BROKER_URL}\n".to_string(),
            ),
        };
        let section = generate_worktree_section(&assignment);
        assert!(
            section.contains("feat-http-broker"),
            "should contain slugified branch"
        );
        assert!(
            !section.contains("{{BRANCH_ID}}"),
            "should not contain literal template placeholder"
        );
    }

    #[test]
    fn worktree_section_skill_preserves_broker_url_placeholder() {
        let assignment = make_assignment_with_skill(
            None,
            None,
            Some("Connect to ${GIT_PAW_BROKER_URL}/messages\n"),
        );
        let section = generate_worktree_section(&assignment);
        assert!(
            section.contains("${GIT_PAW_BROKER_URL}"),
            "broker URL placeholder must be preserved as literal"
        );
    }
}
