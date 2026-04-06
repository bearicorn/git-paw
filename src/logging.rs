//! Session logging via tmux pipe-pane.
//!
//! Captures per-pane terminal output to `.git-paw/logs/<session>/<branch>.log`.
//! Provides log directory management and session/log enumeration.

use std::path::{Path, PathBuf};

use crate::error::PawError;

/// A single log file entry, pairing a branch name with its log path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    /// The original branch name (e.g. `feat/add-auth`), recovered from the sanitized filename.
    pub branch: String,
    /// Absolute path to the log file.
    pub path: PathBuf,
}

/// Replace `/` with `--` so the branch name is safe for use as a filename.
pub fn sanitize_branch_for_filename(branch: &str) -> String {
    branch.replace('/', "--")
}

/// Reverse [`sanitize_branch_for_filename`]: replace `--` with `/` and strip the `.log` suffix.
pub fn unsanitize_branch_from_filename(filename: &str) -> String {
    let stem = filename.strip_suffix(".log").unwrap_or(filename);
    stem.replace("--", "/")
}

/// Build the full log file path for a branch within a session.
///
/// Returns `<repo_root>/.git-paw/logs/<session_id>/<sanitized-branch>.log`.
pub fn log_file_path(repo_root: &Path, session_id: &str, branch: &str) -> PathBuf {
    repo_root
        .join(".git-paw")
        .join("logs")
        .join(session_id)
        .join(format!("{}.log", sanitize_branch_for_filename(branch)))
}

/// Create the session log directory, returning its path.
///
/// Creates `.git-paw/logs/<session_id>/` under `repo_root`. Idempotent — succeeds
/// if the directory already exists.
pub fn ensure_log_dir(repo_root: &Path, session_id: &str) -> Result<PathBuf, PawError> {
    let dir = repo_root.join(".git-paw").join("logs").join(session_id);
    std::fs::create_dir_all(&dir).map_err(|e| {
        PawError::SessionError(format!(
            "failed to create log directory {}: {e}",
            dir.display()
        ))
    })?;
    Ok(dir)
}

/// Returns the logs directory path (`.git-paw/logs/`) under the given repo root.
pub fn logs_dir(repo_root: &Path) -> PathBuf {
    repo_root.join(".git-paw").join("logs")
}

/// List all session log directories under `.git-paw/logs/`.
///
/// Returns an empty list if the logs directory does not exist.
pub fn list_log_sessions(repo_root: &Path) -> Result<Vec<String>, PawError> {
    let logs_dir = repo_root.join(".git-paw").join("logs");
    if !logs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    let entries = std::fs::read_dir(&logs_dir)
        .map_err(|e| PawError::SessionError(format!("failed to read logs directory: {e}")))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| PawError::SessionError(format!("failed to read directory entry: {e}")))?;
        if entry.path().is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            sessions.push(name.to_owned());
        }
    }

    sessions.sort();
    Ok(sessions)
}

/// List log files within a session directory, returning [`LogEntry`] items.
///
/// Returns `PawError::SessionError` if the session directory does not exist.
pub fn list_logs_for_session(repo_root: &Path, session: &str) -> Result<Vec<LogEntry>, PawError> {
    let session_dir = repo_root.join(".git-paw").join("logs").join(session);
    if !session_dir.exists() {
        return Err(PawError::SessionError(format!(
            "session directory not found: {session}"
        )));
    }

    let mut entries = Vec::new();
    let dir_entries = std::fs::read_dir(&session_dir)
        .map_err(|e| PawError::SessionError(format!("failed to read session directory: {e}")))?;

    for entry in dir_entries {
        let entry = entry
            .map_err(|e| PawError::SessionError(format!("failed to read directory entry: {e}")))?;
        let path = entry.path();
        if path.is_file()
            && let Some(filename) = path.file_name().and_then(|f| f.to_str())
            && Path::new(filename)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
        {
            entries.push(LogEntry {
                branch: unsanitize_branch_from_filename(filename),
                path,
            });
        }
    }

    entries.sort_by(|a, b| a.branch.cmp(&b.branch));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // -- sanitize / unsanitize ------------------------------------------------

    #[test]
    fn sanitize_simple_name() {
        assert_eq!(sanitize_branch_for_filename("add-auth"), "add-auth");
    }

    #[test]
    fn sanitize_single_slash() {
        assert_eq!(
            sanitize_branch_for_filename("feat/add-auth"),
            "feat--add-auth"
        );
    }

    #[test]
    fn sanitize_multiple_slashes() {
        assert_eq!(
            sanitize_branch_for_filename("feat/auth/jwt"),
            "feat--auth--jwt"
        );
    }

    #[test]
    fn unsanitize_simple_name() {
        assert_eq!(unsanitize_branch_from_filename("add-auth.log"), "add-auth");
    }

    #[test]
    fn unsanitize_single_slash() {
        assert_eq!(
            unsanitize_branch_from_filename("feat--add-auth.log"),
            "feat/add-auth"
        );
    }

    #[test]
    fn unsanitize_multiple_slashes() {
        assert_eq!(
            unsanitize_branch_from_filename("feat--auth--jwt.log"),
            "feat/auth/jwt"
        );
    }

    // -- log_file_path --------------------------------------------------------

    #[test]
    fn log_file_path_produces_correct_structure() {
        let path = log_file_path(Path::new("/repo"), "paw-myproject", "feat/add-auth");
        assert_eq!(
            path,
            PathBuf::from("/repo/.git-paw/logs/paw-myproject/feat--add-auth.log")
        );
    }

    // -- ensure_log_dir -------------------------------------------------------

    #[test]
    fn ensure_log_dir_creates_directory() {
        let tmp = TempDir::new().unwrap();
        let dir = ensure_log_dir(tmp.path(), "paw-test").unwrap();
        assert!(dir.is_dir());
        assert_eq!(dir, tmp.path().join(".git-paw/logs/paw-test"));
    }

    #[test]
    fn ensure_log_dir_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let first = ensure_log_dir(tmp.path(), "paw-test").unwrap();
        let second = ensure_log_dir(tmp.path(), "paw-test").unwrap();
        assert_eq!(first, second);
        assert!(second.is_dir());
    }

    // -- list_log_sessions ----------------------------------------------------

    #[test]
    fn list_log_sessions_returns_sessions() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git-paw/logs/paw-myproject")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".git-paw/logs/paw-other")).unwrap();

        let sessions = list_log_sessions(tmp.path()).unwrap();
        assert_eq!(sessions, vec!["paw-myproject", "paw-other"]);
    }

    #[test]
    fn list_log_sessions_returns_empty_when_no_sessions() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git-paw/logs")).unwrap();

        let sessions = list_log_sessions(tmp.path()).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn list_log_sessions_returns_empty_when_no_logs_dir() {
        let tmp = TempDir::new().unwrap();
        let sessions = list_log_sessions(tmp.path()).unwrap();
        assert!(sessions.is_empty());
    }

    // -- list_logs_for_session ------------------------------------------------

    #[test]
    fn list_logs_for_session_returns_entries() {
        let tmp = TempDir::new().unwrap();
        let session_dir = tmp.path().join(".git-paw/logs/paw-test");
        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::write(session_dir.join("main.log"), "").unwrap();
        std::fs::write(session_dir.join("feat--auth.log"), "").unwrap();
        std::fs::write(session_dir.join("feat--api--v2.log"), "").unwrap();

        let entries = list_logs_for_session(tmp.path(), "paw-test").unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].branch, "feat/api/v2");
        assert_eq!(entries[1].branch, "feat/auth");
        assert_eq!(entries[2].branch, "main");
    }

    #[test]
    fn list_logs_for_session_returns_empty_when_no_logs() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git-paw/logs/paw-test")).unwrap();

        let entries = list_logs_for_session(tmp.path(), "paw-test").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn list_logs_for_session_errors_when_session_missing() {
        let tmp = TempDir::new().unwrap();
        let result = list_logs_for_session(tmp.path(), "paw-nonexistent");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("paw-nonexistent"));
    }

    // -- LogEntry branch derivation -------------------------------------------

    #[test]
    fn log_entry_branch_from_sanitized_filename() {
        let entry = LogEntry {
            branch: unsanitize_branch_from_filename("feat--add-auth.log"),
            path: PathBuf::from("/repo/.git-paw/logs/paw-test/feat--add-auth.log"),
        };
        assert_eq!(entry.branch, "feat/add-auth");
    }
}
