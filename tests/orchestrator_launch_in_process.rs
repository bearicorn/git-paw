//! `supervisor-launch` / Scenario: An in-process launch receives the
//! announcement through the sink.
//!
//! The supervised launch use case, called in-process with a capturing output
//! sink, hands the lines naming `.git-paw/AGENTS.local.md` and
//! `.git-paw/skills/orchestrator/SKILL.md` to the sink and writes nothing to
//! the process's stdout.
//!
//! The launch reads `HOME` and `TMUX_TMPDIR` from the process environment, so
//! (like `command_library.rs`) the parent builds the fixture and re-executes
//! this binary filtered to the `#[ignore]`d child with the fixture's
//! environment: a private tmux socket, never the default server.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use git_paw::commands::spec_mode::SpecMode;
use git_paw::commands::supervisor::{SupervisorOptions, supervisor};
use git_paw::commands::{CaptureOutput, CommandContext};
use git_paw::interactive::TerminalPrompter;
use tempfile::TempDir;

mod helpers;
use helpers::{TmuxTestEnv, setup_test_repo};

/// The child's environment variable naming the fixture repository.
const CHILD_REPO: &str = "GIT_PAW_ORCHESTRATOR_LAUNCH_REPO";
/// The line a child prints on stderr once its assertions all passed.
const CHILD_PASSED: &str = "orchestrator-launch-child-passed";

#[test]
fn in_process_launch_announces_both_paths_through_the_sink() {
    let tr = setup_test_repo();
    let tmux = TmuxTestEnv::new();
    let home = TempDir::new().expect("fake HOME");
    fs::create_dir_all(tr.path().join(".git-paw")).expect(".git-paw");
    fs::write(
        tr.path().join(".git-paw/config.toml"),
        "default_cli = \"echo\"\n\n[supervisor]\nenabled = true\ncli = \"echo\"\n",
    )
    .expect("config");

    let mut cmd = Command::new(std::env::current_exe().expect("test binary"));
    tmux.apply(&mut cmd)
        .args([
            "child_in_process_launch",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env(CHILD_REPO, tr.path())
        .env("GIT_PAW_ALLOW_LIVE_SESSION", "1");
    let out = cmd.output().expect("run child test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success() && stderr.contains(CHILD_PASSED),
        "the in-process child failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // The libtest harness prints its own lines; none of the launch's may join
    // them.
    for leaked in [
        ".git-paw/AGENTS.local.md",
        ".git-paw/skills/orchestrator/SKILL.md",
        "launched with",
    ] {
        assert!(
            !stdout.contains(leaked),
            "the launch wrote `{leaked}` to the process's stdout:\n{stdout}"
        );
    }
    // `tmux` (the private server) is killed when `tmux` drops.
}

#[test]
#[ignore = "run by in_process_launch_announces_both_paths_through_the_sink"]
fn child_in_process_launch() {
    let Some(repo) = std::env::var_os(CHILD_REPO) else {
        return;
    };
    let ctx = CommandContext::new(PathBuf::from(repo));
    let mut sink = CaptureOutput::default();
    let outcome = supervisor(
        &ctx,
        SupervisorOptions {
            cli: Some("echo".to_string()),
            branches: Some(vec!["a".to_string()]),
            spec_mode: SpecMode::None,
            specs_format: None,
            dry_run: false,
            no_rebase: false,
            unattended: false,
            prompter: &TerminalPrompter,
            interactive: false,
        },
        &mut sink,
    );
    assert!(outcome.is_ok(), "launch failed: {:?}", outcome.err());
    for path in [
        ".git-paw/AGENTS.local.md",
        ".git-paw/skills/orchestrator/SKILL.md",
    ] {
        assert!(
            sink.lines.iter().any(|l| l.contains(path)),
            "the sink must receive a line naming {path}: {:?}",
            sink.lines
        );
    }
    eprintln!("{CHILD_PASSED}");
}
