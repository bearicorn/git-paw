//! Domain newtypes for injection-prone constructed strings.
//!
//! git-paw builds three kinds of string from user-influenced input and feeds
//! them to `tmux`, `git`, and the filesystem: the tmux session name, the
//! branch-derived worktree slug, and the worktree path. [`SessionName`],
//! [`BranchSlug`], and [`WorktreePath`] centralise that construction in one
//! place.
//!
//! **This change is a construction *seam* only.** Each constructor's output is
//! **byte-identical** to the previous inline construction for every current
//! input — no space/dot/quote sanitisation is added here, because adding it
//! would be an observable behaviour change, and "a behaviour change is not a
//! refactor". `path-injection-hardening` later hardens these constructors in
//! this one place (sanitise/quote-at-construction) without having to hunt down
//! scattered `format!` sites. Keeping the seam and the hardening separate is
//! deliberate (design D3).

use std::fmt;
use std::path::{Path, PathBuf};

/// A tmux session name — the string git-paw passes to `tmux … -t <session>`.
///
/// Constructed as `paw-<project>` (plus an optional numeric collision suffix),
/// byte-identical to the previous inline `format!("paw-{project}")`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionName(String);

impl SessionName {
    /// The base session name for `project`: `paw-<project>`.
    ///
    /// Byte-identical to the previous inline `format!("paw-{project}")`; no
    /// sanitisation is applied (see the module docs).
    #[must_use]
    pub fn for_project(project: &str) -> Self {
        Self(format!("paw-{project}"))
    }

    /// This name with a numeric collision suffix appended: `<base>-<n>`.
    ///
    /// Takes any `Display` value so it is byte-identical to the previous inline
    /// `format!("{base}-{suffix}")` regardless of the loop counter's integer
    /// type.
    #[must_use]
    pub fn with_collision_suffix(&self, n: impl fmt::Display) -> Self {
        Self(format!("{}-{n}", self.0))
    }

    /// Borrow the name as a `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the newtype, returning the owned name.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for SessionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A filesystem-safe slug derived from a branch name for a child-layout
/// worktree directory.
///
/// Byte-identical to the previous free `git::branch_slug`: `/` becomes `-`,
/// characters in `[A-Za-z0-9._-]` are kept, everything else is dropped. Thus
/// `feat/auth-flow` → `feat-auth-flow` and `fix/issue#42` → `fix-issue42`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchSlug(String);

impl BranchSlug {
    /// Derive the slug for `branch`. Byte-identical to the previous
    /// `git::branch_slug` (see the type docs); no additional sanitisation.
    #[must_use]
    pub fn for_branch(branch: &str) -> Self {
        Self(
            branch
                .chars()
                .filter_map(|c| match c {
                    '/' => Some('-'),
                    'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '-' => Some(c),
                    _ => None,
                })
                .collect(),
        )
    }

    /// Borrow the slug as a `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the newtype, returning the owned slug.
    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for BranchSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The path of a git worktree directory git-paw creates or manages.
///
/// Wraps the already-resolved [`PathBuf`] produced by the placement logic
/// (`.git-paw/worktrees/<slug>` for the child layout, `<parent>/<project>-<branch>`
/// for the sibling layout). Byte-identical to the previous raw `PathBuf`; this
/// is the construction seam `path-injection-hardening` later hardens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreePath(PathBuf);

impl WorktreePath {
    /// Wrap an already-resolved worktree path.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    /// Borrow the path as a `&Path`.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Consume the newtype, returning the owned path.
    #[must_use]
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_name_is_paw_prefixed_verbatim() {
        assert_eq!(
            SessionName::for_project("my-project").as_str(),
            "paw-my-project"
        );
        // No sanitisation: a space/dot passes through unchanged (byte-identical
        // to the previous inline construction; hardening is #10's job).
        assert_eq!(SessionName::for_project("a b.c").as_str(), "paw-a b.c");
    }

    #[test]
    fn session_name_collision_suffix_matches_inline_format() {
        let base = SessionName::for_project("proj");
        assert_eq!(base.with_collision_suffix(2).as_str(), "paw-proj-2");
        assert_eq!(base.with_collision_suffix(7).into_string(), "paw-proj-7");
    }

    #[test]
    fn branch_slug_matches_previous_free_function() {
        // Mirrors the existing `git::branch_slug` unit tests verbatim.
        assert_eq!(
            BranchSlug::for_branch("feat/auth-flow").as_str(),
            "feat-auth-flow"
        );
        assert_eq!(BranchSlug::for_branch("a/b/c").as_str(), "a-b-c");
        assert_eq!(
            BranchSlug::for_branch("fix/issue#42").as_str(),
            "fix-issue42"
        );
        assert_eq!(
            BranchSlug::for_branch("release/v1.2_rc-3").as_str(),
            "release-v1.2_rc-3"
        );
    }

    #[test]
    fn worktree_path_round_trips_the_wrapped_path() {
        let p = PathBuf::from("/tmp/proj/.git-paw/worktrees/feat-x");
        let wt = WorktreePath::new(p.clone());
        assert_eq!(wt.as_path(), p.as_path());
        assert_eq!(wt.into_path_buf(), p);
    }
}
