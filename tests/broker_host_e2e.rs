//! End-to-end tests for the broker host (`git paw __broker`) and the thin
//! dashboard client (`git paw __dashboard`) — `broker-runtime`'s *Broker host
//! subcommand*, *Dashboard subcommand runs a thin broker client* and the
//! host's *Signal handling* (`broker-host-separation`).
//!
//! The host runs as a plain child process with `$TMUX` unset. Anything that
//! needs a terminal (the dashboard, the "renders nothing" check) runs in a
//! pane of a tmux server on a test-owned private socket only.

mod helpers;

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use git_paw::broker::{BrokerHandle, BrokerState, start_broker};
use git_paw::config::BrokerConfig;
use helpers::{TmuxTestEnv, setup_test_repo, tmux_test_env};
use serial_test::serial;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const BIN: &str = env!("CARGO_BIN_EXE_git-paw");

/// The sessions directory git-paw resolves under a fake `HOME`.
fn sessions_dir_for(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/git-paw/sessions")
    } else {
        home.join(".local/share/git-paw/sessions")
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral")
        .local_addr()
        .expect("local_addr")
        .port()
}

/// Writes a `.git-paw/config.toml` enabling the broker on `port`.
fn write_broker_config(repo: &Path, port: u16) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(
        paw.join("config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\n"),
    )
    .expect("write config");
}

/// Saves a session receipt for `repo` under `home` with the given status.
fn save_session(home: &Path, repo: &Path, session_name: &str, port: u16, status: &str) {
    let dir = sessions_dir_for(home);
    fs::create_dir_all(&dir).expect("create sessions dir");
    let json = serde_json::json!({
        "session_name": session_name,
        "repo_path": repo.to_string_lossy(),
        "project_name": "repo",
        "created_at": "2026-05-01T00:00:00Z",
        "status": status,
        "worktrees": [],
        "broker_port": port,
        "broker_bind": "127.0.0.1",
    });
    fs::write(
        dir.join(format!("{session_name}.json")),
        serde_json::to_string_pretty(&json).expect("serialize session"),
    )
    .expect("write session json");
}

/// One repository + fake home + broker port for a test.
struct Fixture {
    _repo: helpers::TestRepo,
    root: PathBuf,
    home: TempDir,
    port: u16,
}

impl Fixture {
    fn new() -> Self {
        let repo = setup_test_repo();
        let root = repo.path().canonicalize().expect("canonicalize repo");
        let port = free_port();
        write_broker_config(&root, port);
        Self {
            _repo: repo,
            root,
            home: TempDir::new().expect("fake home"),
            port,
        }
    }

    fn log_path(&self, session: &str) -> PathBuf {
        sessions_dir_for(self.home.path()).join(format!("{session}.broker.log"))
    }
}

/// Spawns `git-paw __broker` for `session` with `$TMUX` unset and state
/// isolated under the fixture's fake home.
fn spawn_host(fx: &Fixture, session: &str) -> Child {
    fs::create_dir_all(sessions_dir_for(fx.home.path())).expect("sessions dir");
    Command::new(BIN)
        .current_dir(&fx.root)
        .env("HOME", fx.home.path())
        .env_remove("XDG_DATA_HOME")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .args(["__broker", "--session", session, "--log-path"])
        .arg(fx.log_path(session))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn git-paw __broker")
}

/// Raw HTTP/1.1 request; returns `(status, body)`, or `None` when nothing
/// answers.
fn http(port: u16, method: &str, path: &str, body: &str) -> Option<(u16, String)> {
    use std::fmt::Write as _;

    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let mut req =
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    if !body.is_empty() {
        let _ = write!(
            req,
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).ok()?;
    let code = resp.get(9..12)?.parse().ok()?;
    let body = resp.split_once("\r\n\r\n").map(|(_, b)| b.to_string())?;
    Some((code, body))
}

/// `GET /status` parsed, when the broker answers.
fn status(port: u16) -> Option<serde_json::Value> {
    let (code, body) = http(port, "GET", "/status", "")?;
    (code == 200).then(|| serde_json::from_str(&body).ok())?
}

/// Waits up to `budget` for `cond`.
fn wait_until(budget: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < budget {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    cond()
}

fn publish_status(port: u16, agent: &str, message: &str) {
    let body = format!(
        r#"{{"type":"agent.status","agent_id":"{agent}","payload":{{"status":"working","modified_files":[],"message":"{message}"}}}}"#
    );
    let (code, _) = http(port, "POST", "/publish", &body).expect("publish reaches the broker");
    assert_eq!(code, 202, "publish must be accepted");
}

fn signal(child: &Child, sig: &str) {
    let ok = Command::new("kill")
        .args([&format!("-{sig}"), &child.id().to_string()])
        .status()
        .expect("run kill")
        .success();
    assert!(ok, "kill -{sig} must reach the host");
}

/// Waits up to `budget` for `child` to exit, killing it otherwise.
fn wait_exit(child: &mut Child, budget: Duration) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    while start.elapsed() < budget {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

fn port_free(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("addr"),
        Duration::from_millis(200),
    )
    .is_err()
}

/// Runs `tmux args` on the private socket.
fn tmux(env: &TmuxTestEnv, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("tmux");
    env.apply(&mut cmd);
    cmd.args(args).output().expect("run tmux")
}

/// Starts a private-socket tmux server whose single pane runs `git-paw
/// <sub>` in `repo` under `home`, with `$TMUX` stripped from the git-paw
/// process. The pane's exit status is written to `exit_file`.
fn pane_running(
    env: &TmuxTestEnv,
    session: &str,
    repo: &Path,
    home: &Path,
    sub: &str,
    exit_file: &Path,
) {
    let shell = format!(
        "env -u TMUX -u TMUX_PANE -u XDG_DATA_HOME HOME='{}' '{BIN}' {sub}; echo $? > '{}'; sleep 60",
        home.display(),
        exit_file.display()
    );
    let mut cmd = Command::new("tmux");
    env.apply(&mut cmd);
    let out = cmd
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .args(["-f", "/dev/null", "new-session", "-d", "-s", session])
        .args(["-x", "160", "-y", "40", "-c"])
        .arg(repo)
        .arg(shell)
        .output()
        .expect("start tmux session");
    assert!(
        out.status.success(),
        "tmux new-session failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn capture(env: &TmuxTestEnv, session: &str) -> String {
    String::from_utf8_lossy(&tmux(env, &["capture-pane", "-p", "-t", session]).stdout).to_string()
}

fn kill_server(env: &TmuxTestEnv) {
    let _ = tmux(env, &["kill-server"]);
}

/// An in-process broker for `repo` on `port`.
fn live_broker(repo: &Path, port: u16) -> BrokerHandle {
    let config = BrokerConfig {
        enabled: true,
        port,
        bind: "127.0.0.1".to_string(),
        ..Default::default()
    };
    start_broker(
        &config,
        BrokerState::new(None).with_repo_root(repo),
        Vec::new(),
    )
    .expect("start in-process broker")
}

// ---------------------------------------------------------------------------
// Broker host subcommand
// ---------------------------------------------------------------------------

/// Scenario "Host serves the broker" (and "Host runs outside tmux": the host
/// is spawned with `$TMUX` unset).
#[test]
#[serial]
fn host_serves_the_broker() {
    let fx = Fixture::new();
    save_session(fx.home.path(), &fx.root, "paw-repo", fx.port, "active");
    let mut host = spawn_host(&fx, "paw-repo");

    assert!(
        wait_until(Duration::from_secs(5), || status(fx.port).is_some()),
        "the host must answer /status within 5s"
    );
    let st = status(fx.port).expect("status");
    assert_eq!(st["git_paw"], true);
    assert_eq!(
        Path::new(st["repo_root"].as_str().expect("repo_root")),
        fx.root,
        "the host serves this repository"
    );
    assert!(
        host.try_wait().expect("try_wait").is_none(),
        "the host keeps running"
    );

    signal(&host, "HUP");
    wait_exit(&mut host, Duration::from_secs(10));
}

/// Scenario "Host runs outside tmux".
#[test]
#[serial]
fn host_runs_outside_tmux() {
    let fx = Fixture::new();
    // `spawn_host` removes `$TMUX` and `$TMUX_PANE` from the host's env.
    let mut host = spawn_host(&fx, "paw-outside");
    assert!(
        wait_until(Duration::from_secs(5), || status(fx.port).is_some()),
        "a host started without $TMUX must still serve /status"
    );
    signal(&host, "HUP");
    wait_exit(&mut host, Duration::from_secs(10));
}

/// Scenario "Host of a fresh launch writes the per-session log".
#[test]
#[serial]
fn host_of_a_fresh_launch_writes_the_per_session_log() {
    let fx = Fixture::new();
    let mut host = spawn_host(&fx, "paw-demo");
    assert!(wait_until(Duration::from_secs(5), || status(fx.port).is_some()));

    publish_status(fx.port, "feat-fresh-marker", "fresh launch");
    signal(&host, "HUP");
    assert!(
        wait_exit(&mut host, Duration::from_secs(10)).is_some(),
        "the host exits on SIGHUP"
    );

    let log = fs::read_to_string(fx.log_path("paw-demo")).expect("per-session log written");
    assert!(log.contains("[feat-fresh-marker]"), "got:\n{log}");
    assert!(
        !sessions_dir_for(fx.home.path())
            .join("git-paw.broker.log")
            .exists(),
        "nothing may be written to the git-paw.broker.log fallback"
    );
}

/// Scenario "SIGHUP shuts the host down with a final flush".
#[test]
#[serial]
fn sighup_shuts_the_host_down_with_a_final_flush() {
    let fx = Fixture::new();
    let mut host = spawn_host(&fx, "paw-flush");
    assert!(wait_until(Duration::from_secs(5), || status(fx.port).is_some()));

    for i in 0..3 {
        publish_status(fx.port, &format!("feat-flush-{i}"), "flush");
    }
    signal(&host, "HUP");
    assert!(
        wait_until(Duration::from_secs(5), || port_free(fx.port)),
        "the port must be free within 5s of SIGHUP"
    );
    wait_exit(&mut host, Duration::from_secs(10));

    let log = fs::read_to_string(fx.log_path("paw-flush")).expect("log written");
    for i in 0..3 {
        assert!(
            log.contains(&format!("[feat-flush-{i}]")),
            "message {i} missing; got:\n{log}"
        );
    }
}

/// `Signal handling` scenario "SIGINT does not terminate the broker", on the
/// host process.
#[test]
#[serial]
fn sigint_does_not_terminate_the_host() {
    let fx = Fixture::new();
    let mut host = spawn_host(&fx, "paw-sigint");
    assert!(wait_until(Duration::from_secs(5), || status(fx.port).is_some()));

    signal(&host, "INT");
    std::thread::sleep(Duration::from_millis(800));
    assert!(
        host.try_wait().expect("try_wait").is_none(),
        "SIGINT must not terminate the host"
    );
    assert!(
        status(fx.port).is_some(),
        "the broker keeps answering after SIGINT"
    );

    signal(&host, "HUP");
    wait_exit(&mut host, Duration::from_secs(10));
}

/// Scenario "Bind failure exits instead of looping".
#[test]
#[serial]
fn bind_failure_exits_instead_of_looping() {
    let fx = Fixture::new();
    let listener = TcpListener::bind(("127.0.0.1", fx.port)).expect("occupy the port");
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_srv = std::sync::Arc::clone(&stop);
    listener.set_nonblocking(true).expect("nonblocking");
    let server = std::thread::spawn(move || {
        while !stop_srv.load(std::sync::atomic::Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut s, _)) => {
                    s.set_nonblocking(false).ok();
                    s.set_read_timeout(Some(Duration::from_millis(500))).ok();
                    let mut buf = [0u8; 1024];
                    let _ = s.read(&mut buf);
                    let body = "hello from a foreign server";
                    let _ = s.write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    );
                }
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    });

    let mut host = spawn_host(&fx, "paw-bind");
    let exit = wait_exit(&mut host, Duration::from_secs(10));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    server.join().expect("server thread");

    let exit = exit.expect("the host must exit rather than keep running");
    assert!(!exit.success(), "a bind failure exits non-zero");
    let mut stderr = String::new();
    host.stderr
        .take()
        .expect("stderr piped")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    assert!(
        stderr.contains(&fx.port.to_string()),
        "the diagnostic must name the port; got:\n{stderr}"
    );
}

/// Scenario "Host renders nothing": attached to a pseudo-terminal for 2
/// seconds, the host never enters the alternate screen or draws a TUI.
#[test]
#[serial]
fn host_renders_nothing() {
    let fx = Fixture::new();
    fs::create_dir_all(sessions_dir_for(fx.home.path())).expect("sessions dir");
    let env = tmux_test_env();
    let exit_file = fx.home.path().join("host-exit");
    let sub = format!(
        "__broker --session paw-render --log-path '{}'",
        fx.log_path("paw-render").display()
    );
    pane_running(&env, "host", &fx.root, fx.home.path(), &sub, &exit_file);

    assert!(
        wait_until(Duration::from_secs(5), || status(fx.port).is_some()),
        "the host in the pane must serve"
    );
    std::thread::sleep(Duration::from_secs(2));
    let alt = tmux(&env, &["display", "-p", "-t", "host", "#{alternate_on}"]);
    let alt = String::from_utf8_lossy(&alt.stdout).trim().to_string();
    let screen = capture(&env, "host");
    kill_server(&env);
    assert!(wait_until(Duration::from_secs(5), || port_free(fx.port)));

    assert_eq!(alt, "0", "the host must never enter the alternate screen");
    assert!(
        !screen.contains("agents:") && !screen.contains('\u{2500}'),
        "the host must not draw a TUI; got:\n{screen}"
    );
}

// ---------------------------------------------------------------------------
// Dashboard subcommand runs a thin broker client
// ---------------------------------------------------------------------------

/// Scenario "Dashboard with no live broker renders degraded".
#[test]
#[serial]
fn dashboard_with_no_live_broker_renders_degraded() {
    let fx = Fixture::new();
    save_session(fx.home.path(), &fx.root, "paw-repo", fx.port, "active");
    let env = tmux_test_env();
    let exit_file = fx.home.path().join("dash-exit");
    pane_running(
        &env,
        "dash",
        &fx.root,
        fx.home.path(),
        "__dashboard",
        &exit_file,
    );

    let degraded = wait_until(Duration::from_secs(8), || {
        capture(&env, "dash").contains("broker unreachable")
    });
    let screen = capture(&env, "dash");
    tmux(&env, &["send-keys", "-t", "dash", "q"]);
    let exited = wait_until(Duration::from_secs(5), || exit_file.exists());
    let code = fs::read_to_string(&exit_file).unwrap_or_default();
    kill_server(&env);

    assert!(
        degraded,
        "the dashboard must show the indicator; got:\n{screen}"
    );
    assert!(exited, "`q` must exit the dashboard");
    assert_eq!(code.trim(), "0", "the dashboard exits with status 0");
}

/// Drives a dashboard pane against a live in-process broker carrying agent
/// `feat-x`, returning whether the agent row rendered.
fn dashboard_renders_live_agent(status: &str, save_before: bool) -> (bool, String) {
    let fx = Fixture::new();
    let _broker = live_broker(&fx.root, fx.port);
    publish_status(fx.port, "feat-x", "live-row");
    if save_before {
        save_session(fx.home.path(), &fx.root, "paw-repo", fx.port, status);
    }
    let env = tmux_test_env();
    let exit_file = fx.home.path().join("dash-exit");
    pane_running(
        &env,
        "dash",
        &fx.root,
        fx.home.path(),
        "__dashboard",
        &exit_file,
    );
    if !save_before {
        // A fresh launch saves its receipt after the panes exist.
        std::thread::sleep(Duration::from_millis(1500));
        save_session(fx.home.path(), &fx.root, "paw-repo", fx.port, status);
    }
    let rendered = wait_until(Duration::from_secs(8), || {
        capture(&env, "dash").contains("feat-x")
    });
    let screen = capture(&env, "dash");
    kill_server(&env);
    (rendered, screen)
}

/// Scenario "Dashboard runs outside tmux": the dashboard process runs with
/// `$TMUX` unset (`env -u TMUX`) and renders the live agent table.
#[test]
#[serial]
fn dashboard_runs_outside_tmux() {
    let (rendered, screen) = dashboard_renders_live_agent("active", true);
    assert!(rendered, "the agent row must render; got:\n{screen}");
}

/// Scenario "Dashboard of a session still recorded paused connects".
#[test]
#[serial]
fn dashboard_of_a_session_still_recorded_paused_connects() {
    let (rendered, screen) = dashboard_renders_live_agent("paused", true);
    assert!(
        rendered,
        "a Paused receipt must still resolve the broker URL; got:\n{screen}"
    );
}

/// Scenario "Dashboard started before the receipt connects once it is
/// saved".
#[test]
#[serial]
fn dashboard_started_before_the_receipt_connects_once_it_is_saved() {
    let (rendered, screen) = dashboard_renders_live_agent("active", false);
    assert!(
        rendered,
        "the dashboard must resolve the URL once the receipt is saved; got:\n{screen}"
    );
}

/// Spawns a real host for `session` and waits for it to serve.
fn serving_host(fx: &Fixture, session: &str) -> Child {
    save_session(fx.home.path(), &fx.root, session, fx.port, "active");
    let host = spawn_host(fx, session);
    assert!(
        wait_until(Duration::from_secs(5), || status(fx.port).is_some()),
        "precondition: the host serves"
    );
    host
}

/// Scenario "Dashboard holds no broker": with the broker in its own host
/// process, killing the dashboard pane leaves the broker serving.
#[test]
#[serial]
fn dashboard_holds_no_broker() {
    let fx = Fixture::new();
    let mut host = serving_host(&fx, "paw-repo");
    publish_status(fx.port, "feat-x", "row");
    let env = tmux_test_env();
    let exit_file = fx.home.path().join("dash-exit");
    pane_running(
        &env,
        "dash",
        &fx.root,
        fx.home.path(),
        "__dashboard",
        &exit_file,
    );
    assert!(
        wait_until(Duration::from_secs(8), || capture(&env, "dash")
            .contains("feat-x")),
        "precondition: the dashboard renders from the host"
    );
    kill_server(&env);
    std::thread::sleep(Duration::from_millis(500));
    let serving = status(fx.port).is_some();
    let alive = host.try_wait().expect("try_wait").is_none();
    signal(&host, "HUP");
    wait_exit(&mut host, Duration::from_secs(10));
    assert!(
        serving,
        "/status must keep answering after the dashboard is killed"
    );
    assert!(alive, "the host outlives the dashboard");
}

/// Scenario "Dashboard starts and stops cleanly": with a live broker, `q`
/// exits the dashboard with status 0 and the broker is still serving.
#[test]
#[serial]
fn dashboard_starts_and_stops_cleanly_with_a_live_broker() {
    let fx = Fixture::new();
    let mut host = serving_host(&fx, "paw-repo");
    publish_status(fx.port, "feat-x", "row");
    let env = tmux_test_env();
    let exit_file = fx.home.path().join("dash-exit");
    pane_running(
        &env,
        "dash",
        &fx.root,
        fx.home.path(),
        "__dashboard",
        &exit_file,
    );
    let rendered = wait_until(Duration::from_secs(8), || {
        capture(&env, "dash").contains("feat-x")
    });
    tmux(&env, &["send-keys", "-t", "dash", "q"]);
    let exited = wait_until(Duration::from_secs(5), || exit_file.exists());
    let code = fs::read_to_string(&exit_file).unwrap_or_default();
    kill_server(&env);
    let serving = status(fx.port).is_some();
    signal(&host, "HUP");
    wait_exit(&mut host, Duration::from_secs(10));

    assert!(rendered, "the dashboard renders live data");
    assert!(exited, "`q` exits the dashboard");
    assert_eq!(code.trim(), "0", "the dashboard exits with status 0");
    assert!(
        serving,
        "the broker is still serving after the dashboard quit"
    );
}

/// A session saved without a broker (a supervised launch with the broker
/// disabled) says "broker disabled", not "broker unreachable".
#[test]
#[serial]
fn dashboard_of_a_session_without_a_broker_shows_broker_disabled() {
    let fx = Fixture::new();
    let dir = sessions_dir_for(fx.home.path());
    fs::create_dir_all(&dir).expect("sessions dir");
    let json = serde_json::json!({
        "session_name": "paw-repo",
        "repo_path": fx.root.to_string_lossy(),
        "project_name": "repo",
        "created_at": "2026-05-01T00:00:00Z",
        "status": "active",
        "worktrees": [],
    });
    fs::write(
        dir.join("paw-repo.json"),
        serde_json::to_string_pretty(&json).expect("json"),
    )
    .expect("write receipt");
    let env = tmux_test_env();
    let exit_file = fx.home.path().join("dash-exit");
    pane_running(
        &env,
        "dash",
        &fx.root,
        fx.home.path(),
        "__dashboard",
        &exit_file,
    );
    let disabled = wait_until(Duration::from_secs(8), || {
        capture(&env, "dash").contains("broker disabled")
    });
    let screen = capture(&env, "dash");
    kill_server(&env);
    assert!(disabled, "got:\n{screen}");
    assert!(!screen.contains("broker unreachable"), "got:\n{screen}");
}

/// A receipt with the host's session name but another repository's path is
/// not this host's: the host serves this repository's config port, not the
/// other receipt's.
#[test]
#[serial]
fn host_ignores_a_same_named_receipt_of_another_repository() {
    let fx = Fixture::new();
    let other_repo = TempDir::new().expect("other repo");
    let other_port = free_port();
    save_session(
        fx.home.path(),
        &other_repo.path().canonicalize().expect("canonical"),
        "paw-demo",
        other_port,
        "stopped",
    );
    let mut host = spawn_host(&fx, "paw-demo");
    let ours = wait_until(Duration::from_secs(5), || status(fx.port).is_some());
    let theirs = status(other_port).is_some();
    signal(&host, "HUP");
    wait_exit(&mut host, Duration::from_secs(10));
    assert!(ours, "the host serves this repository's config port");
    assert!(
        !theirs,
        "the host must not bind the other repository's port"
    );
}

/// "...and so SHALL finding this repository's broker already served by
/// another process": the host exits non-zero naming the port instead of
/// idling beside it.
#[test]
#[serial]
fn host_exits_when_this_repositorys_broker_already_serves_the_port() {
    let fx = Fixture::new();
    let _existing = live_broker(&fx.root, fx.port);
    let mut host = spawn_host(&fx, "paw-dup");
    let exit = wait_exit(&mut host, Duration::from_secs(5));
    let exit = exit.expect("the host must exit rather than idle");
    assert!(!exit.success(), "it exits non-zero");
    let mut stderr = String::new();
    host.stderr
        .take()
        .expect("stderr piped")
        .read_to_string(&mut stderr)
        .expect("read stderr");
    assert!(
        stderr.contains(&fx.port.to_string()),
        "the diagnostic names the port; got:\n{stderr}"
    );
}
