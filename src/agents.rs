//! AGENTS.md generation and injection.
//!
//! Provides marker-based section injection into `AGENTS.md` files.
//! Core logic uses pure `&str → String` functions for testability,
//! with a thin I/O wrapper for file operations.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::PawError;
use crate::git::{assume_unchanged, exclude_from_git};

/// Start marker prefix used for detection (ignores trailing comment text).
const START_MARKER_PREFIX: &str = "<!-- git-paw:start";

/// Full start marker line.
const START_MARKER: &str = "<!-- git-paw:start — managed by git-paw, do not edit manually -->";

/// End marker line.
const END_MARKER: &str = "<!-- git-paw:end -->";

/// Marker that identifies git-paw-managed git hook content.
///
/// When a project already has a `post-commit` or `pre-push` hook, git-paw
/// chains its content after the existing hook, wrapped in marker lines so
/// subsequent installs don't duplicate the block.
const HOOK_START_MARKER: &str = "# >>> git-paw managed hook >>>";
const HOOK_END_MARKER: &str = "# <<< git-paw managed hook <<<";

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
    /// Optional inter-agent rules block (file ownership, never-push, proactive
    /// status publishing, cherry-pick) injected by the supervisor. When `None`,
    /// the generated section omits the `## Inter-Agent Rules` subsection
    /// entirely so non-supervisor sessions are byte-identical to pre-supervisor
    /// output.
    pub inter_agent_rules: Option<String>,
}

/// Builds the standard inter-agent rules block that the supervisor injects
/// into every coding agent's `AGENTS.md`.
///
/// `branches` is the list of all peer branches in the session — used to make
/// the file-ownership constraint explicit ("don't touch files owned by ...").
pub fn build_inter_agent_rules(branches: &[&str]) -> String {
    let mut peers = String::new();
    for (i, b) in branches.iter().enumerate() {
        if i > 0 {
            peers.push_str(", ");
        }
        peers.push('`');
        peers.push_str(b);
        peers.push('`');
    }

    let mut out = String::new();
    out.push_str("These rules apply to every agent in this supervisor session. ");
    out.push_str("Violating them blocks the supervisor's verification step.\n\n");
    out.push_str("- **File ownership is exclusive.** You MUST NOT edit files owned by ");
    out.push_str("other agents. Peers in this session: ");
    out.push_str(&peers);
    out.push_str(". Stay inside your declared file ownership list.\n");
    out.push_str("- **Commit, never push.** You MUST commit to your worktree branch and ");
    out.push_str("MUST NOT `git push` to any remote. The supervisor merges branches.\n");
    out.push_str("- **Status publishing is automatic.** git-paw watches your worktree and ");
    out.push_str("publishes `agent.status` with `modified_files` for you whenever your git ");
    out.push_str("status changes. A `post-commit` hook publishes `agent.artifact` on each ");
    out.push_str("commit. You do not need to curl these yourself.\n");
    out.push_str("- **Watch peer status.** Poll `/messages/{{BRANCH_ID}}` to see peer ");
    out.push_str("`agent.artifact` messages so you detect conflicts before the supervisor does.\n");
    out.push_str("- **Cherry-pick peer artifacts.** When you are blocked on a peer, publish ");
    out.push_str("`agent.blocked` and cherry-pick their commit when their artifact arrives ");
    out.push_str("in your inbox. Do not wait for the supervisor to merge.\n");
    out.push_str("- **Match spec field names exactly.** When implementing a spec, use the ");
    out.push_str("exact field, function, and message names from the spec — do not rename ");
    out.push_str("them. The supervisor's spec audit will reject mismatched names.\n");
    out
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

    if let Some(ref rules) = assignment.inter_agent_rules {
        section.push('\n');
        section.push_str("## Inter-Agent Rules\n");
        section.push('\n');
        section.push_str(rules);
        if !rules.ends_with('\n') {
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

/// Returns the path to the agent marker file for a given worktree.
pub fn get_agent_marker_path(worktree: &Path) -> Result<PathBuf, PawError> {
    let linked_git_dir = git_rev_parse_path(worktree, "--git-dir")?;
    Ok(linked_git_dir.join("paw-agent-id"))
}

/// Builds the agent marker file content with optional extended fields.
///
/// Basic format (always included):
/// ```text
/// PAW_AGENT_ID=<agent_id>
/// PAW_BROKER_URL=<broker_url>
/// ```
///
/// Extended format (optional fields):
/// ```text
/// PAW_SUPERVISOR_PID=<pid>
/// PAW_LAST_VERIFIED_COMMIT=<commit_hash>
/// PAW_SESSION_NAME=<session_name>
/// PAW_TIMESTAMP=<iso_timestamp>
/// ```
pub fn build_agent_marker(
    broker_url: &str,
    agent_id: &str,
    supervisor_pid: Option<u32>,
    last_verified_commit: Option<&str>,
    session_name: Option<&str>,
) -> String {
    let mut marker = format!("PAW_AGENT_ID={agent_id}\nPAW_BROKER_URL={broker_url}\n");

    // Add optional extended fields
    if let Some(pid) = supervisor_pid {
        let _ = writeln!(marker, "PAW_SUPERVISOR_PID={pid}");
    }
    if let Some(commit) = last_verified_commit {
        let _ = writeln!(marker, "PAW_LAST_VERIFIED_COMMIT={commit}");
    }
    if let Some(session) = session_name {
        let _ = writeln!(marker, "PAW_SESSION_NAME={session}");
    }

    // Always add timestamp for debugging/tracing
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let _ = writeln!(marker, "PAW_TIMESTAMP={timestamp}");

    marker
}

/// Updates an existing agent marker file with additional fields.
///
/// This allows adding supervisor-specific information after the initial marker creation.
///
/// # Panics
///
/// Panics if the marker file contains malformed content that cannot be processed by
/// the regex replacement logic.
pub fn update_agent_marker(
    marker_path: &Path,
    supervisor_pid: Option<u32>,
    last_verified_commit: Option<&str>,
) -> Result<(), PawError> {
    let content = fs::read_to_string(marker_path)
        .map_err(|e| PawError::AgentsMdError(format!("failed to read marker file: {e}")))?;

    let mut updated = content;

    // Update supervisor PID if provided
    if let Some(pid) = supervisor_pid {
        if updated.contains("PAW_SUPERVISOR_PID=") {
            // Replace existing PID
            updated = regex::Regex::new(r"PAW_SUPERVISOR_PID=\d+")
                .unwrap()
                .replace(&updated, &format!("PAW_SUPERVISOR_PID={pid}"))
                .to_string();
        } else {
            // Add new PID line
            let _ = write!(updated, "\nPAW_SUPERVISOR_PID={pid}");
        }
    }

    // Update last verified commit if provided
    if let Some(commit) = last_verified_commit {
        if updated.contains("PAW_LAST_VERIFIED_COMMIT=") {
            // Replace existing commit
            updated = regex::Regex::new(r"PAW_LAST_VERIFIED_COMMIT=[^\n]+")
                .unwrap()
                .replace(&updated, &format!("PAW_LAST_VERIFIED_COMMIT={commit}"))
                .to_string();
        } else {
            // Add new commit line
            let _ = write!(updated, "\nPAW_LAST_VERIFIED_COMMIT={commit}");
        }
    }

    fs::write(marker_path, updated)
        .map_err(|e| PawError::AgentsMdError(format!("failed to update marker file: {e}")))?;

    Ok(())
}

/// Builds the dispatcher `post-commit` hook installed at the main repo's
/// `.git/hooks/post-commit`.
///
/// Linked git worktrees share the main repo's `hooks/` directory (git-worktree
/// does not use per-worktree hook directories unless `extensions.worktreeConfig`
/// is enabled, which is an intrusive repo-wide setting). Instead we install a
/// single dispatcher and store per-worktree `agent_id` and `broker_url` in
/// `$GIT_DIR/paw-agent-id` — `$GIT_DIR` is set by git to the correct
/// per-worktree gitdir when the hook runs, so the dispatcher reads the right
/// file for whichever worktree just committed.
fn build_post_commit_dispatcher_hook() -> String {
    format!(
        "#!/bin/sh\n\
         {HOOK_START_MARKER}\n\
         # Dispatcher: reads per-worktree $GIT_DIR/paw-agent-id and publishes\n\
         # agent.artifact to the git-paw broker.\n\
         if [ -n \"$GIT_DIR\" ] && [ -f \"$GIT_DIR/paw-agent-id\" ]; then\n\
             . \"$GIT_DIR/paw-agent-id\"\n\
             FILES=$(git diff HEAD~1 --name-only 2>/dev/null | awk '{{printf \"%s\\\"%s\\\"\", (NR>1?\",\":\"\"), $0}}')\n\
             curl -s -X POST \"$PAW_BROKER_URL/publish\" \\\n\
                 -H 'Content-Type: application/json' \\\n\
                 -d \"{{\\\"type\\\":\\\"agent.artifact\\\",\\\"agent_id\\\":\\\"$PAW_AGENT_ID\\\",\\\"payload\\\":{{\\\"status\\\":\\\"committed\\\",\\\"exports\\\":[],\\\"modified_files\\\":[$FILES]}}}}\" \\\n\
                 >/dev/null 2>&1 || true\n\
         fi\n\
         {HOOK_END_MARKER}\n"
    )
}

fn build_pre_push_hook() -> String {
    format!(
        "#!/bin/sh\n\
         {HOOK_START_MARKER}\n\
         echo 'error: git-paw agents must not push. The supervisor handles merges.' >&2\n\
         exit 1\n\
         {HOOK_END_MARKER}\n"
    )
}

/// Chains `new_body` onto `existing`, preserving the existing content.
///
/// If `existing` already contains a complete git-paw marker block, it is
/// replaced. If only the start marker is present (corrupted/truncated block),
/// the existing content is preserved verbatim and `new_body` is appended —
/// never silently discarded — so the user's shebang and original logic stay
/// intact. Otherwise `new_body` is appended after the existing content.
fn chain_hook(existing: &str, new_body: &str) -> String {
    // Complete marker block — replace it. If only the start marker is
    // present (corrupted/truncated block), fall through to the append path
    // so the user's shebang and original logic are preserved instead of
    // being silently discarded.
    if let Some(start) = existing.find(HOOK_START_MARKER)
        && let Some(end_rel) = existing[start..].find(HOOK_END_MARKER)
    {
        let end = start + end_rel + HOOK_END_MARKER.len();
        let mut out = String::with_capacity(existing.len() + new_body.len());
        out.push_str(&existing[..start]);
        // Strip the shebang from the new body when chaining onto an existing
        // hook — the existing file already has one.
        let stripped = new_body.strip_prefix("#!/bin/sh\n").unwrap_or(new_body);
        out.push_str(stripped);
        out.push_str(&existing[end..]);
        return out;
    }
    let mut out = existing.trim_end().to_string();
    if !out.is_empty() {
        out.push('\n');
    }
    let stripped = if out.is_empty() {
        new_body.to_string()
    } else {
        new_body
            .strip_prefix("#!/bin/sh\n")
            .unwrap_or(new_body)
            .to_string()
    };
    out.push_str(&stripped);
    out
}

fn write_hook_file(hook_path: &Path, new_body: &str) -> Result<(), PawError> {
    let existing = match fs::read_to_string(hook_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(PawError::AgentsMdError(format!(
                "failed to read '{}': {e}",
                hook_path.display()
            )));
        }
    };

    let content = if existing.is_empty() {
        new_body.to_string()
    } else {
        chain_hook(&existing, new_body)
    };

    if let Some(parent) = hook_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PawError::AgentsMdError(format!("failed to create '{}': {e}", parent.display()))
        })?;
    }

    fs::write(hook_path, content.as_bytes()).map_err(|e| {
        PawError::AgentsMdError(format!("failed to write '{}': {e}", hook_path.display()))
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(hook_path)
            .map_err(|e| {
                PawError::AgentsMdError(format!("failed to stat '{}': {e}", hook_path.display()))
            })?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(hook_path, perms).map_err(|e| {
            PawError::AgentsMdError(format!("failed to chmod '{}': {e}", hook_path.display()))
        })?;
    }

    Ok(())
}

/// Resolves a path from `git rev-parse <flag>` inside `worktree`.
///
/// Returns the absolute, trimmed path. The output of `git rev-parse` may be
/// relative to the worktree, so we canonicalise it against the worktree root
/// when it is not already absolute.
fn git_rev_parse_path(worktree: &Path, flag: &str) -> Result<PathBuf, PawError> {
    let output = std::process::Command::new("git")
        .current_dir(worktree)
        .args(["rev-parse", flag])
        .output()
        .map_err(|e| PawError::AgentsMdError(format!("failed to run git rev-parse {flag}: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PawError::AgentsMdError(format!(
            "git rev-parse {flag} failed in '{}': {stderr}",
            worktree.display()
        )));
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let path = PathBuf::from(&raw);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(worktree.join(path))
    }
}

/// Installs git-paw's `post-commit` dispatcher and `pre-push` block hook.
///
/// Linked git worktrees share the main repository's `.git/hooks/` directory
/// (unless `extensions.worktreeConfig` is enabled, which is intrusive). This
/// function therefore:
///
/// 1. Resolves the **common** git dir via `git rev-parse --git-common-dir` and
///    installs the dispatcher hooks at `<common>/hooks/post-commit` and
///    `<common>/hooks/pre-push` (chained onto any existing hooks).
/// 2. Resolves the **linked** git dir via `git rev-parse --git-dir` and writes
///    a per-worktree marker file at `<linked>/paw-agent-id` containing the
///    `PAW_AGENT_ID` and `PAW_BROKER_URL` values the dispatcher will source.
///
/// The dispatcher hook reads `$GIT_DIR/paw-agent-id` at commit time — git sets
/// `GIT_DIR` to the correct per-worktree gitdir, so each worktree publishes
/// under its own agent id.
pub fn install_git_hooks(
    worktree: &Path,
    broker_url: &str,
    agent_id: &str,
) -> Result<(), PawError> {
    let common_git_dir = git_rev_parse_path(worktree, "--git-common-dir")?;
    let linked_git_dir = git_rev_parse_path(worktree, "--git-dir")?;
    let hooks_dir = common_git_dir.join("hooks");

    write_hook_file(
        &hooks_dir.join("post-commit"),
        &build_post_commit_dispatcher_hook(),
    )?;
    write_hook_file(&hooks_dir.join("pre-push"), &build_pre_push_hook())?;

    let marker_path = linked_git_dir.join("paw-agent-id");
    if let Some(parent) = marker_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PawError::AgentsMdError(format!("failed to create '{}': {e}", parent.display()))
        })?;
    }
    fs::write(
        &marker_path,
        build_agent_marker(broker_url, agent_id, None, None, None),
    )
    .map_err(|e| {
        PawError::AgentsMdError(format!("failed to write '{}': {e}", marker_path.display()))
    })?;

    Ok(())
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
            inter_agent_rules: None,
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
            inter_agent_rules: None,
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
            Some("## Coordination\nUse the broker at http://127.0.0.1:9119 as feat-foo.\n"),
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
                "Agent ID: feat-http-broker\nURL: http://127.0.0.1:9119\n".to_string(),
            ),
            inter_agent_rules: None,
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
            Some("Connect to http://127.0.0.1:9119/messages\n"),
        );
        let section = generate_worktree_section(&assignment);
        assert!(
            section.contains("http://127.0.0.1:9119"),
            "broker URL must be present"
        );
    }

    // -----------------------------------------------------------------------
    // generate_worktree_section — inter_agent_rules
    // -----------------------------------------------------------------------

    #[test]
    fn worktree_section_with_inter_agent_rules() {
        let mut assignment = make_assignment(Some("Do the widget.\n"), Some(vec!["src/widget.rs"]));
        assignment.inter_agent_rules = Some("Stay in your lane.\nNever push.\n".to_string());
        let section = generate_worktree_section(&assignment);
        assert!(section.contains("## Inter-Agent Rules"));
        assert!(section.contains("Stay in your lane."));
        // Rules section appears before end marker
        let rules_pos = section.find("## Inter-Agent Rules").unwrap();
        let end_pos = section.find(END_MARKER).unwrap();
        assert!(rules_pos < end_pos, "rules must come before end marker");
    }

    #[test]
    fn worktree_section_without_inter_agent_rules_has_no_section() {
        let assignment = make_assignment(Some("Do the widget.\n"), Some(vec!["src/widget.rs"]));
        let section = generate_worktree_section(&assignment);
        assert!(!section.contains("## Inter-Agent Rules"));
    }

    #[test]
    fn worktree_section_inter_agent_rules_none_matches_pre_change() {
        // When inter_agent_rules is None, output must equal the pre-change baseline.
        let baseline = make_assignment(Some("Do.\n"), Some(vec!["src/main.rs"]));
        let with_none = WorktreeAssignment {
            branch: baseline.branch.clone(),
            cli: baseline.cli.clone(),
            spec_content: baseline.spec_content.clone(),
            owned_files: baseline.owned_files.clone(),
            skill_content: None,
            inter_agent_rules: None,
        };
        assert_eq!(
            generate_worktree_section(&baseline),
            generate_worktree_section(&with_none),
        );
    }

    // -----------------------------------------------------------------------
    // build_inter_agent_rules
    // -----------------------------------------------------------------------

    #[test]
    fn build_inter_agent_rules_contains_file_ownership() {
        let rules = build_inter_agent_rules(&["feat/a", "feat/b"]);
        assert!(rules.contains("File ownership"));
        assert!(rules.contains("`feat/a`"));
        assert!(rules.contains("`feat/b`"));
    }

    #[test]
    fn build_inter_agent_rules_contains_never_push() {
        let rules = build_inter_agent_rules(&["feat/a"]);
        assert!(rules.contains("MUST NOT `git push`"));
    }

    #[test]
    fn build_inter_agent_rules_notes_automatic_status() {
        let rules = build_inter_agent_rules(&["feat/a"]);
        assert!(rules.contains("Status publishing is automatic"));
        assert!(rules.contains("post-commit"));
    }

    #[test]
    fn build_inter_agent_rules_contains_match_spec() {
        let rules = build_inter_agent_rules(&["feat/a"]);
        assert!(
            rules
                .to_lowercase()
                .contains("match spec field names exactly")
        );
    }

    #[test]
    fn build_inter_agent_rules_contains_cherry_pick_reference() {
        let rules = build_inter_agent_rules(&["feat/a"]);
        assert!(rules.to_lowercase().contains("cherry-pick"));
    }

    // -----------------------------------------------------------------------
    // Embedded coordination skill — proactive publishing + cherry-pick
    // -----------------------------------------------------------------------

    #[test]
    fn embedded_coordination_contains_cherry_pick() {
        let content = include_str!("../assets/agent-skills/coordination.md");
        assert!(content.contains("git cherry-pick"));
    }

    #[test]
    fn embedded_coordination_documents_automatic_status() {
        let content = include_str!("../assets/agent-skills/coordination.md");
        let lower = content.to_lowercase();
        assert!(lower.contains("automatic"));
        assert!(lower.contains("post-commit"));
    }

    #[test]
    fn embedded_coordination_does_not_require_manual_status_publish() {
        let content = include_str!("../assets/agent-skills/coordination.md");
        assert!(!content.contains("MUST publish `agent.status`"));
        assert!(!content.contains("You MUST publish `agent.status`"));
    }

    #[test]
    fn embedded_coordination_still_contains_optin_operations() {
        let content = include_str!("../assets/agent-skills/coordination.md");
        assert!(content.contains("agent.blocked"));
        assert!(content.contains("agent.artifact"));
        assert!(content.contains("{{GIT_PAW_BROKER_URL}}/messages/{{BRANCH_ID}}"));
    }

    #[test]
    fn embedded_coordination_requires_no_push() {
        let content = include_str!("../assets/agent-skills/coordination.md");
        assert!(content.contains("MUST NOT push"));
    }

    // -----------------------------------------------------------------------
    // Git hook installation
    // -----------------------------------------------------------------------

    #[test]
    fn post_commit_dispatcher_hook_reads_marker_and_publishes() {
        let script = build_post_commit_dispatcher_hook();
        assert!(script.contains("$GIT_DIR/paw-agent-id"));
        assert!(script.contains(". \"$GIT_DIR/paw-agent-id\""));
        assert!(script.contains("$PAW_BROKER_URL/publish"));
        assert!(script.contains("$PAW_AGENT_ID"));
        assert!(script.contains("agent.artifact"));
        assert!(script.contains("|| true"));
    }

    #[test]
    fn agent_marker_is_shell_sourceable() {
        let marker = build_agent_marker("http://127.0.0.1:9119", "feat-x", None, None, None);
        assert!(marker.contains("PAW_AGENT_ID=feat-x"));
        assert!(marker.contains("PAW_BROKER_URL=http://127.0.0.1:9119"));
    }

    #[test]
    fn pre_push_hook_exits_with_error() {
        let script = build_pre_push_hook();
        assert!(script.contains("exit 1"));
        assert!(script.contains("must not push"));
    }

    #[test]
    fn chain_hook_replaces_existing_git_paw_block() {
        let existing = format!(
            "#!/bin/sh\n\
             # user hook\n\
             echo hi\n\
             {HOOK_START_MARKER}\n\
             old git-paw content\n\
             {HOOK_END_MARKER}\n"
        );
        let new_body = format!(
            "#!/bin/sh\n\
             {HOOK_START_MARKER}\n\
             new git-paw content\n\
             {HOOK_END_MARKER}\n"
        );
        let chained = chain_hook(&existing, &new_body);
        assert!(chained.contains("# user hook"));
        assert!(chained.contains("echo hi"));
        assert!(chained.contains("new git-paw content"));
        assert!(!chained.contains("old git-paw content"));
    }

    #[test]
    fn chain_hook_appends_after_existing_content() {
        let existing = "#!/bin/sh\necho existing\n";
        let new_body = format!(
            "#!/bin/sh\n\
             {HOOK_START_MARKER}\n\
             new block\n\
             {HOOK_END_MARKER}\n"
        );
        let chained = chain_hook(existing, &new_body);
        assert!(chained.starts_with("#!/bin/sh\necho existing"));
        assert!(chained.contains("new block"));
        // The new shebang should be stripped when chaining.
        assert_eq!(chained.matches("#!/bin/sh").count(), 1);
    }

    #[test]
    fn chain_hook_preserves_content_when_end_marker_missing() {
        // Corrupted/truncated hook: start marker present, end marker missing.
        // The user's shebang and original logic must be preserved verbatim
        // and the new git-paw block appended.
        let existing = format!(
            "#!/bin/sh\n\
             # important user logic\n\
             echo do_not_lose_me\n\
             {HOOK_START_MARKER}\n\
             leftover but no end marker\n"
        );
        let new_body = format!(
            "#!/bin/sh\n\
             {HOOK_START_MARKER}\n\
             new git-paw content\n\
             {HOOK_END_MARKER}\n"
        );
        let chained = chain_hook(&existing, &new_body);
        // All original lines must survive.
        assert!(chained.contains("#!/bin/sh"));
        assert!(chained.contains("# important user logic"));
        assert!(chained.contains("echo do_not_lose_me"));
        assert!(chained.contains("leftover but no end marker"));
        // The new block must be appended.
        assert!(chained.contains("new git-paw content"));
        assert!(chained.contains(HOOK_END_MARKER));
        // The new shebang should be stripped (only the existing one remains).
        assert_eq!(chained.matches("#!/bin/sh").count(), 1);
    }

    #[test]
    #[serial_test::serial]
    fn install_git_hooks_writes_dispatcher_to_common_git_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let worktree = tmp.path();
        init_git_repo(worktree);

        install_git_hooks(worktree, "http://127.0.0.1:9119", "feat-x").unwrap();

        let post_commit = worktree.join(".git").join("hooks").join("post-commit");
        let pre_push = worktree.join(".git").join("hooks").join("pre-push");
        let marker = worktree.join(".git").join("paw-agent-id");

        assert!(post_commit.exists(), "post-commit should exist");
        assert!(pre_push.exists(), "pre-push should exist");
        assert!(marker.exists(), "paw-agent-id marker should exist");

        let pc = fs::read_to_string(&post_commit).unwrap();
        assert!(pc.contains("$GIT_DIR/paw-agent-id"));
        assert!(pc.contains("agent.artifact"));

        let marker_body = fs::read_to_string(&marker).unwrap();
        assert!(marker_body.contains("PAW_AGENT_ID=feat-x"));
        assert!(marker_body.contains("PAW_BROKER_URL=http://127.0.0.1:9119"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&post_commit).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "post-commit must be executable");
        }
    }

    #[test]
    #[serial_test::serial]
    fn install_git_hooks_preserves_existing_dispatcher_body() {
        let tmp = tempfile::tempdir().unwrap();
        let worktree = tmp.path();
        init_git_repo(worktree);
        let hook_path = worktree.join(".git").join("hooks").join("post-commit");
        fs::write(&hook_path, "#!/bin/sh\necho user hook\n").unwrap();

        install_git_hooks(worktree, "http://127.0.0.1:9119", "feat-x").unwrap();

        let body = fs::read_to_string(&hook_path).unwrap();
        assert!(body.contains("echo user hook"));
        assert!(body.contains("agent.artifact"));
    }

    #[test]
    #[serial_test::serial]
    fn install_git_hooks_writes_linked_marker_for_linked_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let main_repo = tmp.path().join("main");
        fs::create_dir_all(&main_repo).unwrap();
        init_git_repo(&main_repo);

        // Create an empty commit so we can add a worktree.
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "root", "-q"])
            .current_dir(&main_repo)
            .output()
            .unwrap();

        // Add a linked worktree.
        let linked_path = tmp.path().join("linked");
        std::process::Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                "feat-x",
                linked_path.to_str().unwrap(),
            ])
            .current_dir(&main_repo)
            .output()
            .unwrap();

        install_git_hooks(&linked_path, "http://127.0.0.1:9119", "feat-x").unwrap();

        // Dispatcher lives in main .git/hooks/
        let post_commit = main_repo.join(".git").join("hooks").join("post-commit");
        assert!(
            post_commit.exists(),
            "dispatcher must land in main .git/hooks/"
        );
        // Per-worktree marker lives in main .git/worktrees/linked/
        let marker = main_repo
            .join(".git")
            .join("worktrees")
            .join("linked")
            .join("paw-agent-id");
        assert!(
            marker.exists(),
            "marker must land in linked worktree gitdir"
        );
        let body = fs::read_to_string(&marker).unwrap();
        assert!(body.contains("PAW_AGENT_ID=feat-x"));
    }

    // -----------------------------------------------------------------------
    // Enhanced Agent Marker Tests
    // -----------------------------------------------------------------------

    #[test]
    fn build_agent_marker_basic_format() {
        let marker = build_agent_marker("http://127.0.0.1:9119", "feat-test", None, None, None);

        assert!(marker.contains("PAW_AGENT_ID=feat-test"));
        assert!(marker.contains("PAW_BROKER_URL=http://127.0.0.1:9119"));
        assert!(marker.contains("PAW_TIMESTAMP="));
        // Should not contain optional fields
        assert!(!marker.contains("PAW_SUPERVISOR_PID"));
        assert!(!marker.contains("PAW_LAST_VERIFIED_COMMIT"));
        assert!(!marker.contains("PAW_SESSION_NAME"));
    }

    #[test]
    fn build_agent_marker_with_all_extended_fields() {
        let marker = build_agent_marker(
            "http://localhost:9119",
            "feat-errors",
            Some(12345),
            Some("abc123def456"),
            Some("paw-test-session"),
        );

        assert!(marker.contains("PAW_AGENT_ID=feat-errors"));
        assert!(marker.contains("PAW_BROKER_URL=http://localhost:9119"));
        assert!(marker.contains("PAW_SUPERVISOR_PID=12345"));
        assert!(marker.contains("PAW_LAST_VERIFIED_COMMIT=abc123def456"));
        assert!(marker.contains("PAW_SESSION_NAME=paw-test-session"));
        assert!(marker.contains("PAW_TIMESTAMP="));
    }

    #[test]
    fn build_agent_marker_partial_extended_fields() {
        let marker =
            build_agent_marker("http://localhost:9119", "fix-cycle", Some(999), None, None);

        assert!(marker.contains("PAW_SUPERVISOR_PID=999"));
        assert!(!marker.contains("PAW_LAST_VERIFIED_COMMIT"));
        assert!(!marker.contains("PAW_SESSION_NAME"));
    }

    #[test]
    fn update_agent_marker_adds_missing_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let marker_path = tmp.path().join("test-marker");

        // Create initial marker
        let initial = "PAW_AGENT_ID=test\nPAW_BROKER_URL=http://localhost:9119\nPAW_TIMESTAMP=2026-01-01T00:00:00Z\n";
        fs::write(&marker_path, initial).unwrap();

        // Update with supervisor PID
        update_agent_marker(&marker_path, Some(54321), None).unwrap();

        let updated = fs::read_to_string(&marker_path).unwrap();
        assert!(updated.contains("PAW_AGENT_ID=test"));
        assert!(updated.contains("PAW_SUPERVISOR_PID=54321"));
    }

    #[test]
    fn update_agent_marker_replaces_existing_fields() {
        let tmp = tempfile::tempdir().unwrap();
        let marker_path = tmp.path().join("test-marker");

        // Create initial marker with old commit
        let initial = "PAW_AGENT_ID=test\nPAW_BROKER_URL=http://localhost:9119\nPAW_LAST_VERIFIED_COMMIT=old123\nPAW_TIMESTAMP=2026-01-01T00:00:00Z\n";
        fs::write(&marker_path, initial).unwrap();

        // Update with new commit
        update_agent_marker(&marker_path, None, Some("new456")).unwrap();

        let updated = fs::read_to_string(&marker_path).unwrap();
        assert!(updated.contains("PAW_AGENT_ID=test"));
        assert!(updated.contains("PAW_LAST_VERIFIED_COMMIT=new456"));
        assert!(!updated.contains("PAW_LAST_VERIFIED_COMMIT=old123"));
    }

    #[test]
    fn get_agent_marker_path_returns_correct_path() {
        let tmp = tempfile::tempdir().unwrap();
        let worktree = tmp.path();
        init_git_repo(worktree);

        let marker_path = get_agent_marker_path(worktree).unwrap();
        assert!(marker_path.ends_with(".git/paw-agent-id"));
    }
}
