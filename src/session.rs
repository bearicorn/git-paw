//! Session state persistence.
//!
//! Saves and loads session data to disk for recovery after crashes, reboots,
//! or `stop`. One session per repository, stored as JSON under the XDG data
//! directory (`~/.local/share/git-paw/sessions/`).

use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::PawError;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Status of a persisted session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// Tmux session is (believed to be) running.
    Active,
    /// Soft-stopped: tmux session and CLI panes remain running, but the
    /// user's client is detached and the broker has been shut down. A
    /// subsequent `git paw start` re-attaches and restarts the broker
    /// without respawning any CLI processes.
    Paused,
    /// Tmux session has been stopped or crashed; state is recoverable.
    Stopped,
}

/// Pane-layout shape used when the session was created. Drives recovery so
/// `recover_session` rebuilds with the same layout it was launched with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionMode {
    /// Bare-start mode: dashboard at pane 0 (when broker enabled), agents at
    /// pane 1+. Same as v0.4.
    #[default]
    Bare,
    /// Supervisor-as-pane mode (v0.5.0+): supervisor at pane 0, dashboard at
    /// pane 1, agents at pane 2+.
    Supervisor,
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Stopped => write!(f, "stopped"),
        }
    }
}

/// The status git-paw displays for a session, derived from the persisted
/// receipt status combined with a live tmux probe (design D4 of
/// `session-bugfixes`).
///
/// Distinct from [`SessionStatus`] (the on-disk receipt value): a receipt
/// that claims `Active` but whose tmux session has vanished surfaces as
/// [`DisplayStatus::Stale`] rather than silently downgrading to `Stopped`,
/// so the user can tell a clean stop apart from a crashed / carried-over
/// session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayStatus {
    /// Receipt says active and the tmux session is alive.
    Active,
    /// Receipt says paused and the tmux session is alive.
    Paused,
    /// Receipt says stopped, or a paused session whose tmux server died.
    Stopped,
    /// Receipt claims active but the tmux session is gone (crash or
    /// release-boundary carry-over).
    Stale,
}

impl DisplayStatus {
    /// Resolves the display status from the persisted receipt status and a
    /// tmux-liveness probe.
    ///
    /// `🔴 Stale` surfaces only for `Active` receipts whose probe returns
    /// [`crate::tmux::SessionLiveness::Stale`] — the tmux session is absent. An
    /// `Indeterminate` probe (the tmux binary is missing) never yields `Stale`:
    /// it preserves the pre-existing "tmux not alive" display by downgrading
    /// active/paused to `Stopped`. [`crate::tmux::SessionLiveness::AgentsExited`]
    /// never yields `Stale` either — the tmux session is still running, so such
    /// a session displays as `Stopped` for every receipt status.
    #[must_use]
    #[expect(
        clippy::match_same_arms,
        reason = "the AgentsExited mapping is spelled out, not left to the wildcard, so the \
                  stopped-not-stale decision is visible at the match and pinned by its own test"
    )]
    pub fn from_receipt(status: &SessionStatus, liveness: crate::tmux::SessionLiveness) -> Self {
        use crate::tmux::SessionLiveness as L;
        match (status, liveness) {
            (SessionStatus::Active, L::Alive) => Self::Active,
            (SessionStatus::Active, L::Stale) => Self::Stale,
            (SessionStatus::Paused, L::Alive) => Self::Paused,
            // The tmux session exists but every agent CLI has exited: stopped,
            // never stale.
            (_, L::AgentsExited) => Self::Stopped,
            // Stopped receipt (any probe), paused-with-dead-tmux, and every
            // Indeterminate case fall through to the unchanged Stopped display.
            _ => Self::Stopped,
        }
    }

    /// Returns the coloured status icon for terminal display.
    #[must_use]
    pub fn icon(self) -> &'static str {
        match self {
            Self::Active => "\u{1f7e2}",  // 🟢
            Self::Paused => "\u{1f535}",  // 🔵
            Self::Stopped => "\u{1f7e1}", // 🟡
            Self::Stale => "\u{1f534}",   // 🔴
        }
    }

    /// Returns the lowercase status string used in JSON output and logs.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Stopped => "stopped",
            Self::Stale => "stale",
        }
    }
}

impl fmt::Display for DisplayStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A worktree entry within a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeEntry {
    /// The branch checked out in this worktree.
    pub branch: String,
    /// Absolute path to the worktree directory.
    pub worktree_path: PathBuf,
    /// The AI CLI assigned to this worktree.
    pub cli: String,
    /// Whether git-paw created this branch (vs. it already existing).
    /// When `true`, `purge` will delete the branch after removing the worktree.
    #[serde(default)]
    pub branch_created: bool,

    /// The branch this worktree's branch was created from, when git-paw
    /// created it.
    ///
    /// `git worktree add -b <branch> <path>` takes no start point, so a branch
    /// git-paw creates starts at the **main checkout's `HEAD`** — which is the
    /// default branch only when the operator launched the session from it.
    /// Launched from an integration branch, every created branch inherits that
    /// branch's commits, so comparing it against the default branch reports the
    /// whole integration backlog as "unmerged" and the teardown keeps a branch
    /// that has in fact already landed where it belongs. Recording the start
    /// point lets teardown ask the question that matters: is this branch
    /// contained in the default branch *or* in the branch it came from.
    ///
    /// `None` when git-paw did not create the branch, when the start point
    /// could not be read (a detached `HEAD`), and on every session file written
    /// before this field existed. Omitted from JSON when `None`, so a session
    /// that predates it round-trips byte-identically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,

    /// Boot+task prompt awaiting submission.
    ///
    /// Set only when an agent is attached via `git paw add` to a *paused*
    /// session (design D4): the pane is created and the boot block injected,
    /// but the prompt is held unsubmitted so the new agent stays paused with
    /// the rest of the session. `git paw start` (restart-from-pause) submits
    /// any entry carrying a pending prompt and clears the field. `None` for
    /// every start-time agent and for adds to an active session (submitted
    /// immediately). Omitted from JSON when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_boot_prompt: Option<String>,

    /// Runtime port slot assigned to this worktree, when `[worktree.ports]` is
    /// configured.
    ///
    /// The slot indexes the worktree's port block
    /// (`base + slot * stride`; see [`crate::worktree_provision`]). It is
    /// persisted rather than derived from the entry's position because a
    /// positional index shifts when a *middle* worktree is removed, which would
    /// hand a newly added worktree a live worktree's ports. Removing the entry
    /// releases the slot for the next allocation.
    ///
    /// `None` when port allocation is not configured, and on every state file
    /// written before this field existed. Omitted from JSON when `None`, so a
    /// session created without port allocation serializes byte-identically to
    /// prior versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_slot: Option<u16>,

    /// The worker CLI's assigned session/conversation identifier, when the
    /// CLI resolved a `session_id_args` template at launch
    /// (`worker-session-id-capture`).
    ///
    /// `None` when the CLI has no assignment template (unchanged launch),
    /// and on every session file written before this field existed. Omitted
    /// from JSON when `None`, so an entry without an id serializes
    /// byte-identically to before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_session_id: Option<String>,

    /// Id of the [`SpecEntry`](crate::specs::SpecEntry) this agent was
    /// launched from, when the launch was spec-driven.
    ///
    /// Every launch path that creates an agent from a spec — the spec-driven
    /// launch with or without supervisor mode, and `git paw add --from-spec` —
    /// records that spec's id here. It is the launch-time fact consumers need
    /// long after the spec itself may have been archived or renamed: the
    /// learnings aggregator seeds its agent → spec mapping from this field to
    /// classify forward conflicts as intra-spec or cross-spec.
    ///
    /// `None` for a launch without a spec (bare `git paw start`,
    /// `git paw add <branch>`), and on every session file written before this
    /// field existed. Omitted from JSON when `None`, so an entry without a
    /// spec serializes byte-identically to before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_id: Option<String>,
}

/// Persisted session state for a git-paw session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct Session {
    /// Tmux session name (also used as the filename stem).
    pub session_name: String,
    /// Absolute path to the repository root.
    pub repo_path: PathBuf,
    /// Human-readable project name (derived from the repo directory name).
    pub project_name: String,
    /// ISO 8601 timestamp of session creation (UTC).
    #[serde(
        serialize_with = "serialize_system_time",
        deserialize_with = "deserialize_system_time"
    )]
    pub created_at: SystemTime,
    /// Current session status.
    pub status: SessionStatus,
    /// Worktrees managed by this session.
    pub worktrees: Vec<WorktreeEntry>,

    /// Broker port (when broker is enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broker_port: Option<u16>,

    /// Broker bind address (when broker is enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broker_bind: Option<String>,

    /// Path to the broker log file (when broker is enabled).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broker_log_path: Option<PathBuf>,

    /// Pane-layout shape this session was launched with. Missing on sessions
    /// saved by v0.4 binaries, in which case [`SessionMode::Bare`] is assumed
    /// for backwards compatibility.
    #[serde(default)]
    pub mode: SessionMode,

    /// Pane index of the dashboard pane (when broker is enabled). Used by the
    /// restart-from-pause flow to recreate the dashboard pane in its original
    /// position. `None` on v0.4-saved sessions; consumers SHALL default to `0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard_pane: Option<u32>,
}

impl Session {
    /// Returns the effective status by combining the on-disk status with a
    /// tmux liveness check.
    ///
    /// `Active` or `Paused` downgrade to `Stopped` when tmux is not alive
    /// (a paused session whose tmux server died has no live CLI panes to
    /// resume into). `Stopped` is unchanged regardless of tmux liveness.
    pub fn effective_status(&self, is_tmux_alive: impl Fn(&str) -> bool) -> SessionStatus {
        match self.status {
            SessionStatus::Active | SessionStatus::Paused if !is_tmux_alive(&self.session_name) => {
                SessionStatus::Stopped
            }
            _ => self.status.clone(),
        }
    }

    /// Returns the session's creation timestamp formatted as an ISO 8601 UTC
    /// string, or `None` if the timestamp is before the Unix epoch. Used by
    /// the stale-receipt invalidation notice (design D5).
    #[must_use]
    pub fn created_at_iso8601(&self) -> Option<String> {
        format_iso8601(self.created_at).ok()
    }

    /// Index of the first coding-agent pane in this session's tmux window.
    ///
    /// The answer comes from this session's
    /// [`SessionLayout`](crate::surface::SessionLayout), which owns every
    /// reserved-index fact: supervisor mode reserves pane 0 (supervisor) and
    /// pane 1 (dashboard) so agents start at pane 2, while bare mode places
    /// the dashboard at pane 0 when the broker is enabled (agents at pane 1)
    /// or has no dashboard pane at all (agents at pane 0). Used by the GP-03c
    /// agent-pane liveness probe ([`crate::tmux::session_liveness_for`]) to
    /// know which panes are coding-agent panes rather than
    /// supervisor/dashboard panes.
    #[must_use]
    pub fn agent_pane_offset(&self) -> usize {
        crate::surface::SessionLayout::of(self).first_agent_index()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Atomically writes a session to disk.
///
/// Serializes the session to JSON, writes to a temporary file in the same
/// directory, then renames to the final path. This prevents corruption if
/// the process is killed mid-write.
pub fn save_session(session: &Session) -> Result<(), PawError> {
    save_session_in(session, &sessions_dir()?)
}

/// Finds the session associated with a given repository path.
///
/// Scans all `.json` files in the sessions directory and returns the first
/// session whose `repo_path` matches the given path EXACTLY (modulo
/// canonicalisation).
///
/// The exactness is a safety boundary, not an implementation detail. Every
/// mutating command (`stop`, `purge`, `add`, `remove`, `pause`, `start`'s
/// recover/resume dispatch) resolves its repository with
/// [`crate::git::validate_repo`], which inside an agent worktree returns that
/// WORKTREE's root. If this lookup resolved a worktree to its owning
/// repository, those commands would silently act on the parent session from
/// inside an agent's worktree — `purge --force` deleting sibling worktrees and
/// branches, `stop` killing the whole session. Read-only consumers that
/// legitimately want the owning repository's session use
/// [`find_session_for_repo_or_owner`] instead.
pub fn find_session_for_repo(repo_path: &Path) -> Result<Option<Session>, PawError> {
    find_session_for_repo_in(repo_path, &sessions_dir()?)
}

/// Finds the session for `repo_path`, falling back to the session of the
/// repository that OWNS `repo_path` when there is no exact match.
///
/// This is the READ-ONLY lookup for a process that legitimately runs inside an
/// agent worktree and wants the session it belongs to — specifically
/// `git paw mcp`, which an agent's CLI spawns with the agent's worktree as its
/// working directory and no `--repo`. A linked worktree's root never equals
/// the repository root, so the exact lookup alone leaves such a server with no
/// broker URL and nowhere to publish.
///
/// It SHALL NOT be used by a command that mutates session state: see
/// [`find_session_for_repo`] for why.
pub fn find_session_for_repo_or_owner(repo_path: &Path) -> Result<Option<Session>, PawError> {
    find_session_for_repo_or_owner_in(repo_path, &sessions_dir()?)
}

/// Deletes a session file by name.
///
/// Returns `Ok(())` even if the file does not exist (idempotent).
pub fn delete_session(session_name: &str) -> Result<(), PawError> {
    delete_session_in(session_name, &sessions_dir()?)
}

// ---------------------------------------------------------------------------
// Directory-parameterized implementations (public for integration tests)
// ---------------------------------------------------------------------------

/// Atomically writes a session to the given directory.
pub fn save_session_in(session: &Session, dir: &Path) -> Result<(), PawError> {
    fs::create_dir_all(dir)
        .map_err(|e| PawError::SessionError(format!("failed to create sessions dir: {e}")))?;

    let json = serde_json::to_string_pretty(session)
        .map_err(|e| PawError::SessionError(format!("failed to serialize session: {e}")))?;

    let final_path = dir.join(format!("{}.json", session.session_name));
    let tmp_path = dir.join(format!("{}.tmp", session.session_name));

    fs::write(&tmp_path, json.as_bytes())
        .map_err(|e| PawError::SessionError(format!("failed to write temp file: {e}")))?;

    fs::rename(&tmp_path, &final_path)
        .map_err(|e| PawError::SessionError(format!("failed to rename temp file: {e}")))?;

    Ok(())
}

/// Loads a session by name from the given directory.
pub fn load_session_from(session_name: &str, dir: &Path) -> Result<Option<Session>, PawError> {
    let path = dir.join(format!("{session_name}.json"));

    let contents = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(PawError::SessionError(format!(
                "failed to read session file: {e}"
            )));
        }
    };

    let session: Session = serde_json::from_str(&contents)
        .map_err(|e| PawError::SessionError(format!("failed to parse session file: {e}")))?;

    Ok(Some(session))
}

/// Finds the session whose `repo_path` is EXACTLY `repo_path`, scanning the
/// given directory.
///
/// See [`find_session_for_repo`] for why this match must stay exact.
pub fn find_session_for_repo_in(repo_path: &Path, dir: &Path) -> Result<Option<Session>, PawError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(PawError::SessionError(format!(
                "failed to read sessions dir: {e}"
            )));
        }
    };

    // Match on the canonical path so a symlinked or non-normalized repo path
    // (e.g. `/var/...` resolving to `/private/var/...`, or a `..` segment) still
    // resolves to its session. Falls back to the raw path when canonicalization
    // fails (e.g. the repo directory no longer exists).
    let want_canonical = canonical_or_raw(repo_path);

    for entry in entries {
        let entry =
            entry.map_err(|e| PawError::SessionError(format!("failed to read dir entry: {e}")))?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let contents = fs::read_to_string(&path).map_err(|e| {
            PawError::SessionError(format!("failed to read {}: {e}", path.display()))
        })?;

        let session: Session = match serde_json::from_str(&contents) {
            Ok(s) => s,
            Err(_) => continue, // skip malformed files
        };

        if canonical_or_raw(&session.repo_path) == want_canonical {
            return Ok(Some(session));
        }
    }

    Ok(None)
}

/// [`find_session_for_repo_or_owner`] scanning the given directory.
///
/// Tries the exact match FIRST and only resolves the owning repository on a
/// miss, so the common case costs no subprocess at all — the drive loop's
/// session-instance guard calls this on every check.
pub fn find_session_for_repo_or_owner_in(
    repo_path: &Path,
    dir: &Path,
) -> Result<Option<Session>, PawError> {
    if let Some(session) = find_session_for_repo_in(repo_path, dir)? {
        return Ok(Some(session));
    }
    let Some(owner) = owning_repo_root(repo_path) else {
        return Ok(None);
    };
    if owner == canonical_or_raw(repo_path) {
        // `repo_path` IS the owning repository — the exact pass already
        // covered it; a second identical scan would find nothing new.
        return Ok(None);
    }
    find_session_for_repo_in(&owner, dir)
}

/// Canonicalises a path for repo-identity comparison, falling back to the raw
/// path when canonicalization fails (e.g. the directory no longer exists).
fn canonical_or_raw(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Resolves `path` to the root of the repository that owns it.
///
/// `git rev-parse --git-common-dir` names the COMMON git directory — the main
/// repository's `.git`, shared by every linked worktree — so its parent is the
/// owning repository's root whether `path` is the main checkout or sits inside
/// one of its worktrees. The result is canonicalised for comparison with a
/// session's recorded `repo_path`.
///
/// Returns `None` whenever the answer would not be a plain checkout's root:
/// `path` is not inside a git repository, git is unavailable, or the common
/// directory is not a `.git` directory — which is the case for a BARE
/// repository (where `rev-parse` answers `.`, i.e. the repository itself) and
/// for a `--separate-git-dir` layout. Failing closed matters here: the caller
/// uses the answer to decide which session a path belongs to, and a wrong
/// answer is worse than no answer.
///
/// Goes through [`crate::agents::git_rev_parse_path`], which strips an
/// inherited `GIT_DIR` / `GIT_COMMON_DIR` from the child environment so a
/// pane or hook that exported one cannot redirect the lookup at another
/// repository.
fn owning_repo_root(path: &Path) -> Option<PathBuf> {
    let common = crate::agents::git_rev_parse_path(path, "--git-common-dir").ok()?;
    let common = canonical_or_raw(&common);
    if common.file_name() != Some(std::ffi::OsStr::new(".git")) {
        return None;
    }
    Some(canonical_or_raw(common.parent()?))
}

/// Whether `session` belongs to the repository at `repo_root`, comparing
/// canonicalized paths the same way [`find_session_for_repo_in`] resolves a
/// session by repository path. Used by `purge --stale` to scope the default
/// sweep to the current repository.
#[must_use]
pub fn session_belongs_to_repo(session: &Session, repo_root: &Path) -> bool {
    canonical_or_raw(&session.repo_path) == canonical_or_raw(repo_root)
}

/// Loads every session receipt in the given directory.
///
/// Used by `purge --stale` to enumerate all sessions on the machine and probe
/// each for staleness. Malformed files are skipped (matching
/// [`find_session_for_repo_in`]). A missing directory yields an empty list.
pub fn load_all_sessions_in(dir: &Path) -> Result<Vec<Session>, PawError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(PawError::SessionError(format!(
                "failed to read sessions dir: {e}"
            )));
        }
    };

    let mut out = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|e| PawError::SessionError(format!("failed to read dir entry: {e}")))?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        if let Ok(session) = serde_json::from_str::<Session>(&contents) {
            out.push(session);
        }
    }

    Ok(out)
}

/// Deletes a session file by name from the given directory.
pub fn delete_session_in(session_name: &str, dir: &Path) -> Result<(), PawError> {
    let path = dir.join(format!("{session_name}.json"));

    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(PawError::SessionError(format!(
            "failed to delete session file: {e}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Per-repo session discovery file (`<repo>/.git-paw/sessions/<name>.json`)
// ---------------------------------------------------------------------------
//
// Distinct from the global receipt above: this is the lightweight discovery
// surface the bundled `assets/scripts/sweep.sh` helper reads to find the
// active session name and its agent roster from inside the repo, without
// reaching into the XDG state dir. `git paw start` writes it; `purge` removes
// it (design `session-json-location`).

/// One agent entry in the per-repo discovery file.
///
/// The field set and names match exactly what `sweep.sh` expects:
/// `branch_id` (the broker agent id / slugified branch), `worktree_path`,
/// `cli`, and `pane_index`. Adding fields is backwards-compatible — consumers
/// ignore unknown keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoAgentEntry {
    /// Broker agent id (slugified branch), e.g. `feat-add-auth`.
    pub branch_id: String,
    /// Absolute path to the agent's worktree.
    pub worktree_path: PathBuf,
    /// The AI CLI assigned to the agent's pane.
    pub cli: String,
    /// The agent's tmux pane index within the session window.
    pub pane_index: usize,
}

/// The orchestrator (supervisor) pane entry in a supervisor-mode session's
/// discovery file (`session-state`).
///
/// Distinct from [`RepoAgentEntry`]: the orchestrator drives the wave rather
/// than doing the coding work, and its tier is often different (e.g. a
/// `fable`-tier orchestrator driving `sonnet` workers).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoOrchestratorEntry {
    /// The AI CLI running in the orchestrator's pane.
    pub cli: String,
    /// The orchestrator's tmux pane index within the session window (`0` in
    /// the supervisor layout).
    pub pane_index: usize,
}

/// The per-repo discovery document written to
/// `<repo>/.git-paw/sessions/<session_name>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSessionFile {
    /// The tmux session name (also the file stem).
    pub session_name: String,
    /// The coding-agent roster for the session.
    pub agents: Vec<RepoAgentEntry>,
    /// The supervisor/orchestrator pane, present only for a supervisor-mode
    /// session. Additive (`session-state`): `#[serde(default)]` so a file
    /// written before this field existed, or a bare-mode session (which has
    /// no orchestrator pane), loads with this absent, and `sweep.sh` — which
    /// enumerates `agents` by name — ignores the extra key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orchestrator: Option<RepoOrchestratorEntry>,
}

/// Returns the per-repo sessions directory: `<repo>/.git-paw/sessions/`.
#[must_use]
pub fn repo_sessions_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".git-paw").join("sessions")
}

/// Returns the per-repo session file path for a session name.
#[must_use]
pub fn repo_session_path(repo_root: &Path, session_name: &str) -> PathBuf {
    repo_sessions_dir(repo_root).join(format!("{session_name}.json"))
}

/// Atomically writes the per-repo discovery file for a session.
///
/// Creates `<repo>/.git-paw/sessions/` if absent, then writes
/// `<session_name>.json` via a temp-file-and-rename so a concurrent
/// `sweep.sh` read never sees a partial document.
///
/// The scratch `<session_name>.tmp` is UNLINKED first when something is
/// already there. `fs::write` follows a symlink, so a (tracked, therefore
/// attacker-supplied in a hostile repository) link sitting at that path would
/// redirect the write at its target — and this path is written on every
/// launch, recovery and resume. Nothing legitimately owns that scratch name,
/// so removing it is always safe. The final rename needs no such care: rename
/// replaces a symlink rather than following it.
pub fn write_repo_session_file(repo_root: &Path, file: &RepoSessionFile) -> Result<(), PawError> {
    let dir = repo_sessions_dir(repo_root);
    fs::create_dir_all(&dir).map_err(|e| {
        PawError::SessionError(format!("failed to create per-repo sessions dir: {e}"))
    })?;

    let json = serde_json::to_string_pretty(file).map_err(|e| {
        PawError::SessionError(format!("failed to serialize per-repo session file: {e}"))
    })?;

    let final_path = dir.join(format!("{}.json", file.session_name));
    let tmp_path = dir.join(format!("{}.tmp", file.session_name));
    if fs::symlink_metadata(&tmp_path).is_ok() {
        fs::remove_file(&tmp_path).map_err(|e| {
            PawError::SessionError(format!(
                "failed to clear stale per-repo session temp file '{}': {e}",
                tmp_path.display()
            ))
        })?;
    }
    fs::write(&tmp_path, json.as_bytes()).map_err(|e| {
        PawError::SessionError(format!("failed to write per-repo session temp file: {e}"))
    })?;
    fs::rename(&tmp_path, &final_path).map_err(|e| {
        PawError::SessionError(format!("failed to rename per-repo session temp file: {e}"))
    })?;
    Ok(())
}

/// Reads the per-repo discovery file for a session, or `None` when it does
/// not exist.
///
/// Used by `add`/`remove` to carry the existing `orchestrator` entry forward
/// unchanged when they rewrite the roster (the orchestrator pane does not
/// change when a coding agent is added or removed).
pub fn read_repo_session_file(
    repo_root: &Path,
    session_name: &str,
) -> Result<Option<RepoSessionFile>, PawError> {
    let path = repo_session_path(repo_root, session_name);
    let contents = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(PawError::SessionError(format!(
                "failed to read per-repo session file: {e}"
            )));
        }
    };
    let file: RepoSessionFile = serde_json::from_str(&contents).map_err(|e| {
        PawError::SessionError(format!("failed to parse per-repo session file: {e}"))
    })?;
    Ok(Some(file))
}

/// Removes the per-repo discovery file for a session. Idempotent — a missing
/// file is not an error.
pub fn remove_repo_session_file(repo_root: &Path, session_name: &str) -> Result<(), PawError> {
    let path = repo_session_path(repo_root, session_name);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(PawError::SessionError(format!(
            "failed to remove per-repo session file: {e}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Returns the sessions directory (`~/.local/share/git-paw/sessions/`).
///
/// Also used by the broker to place its log alongside session state (see
/// [`broker_log_path`]).
pub fn session_state_dir() -> Result<PathBuf, PawError> {
    sessions_dir()
}

/// Returns the broker log path for a session: `<session_state_dir>/<session_name>.broker.log`.
///
/// The log is named per session rather than shared, so two repositories
/// running git-paw on one machine never write to — or purge — the same file.
/// Session names are already unique per repository on a machine, so they need
/// no further qualification.
pub fn broker_log_path(session_name: &str) -> Result<PathBuf, PawError> {
    Ok(session_state_dir()?.join(format!("{session_name}.broker.log")))
}

/// Resolves the path the broker should write its log to, for a dashboard
/// launched against `session`.
///
/// The session's recorded `broker_log_path` is authoritative when present:
/// it is the exact path `purge` deletes, so writing anywhere else would
/// orphan a log. That is what keeps a session saved by a git-paw which used
/// a single shared `broker.log` consistent — the writer and the deleter
/// agree on the old name rather than diverging.
///
/// A session with no recorded path falls back to [`broker_log_path`] for its
/// own name; no session at all (nothing launched this dashboard) falls back
/// to `git-paw`, which keeps it clear of any real session's log.
///
/// # Errors
///
/// Returns [`PawError::SessionError`] when the session-state directory
/// cannot be determined.
pub fn resolve_broker_log_path(session: Option<&Session>) -> Result<PathBuf, PawError> {
    match session {
        Some(sess) => match &sess.broker_log_path {
            Some(saved) => Ok(saved.clone()),
            None => broker_log_path(&sess.session_name),
        },
        None => broker_log_path("git-paw"),
    }
}

/// Returns the sessions directory (`~/.local/share/git-paw/sessions/`).
fn sessions_dir() -> Result<PathBuf, PawError> {
    let base = crate::dirs::data_dir().ok_or_else(|| {
        PawError::SessionError("could not determine XDG data directory".to_string())
    })?;
    Ok(base.join("git-paw").join("sessions"))
}

// ---------------------------------------------------------------------------
// ISO 8601 helpers
// ---------------------------------------------------------------------------

/// Formats a `SystemTime` as an ISO 8601 UTC string (`YYYY-MM-DDTHH:MM:SSZ`).
fn format_iso8601(time: SystemTime) -> Result<String, PawError> {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map_err(|e| PawError::SessionError(format!("time before unix epoch: {e}")))?
        .as_secs();

    let (year, month, day, hour, min, sec) = secs_to_civil(secs);
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z"
    ))
}

/// Parses an ISO 8601 UTC string (`YYYY-MM-DDTHH:MM:SSZ`) into a `SystemTime`.
fn parse_iso8601(s: &str) -> Result<SystemTime, PawError> {
    let err = || PawError::SessionError(format!("invalid ISO 8601 timestamp: {s}"));

    // Expected format: YYYY-MM-DDTHH:MM:SSZ
    let s = s.strip_suffix('Z').ok_or_else(err)?;
    let (date, time) = s.split_once('T').ok_or_else(err)?;

    let date_parts: Vec<&str> = date.split('-').collect();
    let time_parts: Vec<&str> = time.split(':').collect();

    if date_parts.len() != 3 || time_parts.len() != 3 {
        return Err(err());
    }

    let year: u64 = date_parts[0].parse().map_err(|_| err())?;
    let month: u64 = date_parts[1].parse().map_err(|_| err())?;
    let day: u64 = date_parts[2].parse().map_err(|_| err())?;
    let hour: u64 = time_parts[0].parse().map_err(|_| err())?;
    let min: u64 = time_parts[1].parse().map_err(|_| err())?;
    let sec: u64 = time_parts[2].parse().map_err(|_| err())?;

    let secs = civil_to_secs(year, month, day, hour, min, sec).ok_or_else(err)?;
    Ok(UNIX_EPOCH + Duration::from_secs(secs))
}

/// Converts seconds since Unix epoch to (year, month, day, hour, minute, second) in UTC.
fn secs_to_civil(secs: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec_of_day = secs % 86400;
    let hour = sec_of_day / 3600;
    let min = (sec_of_day % 3600) / 60;
    let sec = sec_of_day % 60;

    // Days since epoch (1970-01-01)
    // Algorithm from Howard Hinnant's chrono-compatible date library.
    #[allow(clippy::cast_possible_wrap)]
    let mut days = (secs / 86400).cast_signed();

    days += 719_468; // shift epoch from 1970-01-01 to 0000-03-01
    let era = days / 146_097;
    let doe = days - era * 146_097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month index [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    #[allow(clippy::cast_sign_loss)]
    (
        y.cast_unsigned(),
        m.cast_unsigned(),
        d.cast_unsigned(),
        hour,
        min,
        sec,
    )
}

/// Converts (year, month, day, hour, min, sec) to seconds since Unix epoch.
fn civil_to_secs(year: u64, month: u64, day: u64, hour: u64, min: u64, sec: u64) -> Option<u64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || min > 59 || sec > 59 {
        return None;
    }

    #[allow(clippy::cast_possible_wrap)]
    let y = year.cast_signed();
    #[allow(clippy::cast_possible_wrap)]
    let m = month.cast_signed();
    #[allow(clippy::cast_possible_wrap)]
    let d = day.cast_signed();

    // Shift to March-based year
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y / 400;
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + d - 1;
    let doe = 365 * yoe + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;

    if days < 0 {
        return None;
    }

    #[allow(clippy::cast_sign_loss)]
    Some(days.cast_unsigned() * 86400 + hour * 3600 + min * 60 + sec)
}

// ---------------------------------------------------------------------------
// Serde helpers for SystemTime ↔ ISO 8601
// ---------------------------------------------------------------------------

fn serialize_system_time<S: Serializer>(time: &SystemTime, ser: S) -> Result<S::Ok, S::Error> {
    let s = format_iso8601(*time).map_err(serde::ser::Error::custom)?;
    ser.serialize_str(&s)
}

fn deserialize_system_time<'de, D: Deserializer<'de>>(de: D) -> Result<SystemTime, D::Error> {
    let s: String = Deserialize::deserialize(de)?;
    parse_iso8601(&s).map_err(serde::de::Error::custom)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Creates a sample session with 3 worktrees for testing.
    fn sample_session() -> Session {
        Session {
            session_name: "paw-my-project".to_string(),
            repo_path: PathBuf::from("/Users/test/code/my-project"),
            project_name: "my-project".to_string(),
            // Fixed unix epoch (2024-03-23 13:20:00 UTC); seconds is the
            // canonical unit for unix timestamps so the literal stays
            // human-readable as a date.
            #[allow(clippy::duration_suboptimal_units)]
            created_at: UNIX_EPOCH + Duration::from_secs(1_711_200_000),
            status: SessionStatus::Active,
            worktrees: vec![
                WorktreeEntry {
                    branch: "feature/auth".to_string(),
                    worktree_path: PathBuf::from("/Users/test/code/my-project-feature-auth"),
                    cli: "claude".to_string(),
                    branch_created: false,
                    base_branch: None,
                    pending_boot_prompt: None,
                    runtime_slot: None,
                    cli_session_id: None,
                    spec_id: None,
                },
                WorktreeEntry {
                    branch: "fix/api".to_string(),
                    worktree_path: PathBuf::from("/Users/test/code/my-project-fix-api"),
                    cli: "gemini".to_string(),
                    branch_created: false,
                    base_branch: None,
                    pending_boot_prompt: None,
                    runtime_slot: None,
                    cli_session_id: None,
                    spec_id: None,
                },
                WorktreeEntry {
                    branch: "feature/logging".to_string(),
                    worktree_path: PathBuf::from("/Users/test/code/my-project-feature-logging"),
                    cli: "claude".to_string(),
                    branch_created: false,
                    base_branch: None,
                    pending_boot_prompt: None,
                    runtime_slot: None,
                    cli_session_id: None,
                    spec_id: None,
                },
            ],
            broker_port: None,
            broker_bind: None,
            broker_log_path: None,
            mode: SessionMode::Bare,
            dashboard_pane: None,
        }
    }

    // -- save_session: GIVEN an active session with 3 worktrees,
    //    WHEN save_session() is called, THEN JSON file created with all fields --

    #[test]
    fn saved_session_can_be_loaded_with_all_fields_intact() {
        let dir = TempDir::new().unwrap();
        let session = sample_session();
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");

        assert_eq!(loaded.session_name, "paw-my-project");
        assert_eq!(
            loaded.repo_path,
            PathBuf::from("/Users/test/code/my-project")
        );
        assert_eq!(loaded.project_name, "my-project");
        assert_eq!(loaded.created_at, session.created_at);
        assert_eq!(loaded.status, SessionStatus::Active);
        assert_eq!(loaded.worktrees.len(), 3);
        assert_eq!(loaded.worktrees[0].branch, "feature/auth");
        assert_eq!(loaded.worktrees[0].cli, "claude");
        assert_eq!(loaded.worktrees[1].branch, "fix/api");
        assert_eq!(loaded.worktrees[1].cli, "gemini");
        assert_eq!(loaded.worktrees[2].branch, "feature/logging");
    }

    // -- save_session: saving again replaces the previous state --

    #[test]
    fn saving_again_replaces_previous_state() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        save_session_in(&session, dir.path()).unwrap();

        session.status = SessionStatus::Stopped;
        session.worktrees.pop();
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");

        assert_eq!(loaded.status, SessionStatus::Stopped);
        assert_eq!(loaded.worktrees.len(), 2);
    }

    // -- runtime_slot: additive optional field (worktree-env-port-provisioning) --

    #[test]
    fn saved_session_round_trips_runtime_slots_intact() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.worktrees[0].runtime_slot = Some(0);
        session.worktrees[1].runtime_slot = Some(1);
        session.worktrees[2].runtime_slot = Some(2);
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");

        let slots: Vec<Option<u16>> = loaded.worktrees.iter().map(|w| w.runtime_slot).collect();
        assert_eq!(slots, vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn state_file_without_runtime_slot_loads_with_none() {
        // A state file written before the field existed must still load, with
        // every worktree reporting no allocated block.
        let dir = TempDir::new().unwrap();
        let json = r#"{
            "session_name": "paw-legacy",
            "repo_path": "/Users/test/code/legacy",
            "project_name": "legacy",
            "created_at": "2024-03-23T13:20:00Z",
            "status": "active",
            "worktrees": [
                {
                    "branch": "feature/auth",
                    "worktree_path": "/Users/test/code/legacy-feature-auth",
                    "cli": "claude",
                    "branch_created": false
                }
            ]
        }"#;
        std::fs::write(dir.path().join("paw-legacy.json"), json).unwrap();

        let loaded = load_session_from("paw-legacy", dir.path())
            .unwrap()
            .expect("legacy session should load");
        assert_eq!(loaded.worktrees[0].runtime_slot, None);
    }

    #[test]
    fn absent_runtime_slot_is_omitted_from_serialized_state() {
        // A session that allocates no ports must serialize byte-identically to
        // one written by a prior version.
        let dir = TempDir::new().unwrap();
        save_session_in(&sample_session(), dir.path()).unwrap();
        let written = std::fs::read_to_string(dir.path().join("paw-my-project.json")).unwrap();
        assert!(
            !written.contains("runtime_slot"),
            "absent slot must not be serialized; got:\n{written}"
        );
    }

    // -- base_branch: additive optional field (purge-unmerged-branch-safety) --

    /// Spec scenario "A session file written before the base branch was
    /// recorded loads with no base". Teardown then compares against the default
    /// branch alone — the behaviour that shipped before the field existed.
    #[test]
    fn state_file_without_base_branch_loads_with_none() {
        let dir = TempDir::new().unwrap();
        let json = r#"{
            "session_name": "paw-legacy",
            "repo_path": "/Users/test/code/legacy",
            "project_name": "legacy",
            "created_at": "2024-03-23T13:20:00Z",
            "status": "active",
            "worktrees": [
                {
                    "branch": "feature/auth",
                    "worktree_path": "/Users/test/code/legacy-feature-auth",
                    "cli": "claude",
                    "branch_created": true
                }
            ]
        }"#;
        std::fs::write(dir.path().join("paw-legacy.json"), json).unwrap();

        let loaded = load_session_from("paw-legacy", dir.path())
            .unwrap()
            .expect("legacy session should load");
        assert_eq!(loaded.worktrees[0].base_branch, None);
        assert!(loaded.worktrees[0].branch_created);
    }

    #[test]
    fn saved_session_round_trips_base_branch_intact() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.worktrees[0].base_branch = Some("feat/v0.17.0-specs".into());
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");
        assert_eq!(
            loaded.worktrees[0].base_branch.as_deref(),
            Some("feat/v0.17.0-specs")
        );
    }

    #[test]
    fn absent_base_branch_is_omitted_from_serialized_state() {
        let dir = TempDir::new().unwrap();
        save_session_in(&sample_session(), dir.path()).unwrap();
        let written = std::fs::read_to_string(dir.path().join("paw-my-project.json")).unwrap();
        assert!(
            !written.contains("base_branch"),
            "an unrecorded base must not be serialized; got:\n{written}"
        );
    }

    // -- cli_session_id: additive optional field (worker-session-id-capture) --

    /// Spec scenario "Assigned id round-trips".
    #[test]
    fn saved_session_round_trips_cli_session_id_intact() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.worktrees[0].cli_session_id = Some("3f2c1a90-aaaa-4bbb-8ccc-1234567890ab".into());
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");

        assert_eq!(
            loaded.worktrees[0].cli_session_id.as_deref(),
            Some("3f2c1a90-aaaa-4bbb-8ccc-1234567890ab")
        );
    }

    /// Spec scenario "Older session files load with None".
    #[test]
    fn state_file_without_cli_session_id_loads_with_none() {
        // A state file written before the field existed must still load, with
        // every worktree reporting no assigned session id.
        let dir = TempDir::new().unwrap();
        let json = r#"{
            "session_name": "paw-legacy",
            "repo_path": "/Users/test/code/legacy",
            "project_name": "legacy",
            "created_at": "2024-03-23T13:20:00Z",
            "status": "active",
            "worktrees": [
                {
                    "branch": "feature/auth",
                    "worktree_path": "/Users/test/code/legacy-feature-auth",
                    "cli": "claude",
                    "branch_created": false
                }
            ]
        }"#;
        std::fs::write(dir.path().join("paw-legacy.json"), json).unwrap();

        let loaded = load_session_from("paw-legacy", dir.path())
            .unwrap()
            .expect("legacy session should load");
        assert_eq!(loaded.worktrees[0].cli_session_id, None);
    }

    /// Spec scenario "Entry without an id serializes as before".
    #[test]
    fn absent_cli_session_id_is_omitted_from_serialized_state() {
        let dir = TempDir::new().unwrap();
        save_session_in(&sample_session(), dir.path()).unwrap();
        let written = std::fs::read_to_string(dir.path().join("paw-my-project.json")).unwrap();
        assert!(
            !written.contains("cli_session_id"),
            "absent id must not be serialized; got:\n{written}"
        );
    }

    // -- spec_id: additive optional field (session-state) --

    /// Spec scenario "A spec-driven launch records each agent's spec id":
    /// an entry launched from a spec round-trips that spec's id.
    #[test]
    fn saved_session_round_trips_spec_id_intact() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.worktrees[0].spec_id = Some("003-user-list".into());
        session.worktrees[1].spec_id = Some("004-error-handling".into());
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");

        assert_eq!(
            loaded.worktrees[0].spec_id.as_deref(),
            Some("003-user-list")
        );
        assert_eq!(
            loaded.worktrees[1].spec_id.as_deref(),
            Some("004-error-handling")
        );
    }

    /// Spec scenario "Older session files load with no spec id".
    #[test]
    fn state_file_without_spec_id_loads_with_none() {
        // A state file written before the field existed must still load, with
        // every worktree reporting no launch spec.
        let dir = TempDir::new().unwrap();
        let json = r#"{
            "session_name": "paw-legacy",
            "repo_path": "/Users/test/code/legacy",
            "project_name": "legacy",
            "created_at": "2024-03-23T13:20:00Z",
            "status": "active",
            "worktrees": [
                {
                    "branch": "feature/auth",
                    "worktree_path": "/Users/test/code/legacy-feature-auth",
                    "cli": "claude",
                    "branch_created": false
                }
            ]
        }"#;
        std::fs::write(dir.path().join("paw-legacy.json"), json).unwrap();

        let loaded = load_session_from("paw-legacy", dir.path())
            .unwrap()
            .expect("legacy session should load");
        assert_eq!(loaded.worktrees[0].spec_id, None);
    }

    /// Spec scenario "A launch without a spec records no spec id": the key is
    /// absent from the written JSON, so a bare launch serializes exactly as
    /// it did before the field existed.
    #[test]
    fn absent_spec_id_is_omitted_from_serialized_state() {
        let dir = TempDir::new().unwrap();
        save_session_in(&sample_session(), dir.path()).unwrap();
        let written = std::fs::read_to_string(dir.path().join("paw-my-project.json")).unwrap();
        assert!(
            !written.contains("spec_id"),
            "absent spec id must not be serialized; got:\n{written}"
        );
    }

    // -- load_session: WHEN load_session("nonexistent") is called, THEN returns None --

    #[test]
    fn loading_nonexistent_session_returns_none() {
        let dir = TempDir::new().unwrap();
        let result = load_session_from("nonexistent", dir.path()).unwrap();
        assert!(result.is_none());
    }

    // -- find_session_for_repo: GIVEN two sessions,
    //    WHEN find_session_for_repo is called, THEN returns the matching one --

    #[test]
    fn finds_correct_session_among_multiple_by_repo_path() {
        let dir = TempDir::new().unwrap();

        let mut session_a = sample_session();
        session_a.session_name = "paw-project-a".to_string();
        session_a.repo_path = PathBuf::from("/Users/test/code/project-a");

        let mut session_b = sample_session();
        session_b.session_name = "paw-project-b".to_string();
        session_b.repo_path = PathBuf::from("/Users/test/code/project-b");

        save_session_in(&session_a, dir.path()).unwrap();
        save_session_in(&session_b, dir.path()).unwrap();

        let found = find_session_for_repo_in(Path::new("/Users/test/code/project-b"), dir.path())
            .unwrap()
            .expect("should find session for project-b");

        assert_eq!(found.session_name, "paw-project-b");
        assert_eq!(found.repo_path, PathBuf::from("/Users/test/code/project-b"));
    }

    /// `fs::write` follows a symlink, so a link at the scratch
    /// `<session>.tmp` path would redirect the discovery write at its target.
    /// The writer must unlink it first and leave the target untouched.
    #[cfg(unix)]
    #[test]
    fn write_repo_session_file_does_not_write_through_a_symlinked_temp_path() {
        let repo = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let victim = outside.path().join("victim.txt");
        std::fs::write(&victim, "untouched").unwrap();

        let dir = repo_sessions_dir(repo.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(&victim, dir.join("paw-x.tmp")).unwrap();

        let file = RepoSessionFile {
            session_name: "paw-x".to_string(),
            agents: Vec::new(),
            orchestrator: None,
        };
        write_repo_session_file(repo.path(), &file).expect("write must succeed");

        assert_eq!(
            std::fs::read_to_string(&victim).unwrap(),
            "untouched",
            "the symlink target must not be written through"
        );
        let written = std::fs::read_to_string(repo_session_path(repo.path(), "paw-x")).unwrap();
        assert!(
            written.contains("paw-x"),
            "the discovery file itself must still be written; got:\n{written}"
        );
    }

    /// Builds a repository with one embedded agent worktree and a saved
    /// session receipt for the repository, returning
    /// `(sessions_dir, sandbox, repo_root, worktree_root)`. The temp dirs are
    /// returned so the caller keeps them alive.
    fn repo_with_worktree_and_session() -> (TempDir, TempDir, PathBuf, PathBuf) {
        let sessions = TempDir::new().unwrap();
        let sandbox = TempDir::new().unwrap();
        let repo = sandbox.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();

        let git = |args: &[&str], cwd: &Path| {
            let out = std::process::Command::new("git")
                .current_dir(cwd)
                .args(args)
                .output()
                .expect("git");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q", "-b", "main"], &repo);
        git(&["config", "user.email", "t@e.st"], &repo);
        git(&["config", "user.name", "Test"], &repo);
        std::fs::write(repo.join("README.md"), "x").unwrap();
        git(&["add", "."], &repo);
        git(&["commit", "-q", "-m", "init"], &repo);

        // An embedded agent worktree, as `git paw start` creates.
        let worktree = repo.join(".git-paw").join("worktrees").join("feat-x");
        git(
            &[
                "worktree",
                "add",
                "-b",
                "feat/x",
                worktree.to_str().unwrap(),
            ],
            &repo,
        );

        let mut session = sample_session();
        session.repo_path = repo.canonicalize().unwrap();
        save_session_in(&session, sessions.path()).unwrap();

        (sessions, sandbox, repo, worktree)
    }

    /// Spec scenario "A worktree root resolves to its repository's session"
    /// (`session-state`): a linked worktree's root never equals the
    /// repository root, so the OWNER-RESOLVING lookup must resolve the
    /// worktree to the repository that owns it before matching.
    #[test]
    fn owner_lookup_matches_a_linked_worktree_root_via_its_owning_repository() {
        let (sessions, _sandbox, _repo, worktree) = repo_with_worktree_and_session();

        let from_root = find_session_for_repo_or_owner_in(&worktree, sessions.path())
            .unwrap()
            .expect("a worktree root must resolve to its repository's session");
        assert_eq!(from_root.session_name, "paw-my-project");

        // A path INSIDE the worktree resolves the same way.
        let nested = worktree.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        let from_nested = find_session_for_repo_or_owner_in(&nested, sessions.path())
            .unwrap()
            .expect("a path inside a worktree must resolve to its repository's session");
        assert_eq!(from_nested.session_name, "paw-my-project");
    }

    /// The SAFETY boundary behind the two lookups: the exact-match lookup —
    /// the one every mutating command uses — must NOT resolve a worktree to
    /// its owning repository's session. If it did, `git paw stop` / `purge`
    /// run from inside an agent worktree would act on the parent session.
    #[test]
    fn exact_lookup_does_not_resolve_a_worktree_to_its_owning_repository() {
        let (sessions, _sandbox, repo, worktree) = repo_with_worktree_and_session();

        assert!(
            find_session_for_repo_in(&worktree, sessions.path())
                .unwrap()
                .is_none(),
            "the exact lookup must not see the parent repository's session from \
             inside an agent worktree — mutating commands resolve their repo to \
             the worktree root and would act on the wrong session"
        );

        // The repository's own root still matches, of course.
        assert!(
            find_session_for_repo_in(&repo, sessions.path())
                .unwrap()
                .is_some()
        );
    }

    /// A BARE repository has no checkout to own anything: `git rev-parse
    /// --git-common-dir` answers `.` (the repository directory itself), whose
    /// parent is the containing directory — an unrelated path. The lookup must
    /// fail closed rather than claim that directory owns the path.
    #[test]
    fn owner_lookup_returns_none_inside_a_bare_repository() {
        let sessions = TempDir::new().unwrap();
        let sandbox = TempDir::new().unwrap();
        let bare = sandbox.path().join("repo.git");
        std::fs::create_dir_all(&bare).unwrap();
        let out = std::process::Command::new("git")
            .current_dir(&bare)
            .args(["init", "-q", "--bare"])
            .output()
            .expect("git init --bare");
        assert!(out.status.success());

        // A session registered for the bare repo's PARENT — what the buggy
        // parent-of-`.` resolution would wrongly match.
        let mut session = sample_session();
        session.repo_path = sandbox.path().canonicalize().unwrap();
        save_session_in(&session, sessions.path()).unwrap();

        assert_eq!(owning_repo_root(&bare), None);
        assert!(
            find_session_for_repo_or_owner_in(&bare, sessions.path())
                .unwrap()
                .is_none(),
            "a bare repository must not resolve to its containing directory's session"
        );
    }

    /// The owner-resolving lookup prefers an EXACT match: when the worktree
    /// itself has a session receipt, that one is returned rather than the
    /// owning repository's.
    #[test]
    fn owner_lookup_prefers_an_exact_match_over_the_owner() {
        let (sessions, _sandbox, _repo, worktree) = repo_with_worktree_and_session();

        let mut own = sample_session();
        own.session_name = "paw-the-worktree-itself".to_string();
        own.repo_path = worktree.canonicalize().unwrap();
        save_session_in(&own, sessions.path()).unwrap();

        let found = find_session_for_repo_or_owner_in(&worktree, sessions.path())
            .unwrap()
            .expect("an exact match exists");
        assert_eq!(found.session_name, "paw-the-worktree-itself");
    }

    #[test]
    fn find_returns_none_when_no_repo_matches() {
        let dir = TempDir::new().unwrap();
        save_session_in(&sample_session(), dir.path()).unwrap();

        let found =
            find_session_for_repo_in(Path::new("/Users/test/code/other-project"), dir.path())
                .unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn find_returns_none_when_no_sessions_exist() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("does-not-exist");
        let found = find_session_for_repo_in(Path::new("/any"), &missing).unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn find_matches_a_symlinked_repo_path() {
        use std::os::unix::fs::symlink;
        let sessions = TempDir::new().unwrap();
        let real_repo = TempDir::new().unwrap();

        // A session registered at the real repo path.
        let mut session = sample_session();
        session.repo_path = real_repo.path().to_path_buf();
        save_session_in(&session, sessions.path()).unwrap();

        // Look it up via a symlink that resolves to the same repo directory.
        let link_parent = TempDir::new().unwrap();
        let link = link_parent.path().join("repo-link");
        symlink(real_repo.path(), &link).unwrap();

        let found = find_session_for_repo_in(&link, sessions.path())
            .unwrap()
            .expect("session should resolve through the symlink to the same repo");
        assert_eq!(found.session_name, session.session_name);
    }

    // -- delete_session: removes file, load returns None afterwards --

    #[test]
    fn deleted_session_is_no_longer_loadable() {
        let dir = TempDir::new().unwrap();
        save_session_in(&sample_session(), dir.path()).unwrap();

        delete_session_in("paw-my-project", dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path()).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn deleting_nonexistent_session_succeeds() {
        let dir = TempDir::new().unwrap();
        delete_session_in("nonexistent", dir.path()).unwrap();
    }

    // -- Status check: combines file existence + tmux liveness --

    #[test]
    fn file_says_active_and_tmux_alive_means_active() {
        let session = sample_session();
        assert_eq!(session.effective_status(|_| true), SessionStatus::Active);
    }

    #[test]
    fn file_says_active_but_tmux_dead_means_stopped() {
        let session = sample_session();
        assert_eq!(session.effective_status(|_| false), SessionStatus::Stopped);
    }

    #[test]
    fn file_says_stopped_stays_stopped_regardless_of_tmux() {
        let mut session = sample_session();
        session.status = SessionStatus::Stopped;
        // Even if tmux is somehow alive, stopped means stopped.
        assert_eq!(session.effective_status(|_| true), SessionStatus::Stopped);
    }

    // -- SessionStatus Display --

    #[test]
    fn session_status_displays_as_lowercase_string() {
        assert_eq!(SessionStatus::Active.to_string(), "active");
        assert_eq!(SessionStatus::Paused.to_string(), "paused");
        assert_eq!(SessionStatus::Stopped.to_string(), "stopped");
    }

    // -- DisplayStatus: receipt × liveness → display (session-bugfixes Bug 2,
    //    tasks 4.1–4.3) --

    #[test]
    fn display_status_active_receipt_alive_tmux_is_active() {
        use crate::tmux::SessionLiveness;
        let d = DisplayStatus::from_receipt(&SessionStatus::Active, SessionLiveness::Alive);
        assert_eq!(d, DisplayStatus::Active);
        assert_eq!(d.as_str(), "active");
        assert_eq!(d.icon(), "\u{1f7e2}");
    }

    #[test]
    fn display_status_active_receipt_stale_tmux_is_stale() {
        use crate::tmux::SessionLiveness;
        let d = DisplayStatus::from_receipt(&SessionStatus::Active, SessionLiveness::Stale);
        assert_eq!(d, DisplayStatus::Stale);
        assert_eq!(d.as_str(), "stale");
        assert_eq!(d.icon(), "\u{1f534}");
    }

    #[test]
    fn display_status_stopped_receipt_is_stopped_regardless_of_tmux() {
        use crate::tmux::SessionLiveness;
        for liveness in [
            SessionLiveness::Alive,
            SessionLiveness::Stale,
            SessionLiveness::AgentsExited,
            SessionLiveness::Indeterminate,
        ] {
            let d = DisplayStatus::from_receipt(&SessionStatus::Stopped, liveness);
            assert_eq!(d, DisplayStatus::Stopped, "liveness {liveness:?}");
            assert_eq!(d.as_str(), "stopped");
        }
    }

    #[test]
    fn display_status_indeterminate_never_reports_stale() {
        use crate::tmux::SessionLiveness;
        // tmux binary missing: an active receipt must NOT surface as stale —
        // it preserves the pre-existing "not alive" display (stopped).
        let d = DisplayStatus::from_receipt(&SessionStatus::Active, SessionLiveness::Indeterminate);
        assert_ne!(d, DisplayStatus::Stale);
        assert_eq!(d, DisplayStatus::Stopped);
    }

    /// Spec scenario "A session whose agent panes all exited reports Stopped":
    /// an exited-agents probe downgrades active and paused receipts to
    /// `Stopped`, and never to `Stale` — the tmux session is still running.
    #[test]
    fn display_status_agents_exited_is_stopped_never_stale() {
        use crate::tmux::SessionLiveness;
        for receipt in [SessionStatus::Active, SessionStatus::Paused] {
            let d = DisplayStatus::from_receipt(&receipt, SessionLiveness::AgentsExited);
            assert_eq!(d, DisplayStatus::Stopped, "receipt {receipt:?}");
            assert_ne!(d, DisplayStatus::Stale, "receipt {receipt:?}");
            assert_eq!(d.as_str(), "stopped");
        }
    }

    #[test]
    fn display_status_paused_alive_is_paused_dead_is_stopped() {
        use crate::tmux::SessionLiveness;
        assert_eq!(
            DisplayStatus::from_receipt(&SessionStatus::Paused, SessionLiveness::Alive),
            DisplayStatus::Paused
        );
        assert_eq!(
            DisplayStatus::from_receipt(&SessionStatus::Paused, SessionLiveness::Stale),
            DisplayStatus::Stopped
        );
    }

    // -- Paused variant: round-trip + effective_status --

    #[test]
    fn paused_status_serializes_lowercase() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.status = SessionStatus::Paused;
        save_session_in(&session, dir.path()).unwrap();

        let json = std::fs::read_to_string(dir.path().join("paw-my-project.json")).unwrap();
        assert!(
            json.contains("\"status\": \"paused\""),
            "JSON should contain `\"status\": \"paused\"`, got: {json}"
        );
    }

    #[test]
    fn paused_session_round_trips() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.status = SessionStatus::Paused;
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");
        assert_eq!(loaded.status, SessionStatus::Paused);
    }

    #[test]
    fn effective_status_paused_alive_remains_paused() {
        let mut session = sample_session();
        session.status = SessionStatus::Paused;
        assert_eq!(session.effective_status(|_| true), SessionStatus::Paused);
    }

    #[test]
    fn effective_status_paused_dead_downgrades_to_stopped() {
        let mut session = sample_session();
        session.status = SessionStatus::Paused;
        assert_eq!(session.effective_status(|_| false), SessionStatus::Stopped);
    }

    // -- agent_pane_offset --

    #[test]
    fn agent_pane_offset_supervisor_mode_is_the_layout_constant() {
        let mut session = sample_session();
        session.mode = SessionMode::Supervisor;
        assert_eq!(
            session.agent_pane_offset(),
            crate::surface::SUPERVISOR_PANE_OFFSET
        );
    }

    #[test]
    fn agent_pane_offset_bare_mode_with_broker_is_one() {
        let mut session = sample_session();
        session.mode = SessionMode::Bare;
        session.broker_port = Some(9999);
        assert_eq!(session.agent_pane_offset(), 1);
    }

    #[test]
    fn agent_pane_offset_bare_mode_without_broker_is_zero() {
        let mut session = sample_session();
        session.mode = SessionMode::Bare;
        session.broker_port = None;
        assert_eq!(session.agent_pane_offset(), 0);
    }

    // -- dashboard_pane field --

    #[test]
    fn dashboard_pane_round_trips() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.dashboard_pane = Some(1);
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");
        assert_eq!(loaded.dashboard_pane, Some(1));
    }

    #[test]
    fn v04_session_without_dashboard_pane_loads_as_none() {
        let dir = TempDir::new().unwrap();
        let json = r#"{
            "session_name": "paw-legacy-dashboard",
            "repo_path": "/tmp/legacy-repo",
            "project_name": "legacy",
            "created_at": "2024-03-23T12:00:00Z",
            "status": "active",
            "worktrees": []
        }"#;
        std::fs::write(dir.path().join("paw-legacy-dashboard.json"), json).unwrap();

        let loaded = load_session_from("paw-legacy-dashboard", dir.path())
            .unwrap()
            .expect("session should load");
        assert!(
            loaded.dashboard_pane.is_none(),
            "v0.4 session should load with dashboard_pane = None"
        );
    }

    #[test]
    fn dashboard_pane_none_is_omitted_from_json() {
        let dir = TempDir::new().unwrap();
        let session = sample_session(); // dashboard_pane is None by default
        save_session_in(&session, dir.path()).unwrap();

        let json = std::fs::read_to_string(dir.path().join("paw-my-project.json")).unwrap();
        assert!(
            !json.contains("dashboard_pane"),
            "JSON should not include dashboard_pane when None, got: {json}"
        );
    }

    // -- Recovery: save → tmux dies → state has everything to reconstruct --

    // -- Broker fields --

    #[test]
    fn broker_log_is_named_per_session() {
        let alpha = broker_log_path("paw-alpha").unwrap();
        let beta = broker_log_path("paw-beta").unwrap();
        assert_ne!(alpha, beta, "two sessions never share one log file");
        assert_eq!(
            alpha.file_name().unwrap(),
            std::ffi::OsStr::new("paw-alpha.broker.log")
        );
        assert_eq!(
            alpha.parent(),
            beta.parent(),
            "both sit in the session-state directory"
        );
    }

    #[test]
    fn a_session_recording_the_old_shared_log_name_still_loads() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.broker_port = Some(9119);
        session.broker_bind = Some("127.0.0.1".to_string());
        // The pre-change name: one shared file for every repository.
        session.broker_log_path = Some(PathBuf::from("/var/state/git-paw/sessions/broker.log"));
        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .unwrap();
        assert_eq!(
            loaded.broker_log_path,
            Some(PathBuf::from("/var/state/git-paw/sessions/broker.log")),
            "the recorded path survives a round-trip unchanged"
        );
        assert_eq!(
            resolve_broker_log_path(Some(&loaded)).unwrap(),
            loaded.broker_log_path.clone().unwrap(),
            "and it is the path the broker writes, matching what purge deletes"
        );
    }

    #[test]
    fn a_saved_broker_log_path_wins_over_the_derived_name() {
        // A session saved by a git-paw that used one shared `broker.log` is
        // exactly the case that must not diverge: `purge` deletes the path
        // the session recorded, so the broker has to write there too.
        let mut legacy = sample_session();
        legacy.broker_log_path = Some(PathBuf::from("/var/state/git-paw/sessions/broker.log"));
        assert_eq!(
            resolve_broker_log_path(Some(&legacy)).unwrap(),
            PathBuf::from("/var/state/git-paw/sessions/broker.log"),
            "the writer must follow the path purge will delete"
        );
    }

    #[test]
    fn a_session_without_a_saved_log_path_gets_the_per_session_name() {
        let mut session = sample_session();
        session.broker_log_path = None;
        assert_eq!(
            resolve_broker_log_path(Some(&session)).unwrap(),
            broker_log_path("paw-my-project").unwrap()
        );
    }

    #[test]
    fn a_dashboard_with_no_session_keeps_clear_of_real_session_logs() {
        let resolved = resolve_broker_log_path(None).unwrap();
        assert_eq!(
            resolved.file_name().unwrap(),
            std::ffi::OsStr::new("git-paw.broker.log")
        );
    }

    #[test]
    fn session_with_broker_fields_round_trips() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.broker_port = Some(9119);
        session.broker_bind = Some("127.0.0.1".to_string());
        session.broker_log_path = Some(PathBuf::from("/tmp/broker.log"));

        save_session_in(&session, dir.path()).unwrap();

        let loaded = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should exist");

        assert_eq!(loaded.broker_port, Some(9119));
        assert_eq!(loaded.broker_bind.as_deref(), Some("127.0.0.1"));
        assert_eq!(
            loaded.broker_log_path,
            Some(PathBuf::from("/tmp/broker.log"))
        );
    }

    #[test]
    fn v020_session_json_loads_with_broker_fields_as_none() {
        let dir = TempDir::new().unwrap();
        // Simulate a v0.2.0 session JSON that has no broker fields
        let json = r#"{
            "session_name": "paw-legacy",
            "repo_path": "/tmp/legacy-repo",
            "project_name": "legacy",
            "created_at": "2024-03-23T12:00:00Z",
            "status": "active",
            "worktrees": []
        }"#;
        std::fs::write(dir.path().join("paw-legacy.json"), json).unwrap();

        let loaded = load_session_from("paw-legacy", dir.path())
            .unwrap()
            .expect("session should load");

        assert!(loaded.broker_port.is_none());
        assert!(loaded.broker_bind.is_none());
        assert!(loaded.broker_log_path.is_none());
        assert_eq!(loaded.session_name, "paw-legacy");
    }

    #[test]
    fn session_with_broker_fields_serializes_them() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.broker_port = Some(9119);
        session.broker_bind = Some("127.0.0.1".to_string());
        session.broker_log_path = Some(PathBuf::from("/tmp/broker.log"));
        save_session_in(&session, dir.path()).unwrap();

        let json = std::fs::read_to_string(dir.path().join("paw-my-project.json")).unwrap();
        assert!(
            json.contains("broker_port"),
            "JSON should contain broker_port"
        );
        assert!(
            json.contains("broker_bind"),
            "JSON should contain broker_bind"
        );
        assert!(
            json.contains("broker_log_path"),
            "JSON should contain broker_log_path"
        );
    }

    #[test]
    fn session_without_broker_fields_omits_them_from_json() {
        let dir = TempDir::new().unwrap();
        let session = sample_session(); // broker fields are all None
        save_session_in(&session, dir.path()).unwrap();

        let json = std::fs::read_to_string(dir.path().join("paw-my-project.json")).unwrap();
        assert!(
            !json.contains("broker_port"),
            "JSON should not contain broker_port when None"
        );
        assert!(
            !json.contains("broker_bind"),
            "JSON should not contain broker_bind when None"
        );
        assert!(
            !json.contains("broker_log_path"),
            "JSON should not contain broker_log_path when None"
        );
    }

    // -- Recovery with broker fields --

    #[test]
    fn recovery_after_tmux_crash_has_all_data_to_reconstruct() {
        let dir = TempDir::new().unwrap();
        let session = sample_session();
        save_session_in(&session, dir.path()).unwrap();

        // Simulate: tmux crashed, we reload from disk.
        let recovered = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session state should survive tmux crash");

        // Has the tmux session name to recreate.
        assert_eq!(recovered.session_name, "paw-my-project");
        // Has the repo path to cd into.
        assert_eq!(
            recovered.repo_path,
            PathBuf::from("/Users/test/code/my-project")
        );
        // Has every worktree's branch, path, and CLI — enough to relaunch.
        assert_eq!(recovered.worktrees.len(), 3);
        for wt in &recovered.worktrees {
            assert!(!wt.branch.is_empty());
            assert!(!wt.worktree_path.as_os_str().is_empty());
            assert!(!wt.cli.is_empty());
        }
        // Status correctly reflects that tmux is gone.
        assert_eq!(
            recovered.effective_status(|_| false),
            SessionStatus::Stopped
        );
    }

    // -- Recovery with broker enabled --

    #[test]
    fn session_with_broker_enabled_has_recovery_data() {
        let dir = TempDir::new().unwrap();
        let mut session = sample_session();
        session.broker_port = Some(9119);
        session.broker_bind = Some("127.0.0.1".to_string());
        save_session_in(&session, dir.path()).unwrap();

        let recovered = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should load");

        // Broker fields are preserved for recovery
        assert_eq!(recovered.broker_port, Some(9119));
        assert_eq!(recovered.broker_bind.as_deref(), Some("127.0.0.1"));
    }

    #[test]
    fn session_without_broker_has_no_recovery_data() {
        let dir = TempDir::new().unwrap();
        let session = sample_session(); // broker fields are None by default
        save_session_in(&session, dir.path()).unwrap();

        let recovered = load_session_from("paw-my-project", dir.path())
            .unwrap()
            .expect("session should load");

        // No broker fields to recover
        assert!(recovered.broker_port.is_none());
        assert!(recovered.broker_bind.is_none());
    }

    // -----------------------------------------------------------------------
    // Per-repo discovery file (session-json-location, Bug 3)
    // -----------------------------------------------------------------------

    fn sample_repo_file() -> RepoSessionFile {
        RepoSessionFile {
            session_name: "paw-my-project".to_string(),
            agents: vec![
                RepoAgentEntry {
                    branch_id: "feat-add-auth".to_string(),
                    worktree_path: PathBuf::from("/repo-feat-add-auth"),
                    cli: "claude".to_string(),
                    pane_index: 2,
                },
                RepoAgentEntry {
                    branch_id: "feat-fix-db".to_string(),
                    worktree_path: PathBuf::from("/repo-feat-fix-db"),
                    cli: "gemini".to_string(),
                    pane_index: 3,
                },
            ],
            orchestrator: None,
        }
    }

    #[test]
    fn write_repo_session_file_writes_sweep_compatible_shape() {
        let repo = TempDir::new().expect("repo");
        let file = sample_repo_file();
        write_repo_session_file(repo.path(), &file).expect("write");

        // Written at the path sweep.sh reads.
        let path = repo_session_path(repo.path(), "paw-my-project");
        assert_eq!(
            path,
            repo.path()
                .join(".git-paw")
                .join("sessions")
                .join("paw-my-project.json")
        );
        assert!(path.exists(), "discovery file should exist");

        // Shape round-trips with the exact field names sweep.sh expects.
        let raw = fs::read_to_string(&path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");
        assert_eq!(parsed["session_name"], "paw-my-project");
        let agents = parsed["agents"].as_array().expect("agents array");
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0]["branch_id"], "feat-add-auth");
        assert_eq!(agents[0]["worktree_path"], "/repo-feat-add-auth");
        assert_eq!(agents[0]["cli"], "claude");
        assert_eq!(agents[0]["pane_index"], 2);
    }

    #[test]
    fn remove_repo_session_file_is_idempotent() {
        let repo = TempDir::new().expect("repo");
        // Removing when nothing exists is a no-op (not an error).
        remove_repo_session_file(repo.path(), "paw-my-project").expect("remove-missing");

        write_repo_session_file(repo.path(), &sample_repo_file()).expect("write");
        let path = repo_session_path(repo.path(), "paw-my-project");
        assert!(path.exists());

        remove_repo_session_file(repo.path(), "paw-my-project").expect("remove");
        assert!(
            !path.exists(),
            "discovery file should be removed by purge path"
        );
    }

    // -----------------------------------------------------------------------
    // Orchestrator pane entry (unattended-wave-lifecycle, task 3)
    // -----------------------------------------------------------------------

    #[test]
    fn orchestrator_entry_is_recorded_distinct_from_agents() {
        let repo = TempDir::new().expect("repo");
        let mut file = sample_repo_file();
        file.orchestrator = Some(RepoOrchestratorEntry {
            cli: "fable".to_string(),
            pane_index: 0,
        });
        write_repo_session_file(repo.path(), &file).expect("write");

        let path = repo_session_path(repo.path(), "paw-my-project");
        let raw = fs::read_to_string(&path).expect("read");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("json");

        assert_eq!(parsed["orchestrator"]["cli"], "fable");
        assert_eq!(parsed["orchestrator"]["pane_index"], 0);
        // Distinct from the coding-agent roster: sweep.sh's `.agents[]` walk
        // (a `dict.get("agents", [])`) is unaffected by the sibling key.
        let agents = parsed["agents"].as_array().expect("agents array");
        assert_eq!(agents.len(), 2, "orchestrator does not alter the roster");

        let loaded = read_repo_session_file(repo.path(), "paw-my-project")
            .expect("read")
            .expect("file present");
        assert_eq!(
            loaded.orchestrator,
            Some(RepoOrchestratorEntry {
                cli: "fable".to_string(),
                pane_index: 0,
            })
        );
    }

    #[test]
    fn repo_session_file_without_orchestrator_loads() {
        // A file written before this field existed (or a bare-mode session,
        // which never had an orchestrator pane).
        let raw = r#"{"session_name":"paw-legacy","agents":[]}"#;
        let file: RepoSessionFile = serde_json::from_str(raw).expect("legacy file must load");
        assert!(file.orchestrator.is_none());
    }

    #[test]
    fn read_repo_session_file_returns_none_when_missing() {
        let repo = TempDir::new().expect("repo");
        let result = read_repo_session_file(repo.path(), "paw-missing").expect("no error");
        assert!(result.is_none());
    }
}
