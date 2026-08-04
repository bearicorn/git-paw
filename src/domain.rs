//! Domain newtypes for injection-prone constructed strings.
//!
//! git-paw builds three kinds of string from user-influenced input and feeds
//! them to `tmux`, `git`, and the filesystem: the tmux session name, the
//! branch-derived worktree slug, and the worktree path. [`SessionName`],
//! [`BranchSlug`], and [`WorktreePath`] centralise that construction in one
//! place.
//!
//! The seam was introduced first (`code-analysis-refactor`) with
//! byte-identical output, then hardened here by `path-injection-hardening`:
//! [`SessionName::from_project`] sanitises the project name to a tmux-safe
//! slug and [`shell_quote`] quotes a path before it is interpolated into a
//! `/bin/sh -c` body or a command typed into a pane. Sanitising at
//! construction is what stops downstream code from interpolating a raw
//! untrusted string; for a well-formed input every constructor's output is
//! unchanged from the seam's.

use std::fmt;
use std::path::{Path, PathBuf};

/// Quote `s` so a POSIX shell reads it as one literal word.
///
/// The string is wrapped in single quotes and any embedded single quote is
/// escaped as `'\''` (close the quote, emit an escaped quote, reopen), which
/// makes every other character — spaces, `>`, `;`, `$`, `` ` `` — literal. Use
/// this for every path or argument interpolated into a shell command body,
/// whether that body is handed to `/bin/sh -c` (tmux's `pipe-pane`) or typed
/// into a pane's shell (`send-keys`).
///
/// The quoted form is behaviour-equivalent to the bare string for a path with
/// no special characters: the shell strips the quotes and the same file is
/// addressed.
///
/// ```
/// use git_paw::domain::shell_quote;
///
/// assert_eq!(shell_quote("/repo/My Project/x.log"), "'/repo/My Project/x.log'");
/// assert_eq!(shell_quote("/repo/it's/x.log"), r"'/repo/it'\''s/x.log'");
/// ```
#[must_use]
pub fn shell_quote(s: &str) -> String {
    let mut quoted = String::with_capacity(s.len() + 2);
    quoted.push('\'');
    for c in s.chars() {
        if c == '\'' {
            quoted.push_str(r"'\''");
        } else {
            quoted.push(c);
        }
    }
    quoted.push('\'');
    quoted
}

/// The slug used when sanitising a project name leaves nothing usable.
///
/// Matches [`crate::git::project_name`]'s own fallback for a repository
/// directory whose name cannot be read, so an unnameable directory keeps
/// producing the same `paw-unknown` session name it does today.
const PROJECT_SLUG_FALLBACK: &str = "unknown";

/// Sanitise a project name into a slug that is safe inside a tmux target.
///
/// ASCII letters, digits, `_`, and `-` are kept verbatim — case included,
/// since tmux session names are case-sensitive and case is not a target
/// separator. Every other character is a separator: `.` and `:` (tmux's own
/// window and pane separators) and whitespace all become `-`, a run of them
/// collapses to a single `-`, and a leading or trailing run is dropped. So
/// `my.app` → `my-app`, `My Project` → `My-Project`, and `my..app` → `my-app`.
///
/// A name made only of already-safe characters is returned unchanged (so
/// `git-paw` → `git-paw` and even `my--app` keeps both dashes), which is what
/// keeps the derived session name byte-identical for well-formed inputs. A
/// name that sanitises to nothing yields [`PROJECT_SLUG_FALLBACK`].
#[must_use]
pub fn project_slug(project: &str) -> String {
    let mut slug = String::with_capacity(project.len());
    // Set when one or more separator characters have been seen but not yet
    // emitted; a pending run collapses to one `-`, and a leading or trailing
    // run is never emitted at all.
    let mut pending_separator = false;
    for c in project.chars() {
        if matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-') {
            if pending_separator && !slug.is_empty() {
                slug.push('-');
            }
            pending_separator = false;
            slug.push(c);
        } else {
            pending_separator = true;
        }
    }

    if slug.is_empty() {
        PROJECT_SLUG_FALLBACK.to_owned()
    } else {
        slug
    }
}

/// A tmux session name — the string git-paw passes to `tmux … -t <session>`.
///
/// Constructed as `paw-<slug>` (plus an optional numeric collision suffix)
/// where `<slug>` is [`project_slug`]'s tmux-safe form of the repository
/// directory name. A `SessionName` therefore never holds a name that tmux
/// would reject or mis-parse: an unsanitised `.` would make tmux read
/// `paw-my.app` as session `paw-my` plus pane `app`, breaking every scoped
/// pane command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionName(String);

impl SessionName {
    /// The base session name for `project`: `paw-<slug>`.
    ///
    /// This is the only way to build a `SessionName` from a project name, so
    /// [`project_slug`]'s sanitisation cannot be bypassed. For a project name
    /// of only `[A-Za-z0-9_-]` the result is byte-identical to the
    /// pre-hardening `paw-<project>`.
    #[must_use]
    pub fn from_project(project: &str) -> Self {
        Self(format!("paw-{}", project_slug(project)))
    }

    /// This name with a numeric collision suffix appended: `<base>-<n>`.
    ///
    /// The suffix is appended to the already-sanitised base, so the whole name
    /// stays a valid tmux target.
    #[must_use]
    pub fn with_collision_suffix(&self, n: u32) -> Self {
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

impl AsRef<str> for SessionName {
    fn as_ref(&self) -> &str {
        &self.0
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
    fn session_name_sanitises_the_project_into_a_tmux_safe_target() {
        for (project, expected) in [
            // Characters tmux reserves inside a target, and whitespace.
            ("my.app", "paw-my-app"),
            ("My Project", "paw-My-Project"),
            ("a:b", "paw-a-b"),
            ("my..app", "paw-my-app"),
            (".leading", "paw-leading"),
            ("trailing.", "paw-trailing"),
            // Already-safe names are byte-identical to the pre-hardening form.
            ("git-paw", "paw-git-paw"),
            ("my-project", "paw-my-project"),
            ("my--app", "paw-my--app"),
            ("snake_case9", "paw-snake_case9"),
            // Nothing usable left => the `project_name` fallback.
            ("", "paw-unknown"),
            ("...", "paw-unknown"),
        ] {
            let name = SessionName::from_project(project);
            assert_eq!(name.as_str(), expected, "project: {project:?}");
            assert!(
                !name
                    .as_str()
                    .contains(|c: char| c == '.' || c == ':' || c.is_whitespace()),
                "sanitised name {name} still holds a tmux target separator"
            );
        }
    }

    #[test]
    fn session_name_collision_suffix_appends_to_the_sanitised_base() {
        let base = SessionName::from_project("my.app");
        assert_eq!(base.with_collision_suffix(2).as_str(), "paw-my-app-2");
        assert_eq!(base.with_collision_suffix(7).into_string(), "paw-my-app-7");
    }

    #[test]
    fn shell_quote_makes_every_input_one_literal_shell_word() {
        for (raw, expected) in [
            // A plain path is behaviour-equivalent: the shell strips the quotes.
            ("/repo/logs/main.log", "'/repo/logs/main.log'"),
            // Spaces and metacharacters become literal.
            ("/repo/My Project/x.log", "'/repo/My Project/x.log'"),
            ("/repo/a;rm -rf b/x.log", "'/repo/a;rm -rf b/x.log'"),
            ("/repo/$HOME/`id`/x.log", "'/repo/$HOME/`id`/x.log'"),
            // An embedded single quote closes, escapes, and reopens.
            ("/repo/it's/x.log", r"'/repo/it'\''s/x.log'"),
            ("", "''"),
        ] {
            assert_eq!(shell_quote(raw), expected, "input: {raw:?}");
        }
    }

    #[test]
    fn shell_quote_survives_a_real_shell_round_trip() {
        // The quoted form must reach `/bin/sh` as exactly one argument, byte
        // for byte — this is the property `pipe-pane` and the pane-typed
        // dashboard command both depend on.
        for raw in [
            "/repo/logs/main.log",
            "/repo/My Project/x.log",
            "/repo/it's/x.log",
            "/repo/a;rm -rf b/x.log",
            "/repo/$HOME/`id`/x.log",
        ] {
            let out = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(format!("printf %s {}", shell_quote(raw)))
                .output()
                .expect("run /bin/sh");
            assert!(out.status.success(), "sh failed for {raw:?}");
            assert_eq!(
                String::from_utf8_lossy(&out.stdout),
                raw,
                "quoted form did not round-trip through the shell: {raw:?}"
            );
        }
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
