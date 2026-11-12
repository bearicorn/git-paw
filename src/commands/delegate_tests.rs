//! Fake-spawner tests for `git paw delegate`: what is spawned, where, with
//! which argv and environment — and what is refused before anything is.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;

use super::*;
use crate::command_runner::RealCommandRunner;
use crate::command_runner::test_support::{FakeChild, FakeCommandRunner};
use crate::commands::CaptureOutput;

/// A committed repository with an executable stub agent at `<tmp>/bin/agent`.
struct Fixture {
    _dir: TempDir,
    repo: PathBuf,
    agent: PathBuf,
}

fn fixture() -> Fixture {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.email", "t@t"],
        &["config", "user.name", "T"],
    ] {
        std::process::Command::new("git")
            .current_dir(&repo)
            .args(args)
            .output()
            .unwrap();
    }
    std::fs::write(repo.join("README.md"), "# t").unwrap();
    // A repository role skill, so the run does not depend on a bundled one.
    let skill = repo.join(".agents/skills/probe-role");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: probe-role\ndescription: Test role\n---\nPROBE-ROLE\n",
    )
    .unwrap();
    for args in [&["add", "."][..], &["commit", "-q", "-m", "i"]] {
        std::process::Command::new("git")
            .current_dir(&repo)
            .args(args)
            .output()
            .unwrap();
    }
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let agent = bin.join("agent");
    std::fs::write(&agent, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&agent, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    Fixture {
        _dir: dir,
        repo,
        agent,
    }
}

fn config(toml: &str) -> PawConfig {
    toml::from_str(toml).expect("valid config")
}

/// `[clis.agent]` on the stub with `headless_args = "--batch"` plus
/// `[profiles.explorer]` (delegate) and `[profiles.routine]` (worker).
fn agent_config(fx: &Fixture, extra: &str) -> PawConfig {
    config(&format!(
        "[clis.agent]\ncommand = \"{}\"\nheadless_args = \"--batch\"\n{extra}\n\
         [profiles.explorer]\nkind = \"delegate\"\ndescription = \"Sweep\"\ncli = \"agent\"\n\
         args = \"--model small\"\nskill = \"probe-role\"\n\n\
         [profiles.routine]\nkind = \"worker\"\ndescription = \"Routine\"\ncli = \"agent\"\n",
        fx.agent.display()
    ))
}

/// Runs the command from `cwd` with `spawner`, returning the exit status (or
/// the refusal) and what it printed.
fn run(
    cwd: &Path,
    cfg: PawConfig,
    spawner: &FakeCommandRunner,
    opts: DelegateOptions,
    marker: Option<&str>,
) -> (Result<i32, PawError>, CaptureOutput) {
    let mut out = CaptureOutput::default();
    let mut stdin = || Ok("task from stdin".to_string());
    let mut host = DelegateHost {
        cwd,
        delegate_marker: marker.map(str::to_string),
        runner: &RealCommandRunner,
        spawner,
        read_stdin: &mut stdin,
        pid: std::process::id(),
        poll_interval: Duration::from_millis(1),
    };
    let result = delegate(&mut host, move |_| Ok(cfg), opts, &mut out);
    (result, out)
}

fn task(name: &str, task: &str) -> DelegateOptions {
    DelegateOptions {
        name: Some(name.to_string()),
        task: Some(task.to_string()),
        list: false,
    }
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap()
}

/// Scenario "A delegate runs at the root with its arguments in order".
#[test]
fn a_delegate_runs_at_the_root_with_its_arguments_in_order() {
    let fx = fixture();
    let spawner = FakeCommandRunner::succeeding("");
    let (result, _) = run(
        &fx.repo.join("src"),
        agent_config(&fx, ""),
        &spawner,
        task("explorer", "map the config loader"),
        None,
    );
    assert_eq!(result.expect("runs"), 0);
    let spawns = spawner.spawns();
    assert_eq!(spawns.len(), 1, "exactly one process");
    let spec = &spawns[0];
    assert_eq!(spec.program, fx.agent.display().to_string());
    assert_eq!(spec.args, ["--model", "small", "--batch"]);
    assert_eq!(canonical(&spec.cwd), canonical(&fx.repo));
    let stdin = String::from_utf8_lossy(&spec.stdin);
    assert!(stdin.contains("map the config loader"));
    assert!(
        !spec
            .args
            .iter()
            .any(|a| a.contains("map the config loader"))
    );
    assert!(spec.new_process_group);
}

/// Scenarios "A delegate gets its CLI's operator isolation" and "An
/// opted-out CLI's delegate keeps the operator configuration".
#[test]
fn a_delegate_gets_its_clis_operator_isolation_unless_opted_out() {
    let fx = fixture();
    for opted_out in [false, true] {
        let cfg = config(&format!(
            "[clis.opus]\ncommand = \"{}\"\nfamily = \"claude\"\ninherit_operator_config = {opted_out}\n\n\
             [profiles.explorer]\nkind = \"delegate\"\ndescription = \"Sweep\"\ncli = \"opus\"\n\
             args = \"--model small\"\nskill = \"probe-role\"\n",
            fx.agent.display()
        ));
        let spawner = FakeCommandRunner::succeeding("");
        let (result, _) = run(&fx.repo, cfg, &spawner, task("explorer", "x"), None);
        assert_eq!(result.expect("runs"), 0);
        let spec = &spawner.spawns()[0];
        let has_memory_off = spec
            .env
            .iter()
            .any(|(k, v)| k == "CLAUDE_CODE_DISABLE_AUTO_MEMORY" && v == "1");
        if opted_out {
            assert!(
                !spec.args.iter().any(|a| a == "--setting-sources"),
                "{spec:?}"
            );
            assert!(!has_memory_off, "{spec:?}");
        } else {
            assert_eq!(
                spec.args[..4],
                ["--model", "small", "--setting-sources", "project,local"]
            );
            assert_eq!(spec.args[4], "--session-id");
            assert_eq!(spec.args.last().map(String::as_str), Some("-p"));
            assert_eq!(spec.args.len(), 7, "{:?}", spec.args);
            assert!(has_memory_off, "{spec:?}");
        }
    }
}

/// Scenario "Environment marks the run and carries no broker URL or
/// credential".
#[test]
fn environment_marks_the_run_and_carries_no_broker_url_or_credential() {
    let fx = fixture();
    let spawner = FakeCommandRunner::succeeding("");
    let (result, _) = run(
        &fx.repo,
        agent_config(&fx, ""),
        &spawner,
        task("explorer", "x"),
        None,
    );
    assert_eq!(result.expect("runs"), 0);
    let spec = &spawner.spawns()[0];
    assert!(
        spec.env
            .iter()
            .any(|(k, v)| k == "GIT_PAW_DELEGATE" && v == "explorer")
    );
    assert!(spec.env_remove.iter().any(|k| k == "GIT_PAW_BROKER_URL"));
    assert!(
        !spec
            .env
            .iter()
            .any(|(k, _)| k.contains("BROKER") || k.contains("AUTHORITY"))
    );
}

/// Scenario "A CLI without a headless form is refused".
#[test]
fn a_cli_without_a_headless_form_is_refused() {
    let fx = fixture();
    let cfg = config(&format!(
        "[clis.agent]\ncommand = \"{}\"\n\n\
         [profiles.explorer]\nkind = \"delegate\"\ndescription = \"Sweep\"\ncli = \"agent\"\n\
         skill = \"probe-role\"\n",
        fx.agent.display()
    ));
    let spawner = FakeCommandRunner::succeeding("");
    let (result, _) = run(&fx.repo, cfg, &spawner, task("explorer", "x"), None);
    let err = result.expect_err("refused").to_string();
    assert!(err.contains("[clis.agent].headless_args"), "{err}");
    assert!(spawner.spawns().is_empty());
}

/// Scenario "A worker profile is not a delegate".
#[test]
fn a_worker_profile_is_not_a_delegate() {
    let fx = fixture();
    let spawner = FakeCommandRunner::succeeding("");
    let (result, _) = run(
        &fx.repo,
        agent_config(&fx, ""),
        &spawner,
        task("routine", "x"),
        None,
    );
    let err = result.expect_err("refused").to_string();
    assert!(err.contains("'routine' is a worker profile"), "{err}");
    assert!(spawner.spawns().is_empty());
}

/// Scenario "An empty task is refused".
#[test]
fn an_empty_task_is_refused() {
    let fx = fixture();
    let spawner = FakeCommandRunner::succeeding("");
    let (result, _) = run(
        &fx.repo,
        agent_config(&fx, ""),
        &spawner,
        task("explorer", "  "),
        None,
    );
    assert!(result.is_err());
    assert!(spawner.spawns().is_empty());
}

/// "No nested delegates", at the command level.
#[test]
fn a_nested_run_is_refused() {
    let fx = fixture();
    let spawner = FakeCommandRunner::succeeding("");
    let (result, _) = run(
        &fx.repo,
        agent_config(&fx, ""),
        &spawner,
        task("explorer", "x"),
        Some("explorer"),
    );
    assert!(result.is_err());
    assert!(spawner.spawns().is_empty());
}

/// The run's stdout is the printed result; a failure exits non-zero and the
/// ledger records it.
#[test]
fn the_result_and_ledger_reflect_the_run() {
    let fx = fixture();
    let spawner = FakeCommandRunner::succeeding("").with_spawn(|spec| {
        if let Some(path) = &spec.stdout_log {
            std::fs::write(path, "MAP: src/config\n").unwrap();
        }
        Ok(Box::new(FakeChild::exited(3)))
    });
    let (result, out) = run(
        &fx.repo,
        agent_config(&fx, ""),
        &spawner,
        task("explorer", "x"),
        None,
    );
    assert_eq!(result.expect("ran"), 1);
    assert_eq!(out.lines, vec!["MAP: src/config".to_string()]);
    let ledger =
        std::fs::read_to_string(fx.repo.join(".git-paw/logs/delegates/runs.jsonl")).unwrap();
    let line: serde_json::Value = serde_json::from_str(ledger.trim()).unwrap();
    assert_eq!(line["outcome"], "failed");
    assert_eq!(line["exit_code"], 3);
    assert!(
        std::fs::read_dir(fx.repo.join(".git-paw/logs/delegates/active"))
            .unwrap()
            .flatten()
            .all(|e| e.file_name() != std::ffi::OsStr::new(&format!("{}.json", std::process::id()))),
        "the active record is released"
    );
}
