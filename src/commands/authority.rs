//! `git paw __authority` — the root session's broker authority credential.
//!
//! The root-side helper verbs that publish a verdict (`sweep.sh verified`,
//! `feedback-gate`, `conflict-ack`) read the credential through this hidden
//! subcommand rather than the user-data session file, so the path resolution
//! stays in Rust. It refuses from a linked worktree — every coding agent runs
//! in one — as defence in depth; the boundary itself is the credential living
//! outside the repository.

use std::path::Path;

use crate::command_runner::CommandRunner;
use crate::error::PawError;

/// Returns the broker authority credential of the session for the
/// repository whose main working tree contains `cwd`.
///
/// The caller prints it; this function writes nothing, so the lookup is
/// reusable from library code.
///
/// # Errors
/// Returns an error when `cwd` is inside a linked worktree (decided as git
/// reports it: the checkout's git directory differs from the common one,
/// which also holds under `--separate-git-dir`), when `cwd` is not in a
/// repository, or when no session with a broker credential exists for the
/// repository.
pub fn authority(runner: &dyn CommandRunner, cwd: &Path) -> Result<String, PawError> {
    let layout = crate::cli_setup::shell::resolve_layout(runner, cwd)?;
    if layout.is_linked_worktree() {
        return Err(PawError::SessionError(
            "the broker authority credential is available only from the repository's main \
             working tree, not from a linked worktree"
                .to_string(),
        ));
    }
    let repo_root = crate::git::validate_repo(cwd)?;
    crate::session::authority_for_repo(&repo_root)?.ok_or_else(|| {
        PawError::SessionError(
            "no session with a broker for this repository; run `git paw start` first".to_string(),
        )
    })
}
