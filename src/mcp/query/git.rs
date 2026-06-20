//! Git-context reads — thin wrappers over `git` invoked against the resolved
//! repository root. These always work (the repo always exists), so there is
//! no degradation path here.

use std::path::Path;
use std::process::Command;

use rmcp::schemars;
use serde::Serialize;

/// One local branch.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Branch {
    /// Branch name.
    pub name: String,
    /// Head commit SHA.
    pub head: String,
    /// Whether this is the currently checked-out branch (in the main worktree).
    pub current: bool,
    /// Whether the branch is checked out in a linked worktree (git-paw managed
    /// agent branches live in linked worktrees).
    pub worktree: bool,
}

/// One commit.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Commit {
    /// Full commit SHA.
    pub sha: String,
    /// Author name.
    pub author: String,
    /// Author date, ISO-8601.
    pub timestamp: String,
    /// Subject line.
    pub subject: String,
}

/// Diff summary for a branch against its base.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct Diff {
    /// Base branch the diff was taken against.
    pub base: String,
    /// Branch the diff describes.
    pub branch: String,
    /// Unified diff text.
    pub diff: String,
    /// Number of files changed.
    pub files_changed: usize,
    /// Lines added.
    pub insertions: u64,
    /// Lines deleted.
    pub deletions: u64,
}

fn git(repo_root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Lists local branches with head SHA, current flag, and worktree flag.
#[must_use]
pub fn branches(repo_root: &Path) -> Vec<Branch> {
    // Branches checked out in *linked* worktrees (git-paw managed). The first
    // `git worktree list --porcelain` block is the primary worktree (the repo
    // root itself) — exclude it so `current` and `worktree` stay distinct.
    let main_canon = repo_root.canonicalize().ok();
    let mut worktree_branches: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(raw) = git(repo_root, &["worktree", "list", "--porcelain"]) {
        for block in raw.split("\n\n") {
            let mut path: Option<std::path::PathBuf> = None;
            let mut branch: Option<String> = None;
            for line in block.lines() {
                if let Some(p) = line.strip_prefix("worktree ") {
                    path = Some(std::path::PathBuf::from(p));
                } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                    branch = Some(b.to_string());
                }
            }
            if let (Some(p), Some(b)) = (path, branch) {
                let is_main = p.canonicalize().ok() == main_canon;
                if !is_main {
                    worktree_branches.insert(b);
                }
            }
        }
    }

    let Some(raw) = git(
        repo_root,
        &[
            "for-each-ref",
            "--format=%(HEAD)%00%(refname:short)%00%(objectname)",
            "refs/heads",
        ],
    ) else {
        return Vec::new();
    };

    raw.lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{0}');
            let head_marker = parts.next()?;
            let name = parts.next()?.to_string();
            let head = parts.next().unwrap_or("").to_string();
            Some(Branch {
                current: head_marker.trim() == "*",
                worktree: worktree_branches.contains(&name),
                name,
                head,
            })
        })
        .collect()
}

/// Returns up to `limit` recent commits on `branch`, newest first.
#[must_use]
pub fn recent_commits(repo_root: &Path, branch: &str, limit: usize) -> Vec<Commit> {
    let limit_arg = format!("-{}", limit.max(1));
    let Some(raw) = git(
        repo_root,
        &[
            "log",
            &limit_arg,
            "--format=%H%x1f%an%x1f%aI%x1f%s",
            branch,
            "--",
        ],
    ) else {
        return Vec::new();
    };

    raw.lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            Some(Commit {
                sha: parts.next()?.to_string(),
                author: parts.next().unwrap_or("").to_string(),
                timestamp: parts.next().unwrap_or("").to_string(),
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Returns the diff of `branch` against `base` (default: the repo's default
/// branch, falling back to `main`), with a changed-files summary.
#[must_use]
pub fn diff(repo_root: &Path, branch: &str, base: Option<&str>) -> Diff {
    let base = base
        .map(str::to_string)
        .or_else(|| crate::git::default_branch(repo_root).ok())
        .unwrap_or_else(|| "main".to_string());

    let range = format!("{base}...{branch}");
    let diff = git(repo_root, &["diff", &range]).unwrap_or_default();

    // numstat: "<added>\t<deleted>\t<path>" per file (added/deleted "-" for binary).
    let mut files_changed = 0usize;
    let mut insertions = 0u64;
    let mut deletions = 0u64;
    if let Some(raw) = git(repo_root, &["diff", "--numstat", &range]) {
        for line in raw.lines() {
            let mut parts = line.split('\t');
            let add = parts.next().unwrap_or("0");
            let del = parts.next().unwrap_or("0");
            files_changed += 1;
            insertions += add.parse::<u64>().unwrap_or(0);
            deletions += del.parse::<u64>().unwrap_or(0);
        }
    }

    Diff {
        base,
        branch: branch.to_string(),
        diff,
        files_changed,
        insertions,
        deletions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn init_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            assert!(
                Command::new("git")
                    .current_dir(dir)
                    .args(&args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(dir.join("a.txt"), "one\n").unwrap();
        for args in [vec!["add", "."], vec!["commit", "-q", "-m", "first"]] {
            assert!(
                Command::new("git")
                    .current_dir(dir)
                    .args(&args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        tmp
    }

    #[test]
    fn branches_lists_current_branch() {
        let tmp = init_repo();
        let bs = branches(tmp.path());
        assert_eq!(bs.len(), 1);
        assert_eq!(bs[0].name, "main");
        assert!(bs[0].current);
        assert!(!bs[0].worktree);
        assert!(!bs[0].head.is_empty());
    }

    #[test]
    fn recent_commits_returns_first_commit() {
        let tmp = init_repo();
        let cs = recent_commits(tmp.path(), "main", 10);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].subject, "first");
        assert_eq!(cs[0].author, "Test");
    }

    #[test]
    fn diff_against_base_summarizes_changes() {
        let tmp = init_repo();
        let dir = tmp.path();
        assert!(
            Command::new("git")
                .current_dir(dir)
                .args(["checkout", "-q", "-b", "feat/x"])
                .status()
                .unwrap()
                .success()
        );
        std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
        for args in [vec!["add", "."], vec!["commit", "-q", "-m", "second"]] {
            assert!(
                Command::new("git")
                    .current_dir(dir)
                    .args(&args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let d = diff(dir, "feat/x", Some("main"));
        assert_eq!(d.base, "main");
        assert_eq!(d.files_changed, 1);
        assert_eq!(d.insertions, 1);
        assert!(d.diff.contains("two"));
    }
}
