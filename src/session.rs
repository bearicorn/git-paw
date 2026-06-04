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
    /// [`crate::tmux::SessionLiveness::Stale`]. An `Indeterminate` probe (the
    /// tmux binary is missing) never yields `Stale`: it preserves the
    /// pre-existing "tmux not alive" display by downgrading active/paused to
    /// `Stopped`.
    #[must_use]
    pub fn from_receipt(status: &SessionStatus, liveness: crate::tmux::SessionLiveness) -> Self {
        use crate::tmux::SessionLiveness as L;
        match (status, liveness) {
            (SessionStatus::Active, L::Alive) => Self::Active,
            (SessionStatus::Active, L::Stale) => Self::Stale,
            (SessionStatus::Paused, L::Alive) => Self::Paused,
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

    /// Boot+task prompt awaiting submission.
    ///
    /// Set only when an agent is attached via `git paw add` to a *paused*
    /// session (design D4): the pane is created and the boot block injected,
    /// but the prompt is held unsubmitted so the new agent stays paused with
    /// the rest of the session. `git paw resume` (restart-from-pause) submits
    /// any entry carrying a pending prompt and clears the field. `None` for
    /// every start-time agent and for adds to an active session (submitted
    /// immediately). Omitted from JSON when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_boot_prompt: Option<String>,
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
/// session whose `repo_path` matches the given path.
pub fn find_session_for_repo(repo_path: &Path) -> Result<Option<Session>, PawError> {
    find_session_for_repo_in(repo_path, &sessions_dir()?)
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

/// Finds the session for a repo path, scanning the given directory.
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

        if session.repo_path == repo_path {
            return Ok(Some(session));
        }
    }

    Ok(None)
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

/// The per-repo discovery document written to
/// `<repo>/.git-paw/sessions/<session_name>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSessionFile {
    /// The tmux session name (also the file stem).
    pub session_name: String,
    /// The coding-agent roster for the session.
    pub agents: Vec<RepoAgentEntry>,
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
    fs::write(&tmp_path, json.as_bytes()).map_err(|e| {
        PawError::SessionError(format!("failed to write per-repo session temp file: {e}"))
    })?;
    fs::rename(&tmp_path, &final_path).map_err(|e| {
        PawError::SessionError(format!("failed to rename per-repo session temp file: {e}"))
    })?;
    Ok(())
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
/// Also used by the broker to place `broker.log` alongside session state.
pub fn session_state_dir() -> Result<PathBuf, PawError> {
    sessions_dir()
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
                    pending_boot_prompt: None,
                },
                WorktreeEntry {
                    branch: "fix/api".to_string(),
                    worktree_path: PathBuf::from("/Users/test/code/my-project-fix-api"),
                    cli: "gemini".to_string(),
                    branch_created: false,
                    pending_boot_prompt: None,
                },
                WorktreeEntry {
                    branch: "feature/logging".to_string(),
                    worktree_path: PathBuf::from("/Users/test/code/my-project-feature-logging"),
                    cli: "claude".to_string(),
                    branch_created: false,
                    pending_boot_prompt: None,
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
}
