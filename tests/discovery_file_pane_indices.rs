//! E2E coverage for `session-state`'s *start writes a per-repo session JSON*
//! requirement: EVERY path through which `git paw start` creates or rebuilds
//! the session's panes SHALL (re)write
//! `.git-paw/sessions/paw-<project>.json`, and each agent entry's
//! `pane_index` SHALL identify the pane that agent actually occupies.
//!
//! Three paths are covered here — the spec-driven launch, recovery of a
//! stopped session, and restart of a paused session. None of them wrote the
//! file before this change (the spec-driven path gained the write in
//! `start-spec-launch-parity`; recovery and resume never had it), and both
//! recovery and resume REBUILD panes, so a file left by the original launch
//! can record indices that no longer describe the live session.
//!
//! Each test asserts the strong form: for every roster entry, the pane at
//! `pane_index` in the live tmux session has that agent's worktree as its
//! `pane_current_path`.
//!
//! Skips when tmux is unavailable.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn skip_if_no_tmux() -> bool {
    let available = StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success());
    if !available {
        eprintln!("skipping: tmux not available");
    }
    !available
}

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("discidx-{tag}-{pid}-{n}")
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> PathBuf {
    let old = tr.path().to_path_buf();
    let renamed = old
        .parent()
        .expect("repo has a parent sandbox dir")
        .join(new_basename);
    fs::rename(&old, &renamed).expect("rename repo dir");
    renamed
}

/// Writes a broker-enabled config using `cat` as the stand-in CLI, so a pane
/// stays alive and consumes the injected prompt as stdin instead of executing
/// its markdown.
fn write_config(repo: &Path, extra: &str) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let port = find_free_port();
    let config = format!(
        r#"
default_cli = "cat"

[broker]
enabled = true
port = {port}

[clis.cat]
command = "cat"
{extra}
"#
    );
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

fn write_committed_spec(repo: &Path, id: &str, body: &str) {
    let dir = repo.join("specs").join(id);
    fs::create_dir_all(&dir).expect("create spec dir");
    fs::write(dir.join("tasks.md"), body).expect("write tasks.md");
    StdCommand::new("git")
        .current_dir(repo)
        .args(["add", "."])
        .output()
        .expect("git add");
    StdCommand::new("git")
        .current_dir(repo)
        .args(["commit", "-m", "add spec"])
        .output()
        .expect("git commit");
}

fn kill_session(env: &TmuxTestEnv, name: &str) {
    let mut kill_cmd = StdCommand::new("tmux");
    env.apply(&mut kill_cmd);
    let _ = kill_cmd.args(["kill-session", "-t", name]).status();
}

fn discovery_path(repo: &Path, session: &str) -> PathBuf {
    repo.join(".git-paw")
        .join("sessions")
        .join(format!("{session}.json"))
}

fn read_discovery(repo: &Path, session: &str) -> git_paw::session::RepoSessionFile {
    let path = discovery_path(repo, session);
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read discovery file {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("parse discovery file")
}

/// `(pane_index, pane_current_path)` for every pane in the live session's
/// first window, retried briefly because a freshly-spawned pane's working
/// directory can lag the `new-session`/`split-window` call by a few ms.
fn live_pane_paths(env: &TmuxTestEnv, session: &str, want_panes: usize) -> Vec<(usize, PathBuf)> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut panes = Vec::new();
    while Instant::now() < deadline {
        let mut c = StdCommand::new("tmux");
        env.apply(&mut c);
        let out = c
            .args([
                "list-panes",
                "-t",
                &format!("{session}:0"),
                "-F",
                "#{pane_index} #{pane_current_path}",
            ])
            .output()
            .expect("tmux list-panes");
        panes = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                let (idx, path) = line.trim_end().split_once(' ')?;
                Some((idx.parse::<usize>().ok()?, PathBuf::from(path.trim())))
            })
            .collect();
        if panes.len() >= want_panes {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panes
}

fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Asserts the discovery file exists and every roster entry's `pane_index`
/// names the live pane sitting at that agent's worktree.
fn assert_roster_pane_indices_match_live_session(
    env: &TmuxTestEnv,
    repo: &Path,
    session: &str,
    expected_agents: usize,
) {
    let path = discovery_path(repo, session);
    assert!(
        path.exists(),
        "the per-repo discovery file must exist at {}",
        path.display()
    );
    let file = read_discovery(repo, session);
    assert_eq!(file.session_name, session);
    assert_eq!(
        file.agents.len(),
        expected_agents,
        "unexpected roster size; file:\n{file:?}"
    );

    let panes = live_pane_paths(env, session, expected_agents + 1);
    for agent in &file.agents {
        let pane_path = panes
            .iter()
            .find(|(idx, _)| *idx == agent.pane_index)
            .map_or_else(
                || {
                    panic!(
                        "roster names pane {} for {}, but the live session has panes {:?}",
                        agent.pane_index, agent.branch_id, panes
                    )
                },
                |(_, p)| p.clone(),
            );
        assert_eq!(
            canonical(&pane_path),
            canonical(&agent.worktree_path),
            "roster pane_index {} for {} must be the pane sitting at its worktree; \
             live panes: {:?}",
            agent.pane_index,
            agent.branch_id,
            panes
        );
    }
}

/// Spec scenario "A spec-driven launch without supervisor mode writes the
/// per-repo JSON", in its strong form: the recorded pane indices describe the
/// live session.
#[test]
#[serial]
fn spec_driven_launch_records_live_pane_indices() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("specs");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "\n[specs]\ntype = \"openspec\"\ndir = \"specs\"\n");
    write_committed_spec(&repo, "feature-indices", "Implement indices.\n");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");
    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--from-specs", "--cli", "cat"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --from-specs");
    assert!(
        out.status.success(),
        "spec-driven launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert_roster_pane_indices_match_live_session(&tmux_env, &repo, &session_name, 1);
    kill_session(&tmux_env, &session_name);
}

/// Spec scenario "Recovery rewrites the per-repo JSON": a stopped session
/// whose discovery file was removed gets it back, listing every recovered
/// agent with the pane index it occupies in the REBUILT session.
#[test]
#[serial]
fn recovery_rewrites_the_discovery_file_with_rebuilt_pane_indices() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("recover");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--branches",
            "feat/alpha,feat/beta",
            "--cli",
            "cat",
        ])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --branches");
    assert!(
        out.status.success(),
        "initial launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut stop = cmd();
    tmux_env.apply_assert(&mut stop);
    let stop_out = stop
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run stop");
    assert!(
        stop_out.status.success(),
        "stop should succeed; stderr:\n{}",
        String::from_utf8_lossy(&stop_out.stderr)
    );

    // The scenario's GIVEN: the per-repo JSON is missing when recovery runs.
    let path = discovery_path(&repo, &session_name);
    fs::remove_file(&path).expect("remove discovery file before recovery");

    let mut recover = cmd();
    tmux_env.apply_assert(&mut recover);
    let rec_out = recover
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start (recovery)");
    let rec_stdout = String::from_utf8_lossy(&rec_out.stdout);
    assert!(
        rec_out.status.success(),
        "recovery should succeed; stdout:\n{rec_stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&rec_out.stderr)
    );
    assert!(
        rec_stdout.contains("Recovering session"),
        "the second start must take the recovery path; stdout:\n{rec_stdout}"
    );

    assert_roster_pane_indices_match_live_session(&tmux_env, &repo, &session_name, 2);
    kill_session(&tmux_env, &session_name);
}

/// Spec scenario "Restart from pause rewrites the per-repo JSON": a resume
/// re-splits the dashboard pane at whatever index tmux assigns, so the roster
/// must carry the pane indices the agents occupy in the RESUMED session.
#[test]
#[serial]
fn restart_from_pause_rewrites_the_discovery_file_with_live_pane_indices() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("pause");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args([
            "start",
            "--branches",
            "feat/alpha,feat/beta",
            "--cli",
            "cat",
        ])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --branches");
    assert!(
        out.status.success(),
        "initial launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut pause = cmd();
    tmux_env.apply_assert(&mut pause);
    let pause_out = pause
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["pause"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run pause");
    assert!(
        pause_out.status.success(),
        "pause should succeed; stderr:\n{}",
        String::from_utf8_lossy(&pause_out.stderr)
    );

    // Remove the file the launch wrote so the assertion can only pass if the
    // resume path writes it.
    fs::remove_file(discovery_path(&repo, &session_name)).expect("remove discovery file");

    let mut resume = cmd();
    tmux_env.apply_assert(&mut resume);
    let resume_out = resume
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start (resume)");
    let resume_stdout = String::from_utf8_lossy(&resume_out.stdout);
    assert!(
        resume_out.status.success(),
        "resume should succeed; stdout:\n{resume_stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&resume_out.stderr)
    );
    assert!(
        resume_stdout.contains("Restarting paused session"),
        "the second start must take the restart-from-pause path; stdout:\n{resume_stdout}"
    );

    assert_roster_pane_indices_match_live_session(&tmux_env, &repo, &session_name, 2);
    kill_session(&tmux_env, &session_name);
}

/// A supervisor-mode resume must still record the orchestrator entry when the
/// discovery file is MISSING — the case the rewrite exists for. Reading the
/// entry back from the old file would lose it exactly then, so it is derived
/// from the resumed session's mode instead.
#[test]
#[serial]
fn restart_from_pause_derives_the_orchestrator_entry_when_the_file_is_gone() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("orch");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "\n[supervisor]\nenabled = true\ncli = \"cat\"\n");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--supervisor", "--branches", "feat/alpha"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --supervisor");
    assert!(
        out.status.success(),
        "supervisor launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut pause = cmd();
    tmux_env.apply_assert(&mut pause);
    let pause_out = pause
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["pause"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run pause");
    assert!(
        pause_out.status.success(),
        "pause should succeed; stderr:\n{}",
        String::from_utf8_lossy(&pause_out.stderr)
    );

    // The GIVEN: nothing to read the orchestrator entry back from.
    fs::remove_file(discovery_path(&repo, &session_name)).expect("remove discovery file");

    let mut resume = cmd();
    tmux_env.apply_assert(&mut resume);
    let resume_out = resume
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start (resume)");
    let resume_stdout = String::from_utf8_lossy(&resume_out.stdout);
    assert!(
        resume_out.status.success(),
        "resume should succeed; stdout:\n{resume_stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&resume_out.stderr)
    );

    let file = read_discovery(&repo, &session_name);
    kill_session(&tmux_env, &session_name);

    let orchestrator = file.orchestrator.clone().unwrap_or_else(|| {
        panic!("a resumed supervisor session must record its orchestrator entry; file:\n{file:?}")
    });
    assert_eq!(orchestrator.pane_index, 0, "the supervisor pane is index 0");
    assert_eq!(
        orchestrator.cli, "cat",
        "it names the configured supervisor CLI"
    );
}

/// A resume recreates the dashboard pane and re-spawns the broker BEFORE the
/// discovery file is rewritten, so a config that no longer parses must not
/// abort what remains — that would leave the session half-resumed, with no
/// saved state update and no discovery file. The config is loaded best-effort;
/// the resume completes on defaults.
#[test]
#[serial]
fn restart_from_pause_survives_a_malformed_config() {
    if skip_if_no_tmux() {
        return;
    }
    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("badcfg");
    let repo = rename_repo_basename(&tr, &project);
    write_config(&repo, "");

    let session_name = format!("paw-{project}");
    let home = TempDir::new().expect("home tempdir");

    let mut start = cmd();
    tmux_env.apply_assert(&mut start);
    let out = start
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start", "--branches", "feat/alpha", "--cli", "cat"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start --branches");
    assert!(
        out.status.success(),
        "initial launch should succeed; stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let mut pause = cmd();
    tmux_env.apply_assert(&mut pause);
    let pause_out = pause
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["pause"])
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run pause");
    assert!(
        pause_out.status.success(),
        "pause should succeed; stderr:\n{}",
        String::from_utf8_lossy(&pause_out.stderr)
    );

    // Break the config AFTER pausing, and remove the discovery file so the
    // assertion can only pass if the resume got far enough to rewrite it.
    fs::write(
        repo.join(".git-paw").join("config.toml"),
        "default_cli = \"cat\"\n[broker\nenabled = true\n",
    )
    .expect("write malformed config");
    fs::remove_file(discovery_path(&repo, &session_name)).expect("remove discovery file");

    let mut resume = cmd();
    tmux_env.apply_assert(&mut resume);
    let resume_out = resume
        .current_dir(&repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["start"])
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run start (resume)");
    let resume_stdout = String::from_utf8_lossy(&resume_out.stdout);
    let resume_stderr = String::from_utf8_lossy(&resume_out.stderr);

    let discovery_exists = discovery_path(&repo, &session_name).exists();
    kill_session(&tmux_env, &session_name);

    assert!(
        resume_out.status.success(),
        "a malformed config must not fail a resume that has already recreated \
         the dashboard pane; stdout:\n{resume_stdout}\nstderr:\n{resume_stderr}"
    );
    assert!(
        resume_stdout.contains("Restarting paused session"),
        "the second start must take the restart-from-pause path; \
         stdout:\n{resume_stdout}"
    );
    assert!(
        discovery_exists,
        "the resume must still rewrite the per-repo discovery file; \
         stdout:\n{resume_stdout}\nstderr:\n{resume_stderr}"
    );
}
