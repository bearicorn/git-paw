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
        guard_signals: false,
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

/// A planted or corrupt record — pgid `0`, `1`, git-paw's own group, or a
/// group whose leader no longer runs the delegate program — is never
/// signalled, even overdue with its holder gone.
#[test]
fn a_planted_or_reused_group_is_never_signalled() {
    let dir = TempDir::new().unwrap();
    let dirs = RunDirs::new(dir.path());
    dirs.create().unwrap();
    let own_pid = std::process::id();
    let records = [
        (0, "/opt/agent"),
        (1, "/opt/agent"),
        (777, "/opt/agent"),
        (888, "/opt/agent"),
    ];
    for (i, (pgid, program)) in records.iter().enumerate() {
        let rec = ActiveRun {
            profile: format!("p{i}"),
            level: crate::config::ApprovalLevel::Auto,
            pid: 900_000 + u32::try_from(i).unwrap(),
            pgid: Some(*pgid),
            deadline: Some(1),
            program: Some((*program).to_string()),
            leader_start: Some(START.to_string()),
        };
        write_record(&dirs.active.join(format!("{}.json", rec.pid)), &rec).unwrap();
    }
    let ok = |stdout: &str| {
        Ok(crate::command_runner::CommandOutput {
            success: true,
            code: Some(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    };
    let runner = FakeCommandRunner::scripted(move |program, args| match (program, args) {
        // Holders are gone; every group probe answers alive.
        ("kill", ["-0", "--", target]) if target.starts_with('-') => ok(""),
        ("kill", ["-0", ..]) => Ok(crate::command_runner::CommandOutput {
            success: false,
            code: Some(1),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }),
        // git-paw's own group is 777; group 888's leader is an unrelated
        // program reusing the id.
        ("ps", [_, "pgid=", ..]) => ok("777\n"),
        ("ps", [_, "pgid=,command=", _, "888"]) => ok("888 /usr/bin/vim notes.txt\n"),
        _ => ok(""),
    });
    let me = ActiveRun {
        profile: "explorer".to_string(),
        level: crate::config::ApprovalLevel::Manual,
        pid: own_pid,
        pgid: None,
        deadline: None,
        program: None,
        leader_start: None,
    };
    // Records 0, 1 and 777 name no group: dead and dropped. Record 888's
    // group is alive but unverifiable, so it is kept and still counts.
    let err = admit_run(&dirs, &me, &runner)
        .expect_err("the unverified overdue writer still counts")
        .to_string();
    assert!(err.contains("p3"), "{err}");
    for pid in [900_000, 900_001, 900_002] {
        assert!(
            !dirs.active.join(format!("{pid}.json")).exists(),
            "{pid} dropped"
        );
    }
    assert!(
        dirs.active.join("900003.json").exists(),
        "the unkilled record is kept"
    );
    let kills: Vec<_> = runner
        .calls()
        .into_iter()
        .filter(|(p, a)| p == "kill" && a.first().is_some_and(|f| f == "-KILL"))
        .collect();
    assert!(kills.is_empty(), "nothing may be killed: {kills:?}");
    let probed: Vec<String> = runner
        .calls()
        .into_iter()
        .filter(|(p, a)| p == "kill" && a.first().is_some_and(|f| f == "-0"))
        .filter_map(|(_, a)| a.last().cloned())
        .collect();
    for forbidden in ["-0", "-1", "-777"] {
        assert!(
            !probed.iter().any(|t| t == forbidden),
            "{forbidden} was probed: {probed:?}"
        );
    }
}

/// An overdue orphan whose leader is still the delegate program is killed.
#[test]
fn a_verified_overdue_group_is_killed() {
    let dir = TempDir::new().unwrap();
    let dirs = RunDirs::new(dir.path());
    dirs.create().unwrap();
    let rec = ActiveRun {
        profile: "fixer".to_string(),
        level: crate::config::ApprovalLevel::Auto,
        pid: 900_100,
        pgid: Some(4242),
        deadline: Some(1),
        program: Some("/opt/agent".to_string()),
        leader_start: Some(START.to_string()),
    };
    write_record(&dirs.active.join("900100.json"), &rec).unwrap();
    let ok = |success: bool, stdout: &str| {
        Ok(crate::command_runner::CommandOutput {
            success,
            code: Some(i32::from(!success)),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    };
    let runner = FakeCommandRunner::scripted(move |program, args| match (program, args) {
        ("kill", ["-0", "--", "-4242"]) => ok(true, ""),
        ("kill", ["-0", ..]) => ok(false, ""),
        ("ps", [_, "pgid=", ..]) => ok(true, "777\n"),
        ("ps", [_, "pgid=,command=", _, "4242"]) => ok(true, "4242 /opt/agent --batch\n"),
        ("ps", [_, "lstart=", _, "4242"]) => ok(true, &format!("{START}\n")),
        _ => ok(true, ""),
    });
    let me = ActiveRun {
        profile: "explorer".to_string(),
        level: crate::config::ApprovalLevel::Manual,
        pid: std::process::id(),
        pgid: None,
        deadline: None,
        program: None,
        leader_start: None,
    };
    admit_run(&dirs, &me, &runner).expect("admitted");
    assert!(
        runner
            .calls()
            .iter()
            .any(|(p, a)| p == "kill" && a == &["-KILL", "--", "-4242"]),
        "{:?}",
        runner.calls()
    );
}

const START: &str = "Thu Nov 13 10:00:00 2026";

/// A leader whose start time differs from the recorded one is a different
/// process reusing the group id: it is not killed, and the record is kept.
#[test]
fn a_group_whose_leader_restarted_is_not_killed() {
    let dir = TempDir::new().unwrap();
    let dirs = RunDirs::new(dir.path());
    dirs.create().unwrap();
    let rec = ActiveRun {
        profile: "fixer".to_string(),
        level: crate::config::ApprovalLevel::Auto,
        pid: 900_200,
        pgid: Some(4343),
        deadline: Some(1),
        program: Some("/opt/agent".to_string()),
        leader_start: Some(START.to_string()),
    };
    write_record(&dirs.active.join("900200.json"), &rec).unwrap();
    let ok = |success: bool, stdout: &str| {
        Ok(crate::command_runner::CommandOutput {
            success,
            code: Some(i32::from(!success)),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    };
    let runner = FakeCommandRunner::scripted(move |program, args| match (program, args) {
        ("kill", ["-0", "--", "-4343"]) => ok(true, ""),
        ("kill", ["-0", ..]) => ok(false, ""),
        ("ps", [_, "pgid=", ..]) => ok(true, "777\n"),
        ("ps", [_, "pgid=,command=", _, "4343"]) => ok(true, "4343 /opt/agent\n"),
        ("ps", [_, "lstart=", _, "4343"]) => ok(true, "Fri Nov 14 09:00:00 2026\n"),
        _ => ok(true, ""),
    });
    let me = ActiveRun {
        profile: "explorer".to_string(),
        level: crate::config::ApprovalLevel::Manual,
        pid: std::process::id(),
        pgid: None,
        deadline: None,
        program: None,
        leader_start: None,
    };
    assert!(
        admit_run(&dirs, &me, &runner).is_err(),
        "the kept record still counts"
    );
    assert!(
        !runner
            .calls()
            .iter()
            .any(|(p, a)| p == "kill" && a.first().is_some_and(|f| f == "-KILL")),
        "{:?}",
        runner.calls()
    );
    assert!(dirs.active.join("900200.json").exists());
}

/// When the post-spawn record cannot be written, the delegate's group is
/// killed and the run fails — an unrecorded group never runs on.
#[test]
fn a_failed_post_spawn_record_kills_the_group_and_fails_the_run() {
    use std::cell::Cell;
    use std::rc::Rc;
    let fx = fixture();
    let killed = Rc::new(Cell::new(false));
    let seen = Rc::clone(&killed);
    let active = fx.repo.join(".git-paw/logs/delegates/active");
    let spawner = FakeCommandRunner::succeeding("").with_spawn(move |_| {
        // The record's directory vanishes between admission and the rewrite.
        let _ = std::fs::remove_dir_all(&active);
        Ok(Box::new(FakeChild::new(
            Rc::new(Cell::new(crate::command_runner::ChildState::Running)),
            Rc::clone(&seen),
        )))
    });
    let (result, _) = run(
        &fx.repo,
        agent_config(&fx, ""),
        &spawner,
        task("explorer", "x"),
        None,
    );
    assert!(result.is_err(), "the run fails");
    assert!(killed.get(), "the spawned group is killed");
}
