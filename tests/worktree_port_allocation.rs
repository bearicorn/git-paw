//! Per-worktree port allocation integration tests (`session-runtime-isolation`).
//!
//! Covers the spec requirement *Per-worktree port allocation writes a generated
//! `.env.local`*: the first worktree receives the base block, subsequent
//! worktrees receive non-overlapping blocks, the copied `.env` is never
//! mutated, and a removed worktree's block is freed for reuse.
//!
//! Each test creates real git worktrees under a `tempfile` sandbox and drives
//! the provisioning seam the add/start command flows call, so the assertions
//! are on the files an agent would actually read.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use tempfile::TempDir;

use git_paw::config::{WorktreeConfig, WorktreeEnvConfig, WorktreePortsConfig};
use git_paw::git;
use git_paw::session::WorktreeEntry;
use git_paw::worktree_provision::{ENV_LOCAL_FILE, allocate_slot, provision_worktree};

struct Sandbox {
    _dir: TempDir,
    repo: PathBuf,
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = StdCommand::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("run git command");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn sandbox_repo() -> Sandbox {
    let dir = TempDir::new().expect("create temp dir");
    let repo = dir.path().join("repo");
    fs::create_dir_all(&repo).expect("create repo dir");

    run_git(&repo, &["init", "-q", "-b", "main"]);
    run_git(&repo, &["config", "user.email", "test@test.com"]);
    run_git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("README.md"), "# test").expect("write README");
    run_git(&repo, &["add", "."]);
    run_git(&repo, &["commit", "-q", "-m", "initial"]);

    Sandbox { _dir: dir, repo }
}

fn create_worktree(repo: &Path, branch: &str) -> PathBuf {
    git::create_worktree(
        repo,
        branch,
        false,
        git_paw::config::WorktreePlacement::Sibling,
    )
    .expect("create worktree")
    .path
}

/// `[worktree.ports] base = 3000, stride = 10, vars = ["PORT", "VITE_PORT"]` —
/// the configuration the spec scenarios are written against.
fn spec_ports() -> WorktreeConfig {
    WorktreeConfig {
        env: None,
        ports: Some(WorktreePortsConfig {
            base: 3000,
            stride: 10,
            vars: vec!["PORT".to_string(), "VITE_PORT".to_string()],
        }),
        hooks: None,
    }
}

/// Parses `VAR=value` lines out of a worktree's generated `.env.local`.
fn env_local_lines(worktree: &Path) -> Vec<String> {
    fs::read_to_string(worktree.join(ENV_LOCAL_FILE))
        .unwrap_or_else(|e| panic!("read {ENV_LOCAL_FILE} in {}: {e}", worktree.display()))
        .lines()
        .filter(|l| l.contains('=') && !l.trim_start().starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn entry(branch: &str, path: &Path, slot: Option<u16>) -> WorktreeEntry {
    WorktreeEntry {
        branch: branch.to_string(),
        worktree_path: path.to_path_buf(),
        cli: "claude".to_string(),
        branch_created: true,
        pending_boot_prompt: None,
        runtime_slot: slot,
    }
}

// --- Scenario: First worktree receives the base port block ---

#[test]
fn first_worktree_receives_the_base_port_block() {
    let sb = sandbox_repo();
    let worktree = create_worktree(&sb.repo, "feature/one");

    let result = provision_worktree(&sb.repo, &worktree, "feat-x", &spec_ports(), 0).unwrap();

    assert_eq!(result.runtime_slot, Some(0));
    assert_eq!(
        env_local_lines(&worktree),
        vec!["PORT=3000".to_string(), "VITE_PORT=3001".to_string()]
    );
}

// --- Scenario: Subsequent worktrees receive non-overlapping blocks ---

#[test]
fn subsequent_worktrees_receive_non_overlapping_blocks() {
    let sb = sandbox_repo();
    let config = spec_ports();
    let first = create_worktree(&sb.repo, "feature/one");
    let second = create_worktree(&sb.repo, "feature/two");

    provision_worktree(&sb.repo, &first, "feat-x", &config, 0).unwrap();
    provision_worktree(&sb.repo, &second, "feat-x", &config, 1).unwrap();

    assert_eq!(
        env_local_lines(&second),
        vec!["PORT=3010".to_string(), "VITE_PORT=3011".to_string()]
    );

    let first_ports = env_local_lines(&first);
    for line in env_local_lines(&second) {
        let port = line.split('=').next_back().expect("port value").to_string();
        assert!(
            !first_ports.iter().any(|l| l.ends_with(&format!("={port}"))),
            "port {port} collides with the first worktree's block {first_ports:?}"
        );
    }
}

// --- Scenario: Port allocation does not mutate the copied env file ---

#[test]
fn port_allocation_does_not_mutate_the_copied_env_file() {
    let sb = sandbox_repo();
    fs::write(sb.repo.join(".env"), "PORT=3000\nSECRET=abc\n").unwrap();
    let worktree = create_worktree(&sb.repo, "feature/one");

    let config = WorktreeConfig {
        env: Some(WorktreeEnvConfig {
            copy: vec![".env".to_string()],
        }),
        ports: spec_ports().ports,
        hooks: None,
    };
    provision_worktree(&sb.repo, &worktree, "feat-x", &config, 1).unwrap();

    assert_eq!(
        fs::read(worktree.join(".env")).unwrap(),
        fs::read(sb.repo.join(".env")).unwrap(),
        "the copied .env stays byte-for-byte identical to the source"
    );
    let local = env_local_lines(&worktree);
    assert_eq!(
        local,
        vec!["PORT=3010".to_string(), "VITE_PORT=3011".to_string()],
        "the offset ports appear only in {ENV_LOCAL_FILE}"
    );
}

// --- Scenario: A removed worktree's port block is freed for reuse ---

#[test]
fn removed_worktree_block_is_freed_and_reused_without_collision() {
    let sb = sandbox_repo();
    let config = spec_ports();

    // Three agents take slots 0, 1, 2.
    let mut roster: Vec<WorktreeEntry> = Vec::new();
    for branch in ["feature/one", "feature/two", "feature/three"] {
        let path = create_worktree(&sb.repo, branch);
        let slot = allocate_slot(&roster);
        let result = provision_worktree(&sb.repo, &path, "feat-x", &config, slot).unwrap();
        roster.push(entry(branch, &path, result.runtime_slot));
    }
    assert_eq!(
        roster
            .iter()
            .map(|e| e.runtime_slot)
            .collect::<Vec<Option<u16>>>(),
        vec![Some(0), Some(1), Some(2)]
    );

    // The middle agent is removed; dropping its entry releases its block.
    let removed = roster.remove(1);
    assert_eq!(removed.runtime_slot, Some(1));

    // The next add reuses the freed block rather than climbing to 3.
    let path = create_worktree(&sb.repo, "feature/four");
    let slot = allocate_slot(&roster);
    let result = provision_worktree(&sb.repo, &path, "feat-x", &config, slot).unwrap();
    assert_eq!(result.runtime_slot, Some(1));
    assert_eq!(
        env_local_lines(&path),
        vec!["PORT=3010".to_string(), "VITE_PORT=3011".to_string()]
    );
    roster.push(entry("feature/four", &path, result.runtime_slot));

    // No two concurrently active worktrees share a port across the churn.
    let mut seen: Vec<String> = Vec::new();
    for e in &roster {
        for line in env_local_lines(&e.worktree_path) {
            assert!(
                !seen.contains(&line),
                "'{line}' is assigned to two active worktrees; roster: {seen:?}"
            );
            seen.push(line);
        }
    }
}
