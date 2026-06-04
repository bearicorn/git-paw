//! Advisory lock for mutating a live session's branch set.
//!
//! `git paw add` and `git paw remove` both splice panes into / out of a
//! running tmux session and rewrite the session JSON. Two of these running
//! concurrently — or one racing a supervisor sweep that is itself sending
//! keys to panes — can corrupt the grid or the session receipt (design
//! "Risks / Trade-offs: Race: add/remove while a sweep is mutating panes").
//!
//! The mitigation is a single advisory lock file under the repo's `.git-paw/`
//! directory, taken by both subcommands. It is *advisory*: it guards git-paw's
//! own `add`/`remove` invocations against each other, not arbitrary external
//! tmux activity. Acquisition is an atomic `create_new` — the first caller
//! wins, a second concurrent caller gets a clear "operation in progress"
//! error rather than interleaving its mutations. The lock is released (file
//! removed) when the [`SessionLock`] guard drops, including on early-return
//! error paths.

use std::fs::{self, File, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::error::PawError;

/// File name of the add/remove advisory lock within `<repo>/.git-paw/`.
pub const LOCK_FILE_NAME: &str = ".add-remove.lock";

/// Returns the advisory lock path for a repository:
/// `<repo>/.git-paw/.add-remove.lock`.
#[must_use]
pub fn lock_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".git-paw").join(LOCK_FILE_NAME)
}

/// RAII guard for the add/remove advisory lock.
///
/// Acquired with [`SessionLock::acquire`]; the lock file is removed when the
/// guard drops. Hold it for the entire mutate-the-session critical section of
/// `cmd_add` / `cmd_remove`.
#[derive(Debug)]
pub struct SessionLock {
    path: PathBuf,
    // Held only so the underlying handle lives as long as the guard; the file
    // is removed on drop via `path`.
    _file: File,
}

impl SessionLock {
    /// Attempts to acquire the advisory lock for `repo_root`.
    ///
    /// Creates `<repo>/.git-paw/` if needed, then atomically creates the lock
    /// file. Returns [`PawError::SessionError`] with an actionable
    /// "operation in progress" message when the lock is already held (the
    /// file already exists) — the second concurrent `add`/`remove` SHALL see
    /// this rather than proceed.
    pub fn acquire(repo_root: &Path) -> Result<Self, PawError> {
        let path = lock_path(repo_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                PawError::SessionError(format!(
                    "failed to create lock directory {}: {e}",
                    parent.display()
                ))
            })?;
        }

        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => Ok(Self { path, _file: file }),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(PawError::SessionError(format!(
                "another `git paw add` / `git paw remove` operation is in progress for this \
                 repository.\n\
                 \n\
                 Wait for it to finish, then retry. If no such command is running, a previous \
                 invocation crashed mid-operation — remove the stale lock and retry:\n  \
                 rm {}",
                path.display()
            ))),
            Err(e) => Err(PawError::SessionError(format!(
                "failed to acquire session lock {}: {e}",
                path.display()
            ))),
        }
    }
}

impl Drop for SessionLock {
    fn drop(&mut self) {
        // Best-effort release; a leftover lock surfaces the actionable
        // "remove the stale lock" hint on the next acquire.
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn lock_path_is_under_git_paw_dir() {
        let repo = TempDir::new().unwrap();
        let p = lock_path(repo.path());
        assert_eq!(p, repo.path().join(".git-paw").join(".add-remove.lock"));
    }

    #[test]
    fn acquire_creates_the_lock_file() {
        let repo = TempDir::new().unwrap();
        let _guard = SessionLock::acquire(repo.path()).expect("first acquire should succeed");
        assert!(
            lock_path(repo.path()).exists(),
            "lock file should exist while the guard is held"
        );
    }

    #[test]
    fn second_concurrent_acquire_errors_with_in_progress_message() {
        let repo = TempDir::new().unwrap();
        let _guard = SessionLock::acquire(repo.path()).expect("first acquire should succeed");

        let err = SessionLock::acquire(repo.path())
            .expect_err("second concurrent acquire must fail while the first is held");
        let msg = err.to_string();
        assert!(
            msg.contains("in progress"),
            "second acquire should report an operation in progress; got: {msg}"
        );
        assert!(
            msg.contains(".add-remove.lock"),
            "error should name the lock file so a stale lock can be removed; got: {msg}"
        );
    }

    #[test]
    fn lock_is_released_on_drop_allowing_reacquire() {
        let repo = TempDir::new().unwrap();
        {
            let _guard = SessionLock::acquire(repo.path()).expect("acquire");
        }
        assert!(
            !lock_path(repo.path()).exists(),
            "lock file should be removed when the guard drops"
        );
        // A fresh acquire after release must succeed — serialized, not blocked.
        let _again = SessionLock::acquire(repo.path())
            .expect("re-acquire after the previous guard dropped should succeed");
    }

    #[test]
    fn acquire_creates_git_paw_dir_when_absent() {
        let repo = TempDir::new().unwrap();
        // No .git-paw/ yet.
        assert!(!repo.path().join(".git-paw").exists());
        let _guard = SessionLock::acquire(repo.path()).expect("acquire should create .git-paw/");
        assert!(repo.path().join(".git-paw").is_dir());
    }
}
