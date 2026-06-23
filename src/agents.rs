//! AGENTS.md generation and injection.
//!
//! Provides marker-based section injection into `AGENTS.md` files.
//! Core logic uses pure `&str → String` functions for testability,
//! with a thin I/O wrapper for file operations.

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::error::PawError;
use crate::git::{exclude_from_git, no_assume_unchanged};

/// Matches `PAW_SUPERVISOR_PID=<digits>` lines inside the agent marker file.
///
/// Compiled once on first use via `LazyLock`. The `expect` is allowed by the
/// project's panic-surface rules because the pattern is a static literal and
/// the failure is unreachable at runtime.
static SUPERVISOR_PID_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"PAW_SUPERVISOR_PID=\d+").expect("static regex compiles"));

/// Matches `PAW_LAST_VERIFIED_COMMIT=<value>` lines inside the agent marker file.
static LAST_VERIFIED_COMMIT_REGEX: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"PAW_LAST_VERIFIED_COMMIT=[^\n]+").expect("static regex compiles")
});

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

/// Worktree-relative path of the gitignored **sidecar** instruction file that
/// carries git-paw's managed block.
///
/// The combined view — the project's `AGENTS.md` content followed by the
/// per-worktree assignment section — is written here, never to the tracked
/// `AGENTS.md`. `.git-paw/` is already gitignored and used for session
/// learnings, logs, and helper scripts, so the ephemeral injection lives
/// alongside them and is never committed.
pub const SIDECAR_REL_PATH: &str = ".git-paw/AGENTS.local.md";

/// Returns `true` when the worktree-relative path `rel` names a file that
/// git-paw injects or manages, rather than user-authored work.
///
/// `git paw remove`'s uncommitted-work safety check uses this predicate to
/// filter git-paw's own bookkeeping out of the "dirty" set, so a
/// just-provisioned but otherwise-clean worktree is not refused because of the
/// injected sidecar (the v0.8.0 regression). A path is git-paw-managed when:
///
/// - it is the injected sidecar [`SIDECAR_REL_PATH`]
///   (`.git-paw/AGENTS.local.md`), which is always ephemeral git-paw state; or
/// - it is the tracked `AGENTS.md` whose ONLY uncommitted change is the
///   presence of git-paw's managed `<!-- git-paw:start -->` block — i.e. the
///   file is byte-identical to its `HEAD` revision once that block is removed.
///   This covers worktrees provisioned by an older git-paw that still injected
///   the block into the tracked file; such a block is git-paw injection, not
///   the user's work.
///
/// Any other path — including an `AGENTS.md` with a user edit *outside* the
/// managed block, an `AGENTS.md` that carries no managed block at all, or one
/// that is untracked at `HEAD` — is treated as user work and returns `false`,
/// so `remove` still refuses on it.
pub fn is_managed_path(worktree_root: &Path, rel: &str) -> bool {
    if rel == SIDECAR_REL_PATH {
        return true;
    }
    if rel != "AGENTS.md" {
        return false;
    }

    // AGENTS.md is git-paw-managed only when it still carries the managed block
    // AND is otherwise unmodified vs HEAD. Read the on-disk content; an
    // unreadable file is not something we can vouch for, so treat it as user
    // work.
    let Ok(on_disk) = fs::read_to_string(worktree_root.join("AGENTS.md")) else {
        return false;
    };
    if !has_git_paw_section(&on_disk) {
        return false;
    }
    let Some(head) = file_content_at_head(worktree_root, "AGENTS.md") else {
        return false;
    };
    // Strip the managed block and compare to HEAD (ignoring trailing
    // whitespace the injection's blank-line spacing introduces). If they match,
    // the only uncommitted change is git-paw's block.
    replace_git_paw_section(&on_disk, "").trim_end() == head.trim_end()
}

/// Reads the root repo's `AGENTS.md`, injects the worktree assignment section,
/// and writes the combined view to a gitignored **sidecar** instruction file
/// ([`SIDECAR_REL_PATH`]) in the worktree — never the tracked `AGENTS.md`.
///
/// The sidecar keeps the ephemeral per-session injection out of version
/// control entirely, so the tracked `AGENTS.md` stays a normal committable
/// file: a hand edit to it shows in `git status` and stages via `git add -A`.
/// This resolves the v0.7.0 footgun (finding F10), where a file-level
/// `git update-index --assume-unchanged AGENTS.md` bit silently hid *every*
/// edit to the file — including legitimate ones — and blocked agents from
/// committing real `AGENTS.md` changes.
///
/// The launched CLI is pointed at the sidecar's combined view via its boot
/// prompt (see `build_task_prompt`), since the supported CLIs only auto-load
/// the worktree-root `AGENTS.md`.
///
/// Two steps make the upgrade self-healing for worktrees set up by an older
/// git-paw version:
/// 1. Any stale `assume-unchanged` bit on the tracked `AGENTS.md` is cleared
///    (`git update-index --no-assume-unchanged AGENTS.md`) so the file becomes
///    committable again.
/// 2. The sidecar path is added to the worktree ignore set, so the injection
///    is never accidentally committed.
///
/// The sidecar exclude entry is registered **before** the sidecar file is
/// written. `info/exclude` is a git-level ignore list, so adding the path while
/// the file does not yet exist is valid and guarantees `git status` never
/// reports the sidecar — closing the write-then-exclude race (the v0.8.0
/// regression where `git paw remove` refused a just-started clean worktree
/// because the freshly written sidecar briefly showed as an untracked file).
/// This ordering is defense in depth alongside `remove`'s managed-path filter
/// ([`is_managed_path`]).
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

    // Resolve the sidecar path and create the `.git-paw/` parent directory if
    // it is absent (needed before either the exclude or the write).
    let sidecar = worktree_root.join(SIDECAR_REL_PATH);
    if let Some(parent) = sidecar.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PawError::AgentsMdError(format!("failed to create '{}': {e}", parent.display()))
        })?;
    }

    // Exclude the sidecar BEFORE writing it (race fix, defense in depth).
    // `info/exclude` is a git-level ignore list, so registering the path while
    // the file does not yet exist is valid and means `git status` never reports
    // the sidecar — not even in the window between the write and the exclude
    // that the earlier ordering left open (the v0.8.0 `git paw remove`
    // regression). `.git-paw/AGENTS.local.md` is gitignored at the repo level
    // too, but the explicit worktree-level entry is idempotent and pins the
    // guarantee even for repos whose `.gitignore` predates this file.
    exclude_from_git(worktree_root, SIDECAR_REL_PATH)?;

    // Write the combined view to the now-excluded sidecar, NOT the tracked
    // AGENTS.md.
    fs::write(&sidecar, &output).map_err(|e| {
        PawError::AgentsMdError(format!("failed to write '{}': {e}", sidecar.display()))
    })?;

    // Self-healing: clear any stale assume-unchanged bit a prior git-paw
    // version set on the tracked AGENTS.md so it is committable again. The
    // tracked AGENTS.md is otherwise left untouched — git-paw no longer
    // injects into it, hides it from `git status`, or excludes it.
    let _ = no_assume_unchanged(worktree_root, "AGENTS.md");

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
            updated = SUPERVISOR_PID_REGEX
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
            updated = LAST_VERIFIED_COMMIT_REGEX
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
         # Dispatcher: reads the per-worktree paw-agent-id marker and publishes\n\
         # agent.artifact to the git-paw broker. Resolve the gitdir via\n\
         # rev-parse with a GIT_DIR fallback (git does not always export it).\n\
         PAW_GD=\"${{GIT_DIR:-$(git rev-parse --git-dir 2>/dev/null)}}\"\n\
         if [ -n \"$PAW_GD\" ] && [ -f \"$PAW_GD/paw-agent-id\" ]; then\n\
             . \"$PAW_GD/paw-agent-id\"\n\
             FILES=$(git diff HEAD~1 --name-only 2>/dev/null | awk '{{printf \"%s\\\"%s\\\"\", (NR>1?\",\":\"\"), $0}}')\n\
             curl -s -X POST \"$PAW_BROKER_URL/publish\" \\\n\
                 -H 'Content-Type: application/json' \\\n\
                 -d \"{{\\\"type\\\":\\\"agent.artifact\\\",\\\"agent_id\\\":\\\"$PAW_AGENT_ID\\\",\\\"payload\\\":{{\\\"status\\\":\\\"committed\\\",\\\"exports\\\":[],\\\"modified_files\\\":[$FILES]}}}}\" \\\n\
                 >/dev/null 2>&1 || true\n\
             # Branch-mismatch detection (detection without enforcement — fires\n\
             # regardless of PAW_STRICT_BRANCH_GUARD; the pre-commit hook owns\n\
             # blocking). Publishes agent.feedback + an agent.learning record\n\
             # (category permission_pattern) identifying the contamination.\n\
             if [ -n \"$PAW_EXPECTED_BRANCH\" ]; then\n\
                 PAW_CUR=$(git symbolic-ref --short HEAD 2>/dev/null)\n\
                 if [ -n \"$PAW_CUR\" ] && [ \"$PAW_CUR\" != \"$PAW_EXPECTED_BRANCH\" ]; then\n\
                     PAW_SHA=$(git rev-parse HEAD 2>/dev/null)\n\
                     curl -s -X POST \"$PAW_BROKER_URL/publish\" \\\n\
                         -H 'Content-Type: application/json' \\\n\
                         -d \"{{\\\"type\\\":\\\"agent.feedback\\\",\\\"agent_id\\\":\\\"$PAW_AGENT_ID\\\",\\\"payload\\\":{{\\\"from\\\":\\\"branch-guard\\\",\\\"errors\\\":[\\\"commit $PAW_SHA advanced '$PAW_CUR' but this worktree is for '$PAW_EXPECTED_BRANCH'; cherry-pick onto '$PAW_EXPECTED_BRANCH' and reset '$PAW_CUR'\\\"]}}}}\" \\\n\
                         >/dev/null 2>&1 || true\n\
                     curl -s -X POST \"$PAW_BROKER_URL/publish\" \\\n\
                         -H 'Content-Type: application/json' \\\n\
                         -d \"{{\\\"type\\\":\\\"agent.learning\\\",\\\"agent_id\\\":\\\"$PAW_AGENT_ID\\\",\\\"payload\\\":{{\\\"category\\\":\\\"permission_pattern\\\",\\\"body\\\":\\\"cross-worktree contamination: commit $PAW_SHA landed on '$PAW_CUR' instead of expected '$PAW_EXPECTED_BRANCH'\\\"}}}}\" \\\n\
                         >/dev/null 2>&1 || true\n\
                 fi\n\
             fi\n\
         fi\n\
         {HOOK_END_MARKER}\n"
    )
}

fn build_pre_push_hook() -> String {
    // Only reject when the calling worktree is an agent worktree — i.e.
    // a `paw-agent-id` marker exists in this worktree's gitdir. The hook
    // installs into the common gitdir (shared with the main repo and all
    // linked worktrees), so without this gate the hook would also block
    // legitimate pushes from the main repo. Mirror the post-commit
    // dispatcher's gate at line 388 so behaviour is consistent.
    format!(
        "#!/bin/sh\n\
         {HOOK_START_MARKER}\n\
         if [ -n \"$GIT_DIR\" ] && [ -f \"$GIT_DIR/paw-agent-id\" ]; then\n\
         echo 'error: git-paw agents must not push. The supervisor handles merges.' >&2\n\
         exit 1\n\
         fi\n\
         {HOOK_END_MARKER}\n"
    )
}

/// Builds the `pre-commit` branch-guard hook.
///
/// Sources the per-worktree `$GIT_DIR/paw-agent-id` marker and, when
/// `PAW_STRICT_BRANCH_GUARD` is not `false`, refuses a commit whose
/// `git symbolic-ref --short HEAD` differs from `PAW_EXPECTED_BRANCH` — the
/// branch the worktree was created for. This blocks cross-worktree
/// contamination, where a commit advances the wrong branch because linked
/// worktrees share `.git/refs`. The opt-out (`strict_branch_guard = false`)
/// turns enforcement off while leaving the post-commit detection in place.
/// Gated on the marker's presence so non-agent checkouts (the main repo)
/// committing through the shared hooks dir are never blocked.
fn build_pre_commit_branch_guard_hook() -> String {
    format!(
        "#!/bin/sh\n\
         {HOOK_START_MARKER}\n\
         # Branch guard: refuse a commit that would advance a branch other than\n\
         # the one this worktree was created for (cross-worktree contamination).\n\
         # git does not reliably export GIT_DIR to pre-commit, so resolve the\n\
         # per-worktree gitdir via rev-parse with a GIT_DIR fallback.\n\
         PAW_GD=\"${{GIT_DIR:-$(git rev-parse --git-dir 2>/dev/null)}}\"\n\
         if [ -n \"$PAW_GD\" ] && [ -f \"$PAW_GD/paw-agent-id\" ]; then\n\
             . \"$PAW_GD/paw-agent-id\"\n\
             if [ -n \"$PAW_EXPECTED_BRANCH\" ] && [ \"$PAW_STRICT_BRANCH_GUARD\" != \"false\" ]; then\n\
                 PAW_CUR=$(git symbolic-ref --short HEAD 2>/dev/null)\n\
                 if [ -n \"$PAW_CUR\" ] && [ \"$PAW_CUR\" != \"$PAW_EXPECTED_BRANCH\" ]; then\n\
                     echo \"error: git-paw branch guard refused this commit\" >&2\n\
                     echo \"  HEAD is on '$PAW_CUR' but this worktree is for '$PAW_EXPECTED_BRANCH'.\" >&2\n\
                     echo \"  The commit would advance the wrong branch. Switch back to '$PAW_EXPECTED_BRANCH'\" >&2\n\
                     echo \"  (or set [supervisor] strict_branch_guard = false to override).\" >&2\n\
                     exit 1\n\
                 fi\n\
             fi\n\
         fi\n\
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

/// Returns the content of `rel` at the worktree's `HEAD` revision, or `None`
/// when the path is untracked at `HEAD`, `HEAD` is unresolvable, or git cannot
/// be run. Used by [`is_managed_path`] to compare a worktree's on-disk
/// `AGENTS.md` against its committed revision.
fn file_content_at_head(worktree_root: &Path, rel: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .current_dir(worktree_root)
        .args(["show", &format!("HEAD:{rel}")])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
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
    expected_branch: &str,
    strict_branch_guard: bool,
) -> Result<(), PawError> {
    let common_git_dir = git_rev_parse_path(worktree, "--git-common-dir")?;
    let linked_git_dir = git_rev_parse_path(worktree, "--git-dir")?;
    let hooks_dir = common_git_dir.join("hooks");

    write_hook_file(
        &hooks_dir.join("post-commit"),
        &build_post_commit_dispatcher_hook(),
    )?;
    write_hook_file(&hooks_dir.join("pre-push"), &build_pre_push_hook())?;
    write_hook_file(
        &hooks_dir.join("pre-commit"),
        &build_pre_commit_branch_guard_hook(),
    )?;

    let marker_path = linked_git_dir.join("paw-agent-id");
    if let Some(parent) = marker_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            PawError::AgentsMdError(format!("failed to create '{}': {e}", parent.display()))
        })?;
    }
    // The branch-guard fields are appended to the marker the hooks source.
    // `PAW_EXPECTED_BRANCH` is the branch this worktree was created for;
    // `PAW_STRICT_BRANCH_GUARD` controls whether the pre-commit hook *blocks*
    // (vs. detection-only via post-commit).
    let mut marker = build_agent_marker(broker_url, agent_id, None, None, None);
    let _ = writeln!(marker, "PAW_EXPECTED_BRANCH={expected_branch}");
    let _ = writeln!(
        marker,
        "PAW_STRICT_BRANCH_GUARD={}",
        if strict_branch_guard { "true" } else { "false" }
    );
    fs::write(&marker_path, marker).map_err(|e| {
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

/// Removes the git-paw managed block (start marker through end marker,
/// inclusive — plus any blank lines immediately adjacent) from `content`.
///
/// If `content` has no start marker, returns `content` unchanged. This makes
/// the helper safe to call unconditionally during teardown.
///
/// Adjacency rule: the helper consumes ONE leading blank line and ONE trailing
/// blank line that surround the block, restoring the file to its
/// pre-injection shape (`inject_into_content` inserts a leading blank line
/// when appending to a non-empty file).
pub fn remove_git_paw_section(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();

    let Some(start_idx) = lines
        .iter()
        .position(|l| l.starts_with(START_MARKER_PREFIX))
    else {
        // No marker — preserve content byte-for-byte.
        return content.to_string();
    };

    let end_idx = lines[start_idx..]
        .iter()
        .position(|l| l.contains(END_MARKER))
        .map(|rel| start_idx + rel);

    // Compute the range to delete: [delete_start, delete_end_exclusive).
    let delete_start = start_idx;
    let delete_end_exclusive = end_idx.map_or(lines.len(), |e| e + 1);

    // Consume at most ONE adjacent blank line to avoid collapsing the
    // surrounding paragraph spacing. Prefer the trailing blank because
    // `inject_into_content` inserts a leading blank when appending the
    // block, so the trailing blank is more likely to be vestigial.
    // If only a leading blank exists, fall back to that.
    let mut delete_end = delete_end_exclusive;
    let mut adjusted_start = delete_start;
    if delete_end < lines.len() && lines[delete_end].is_empty() {
        delete_end += 1;
    } else if adjusted_start > 0 && lines[adjusted_start - 1].is_empty() {
        adjusted_start -= 1;
    }
    let delete_start = adjusted_start;

    let mut result = String::new();
    for line in &lines[..delete_start] {
        result.push_str(line);
        result.push('\n');
    }
    for line in &lines[delete_end..] {
        result.push_str(line);
        result.push('\n');
    }

    // Preserve trailing-newline behaviour of the original file when the
    // result is not already terminated with one.
    if content.ends_with('\n') && !result.ends_with('\n') && !result.is_empty() {
        result.push('\n');
    }

    // If the original file lacked a trailing newline AND the result
    // gained one from our line-by-line reconstruction, trim it back.
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

/// Reads `path` (treating a missing file as empty), removes any
/// git-paw managed block, and writes the result back. Idempotent: a file
/// with no markers is a no-op and the original content is preserved
/// byte-for-byte.
///
/// v0-5-0-audit-cleanup Bug E — `cmd_stop` and `cmd_purge` invoke this
/// against the repo-root `AGENTS.md` after teardown so the supervisor-
/// pane boot block does not accumulate across sessions.
pub fn remove_session_boot_block(repo_root: &Path) -> Result<(), PawError> {
    let agents_md = repo_root.join("AGENTS.md");
    let content = match fs::read_to_string(&agents_md) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(PawError::AgentsMdError(format!(
                "failed to read '{}': {e}",
                agents_md.display()
            )));
        }
    };

    let new_content = remove_git_paw_section(&content);
    if new_content == content {
        // No marker block — nothing to write.
        return Ok(());
    }

    fs::write(&agents_md, &new_content).map_err(|e| {
        PawError::AgentsMdError(format!("failed to write '{}': {e}", agents_md.display()))
    })
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

    /// Absolute path of the gitignored sidecar inside a worktree.
    fn sidecar_path(wt: &Path) -> PathBuf {
        wt.join(SIDECAR_REL_PATH)
    }

    /// Reads the `git ls-files -v` flag character for `AGENTS.md`. A lowercase
    /// flag (e.g. `h`) means the assume-unchanged bit is set; uppercase `H`
    /// means a normal tracked file.
    fn agents_md_ls_files_flag(wt: &Path) -> char {
        let out = std::process::Command::new("git")
            .current_dir(wt)
            .args(["ls-files", "-v", "AGENTS.md"])
            .output()
            .expect("git ls-files -v");
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout
            .lines()
            .next()
            .and_then(|l| l.chars().next())
            .unwrap_or('?')
    }

    /// Tracks `AGENTS.md` in the worktree index with `body` and commits it, so
    /// assume-unchanged / commit semantics apply to a real tracked file.
    fn commit_tracked_agents_md(wt: &Path, body: &str) {
        fs::write(wt.join("AGENTS.md"), body).unwrap();
        std::process::Command::new("git")
            .current_dir(wt)
            .args(["add", "AGENTS.md"])
            .output()
            .expect("git add AGENTS.md");
        std::process::Command::new("git")
            .current_dir(wt)
            .args(["commit", "-m", "add agents"])
            .output()
            .expect("git commit");
    }

    #[test]
    fn setup_worktree_root_exists() {
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        fs::write(repo.path().join("AGENTS.md"), "# Project Rules\n").unwrap();
        commit_tracked_agents_md(wt.path(), "# placeholder\n");

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        // The combined view lands in the gitignored sidecar, not AGENTS.md.
        let sidecar = fs::read_to_string(sidecar_path(wt.path())).unwrap();
        assert!(sidecar.contains("# Project Rules"));
        assert!(sidecar.contains("`feat/foo`"));
        assert!(sidecar.contains(START_MARKER));

        // 5.1: the tracked AGENTS.md is NOT marked assume-unchanged.
        assert_eq!(
            agents_md_ls_files_flag(wt.path()),
            'H',
            "tracked AGENTS.md must not carry the assume-unchanged bit"
        );

        // 5.1: a hand edit to the tracked AGENTS.md appears in git status.
        fs::write(wt.path().join("AGENTS.md"), "# placeholder\n\nhand edit\n").unwrap();
        let status = std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["status", "--porcelain"])
            .output()
            .expect("git status");
        let status_output = String::from_utf8_lossy(&status.stdout);
        assert!(
            status_output.contains("AGENTS.md"),
            "a hand edit to AGENTS.md must appear in git status, got: {status_output}"
        );
    }

    #[test]
    fn setup_worktree_hand_edit_stages_and_commits() {
        // 5.2: a hand edit to the tracked AGENTS.md stages via `git add -A`
        // and commits — the v0.7.0 footgun (blocked commit) is gone.
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        fs::write(repo.path().join("AGENTS.md"), "# Project Rules\n").unwrap();
        commit_tracked_agents_md(wt.path(), "# Project Rules\n");

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        // Legitimate edit an agent might make (e.g. adding a dependency row).
        fs::write(
            wt.path().join("AGENTS.md"),
            "# Project Rules\n\n- approved dependency: rmcp\n",
        )
        .unwrap();

        std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["add", "-A"])
            .output()
            .expect("git add -A");
        let commit = std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["commit", "-m", "edit agents"])
            .output()
            .expect("git commit");
        assert!(commit.status.success(), "commit should succeed");

        // The committed tip contains the edit and the working tree is clean.
        let show = std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["show", "--stat", "HEAD"])
            .output()
            .expect("git show");
        assert!(String::from_utf8_lossy(&show.stdout).contains("AGENTS.md"));
        let status = std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["status", "--porcelain", "AGENTS.md"])
            .output()
            .expect("git status");
        assert!(
            String::from_utf8_lossy(&status.stdout).trim().is_empty(),
            "AGENTS.md should be clean after committing the edit"
        );
    }

    #[test]
    fn setup_worktree_managed_block_in_sidecar_combined_view() {
        // 5.3: the managed `<!-- git-paw:start -->` block is present in the
        // sidecar and the sidecar is the combined view (root + block).
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        fs::write(repo.path().join("AGENTS.md"), "# Project Rules\n").unwrap();

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        let sidecar = fs::read_to_string(sidecar_path(wt.path())).unwrap();
        assert!(
            sidecar.contains(START_MARKER),
            "sidecar must carry the block"
        );
        // Combined = root content first, then the managed block.
        let root_idx = sidecar
            .find("# Project Rules")
            .expect("root content present");
        let block_idx = sidecar.find(START_MARKER).expect("block present");
        assert!(
            root_idx < block_idx,
            "root content must precede the managed block in the combined view"
        );
    }

    #[test]
    fn setup_worktree_tracked_agents_md_untouched_and_not_excluded() {
        // 5.4: git-paw writes no block into the tracked AGENTS.md, and does
        // not add AGENTS.md to the worktree's `.git/info/exclude`.
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        fs::write(repo.path().join("AGENTS.md"), "# Project Rules\n").unwrap();
        commit_tracked_agents_md(wt.path(), "# Project Rules\n");

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        let tracked = fs::read_to_string(wt.path().join("AGENTS.md")).unwrap();
        assert!(
            !tracked.contains(START_MARKER_PREFIX),
            "git-paw must not write its managed block into the tracked AGENTS.md"
        );

        let exclude = fs::read_to_string(wt.path().join(".git/info/exclude")).unwrap_or_default();
        assert!(
            !exclude.lines().any(|l| l.trim() == "AGENTS.md"),
            "AGENTS.md must NOT be added to .git/info/exclude, got: {exclude}"
        );
    }

    #[test]
    fn setup_worktree_sidecar_in_ignore_set() {
        // 5.5: the sidecar path IS in the worktree ignore set.
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        let exclude = fs::read_to_string(wt.path().join(".git/info/exclude")).unwrap();
        assert!(
            exclude.lines().any(|l| l.trim() == SIDECAR_REL_PATH),
            "sidecar path must be in the worktree ignore set, got: {exclude}"
        );
    }

    #[test]
    fn setup_worktree_sidecar_not_reported_by_status() {
        // 4.5 / Scenario "Sidecar is excluded the moment it is written": after
        // setup completes, `git status --porcelain` in the worktree does NOT
        // report the injected sidecar (it was excluded before being written).
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        fs::write(repo.path().join("AGENTS.md"), "# Project Rules\n").unwrap();

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        // Sanity: the sidecar file really was written to disk.
        assert!(
            sidecar_path(wt.path()).exists(),
            "setup must write the sidecar to disk"
        );

        let status = std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["status", "--porcelain"])
            .output()
            .expect("git status");
        let out = String::from_utf8_lossy(&status.stdout);
        assert!(
            !out.contains(SIDECAR_REL_PATH),
            "sidecar must not appear in `git status` after setup; got: {out}"
        );
    }

    // -----------------------------------------------------------------------
    // is_managed_path
    // -----------------------------------------------------------------------

    #[test]
    fn is_managed_path_classifies_sidecar_managed_and_user_files_unmanaged() {
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        // Commit a clean AGENTS.md at HEAD so the managed-block comparison has a
        // baseline to diff against.
        commit_tracked_agents_md(wt.path(), "# Project Rules\n");

        // The injected sidecar is always git-paw-managed.
        assert!(
            is_managed_path(wt.path(), SIDECAR_REL_PATH),
            "sidecar path must be classified managed"
        );

        // A genuine source file is user work.
        assert!(
            !is_managed_path(wt.path(), "src/foo.rs"),
            "an ordinary source file must NOT be classified managed"
        );

        // An AGENTS.md carrying ONLY the managed block (otherwise identical to
        // HEAD) is git-paw injection, not user work.
        let block = generate_worktree_section(&make_assignment(None, None));
        let managed_only = inject_into_content("# Project Rules\n", &block);
        fs::write(wt.path().join("AGENTS.md"), &managed_only).unwrap();
        assert!(
            is_managed_path(wt.path(), "AGENTS.md"),
            "a managed-block-only AGENTS.md must be classified managed"
        );

        // An AGENTS.md with a user edit OUTSIDE the managed block must NOT be
        // classified managed — that hunk is real user work.
        let user_edited = inject_into_content("# Project Rules\n\nuser added a line\n", &block);
        fs::write(wt.path().join("AGENTS.md"), &user_edited).unwrap();
        assert!(
            !is_managed_path(wt.path(), "AGENTS.md"),
            "an AGENTS.md edited outside the managed block must NOT be managed"
        );
    }

    #[test]
    fn setup_worktree_clears_stale_assume_unchanged() {
        // 5.6: a stale assume-unchanged bit set before setup is cleared.
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        fs::write(repo.path().join("AGENTS.md"), "# Project Rules\n").unwrap();
        commit_tracked_agents_md(wt.path(), "# placeholder\n");

        // Simulate an older git-paw version having hidden the file.
        std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["update-index", "--assume-unchanged", "AGENTS.md"])
            .output()
            .expect("git update-index --assume-unchanged");
        assert_eq!(
            agents_md_ls_files_flag(wt.path()),
            'h',
            "precondition: the stale assume-unchanged bit is set"
        );

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        assert_eq!(
            agents_md_ls_files_flag(wt.path()),
            'H',
            "setup must clear the stale assume-unchanged bit"
        );
        // And a hand edit now surfaces in git status.
        fs::write(wt.path().join("AGENTS.md"), "# placeholder\n\nedited\n").unwrap();
        let status = std::process::Command::new("git")
            .current_dir(wt.path())
            .args(["status", "--porcelain"])
            .output()
            .expect("git status");
        assert!(
            String::from_utf8_lossy(&status.stdout).contains("AGENTS.md"),
            "after clearing the bit, a hand edit must appear in git status"
        );
    }

    #[test]
    fn setup_worktree_root_missing() {
        // 5.7: read the sidecar, not the worktree AGENTS.md.
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        let sidecar = fs::read_to_string(sidecar_path(wt.path())).unwrap();
        assert!(!sidecar.contains("# Project Rules"));
        assert!(sidecar.contains("`feat/foo`"));
    }

    #[test]
    fn setup_worktree_replaces_root_section() {
        // 5.7: read the sidecar, not the worktree AGENTS.md.
        let repo = tempfile::tempdir().unwrap();
        let wt = tempfile::tempdir().unwrap();
        init_git_repo(wt.path());
        let root_content =
            format!("# Rules\n\n{START_MARKER}\nold root section\n{END_MARKER}\n\n## Footer\n");
        fs::write(repo.path().join("AGENTS.md"), &root_content).unwrap();

        let assignment = make_assignment(None, None);
        setup_worktree_agents_md(repo.path(), wt.path(), &assignment).unwrap();

        let sidecar = fs::read_to_string(sidecar_path(wt.path())).unwrap();
        assert!(sidecar.contains("# Rules"));
        assert!(sidecar.contains("## Footer"));
        assert!(!sidecar.contains("old root section"));
        assert!(sidecar.contains("`feat/foo`"));
        assert_eq!(
            sidecar.matches(START_MARKER_PREFIX).count(),
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
    fn pre_commit_guard_hook_blocks_on_branch_mismatch() {
        let script = build_pre_commit_branch_guard_hook();
        // Resolves the gitdir robustly (git does not export GIT_DIR to
        // pre-commit), gates on the marker, compares HEAD vs expected branch,
        // honours the strict opt-out, and exits non-zero on mismatch.
        assert!(script.contains("git rev-parse --git-dir"));
        assert!(script.contains("PAW_EXPECTED_BRANCH"));
        assert!(script.contains("PAW_STRICT_BRANCH_GUARD"));
        assert!(script.contains("git symbolic-ref --short HEAD"));
        assert!(script.contains("exit 1"));
    }

    #[test]
    fn post_commit_dispatcher_detects_branch_mismatch() {
        let script = build_post_commit_dispatcher_hook();
        // Detection (without enforcement) publishes both feedback and a
        // permission_pattern learning when HEAD differs from the expected branch.
        assert!(script.contains("agent.feedback"));
        assert!(script.contains("agent.learning"));
        assert!(script.contains("permission_pattern"));
        assert!(script.contains("PAW_EXPECTED_BRANCH"));
    }

    #[test]
    fn post_commit_dispatcher_hook_reads_marker_and_publishes() {
        let script = build_post_commit_dispatcher_hook();
        assert!(script.contains("$PAW_GD/paw-agent-id"));
        assert!(script.contains(". \"$PAW_GD/paw-agent-id\""));
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
    fn pre_push_hook_only_rejects_agent_worktrees() {
        let script = build_pre_push_hook();
        // The reject path must still be there so agent worktrees can't push.
        assert!(script.contains("exit 1"));
        assert!(script.contains("must not push"));
        // But it MUST be gated on the agent marker so the main repo and
        // non-agent worktrees can still push freely.
        assert!(
            script.contains("paw-agent-id"),
            "pre-push hook must gate the reject on $GIT_DIR/paw-agent-id; \
             without the gate, every push from this gitdir is blocked, \
             including legitimate pushes from the main repo"
        );
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

        install_git_hooks(worktree, "http://127.0.0.1:9119", "feat-x", "feat/x", true).unwrap();

        let post_commit = worktree.join(".git").join("hooks").join("post-commit");
        let pre_push = worktree.join(".git").join("hooks").join("pre-push");
        let marker = worktree.join(".git").join("paw-agent-id");

        assert!(post_commit.exists(), "post-commit should exist");
        assert!(pre_push.exists(), "pre-push should exist");
        assert!(marker.exists(), "paw-agent-id marker should exist");

        let pc = fs::read_to_string(&post_commit).unwrap();
        assert!(pc.contains("$PAW_GD/paw-agent-id"));
        assert!(pc.contains("agent.artifact"));

        // pre-commit branch guard installed alongside the dispatcher.
        let pre_commit = worktree.join(".git").join("hooks").join("pre-commit");
        assert!(pre_commit.exists(), "pre-commit guard should exist");
        let prc = fs::read_to_string(&pre_commit).unwrap();
        assert!(prc.contains("branch guard"));
        assert!(prc.contains("PAW_EXPECTED_BRANCH"));

        let marker_body = fs::read_to_string(&marker).unwrap();
        assert!(marker_body.contains("PAW_AGENT_ID=feat-x"));
        assert!(marker_body.contains("PAW_BROKER_URL=http://127.0.0.1:9119"));
        assert!(marker_body.contains("PAW_EXPECTED_BRANCH=feat/x"));
        assert!(marker_body.contains("PAW_STRICT_BRANCH_GUARD=true"));

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

        install_git_hooks(worktree, "http://127.0.0.1:9119", "feat-x", "feat/x", true).unwrap();

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

        install_git_hooks(
            &linked_path,
            "http://127.0.0.1:9119",
            "feat-x",
            "feat/x",
            true,
        )
        .unwrap();

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
    fn update_agent_marker_reuses_lazy_regex_across_calls() {
        // Smoke test for the LazyLock<Regex> hoisting: invoking
        // `update_agent_marker` twice in succession must not panic and the
        // second call must replace the first call's substituted value.
        // Reaching this point at all proves both regexes initialised cleanly.
        let tmp = tempfile::tempdir().unwrap();
        let marker_path = tmp.path().join("test-marker");

        let initial = "PAW_AGENT_ID=test\nPAW_BROKER_URL=http://localhost:9119\nPAW_SUPERVISOR_PID=111\nPAW_LAST_VERIFIED_COMMIT=abc\n";
        fs::write(&marker_path, initial).unwrap();

        update_agent_marker(&marker_path, Some(222), Some("def")).unwrap();
        update_agent_marker(&marker_path, Some(333), Some("ghi")).unwrap();

        let updated = fs::read_to_string(&marker_path).unwrap();
        assert!(updated.contains("PAW_SUPERVISOR_PID=333"));
        assert!(updated.contains("PAW_LAST_VERIFIED_COMMIT=ghi"));
        assert!(!updated.contains("PAW_SUPERVISOR_PID=111"));
        assert!(!updated.contains("PAW_SUPERVISOR_PID=222"));
        assert!(!updated.contains("PAW_LAST_VERIFIED_COMMIT=abc"));
        assert!(!updated.contains("PAW_LAST_VERIFIED_COMMIT=def"));
    }

    #[test]
    fn get_agent_marker_path_returns_correct_path() {
        let tmp = tempfile::tempdir().unwrap();
        let worktree = tmp.path();
        init_git_repo(worktree);

        let marker_path = get_agent_marker_path(worktree).unwrap();
        assert!(marker_path.ends_with(".git/paw-agent-id"));
    }

    // v0-5-0-audit-cleanup §9c (Bug E) — remove_session_boot_block must
    // strip a marker-delimited block from AGENTS.md byte-for-byte and
    // remain a no-op for files without markers.

    #[test]
    fn remove_session_boot_block_strips_marked_block() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        let agents_md = repo_root.join("AGENTS.md");

        let header = "# Project AGENTS";
        let footer = "## Footer\n";
        let original = format!(
            "{header}\n\n<!-- git-paw:start — managed by git-paw, do not edit manually -->\n## boot block\nsome content\n<!-- git-paw:end -->\n\n{footer}"
        );
        fs::write(&agents_md, &original).unwrap();

        remove_session_boot_block(repo_root).unwrap();

        let after = fs::read_to_string(&agents_md).unwrap();
        let expected = format!("{header}\n\n{footer}");
        assert_eq!(
            after, expected,
            "after removal the file must match HEADER + blank + FOOTER byte-for-byte; got:\n{after:?}",
        );
        assert!(
            !after.contains("git-paw:start"),
            "no git-paw:start marker may remain after removal",
        );
    }

    #[test]
    fn remove_session_boot_block_no_marker_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        let agents_md = repo_root.join("AGENTS.md");

        let original = "# Project AGENTS\n\nNo boot block here.\n";
        fs::write(&agents_md, original).unwrap();

        remove_session_boot_block(repo_root).unwrap();

        let after = fs::read_to_string(&agents_md).unwrap();
        assert_eq!(
            after, original,
            "files without a boot-block marker must be preserved byte-for-byte",
        );
    }

    #[test]
    fn remove_session_boot_block_missing_agents_md_is_noop() {
        // The helper SHALL be idempotent — calling it against a repo
        // root that has no AGENTS.md at all is not an error.
        let tmp = tempfile::tempdir().unwrap();
        remove_session_boot_block(tmp.path()).unwrap();
        assert!(
            !tmp.path().join("AGENTS.md").exists(),
            "remove_session_boot_block must not create AGENTS.md when none exists",
        );
    }

    #[test]
    fn remove_session_boot_block_preserves_no_trailing_newline() {
        // If the original file lacks a trailing newline, the helper
        // must preserve that shape.
        let tmp = tempfile::tempdir().unwrap();
        let repo_root = tmp.path();
        let agents_md = repo_root.join("AGENTS.md");

        let original = "# Header no newline";
        fs::write(&agents_md, original).unwrap();

        remove_session_boot_block(repo_root).unwrap();

        let after = fs::read_to_string(&agents_md).unwrap();
        assert_eq!(
            after, original,
            "file without trailing newline must be preserved exactly"
        );
    }
}
