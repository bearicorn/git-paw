//! `cli-agent-profiles` worker-profile scenarios observed through the binary:
//! `git paw start --profile` (dry run and refused launches) and
//! `git paw add --profile`. No tmux session is created by any of these.

use std::fs;
use std::path::Path;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn write_config(repo: &Path, body: &str) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(paw.join("config.toml"), body).expect("write config");
}

/// Runs the binary in `repo` with an empty user-level config.
fn run(repo: &Path, args: &[&str]) -> std::process::Output {
    let home = tempfile::TempDir::new().expect("home tempdir");
    Command::cargo_bin("git-paw")
        .expect("binary exists")
        .current_dir(repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("GIT_PAW_DELEGATE")
        .args(args)
        .output()
        .expect("run git-paw")
}

fn worktree_count(repo: &Path) -> usize {
    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .expect("git worktree list");
    String::from_utf8_lossy(&out.stdout)
        .matches("worktree ")
        .count()
}

const ROUTINE: &str = "[clis.agent]\ncommand = \"echo\"\nargs = \"--quiet\"\n\n\
     [profiles.routine]\nkind = \"worker\"\ndescription = \"Routine implementation\"\n\
     cli = \"agent\"\nargs = \"--model small\"\n";

/// Scenario "--profile maps every branch": both workers launch with the
/// profile and no CLI prompt is shown (the dry run has no terminal to ask).
#[test]
fn profile_flag_maps_every_branch() {
    let tr = setup_test_repo();
    write_config(tr.path(), ROUTINE);
    let out = run(
        tr.path(),
        &[
            "start",
            "--profile",
            "routine",
            "--branches",
            "feat-a,feat-b",
            "--dry-run",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for branch in ["feat-a", "feat-b"] {
        assert!(
            stdout.contains(&format!("{branch} \u{2192} routine [agent]")),
            "{branch} should map to the profile:\n{stdout}"
        );
    }
    assert_eq!(
        stdout.matches("runs: echo --quiet --model small").count(),
        2,
        "the profile's args follow the CLI's in each pane command:\n{stdout}"
    );
}

/// Scenario "A full-auto profile on an unsandboxed CLI warns": the worker
/// default is `manual`, so only the profile's own level can warn — once.
#[test]
fn a_full_auto_profile_on_an_unsandboxed_cli_warns_once() {
    let tr = setup_test_repo();
    write_config(
        tr.path(),
        "[supervisor]\nagent_approval = \"manual\"\n\n\
         [clis.claude]\ncommand = \"echo\"\n\n\
         [profiles.trusted]\nkind = \"worker\"\ndescription = \"Trusted\"\n\
         cli = \"claude\"\napproval = \"full-auto\"\n",
    );
    let out = run(
        tr.path(),
        &[
            "start",
            "--profile",
            "trusted",
            "--branches",
            "feat/a",
            "--dry-run",
        ],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "a warning does not refuse:\n{stderr}");
    assert!(
        stderr.contains("`claude` runs at full-auto with no declared sandbox"),
        "{stderr}"
    );
    assert_eq!(
        stderr.matches("no declared sandbox").count(),
        1,
        "warned once:\n{stderr}"
    );
}

/// Scenario "An unavailable profile CLI fails clearly": the launch fails
/// naming the profile and the CLI, and no worktree is created.
#[test]
fn an_unavailable_profile_cli_fails_before_any_worktree() {
    let tr = setup_test_repo();
    write_config(
        tr.path(),
        "[profiles.routine]\nkind = \"worker\"\ndescription = \"Routine\"\n\
         cli = \"missing-agent\"\n",
    );
    let out = run(
        tr.path(),
        &["start", "--profile", "routine", "--branches", "feat/a"],
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "the launch must fail");
    assert!(
        stderr.contains("routine") && stderr.contains("missing-agent"),
        "{stderr}"
    );
    assert_eq!(worktree_count(tr.path()), 1, "no worktree may be created");
}

/// Scenario "A delegate profile cannot launch a worker".
#[test]
fn a_delegate_profile_cannot_launch_a_worker() {
    let tr = setup_test_repo();
    write_config(
        tr.path(),
        "[profiles.explorer]\nkind = \"delegate\"\ndescription = \"Sweep\"\n\
         cli = \"agent\"\nskill = \"explorer\"\n",
    );
    let out = run(tr.path(), &["add", "feat/a", "--profile", "explorer"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "add must refuse");
    assert!(
        stderr.contains("'explorer' is a delegate profile"),
        "{stderr}"
    );
    let out = run(
        tr.path(),
        &[
            "start",
            "--profile",
            "explorer",
            "--branches",
            "feat/a",
            "--dry-run",
        ],
    );
    assert!(!out.status.success(), "start must refuse");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("'explorer' is a delegate profile"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Scenario "No profiles leaves behavior unchanged", as the launch plan
/// shows it: a profile-free config plans exactly the pre-profile lines — the
/// worker header names the CLI alone and the pane command carries no profile
/// arguments.
#[test]
fn no_profiles_leaves_the_launch_plan_unchanged() {
    let tr = setup_test_repo();
    write_config(
        tr.path(),
        "[clis.agent]\ncommand = \"echo\"\nargs = \"--quiet\"\n",
    );
    let out = run(
        tr.path(),
        &[
            "start",
            "--cli",
            "agent",
            "--branches",
            "feat-a",
            "--dry-run",
        ],
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let header = stdout
        .lines()
        .find(|l| l.trim_start().starts_with("feat-a \u{2192}"))
        .unwrap_or_else(|| panic!("no worker header:\n{stdout}"));
    assert!(
        header.trim_start().starts_with("feat-a \u{2192} agent ("),
        "{header}"
    );
    assert!(
        stdout.lines().any(|l| l == "      runs: echo --quiet"),
        "{stdout}"
    );
}
