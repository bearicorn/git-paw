//! `cli-provider` scenarios observed through `git paw start --dry-run` and
//! `git paw doctor`: the plan shows the real pane command, and an ambiguous
//! PATH shim is refused by the dry run and flagged by doctor. No tmux needed.

use std::fs;
use std::path::Path;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn write_config(repo: &Path, body: &str) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(paw.join("config.toml"), body).expect("write config");
}

fn dry_run(repo: &Path, cli: &str) -> std::process::Output {
    let home = tempfile::TempDir::new().expect("home tempdir");
    cmd()
        .current_dir(repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--cli", cli, "--branches", "feat-a", "--dry-run"])
        .output()
        .expect("run start --dry-run")
}

/// Scenario "A dry run shows the real launch command", with the claude
/// family's operator isolation in it.
#[test]
fn a_dry_run_shows_the_command_and_args_the_pane_would_run() {
    let tr = setup_test_repo();
    write_config(
        tr.path(),
        "[clis.sonnet]\ncommand = \"echo\"\nfamily = \"claude\"\nargs = \"--model sonnet\"\n",
    );

    let out = dry_run(tr.path(), "sonnet");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains(
            "runs: CLAUDE_CODE_DISABLE_AUTO_MEMORY=1 CLAUDE_CODE_PLUGIN_DIRS= echo --model sonnet \
             --setting-sources project,local"
        ),
        "plan should show the isolated pane command; got:\n{stdout}"
    );
    assert!(
        stdout.contains(WORKER_HEADER),
        "an accepted plan prints the worker header; got:\n{stdout}"
    );
}

/// The worker header a dry run prints for `feat-a`; a refused dry run must
/// print none.
const WORKER_HEADER: &str = "feat-a \u{2192}";

/// Scenarios "An ambiguous shim is refused" (dry run) and "Doctor flags an
/// ambiguous shim". `git` is always on PATH and is not a known agent CLI, so
/// a `[clis.git]` entry launching something else is ambiguous.
#[test]
fn an_ambiguous_shim_fails_the_dry_run_and_doctor() {
    let tr = setup_test_repo();
    write_config(tr.path(), "[clis.git]\ncommand = \"echo\"\n");

    let out = dry_run(tr.path(), "git");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "the dry run must refuse");
    assert!(stderr.contains("[clis.git] is ambiguous"), "{stderr}");
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains(WORKER_HEADER),
        "the refusal comes before any worker header"
    );

    let home = tempfile::TempDir::new().expect("home tempdir");
    let doctor = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["doctor", "--json"])
        .output()
        .expect("run doctor");
    let report = String::from_utf8_lossy(&doctor.stdout);
    let json: serde_json::Value = serde_json::from_str(&report).expect("doctor json");
    let row = json["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|c| c["name"] == "[clis.git]")
        .unwrap_or_else(|| panic!("no [clis.git] row in:\n{report}"));
    assert_eq!(row["status"], "fail", "{row}");
    assert!(
        row["remedy"]
            .as_str()
            .unwrap_or_default()
            .contains("rename the [clis.git] key"),
        "{row}"
    );
}

/// Scenario "An unsandboxed full-auto launch warns": the CLI comes from
/// `--cli`, not the config, so only the launch itself can warn.
#[test]
fn a_full_auto_launch_with_no_declared_sandbox_warns() {
    let tr = setup_test_repo();
    write_config(
        tr.path(),
        "[supervisor]\nagent_approval = \"full-auto\"\n\n[clis.codex]\ncommand = \"echo\"\n",
    );
    let out = dry_run(tr.path(), "codex");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "a warning does not refuse:\n{stderr}");
    assert!(
        stderr.contains("`codex` runs at full-auto with no declared sandbox"),
        "{stderr}"
    );
    assert_eq!(
        stderr.matches("no declared sandbox").count(),
        1,
        "warned once per CLI:\n{stderr}"
    );
}

/// Scenario "Codex full-auto under a native sandbox is refused": both the dry
/// run and the real launch fail before any worktree (and so any pane) exists.
#[test]
fn a_native_sandbox_disabled_by_full_auto_refuses_the_launch_with_nothing_created() {
    let tr = setup_test_repo();
    write_config(
        tr.path(),
        "[supervisor]\nagent_approval = \"full-auto\"\n\n\
         [clis.codex]\ncommand = \"echo\"\nsandbox = \"native\"\n",
    );

    let out = dry_run(tr.path(), "codex");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "the dry run must refuse");
    for needle in ["codex", "full-auto", "[clis.codex].approval_args"] {
        assert!(stderr.contains(needle), "should name {needle}: {stderr}");
    }
    assert!(
        !stdout.contains(WORKER_HEADER),
        "the refusal comes before any worker header:\n{stdout}"
    );

    let home = tempfile::TempDir::new().expect("home tempdir");
    let out = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--cli", "codex", "--branches", "feat-a"])
        .output()
        .expect("run start");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "the launch must refuse");
    assert!(stderr.contains("[clis.codex].approval_args"), "{stderr}");

    let worktrees = std::process::Command::new("git")
        .current_dir(tr.path())
        .args(["worktree", "list", "--porcelain"])
        .output()
        .expect("git worktree list");
    let listed = String::from_utf8_lossy(&worktrees.stdout);
    assert_eq!(
        listed.matches("worktree ").count(),
        1,
        "no worktree, so no pane, may be created:\n{listed}"
    );
}

/// Scenario "A dry run reports the refusal instead of a command": the plan
/// names the refusal for the refused worker and shows no command for it — no
/// `runs:` line and no approval flags that would switch its sandbox off —
/// whether or not the session is supervised.
#[test]
fn a_dry_run_reports_the_native_sandbox_refusal_instead_of_a_command() {
    for supervisor in ["", "enabled = true\ncli = \"codex\"\n"] {
        let tr = setup_test_repo();
        write_config(
            tr.path(),
            &format!(
                "[supervisor]\n{supervisor}agent_approval = \"full-auto\"\n\n\
                 [clis.codex]\ncommand = \"echo\"\nsandbox = \"native\"\n"
            ),
        );

        let out = dry_run(tr.path(), "codex");
        let output = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        for needle in ["codex", "full-auto", "[clis.codex].approval_args"] {
            assert!(output.contains(needle), "should name {needle}:\n{output}");
        }
        assert!(
            !output.contains("runs:"),
            "no runs: line for the refused worker:\n{output}"
        );
        assert!(
            !output.contains("echo --dangerously-bypass-approvals-and-sandbox")
                && !output.contains("codex --dangerously-bypass-approvals-and-sandbox"),
            "the plan must not show the refused command:\n{output}"
        );
    }
}
