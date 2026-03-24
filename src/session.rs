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
    /// Tmux session has been stopped or crashed; state is recoverable.
    Stopped,
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Stopped => write!(f, "stopped"),
        }
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
}

impl Session {
    /// Returns the effective status by combining the on-disk status with a
    /// tmux liveness check.
    ///
    /// If the recorded status is `Active` but the tmux session is not alive,
    /// returns `Stopped`.
    pub fn effective_status(&self, is_tmux_alive: impl Fn(&str) -> bool) -> SessionStatus {
        if self.status == SessionStatus::Active && !is_tmux_alive(&self.session_name) {
            return SessionStatus::Stopped;
        }
        self.status.clone()
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

/// Loads a session by name, returning `None` if the file does not exist.
#[allow(dead_code)]
pub fn load_session(session_name: &str) -> Result<Option<Session>, PawError> {
    load_session_from(session_name, &sessions_dir()?)
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
// Internal implementations (accept dir for testability)
// ---------------------------------------------------------------------------

fn save_session_in(session: &Session, dir: &Path) -> Result<(), PawError> {
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

#[allow(dead_code)]
fn load_session_from(session_name: &str, dir: &Path) -> Result<Option<Session>, PawError> {
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

fn find_session_for_repo_in(repo_path: &Path, dir: &Path) -> Result<Option<Session>, PawError> {
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

fn delete_session_in(session_name: &str, dir: &Path) -> Result<(), PawError> {
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
// Path helpers
// ---------------------------------------------------------------------------

/// Returns the sessions directory (`~/.local/share/git-paw/sessions/`).
fn sessions_dir() -> Result<PathBuf, PawError> {
    let base = dirs::data_dir().ok_or_else(|| {
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
            created_at: UNIX_EPOCH + Duration::from_secs(1_711_200_000),
            status: SessionStatus::Active,
            worktrees: vec![
                WorktreeEntry {
                    branch: "feature/auth".to_string(),
                    worktree_path: PathBuf::from("/Users/test/code/my-project-feature-auth"),
                    cli: "claude".to_string(),
                },
                WorktreeEntry {
                    branch: "fix/api".to_string(),
                    worktree_path: PathBuf::from("/Users/test/code/my-project-fix-api"),
                    cli: "gemini".to_string(),
                },
                WorktreeEntry {
                    branch: "feature/logging".to_string(),
                    worktree_path: PathBuf::from("/Users/test/code/my-project-feature-logging"),
                    cli: "claude".to_string(),
                },
            ],
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

    // -- Recovery: save → tmux dies → state has everything to reconstruct --

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
}
