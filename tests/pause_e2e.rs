//! Integration tests for `git paw pause` and the restart-from-pause
//! flow. Each test wires together a temporary git repo, a manually
//! crafted session JSON (since `cmd_start` interactivity is hard to
//! drive headlessly), and a real tmux session that mimics the
//! dashboard + agent pane layout. The pause flow then runs against
//! that simulated session.

use std::fs;
use std::net::TcpStream;
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

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_project_name(tag: &str) -> String {
    let pid = std::process::id();
    let n = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("pause-{tag}-{pid}-{n}")
}

fn skip_if_no_tmux() -> bool {
    if which::which("tmux").is_err() {
        eprintln!("skipping: tmux not available on PATH");
        return true;
    }
    false
}

fn kill_tmux_session(name: &str) {
    let _ = StdCommand::new("tmux")
        .args(["kill-session", "-t", name])
        .status();
}

fn rename_repo_basename(tr: &TestRepo, new_basename: &str) -> std::path::PathBuf {
    let original = tr.path().to_path_buf();
    let parent = original.parent().expect("repo has parent").to_path_buf();
    let renamed = parent.join(new_basename);
    fs::rename(&original, &renamed).expect("rename repo dir");
    renamed
}

fn canonical_repo_root(repo: &std::path::Path) -> std::path::PathBuf {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .expect("git rev-parse");
    assert!(out.status.success(), "git rev-parse must succeed");
    std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

fn sessions_dir_for(fake_home: &std::path::Path) -> std::path::PathBuf {
    if cfg!(target_os = "macos") {
        fake_home.join("Library/Application Support/git-paw/sessions")
    } else {
        fake_home.join(".local/share/git-paw/sessions")
    }
}

fn find_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

fn port_is_listening(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(200),
    )
    .is_ok()
}

fn wait_until_port_free(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !port_is_listening(port) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    !port_is_listening(port)
}

/// Builds a tmux session that mirrors the bare-mode broker layout:
/// pane 0 = dashboard (running `git-paw __dashboard`), pane 1 = a
/// long-lived `sleep` agent stand-in.
fn build_simulated_session(
    session_name: &str,
    repo_path: &std::path::Path,
    broker_url: &str,
    sessions_dir: &std::path::Path,
    fake_home: &std::path::Path,
) {
    let bin = assert_cmd::cargo::cargo_bin("git-paw");
    let dashboard_cmd = format!(
        "GIT_PAW_BROKER_URL='{broker_url}' XDG_DATA_HOME='{xdg}' HOME='{home}' '{bin}' __dashboard",
        xdg = sessions_dir
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(sessions_dir)
            .display(),
        home = fake_home.display(),
        bin = bin.display(),
    );

    // Pane 0: dashboard (runs the broker subprocess).
    // -x/-y required when tmux runs without an attached client (CI).
    let st = StdCommand::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            session_name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            repo_path.to_str().unwrap(),
        ])
        .status()
        .expect("tmux new-session");
    assert!(
        st.success(),
        "tmux new-session for {session_name} must succeed"
    );

    let pane0_target = format!("{session_name}:0.0");
    let st = StdCommand::new("tmux")
        .args(["send-keys", "-t", &pane0_target, &dashboard_cmd, "Enter"])
        .status()
        .expect("tmux send-keys pane 0");
    assert!(st.success());

    // Pane 1: long-lived agent stand-in.
    let st = StdCommand::new("tmux")
        .args(["split-window", "-t", session_name])
        .status()
        .expect("tmux split-window");
    assert!(st.success());

    let pane1_target = format!("{session_name}:0.1");
    let st = StdCommand::new("tmux")
        .args(["send-keys", "-t", &pane1_target, "sleep 3600", "Enter"])
        .status()
        .expect("tmux send-keys pane 1");
    assert!(st.success());
}

/// Writes a Session JSON to the simulated sessions dir.
#[allow(clippy::too_many_arguments)]
fn write_session_json(
    sessions_dir: &std::path::Path,
    session_name: &str,
    repo_canonical: &std::path::Path,
    project: &str,
    branch: &str,
    wt_path: &std::path::Path,
    broker_port: u16,
    broker_log: &std::path::Path,
) {
    let session_json = serde_json::json!({
        "session_name": session_name,
        "repo_path": repo_canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "active",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "sleep 3600",
            "branch_created": true,
        }],
        "broker_port": broker_port,
        "broker_bind": "127.0.0.1",
        "broker_log_path": broker_log.to_string_lossy(),
        "dashboard_pane": 0,
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session json");
}

fn create_branch_worktree(
    repo: &std::path::Path,
    project: &str,
    branch: &str,
) -> std::path::PathBuf {
    let parent = repo.parent().expect("repo has parent").to_path_buf();
    let slug = branch.replace('/', "-");
    let wt_path = parent.join(format!("{project}-{slug}"));

    let st = StdCommand::new("git")
        .current_dir(repo)
        .args(["branch", branch])
        .status()
        .expect("git branch");
    assert!(st.success());

    let st = StdCommand::new("git")
        .current_dir(repo)
        .args(["worktree", "add", wt_path.to_str().unwrap(), branch])
        .status()
        .expect("git worktree add");
    assert!(st.success());

    wt_path
}

// ---------------------------------------------------------------------------
// 10.1 pause_detaches_and_stops_broker
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn pause_detaches_and_stops_broker() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let project = unique_project_name("detach");
    let repo = rename_repo_basename(&tr, &project);
    let canonical = canonical_repo_root(&repo);

    let branch = "feat/alpha";
    let wt_path = create_branch_worktree(&repo, &project, branch);

    let broker_port = find_free_port();
    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");
    let broker_log = sessions_dir.join(format!("{session_name}.broker.log"));
    let broker_url = format!("http://127.0.0.1:{broker_port}");

    // Write a minimal config so __dashboard can find broker settings.
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!("[broker]\nenabled = true\nport = {broker_port}\n"),
    )
    .expect("write config");

    build_simulated_session(
        &session_name,
        &repo,
        &broker_url,
        &sessions_dir,
        fake_home.path(),
    );
    write_session_json(
        &sessions_dir,
        &session_name,
        &canonical,
        &project,
        branch,
        &wt_path,
        broker_port,
        &broker_log,
    );

    // Give the dashboard pane a moment to bind the broker port.
    let bind_deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < bind_deadline && !port_is_listening(broker_port) {
        std::thread::sleep(Duration::from_millis(100));
    }

    // Drive pause.
    let out = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["pause"])
        .output()
        .expect("run pause");

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    // Assertion 1: pause exited 0.
    assert!(
        out.status.success(),
        "pause should exit 0, got status {:?}\nstdout: {stdout}\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );

    // Assertion 2: stdout includes session name + resume hint.
    assert!(
        stdout.contains(&session_name),
        "stdout should mention session name. got: {stdout}"
    );
    assert!(
        stdout.contains("Run 'git paw start' to resume")
            || stdout.contains("Run `git paw start` to resume"),
        "stdout should include resume hint. got: {stdout}"
    );

    // Assertion 3: tmux session is still alive.
    let alive = StdCommand::new("tmux")
        .args(["has-session", "-t", &session_name])
        .status()
        .expect("tmux has-session")
        .success();

    // Always tear down the tmux session before asserting.
    //
    // De-flake (selftest-harness §3.4): the broker shuts down when `pause`
    // kills the dashboard pane, but under full-suite load (especially under
    // `cargo llvm-cov` + concurrent shards) the OS can take longer than the
    // former fixed 5s to actually release the listener socket. This is a
    // port-RELEASE timing race, not a selection collision — the port was
    // already OS-ephemeral. `wait_until_port_free` polls in a bounded loop and
    // returns the instant the port frees, so a wider 30s bound costs nothing
    // in the common case and absorbs the slow-release tail.
    let port_free = wait_until_port_free(broker_port, Duration::from_secs(30));
    kill_tmux_session(&session_name);

    assert!(alive, "tmux session should still be alive after pause");

    // Assertion 4: broker port released within the bounded retry window.
    assert!(
        port_free,
        "broker port {broker_port} should be free after pause (waited up to 30s for release)"
    );

    // Assertion 5: session JSON status flipped to paused.
    let json = fs::read_to_string(sessions_dir.join(format!("{session_name}.json")))
        .expect("read session json");
    assert!(
        json.contains("\"status\": \"paused\""),
        "session.status should be paused after pause. JSON: {json}"
    );
}

// ---------------------------------------------------------------------------
// 10.3 pause_idempotent_on_already_paused
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn pause_idempotent_on_already_paused() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let project = unique_project_name("idem");
    let repo = rename_repo_basename(&tr, &project);
    let canonical = canonical_repo_root(&repo);

    let branch = "feat/alpha";
    let wt_path = create_branch_worktree(&repo, &project, branch);

    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");

    // Write a pre-paused session JSON. We don't need a real broker or
    // tmux pane 0 — just a live tmux session so effective_status returns
    // Paused (rather than degrading to Stopped).
    let st = StdCommand::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            repo.to_str().unwrap(),
        ])
        .status()
        .expect("tmux new-session");
    assert!(st.success());

    let json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "paused",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "sleep 3600",
            "branch_created": true,
        }],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&json).expect("serialize"),
    )
    .expect("write session json");

    let out = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["pause"])
        .output()
        .expect("run pause");

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    kill_tmux_session(&session_name);

    assert!(
        out.status.success(),
        "pause should exit 0 on already-paused"
    );
    assert!(
        stdout.to_lowercase().contains("already paused"),
        "stdout should mention 'already paused', got: {stdout}"
    );
}

// ---------------------------------------------------------------------------
// 10.4 stop_after_pause_kills_remaining_panes
// ---------------------------------------------------------------------------

#[test]
#[serial]
fn stop_after_pause_kills_remaining_panes() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let project = unique_project_name("stop-after-pause");
    let repo = rename_repo_basename(&tr, &project);
    let canonical = canonical_repo_root(&repo);

    let branch = "feat/alpha";
    let wt_path = create_branch_worktree(&repo, &project, branch);

    let fake_home = TempDir::new().expect("create temp HOME");
    let sessions_dir = sessions_dir_for(fake_home.path());
    fs::create_dir_all(&sessions_dir).expect("create sessions dir");

    let session_name = format!("paw-{project}");

    // Spawn a live tmux session and write a paused JSON.
    let st = StdCommand::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session_name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            repo.to_str().unwrap(),
        ])
        .status()
        .expect("tmux new-session");
    assert!(st.success());

    let json = serde_json::json!({
        "session_name": session_name,
        "repo_path": canonical.to_string_lossy(),
        "project_name": project,
        "created_at": "2026-05-01T00:00:00Z",
        "status": "paused",
        "worktrees": [{
            "branch": branch,
            "worktree_path": wt_path.to_string_lossy(),
            "cli": "sleep 3600",
            "branch_created": true,
        }],
    });
    fs::write(
        sessions_dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&json).expect("serialize"),
    )
    .expect("write session json");

    // Stop: kills the tmux session.
    let out = cmd()
        .current_dir(&repo)
        .env("HOME", fake_home.path())
        .env_remove("XDG_DATA_HOME")
        .args(["stop"])
        .output()
        .expect("run stop");

    let alive_after = StdCommand::new("tmux")
        .args(["has-session", "-t", &session_name])
        .status()
        .is_ok_and(|s| s.success());
    kill_tmux_session(&session_name); // belt-and-braces cleanup

    assert!(
        out.status.success(),
        "stop should exit 0. stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!alive_after, "tmux session should be gone after stop");

    let final_json = fs::read_to_string(sessions_dir.join(format!("{session_name}.json")))
        .expect("read final session json");
    assert!(
        final_json.contains("\"status\": \"stopped\""),
        "session.status should be stopped. JSON: {final_json}"
    );
}

// ---------------------------------------------------------------------------
// `tmux-orchestration` — "Launch-readiness gate before boot-block injection":
// scenario "Restart from pause gates a held prompt".
// ---------------------------------------------------------------------------

/// A CLI stub whose pane prints a first-run dialog heading and then blocks
/// forever — the gate classifies it `DialogStuck`. Copied from
/// `tests/start_spec_launch_parity_integration.rs`, which pins the same
/// condition on the bare and spec-driven launch paths.
fn write_dialog_stub_cli(bin_dir: &std::path::Path) -> &'static str {
    use std::os::unix::fs::PermissionsExt;
    let script_path = bin_dir.join("paw-test-dialog");
    fs::write(
        &script_path,
        "#!/bin/sh\necho 'Bypass Permissions mode'\nwhile true; do sleep 5; done\n",
    )
    .expect("write dialog stub script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("chmod dialog stub script");
    "paw-test-dialog"
}

/// A session paused with a held boot prompt for an agent added while paused,
/// whose pane is blocked on an unanswered first-run dialog: the restart does
/// NOT submit the prompt on top of the dialog, keeps it held for the next
/// restart, names the blocked pane on stderr, and still restarts the rest of
/// the session.
#[test]
#[serial]
fn restart_from_pause_gates_a_held_prompt() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("gatehold");
    let repo = rename_repo_basename(&tr, &project);
    let session_name = format!("paw-{project}");

    let bin_dir = TempDir::new().expect("bin dir");
    let dialog_cli = write_dialog_stub_cli(bin_dir.path());
    let test_path = format!(
        "{}:{}",
        bin_dir.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    // `cat` for the pre-pause agent (it never shows a dialog, so the restart
    // has something that must still succeed); the stub for the agent added
    // while paused.
    //
    // The broker is OFF so the agent pane indices are stable across the
    // pause: pausing a broker-enabled session kills its dashboard pane, and
    // tmux renumbers the survivors, which `add` does not currently account for
    // (unrelated to the gate under test here).
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        format!(
            "default_cli = \"cat\"\n\n[broker]\nenabled = false\n\n\
             [clis.{dialog_cli}]\ncommand = \"{dialog_cli}\"\n"
        ),
    )
    .expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let sessions_dir = sessions_dir_for(home.path());
    let receipt = sessions_dir.join(format!("{session_name}.json"));

    let run = |args: &[&str], timeout_ms: Option<&str>| {
        let mut c = cmd();
        tmux_env.apply_assert(&mut c);
        c.current_dir(&repo)
            .env("PATH", &test_path)
            .env("HOME", home.path())
            .env_remove("XDG_DATA_HOME");
        if let Some(ms) = timeout_ms {
            c.env("GIT_PAW_READINESS_TIMEOUT_MS", ms);
        }
        c.args(args)
            .timeout(Duration::from_secs(60))
            .output()
            .expect("run git paw")
    };

    let start = run(&["start", "--cli", "cat", "--branches", "a"], None);
    assert!(
        start.status.success(),
        "initial start failed; stderr:\n{}",
        String::from_utf8_lossy(&start.stderr)
    );

    let pause = run(&["pause"], None);
    assert!(
        pause.status.success(),
        "pause failed; stderr:\n{}",
        String::from_utf8_lossy(&pause.stderr)
    );

    // Adding to a PAUSED session creates the pane and holds the boot prompt
    // for the restart rather than submitting it — the prompt this scenario is
    // about. The pane runs the dialog stub, so it is stuck.
    let add = run(&["add", "stuck-agent", "--cli", dialog_cli], None);
    assert!(
        add.status.success(),
        "add to the paused session failed; stderr:\n{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let held = fs::read_to_string(&receipt).expect("read receipt after add");
    assert!(
        held.contains("pending_boot_prompt"),
        "add while paused must hold a boot prompt for the restart; receipt:\n{held}"
    );

    // Restart. The readiness budget is raised over the suite-wide compressed
    // default for the same reason the bare/spec-driven dialog tests raise it:
    // a short budget races the gate's relaunch cycle and falls back to
    // injecting before the stub's dialog line renders.
    let restart = run(&["start"], Some("3000"));
    let restart_stdout = String::from_utf8_lossy(&restart.stdout).to_string();
    let restart_stderr = String::from_utf8_lossy(&restart.stderr).to_string();
    let after = fs::read_to_string(&receipt).expect("read receipt after restart");

    let mut kill = StdCommand::new("tmux");
    tmux_env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session_name]).status();

    // The rest of the session restarted: this is a warning, not a failure.
    assert!(
        restart_stdout.contains("Restarting paused session"),
        "the restart must run; stdout:\n{restart_stdout}\nstderr:\n{restart_stderr}"
    );
    assert!(
        restart_stderr.contains("acceptance dialog"),
        "stderr must report the blocked pane's condition; stderr:\n{restart_stderr}"
    );
    assert!(
        restart_stderr.contains("stuck-agent"),
        "stderr must name the blocked agent; stderr:\n{restart_stderr}"
    );
    assert!(
        restart_stderr.contains("stay held") && restart_stderr.contains("resumed from pause"),
        "stderr must say the prompt is kept for the next resume from pause; \
         stderr:\n{restart_stderr}"
    );

    // And the prompt is still held, so the next restart can submit it.
    assert!(
        after.contains("pending_boot_prompt"),
        "a dialog-stuck pane must KEEP its held prompt; receipt:\n{after}"
    );
}

/// Scenario "A held prompt whose pane cannot be found stays held": on restart
/// from pause, an agent whose worktree no longer resolves to a live pane keeps
/// its held prompt and is reported — the prompt is never submitted at a
/// computed index, where another pane's ready CLI would take it and its Enter.
///
/// The held agent's pane is replaced by a foreign pane running `cat` at the
/// SAME index, standing in for another agent's ready CLI: a submit by computed
/// index would type the boot prompt into it.
#[test]
#[serial]
fn restart_from_pause_keeps_a_held_prompt_whose_pane_is_gone() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("gonepane");
    let repo = rename_repo_basename(&tr, &project);
    let session_name = format!("paw-{project}");

    // Broker OFF so pane indices are stable across the pause (see
    // `restart_from_pause_gates_a_held_prompt`).
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n[clis.cat]\ncommand = \"cat\"\n\n[broker]\nenabled = false\n",
    )
    .expect("write config");

    let home = TempDir::new().expect("home tempdir");
    let receipt = sessions_dir_for(home.path()).join(format!("{session_name}.json"));
    let foreign_dir = TempDir::new().expect("foreign pane dir");

    let run = |args: &[&str]| {
        let mut c = cmd();
        tmux_env.apply_assert(&mut c);
        c.current_dir(&repo)
            .env("HOME", home.path())
            .env_remove("XDG_DATA_HOME")
            .args(args)
            .timeout(Duration::from_secs(60))
            .output()
            .expect("run git paw")
    };
    let tmux = |args: &[&str]| {
        let mut c = StdCommand::new("tmux");
        tmux_env.apply(&mut c);
        c.args(args).output().expect("run tmux")
    };

    let start = run(&["start", "--cli", "cat", "--branches", "a"]);
    assert!(
        start.status.success(),
        "initial start failed; stderr:\n{}",
        String::from_utf8_lossy(&start.stderr)
    );
    let pause = run(&["pause"]);
    assert!(
        pause.status.success(),
        "pause failed; stderr:\n{}",
        String::from_utf8_lossy(&pause.stderr)
    );
    let add = run(&["add", "b", "--cli", "cat"]);
    assert!(
        add.status.success(),
        "add to the paused session failed; stderr:\n{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let held = fs::read_to_string(&receipt).expect("read receipt after add");
    assert!(
        held.contains("pending_boot_prompt"),
        "add while paused must hold a boot prompt; receipt:\n{held}"
    );

    // Replace b's pane (pane 1: agents start at 0 with the broker off) with a
    // foreign `cat` pane at the same index, outside every worktree.
    let target = format!("{session_name}:0.1");
    let killed = tmux(&["kill-pane", "-t", &target]);
    assert!(killed.status.success(), "could not kill b's pane");
    let split = tmux(&[
        "split-window",
        "-h",
        "-t",
        &format!("{session_name}:0.0"),
        "-c",
        &foreign_dir.path().to_string_lossy(),
        "cat",
    ]);
    assert!(split.status.success(), "could not create the foreign pane");

    let restart = run(&["start"]);
    let restart_stderr = String::from_utf8_lossy(&restart.stderr).to_string();
    let after = fs::read_to_string(&receipt).expect("read receipt after restart");
    let foreign = tmux(&["capture-pane", "-p", "-S", "-", "-t", &target]);
    let foreign_text = String::from_utf8_lossy(&foreign.stdout).to_string();
    let _ = tmux(&["kill-session", "-t", &session_name]);

    assert!(
        restart_stderr.contains("b: pane not found"),
        "stderr must report the held agent whose pane is gone; stderr:\n{restart_stderr}"
    );
    assert!(
        after.contains("pending_boot_prompt"),
        "a held prompt whose pane cannot be found must stay held; receipt:\n{after}"
    );
    assert!(
        foreign_text.trim().is_empty(),
        "the boot prompt must not be typed into the pane at the computed index; \
         it shows:\n{foreign_text}"
    );
}

/// Scenario "An agent added while paused holds its entry point" — the held
/// half of it: when the held prompt STAYS held, its workflow entry point
/// stays held with it.
///
/// The pair is kept together structurally rather than by a rule: the entry
/// point is taken inside the same arm that takes its prompt, so a prompt the
/// restart does not submit can never reach the take of its entry point. This
/// pins that, using the same pane-not-found path as
/// `restart_from_pause_keeps_a_held_prompt_whose_pane_is_gone` — the held
/// agent's pane is replaced by a foreign `cat` pane at the same index, so a
/// submit by computed index would be visible in it.
///
/// Only testable once the restart resolves panes live and fails closed; both
/// arrived with `pane-less-session-parity`.
#[test]
#[serial]
fn restart_from_pause_keeps_a_held_entry_point_with_its_held_prompt() {
    if skip_if_no_tmux() {
        return;
    }

    let tr = setup_test_repo();
    let tmux_env = tmux_test_env();
    let project = unique_project_name("heldentry");
    let repo = rename_repo_basename(&tr, &project);
    let session_name = format!("paw-{project}");

    // Broker OFF so pane indices are stable across the pause; supervisor ON
    // so `add --from-spec` is available. A supervised launch builds the
    // dashboard at pane 0 either way, so agent `a` is pane 1 and the added
    // agent is pane 2.
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"cat\"\n\n\
         [specs]\ntype = \"openspec\"\ndir = \"specs\"\n\n\
         [broker]\nenabled = false\n\n\
         [supervisor]\nenabled = true\ncli = \"cat\"\n",
    )
    .expect("write config");

    // A pending OpenSpec change, so the added agent's format declares an
    // entry point to hold.
    let spec_id = "held-pair";
    let change_dir = repo.join("specs").join(spec_id);
    fs::create_dir_all(&change_dir).expect("create change dir");
    fs::write(change_dir.join("tasks.md"), "Implement it.\n").expect("write tasks.md");
    for args in [vec!["add", "."], vec!["commit", "-m", "add spec"]] {
        StdCommand::new("git")
            .current_dir(&repo)
            .args(&args)
            .output()
            .expect("git");
    }

    let home = TempDir::new().expect("home tempdir");
    let receipt = sessions_dir_for(home.path()).join(format!("{session_name}.json"));
    let foreign_dir = TempDir::new().expect("foreign pane dir");

    let run = |args: &[&str]| {
        let mut c = cmd();
        tmux_env.apply_assert(&mut c);
        c.current_dir(&repo)
            .env("HOME", home.path())
            .env_remove("XDG_DATA_HOME")
            .env("GIT_PAW_ENTRY_POINT_BUDGET_MS", "2000")
            .args(args)
            .timeout(Duration::from_secs(60))
            .output()
            .expect("run git paw")
    };
    let tmux = |args: &[&str]| {
        let mut c = StdCommand::new("tmux");
        tmux_env.apply(&mut c);
        c.args(args).output().expect("run tmux")
    };

    let start = run(&["start", "--supervisor", "--branches", "a", "--cli", "cat"]);
    assert!(
        start.status.success(),
        "initial start failed; stderr:\n{}",
        String::from_utf8_lossy(&start.stderr)
    );
    let pause = run(&["pause"]);
    assert!(
        pause.status.success(),
        "pause failed; stderr:\n{}",
        String::from_utf8_lossy(&pause.stderr)
    );
    let add = run(&["add", "--from-spec", spec_id]);
    assert!(
        add.status.success(),
        "add --from-spec to the paused session failed; stderr:\n{}",
        String::from_utf8_lossy(&add.stderr)
    );
    let held = fs::read_to_string(&receipt).expect("read receipt after add");
    assert!(
        held.contains("pending_boot_prompt"),
        "add while paused must hold a boot prompt; receipt:\n{held}"
    );
    assert!(
        held.contains("pending_entry_point"),
        "and an OpenSpec agent's entry point with it; receipt:\n{held}"
    );

    // Replace the added agent's pane (index 2) with a foreign `cat` pane at
    // the same index, outside every worktree.
    let target = format!("{session_name}:0.2");
    let killed = tmux(&["kill-pane", "-t", &target]);
    assert!(killed.status.success(), "could not kill the added pane");
    let split = tmux(&[
        "split-window",
        "-h",
        "-t",
        &format!("{session_name}:0.1"),
        "-c",
        &foreign_dir.path().to_string_lossy(),
        "cat",
    ]);
    assert!(split.status.success(), "could not create the foreign pane");

    let restart = run(&["start"]);
    let restart_stderr = String::from_utf8_lossy(&restart.stderr).to_string();
    let after = fs::read_to_string(&receipt).expect("read receipt after restart");
    let foreign = tmux(&["capture-pane", "-p", "-S", "-", "-t", &target]);
    let foreign_text = String::from_utf8_lossy(&foreign.stdout).to_string();
    let _ = tmux(&["kill-session", "-t", &session_name]);

    assert!(
        restart_stderr.contains("pane not found"),
        "stderr must report the held agent whose pane is gone; stderr:\n{restart_stderr}"
    );
    assert!(
        after.contains("pending_boot_prompt"),
        "a held prompt whose pane cannot be found must stay held; receipt:\n{after}"
    );
    assert!(
        after.contains("pending_entry_point"),
        "and its entry point must stay held WITH it — a submitted entry point \
         whose prompt was never submitted would arrive first, and a cleared one \
         would be lost for good; receipt:\n{after}"
    );
    assert!(
        after.contains(&format!("/opsx:apply {spec_id}")),
        "the held entry point is still the rendered command; receipt:\n{after}"
    );
    assert!(
        foreign_text.trim().is_empty(),
        "nothing may be typed into the pane at the computed index; it shows:\n{foreign_text}"
    );
}
