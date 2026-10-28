//! The repository-write gate and write confinement for MCP tools that write
//! file content into the working tree (the spec-authoring tools).
//!
//! Two layers, both applied before any byte is written:
//!
//! - **Consent** ([`repo_write_access`]): a pure decision over the
//!   startup-resolved root-session flag and the repository's
//!   `[mcp] allow_repo_writes` switch. The server applies it to the tool list
//!   and as the first step of every write tool, so listing and calling agree.
//! - **Confinement** ([`resolve_spec_dir`], [`check_change_segment`],
//!   [`check_artifact_path`], [`write_in_change`]): lexical checks on the
//!   caller's input, then canonical checks that the spec directory lies inside
//!   the repository and outside `.git`, and that no symbolic link on the way
//!   leads out of the change directory. Content lands through a temporary file
//!   renamed into place, so a write never follows a link at the target.
//!
//! Nothing here spawns a process, stages, or commits.

use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::PawError;

/// Why the repository-write tools are unavailable to this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoWriteRefusal {
    /// The server runs in a linked worktree, not the main working tree.
    NotRootSession,
    /// The repository's own config does not set `[mcp] allow_repo_writes`.
    SwitchOff,
}

impl std::fmt::Display for RepoWriteRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotRootSession => f.write_str(
                "repository writes are not available from a worktree session; they are \
                 available only from the repository's main working tree (a root session)",
            ),
            Self::SwitchOff => f.write_str(
                "repository writes are off: set `allow_repo_writes = true` under `[mcp]` in \
                 the repository's .git-paw/config.toml and restart the MCP server",
            ),
        }
    }
}

/// Decides whether the repository-write tools are available: only to a root
/// session of a repository that consented. A worktree session is refused
/// whatever its switch says.
///
/// # Errors
///
/// Returns the [`RepoWriteRefusal`] naming the failed condition.
pub fn repo_write_access(
    is_main_worktree: bool,
    allow_repo_writes: bool,
) -> Result<(), RepoWriteRefusal> {
    if !is_main_worktree {
        Err(RepoWriteRefusal::NotRootSession)
    } else if !allow_repo_writes {
        Err(RepoWriteRefusal::SwitchOff)
    } else {
        Ok(())
    }
}

/// Sequence for temporary file names: unique per process and per call, so
/// concurrent writes never share one.
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn refuse(message: impl Into<String>) -> PawError {
    PawError::McpError(message.into())
}

/// Whether a path component is `.git`, compared case-insensitively so a
/// case-insensitive filesystem cannot alias it.
fn is_git_component(component: &std::ffi::OsStr) -> bool {
    component.eq_ignore_ascii_case(".git")
}

/// Checks that `change` is a single, ordinary path segment.
///
/// # Errors
///
/// Returns a [`PawError::McpError`] naming `change` otherwise.
pub fn check_change_segment(change: &str) -> Result<(), PawError> {
    let mut components = Path::new(change).components();
    let single = matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(c)), None) if !is_git_component(c)
    );
    if single && !change.contains(['/', '\\']) {
        Ok(())
    } else {
        Err(refuse(format!(
            "invalid `change` {change:?}: it must be a single change directory name"
        )))
    }
}

/// Checks that `path` is relative, non-empty, and carries no `..` or `.git`
/// component, returning its ordinary components.
///
/// # Errors
///
/// Returns a [`PawError::McpError`] naming `path` otherwise.
pub fn check_artifact_path(path: &str) -> Result<Vec<String>, PawError> {
    let bad = |why: &str| refuse(format!("invalid `path` {path:?}: {why}"));
    if path.contains('\\') {
        return Err(bad("use forward slashes"));
    }
    let mut parts = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(c) if is_git_component(c) => {
                return Err(bad("a .git component is not allowed"));
            }
            Component::Normal(c) => parts.push(c.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => return Err(bad("`..` is not allowed")),
            Component::RootDir | Component::Prefix(_) => {
                return Err(bad("it must be relative to the change directory"));
            }
        }
    }
    if parts.is_empty() {
        return Err(bad("it names no file"));
    }
    Ok(parts)
}

/// Resolves the configured spec directory to its canonical form, refusing one
/// that lies outside the canonical repository root or inside its `.git`
/// directory, and one that is the root itself or inside `.git-paw` (where a
/// change name could address arbitrary top-level directories or git-paw's
/// config). Returns `(canonical root, canonical spec dir)`.
///
/// # Errors
///
/// Returns a [`PawError::McpError`] when the directory is missing or escapes.
pub fn resolve_spec_dir(root: &Path, configured_dir: &str) -> Result<(PathBuf, PathBuf), PawError> {
    let root = root
        .canonicalize()
        .map_err(|e| refuse(format!("cannot resolve the repository root: {e}")))?;
    let dir = root.join(configured_dir).canonicalize().map_err(|e| {
        refuse(format!(
            "the [specs] directory {configured_dir:?} cannot be resolved: {e}"
        ))
    })?;
    if !dir.starts_with(&root) || dir.starts_with(root.join(".git")) {
        return Err(refuse(format!(
            "the [specs] directory {configured_dir:?} resolves outside the repository working \
             tree; repository writes are refused"
        )));
    }
    // The root itself or git-paw's control directory would let a "change"
    // name an arbitrary top-level directory (`.github`, `src`) or the config.
    if dir == root || dir.starts_with(root.join(".git-paw")) {
        return Err(refuse(format!(
            "the [specs] directory {configured_dir:?} resolves to the repository root or \
             .git-paw; repository writes are refused"
        )));
    }
    if !dir.is_dir() {
        return Err(refuse(format!(
            "the [specs] directory {configured_dir:?} is not a directory"
        )));
    }
    Ok((root, dir))
}

/// Resolves the existing change directory `<spec_dir>/<change>`, refusing a
/// name that is not a single kebab-case segment, a symbolic link, or a
/// non-directory. `spec_dir` must be canonical.
///
/// # Errors
///
/// Returns a [`PawError::McpError`] naming the change when it does not exist.
pub fn existing_change_dir(spec_dir: &Path, change: &str) -> Result<PathBuf, PawError> {
    check_change_segment(change)?;
    if !crate::specs::openspec::is_valid_change_name(change) {
        return Err(refuse(format!(
            "invalid `change` {change:?}: a change name is lowercase letters, digits and hyphens"
        )));
    }
    let dir = spec_dir.join(change);
    match fs::symlink_metadata(&dir) {
        Ok(meta) if meta.is_dir() => Ok(dir),
        Ok(meta) if meta.file_type().is_symlink() => Err(refuse(format!(
            "change {change:?} is a symbolic link; repository writes are refused"
        ))),
        _ => Err(refuse(format!("change {change:?} does not exist"))),
    }
}

/// Writes `content` to `parts` (from [`check_artifact_path`]) inside the
/// canonical change directory `change_dir`, returning whether an existing
/// file was replaced.
///
/// Each existing directory on the way is checked with `symlink_metadata`; a
/// symbolic link is followed only to verify it stays inside `change_dir`, and
/// a missing directory is created one level at a time. A symbolic-link target
/// is refused. The content goes to a temporary file beside the target and is
/// renamed into place.
///
/// # Errors
///
/// Returns a [`PawError::McpError`] naming the offending component, or the
/// I/O failure.
pub fn write_in_change(
    change_dir: &Path,
    parts: &[String],
    content: &str,
) -> Result<bool, PawError> {
    let Some((file_name, dirs)) = parts.split_last() else {
        return Err(refuse("the artifact path names no file"));
    };
    let mut current = change_dir.to_path_buf();
    for dir in dirs {
        let next = current.join(dir);
        match fs::symlink_metadata(&next) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let resolved = next.canonicalize().map_err(|e| {
                    refuse(format!("cannot resolve the symbolic link {dir:?}: {e}"))
                })?;
                if !resolved.starts_with(change_dir) || !resolved.is_dir() {
                    return Err(refuse(format!(
                        "{dir:?} is a symbolic link leading outside the change directory; \
                         repository writes are refused"
                    )));
                }
                current = resolved;
                continue;
            }
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(refuse(format!("{dir:?} exists and is not a directory"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&next)
                    .map_err(|e| refuse(format!("cannot create directory {dir:?}: {e}")))?;
            }
            Err(e) => return Err(refuse(format!("cannot inspect {dir:?}: {e}"))),
        }
        current = next;
    }

    let target = current.join(file_name);
    let replaced = match fs::symlink_metadata(&target) {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(refuse(format!(
                "{file_name:?} is a symbolic link; repository writes are refused"
            )));
        }
        Ok(meta) if meta.is_file() => true,
        Ok(_) => return Err(refuse(format!("{file_name:?} is not a regular file"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(refuse(format!("cannot inspect {file_name:?}: {e}"))),
    };

    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = current.join(format!(
        ".{file_name}.git-paw-{}-{seq}.tmp",
        std::process::id()
    ));
    let written = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| f.write_all(content.as_bytes()))
        .and_then(|()| fs::rename(&tmp, &target));
    if let Err(e) = written {
        let _ = fs::remove_file(&tmp);
        return Err(refuse(format!("cannot write {file_name:?}: {e}")));
    }
    Ok(replaced)
}

#[cfg(test)]
mod tests {
    use super::*;

    // design D2: one decision over the four (root × switch) cases.
    #[test]
    fn access_table_over_root_and_switch() {
        let cases = [
            (true, true, Ok(())),
            (true, false, Err(RepoWriteRefusal::SwitchOff)),
            (false, true, Err(RepoWriteRefusal::NotRootSession)),
            (false, false, Err(RepoWriteRefusal::NotRootSession)),
        ];
        for (root, switch, expected) in cases {
            assert_eq!(repo_write_access(root, switch), expected, "{root} {switch}");
        }
    }

    #[test]
    fn refusal_messages_name_the_switch_file_or_worktree() {
        let off = RepoWriteRefusal::SwitchOff.to_string();
        assert!(off.contains("allow_repo_writes"), "{off}");
        assert!(off.contains(".git-paw/config.toml"), "{off}");
        assert!(off.contains("restart"), "{off}");
        let wt = RepoWriteRefusal::NotRootSession.to_string();
        assert!(wt.contains("worktree"), "{wt}");
    }

    #[test]
    fn change_segment_table() {
        for ok in ["add-export", "a1", "x.y"] {
            assert!(check_change_segment(ok).is_ok(), "{ok}");
        }
        for bad in ["", "..", ".", "a/b", "../x", "/abs", ".git", "a\\b"] {
            assert!(check_change_segment(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn artifact_path_table() {
        assert_eq!(
            check_artifact_path("specs/data-export/spec.md").unwrap(),
            ["specs", "data-export", "spec.md"]
        );
        assert_eq!(check_artifact_path("./tasks.md").unwrap(), ["tasks.md"]);
        for bad in [
            "",
            ".",
            "../../../src/main.rs",
            "specs/../../x",
            "/tmp/x.md",
            ".git/hooks/pre-commit",
            "specs/.GIT/x",
            "a\\b.md",
        ] {
            assert!(check_artifact_path(bad).is_err(), "{bad:?}");
        }
    }
}
