//! E2E tests for the broker host's lifecycle across start, recovery, pause,
//! restart-from-pause, stop and status (`broker-host-separation`,
//! `broker-runtime` *Broker host lifecycle across start, recovery, pause,
//! stop* and the MODIFIED stop / pause / restart / status requirements).
//!
//! Every tmux invocation — direct, or through the `git-paw` binary — goes
//! through a [`helpers::TmuxTestEnv`] private socket; nothing here touches the
//! default tmux server.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::{TestRepo, TmuxTestEnv, setup_test_repo, tmux_test_env};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn tmux_available() -> bool {
    which::which("tmux").is_ok()
}

/// An ephemeral loopback port, released for the broker to bind.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

/// One isolated sandbox: a repository with a broker config, a private tmux
/// socket and a private `HOME` for session state.
struct Sandbox {
    repo: TestRepo,
    tmux: TmuxTestEnv,
    home: TempDir,
    port: u16,
}

impl Sandbox {
    fn new(extra_config: &str) -> Self {
        let repo = setup_test_repo();
        let tmux = tmux_test_env();
        let home = TempDir::new().expect("home tempdir");
        let port = free_port();
        let paw_dir = repo.path().join(".git-paw");
        fs::create_dir_all(&paw_dir).expect("create .git-paw");
        fs::write(
            paw_dir.join("config.toml"),
            format!(
                "default_cli = \"cat\"\n[clis.cat]\ncommand = \"cat\"\n{extra_config}\n[broker]\nenabled = true\nport = {port}\n"
            ),
        )
        .expect("write config");
        Self {
            repo,
            tmux,
            home,
            port,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Rewrites the repository config's `[broker]` section.
    fn write_broker_config(&self, broker: &str) {
        fs::write(
            self.repo.path().join(".git-paw").join("config.toml"),
            format!("default_cli = \"cat\"\n[clis.cat]\ncommand = \"cat\"\n[broker]\n{broker}\n"),
        )
        .expect("rewrite config");
    }

    fn session_alive(&self) -> bool {
        self.tmux(&["has-session", "-t", &self.session_name()]).0
    }

    /// `tmux display -p <format>` for `target` on the private socket.
    fn display(&self, target: &str, format: &str) -> String {
        self.tmux(&["display-message", "-p", "-t", target, format])
            .1
            .trim()
            .to_string()
    }

    /// Runs `git-paw <args>` in the repository, isolated.
    fn paw(&self, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::cargo_bin("git-paw").expect("binary exists");
        self.tmux.apply_assert(&mut cmd);
        cmd.current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .args(args)
            .timeout(Duration::from_secs(40))
            .output()
            .expect("run git-paw")
    }

    /// Runs `git-paw <args>` and requires success.
    fn paw_ok(&self, args: &[&str]) -> String {
        let out = self.paw(args);
        assert!(
            out.status.success(),
            "git paw {args:?} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    /// Runs `tmux <args>` on the private socket, returning stdout and success.
    fn tmux(&self, args: &[&str]) -> (bool, String) {
        let out = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", self.tmux.socket_dir())
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .args(args)
            .output()
            .expect("run tmux");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
        )
    }

    fn session_name(&self) -> String {
        format!("paw-{}", git_paw::git::project_name(self.repo.path()))
    }

    fn window_names(&self) -> Vec<String> {
        let (_, out) = self.tmux(&[
            "list-windows",
            "-t",
            &self.session_name(),
            "-F",
            "#{window_name}",
        ]);
        out.lines().map(str::to_string).collect()
    }

    fn has_broker_window(&self) -> bool {
        self.window_names().iter().any(|w| w == "broker")
    }

    /// `(pane_index, pane_current_path)` of window 0.
    fn window0_panes(&self) -> Vec<(String, String)> {
        let (_, out) = self.tmux(&[
            "list-panes",
            "-t",
            &format!("{}:0", self.session_name()),
            "-F",
            "#{pane_index}|#{pane_current_path}",
        ]);
        out.lines()
            .filter_map(|l| {
                let (i, p) = l.split_once('|')?;
                Some((i.to_string(), p.to_string()))
            })
            .collect()
    }

    fn receipt_path(&self) -> Option<PathBuf> {
        find_file(self.home.path(), &format!("{}.json", self.session_name()))
    }

    fn receipt(&self) -> Option<serde_json::Value> {
        self.receipt_path()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
    }
}

/// Finds `name` anywhere under `dir`.
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().is_some_and(|n| n == name) {
            return Some(path);
        }
    }
    None
}

/// Raw HTTP/1.1 exchange; `None` when nothing answers.
fn http(port: u16, method: &str, path: &str, body: &str) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("addr"),
        Duration::from_millis(500),
    )
    .ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let req = if body.is_empty() {
        format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
    } else {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    };
    stream.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
    let code = resp.get(9..12)?.parse().ok()?;
    let body = resp
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    Some((code, body))
}

fn status_json(port: u16) -> Option<serde_json::Value> {
    let (code, body) = http(port, "GET", "/status", "")?;
    (code == 200).then(|| serde_json::from_str(&body).ok())?
}

fn publish_status(port: u16, agent: &str) {
    let body = format!(
        r#"{{"type":"agent.status","agent_id":"{agent}","payload":{{"status":"working","modified_files":[]}}}}"#
    );
    let (code, _) = http(port, "POST", "/publish", &body).expect("broker answers /publish");
    assert_eq!(code, 202, "publish for {agent} accepted");
}

/// Polls `cond` every 100ms until it holds or `budget` elapses.
fn wait_until(budget: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + budget;
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn port_refuses(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("addr"),
        Duration::from_millis(200),
    )
    .is_err()
}

fn canonical(path: &Path) -> String {
    path.canonicalize()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

/// Scenario "Start launches the host before the first boot prompt": the
/// session gains a `broker` window running `git-paw __broker`, `/status`
/// answers for this repository, and the receipt (saved only after the host
/// answered) exists.
#[test]
#[serial]
fn start_launches_the_host_before_the_first_boot_prompt() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);

    assert!(sb.has_broker_window(), "windows: {:?}", sb.window_names());
    let (_, start_cmd) = sb.tmux(&[
        "list-panes",
        "-t",
        &format!("{}:broker", sb.session_name()),
        "-F",
        "#{pane_start_command}",
    ]);
    assert!(
        start_cmd.contains("__broker"),
        "the broker window runs `git-paw __broker`; got {start_cmd:?}"
    );

    let status = status_json(sb.port).expect("/status answers after start");
    assert_eq!(status["git_paw"], true);
    assert_eq!(
        canonical(Path::new(status["repo_root"].as_str().unwrap_or_default())),
        canonical(sb.repo.path()),
        "the host serves this repository"
    );
    assert!(
        sb.receipt().is_some(),
        "the receipt is saved after the host answered"
    );
}

/// Scenario "Pane indices are unaffected by the host": a supervised launch
/// with 3 agents keeps exactly 4 panes in window 0, dashboard at index 0.
#[test]
#[serial]
fn pane_indices_are_unaffected_by_the_host() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("[supervisor]\nenabled = true\ncli = \"cat\"\n");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&[
        "start",
        "--supervisor",
        "--cli",
        "cat",
        "--branches",
        "a,b,c",
    ]);

    let panes = sb.window0_panes();
    let indices: Vec<&str> = panes.iter().map(|(i, _)| i.as_str()).collect();
    assert_eq!(indices, ["0", "1", "2", "3"], "dashboard + 3 agents");
    assert_eq!(
        canonical(Path::new(&panes[0].1)),
        canonical(sb.repo.path()),
        "pane 0 is the dashboard at the repo root"
    );
    assert!(sb.has_broker_window(), "the host lives outside window 0");
}

/// A foreign HTTP server holding `port` until the returned flag is cleared.
fn hold_port_with_foreign_server(port: u16) -> Arc<AtomicBool> {
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("hold port");
    listener.set_nonblocking(true).expect("nonblocking");
    let running = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&running);
    std::thread::spawn(move || {
        while flag.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_nonblocking(false);
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                    let mut buf = [0u8; 1024];
                    let _ = stream.read(&mut buf);
                    let body = "<html>not git-paw</html>";
                    let _ = stream.write_all(
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
    running
}

/// Runs a fresh start whose broker port is held by a foreign server.
fn start_against_a_held_port(sb: &Sandbox) -> (std::process::Output, Duration) {
    let started = Instant::now();
    let out = sb.paw(&["start", "--cli", "cat", "--branches", "a"]);
    (out, started.elapsed())
}

/// Scenario "A launch that fails after the host starts saves no receipt":
/// the host cannot serve (its port is held), the launch fails, and no
/// receipt is saved.
#[test]
#[serial]
fn a_launch_that_fails_after_the_host_starts_saves_no_receipt() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    let holder = hold_port_with_foreign_server(sb.port);
    let (out, _) = start_against_a_held_port(&sb);
    holder.store(false, Ordering::Relaxed);

    assert!(
        !out.status.success(),
        "start must fail; stdout:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        sb.receipt_path().is_none(),
        "no session receipt may be saved for a failed launch"
    );
    assert!(
        !sb.session_alive(),
        "the session the failed launch opened must be closed, since no receipt names it"
    );
}

/// Scenario "Host that never answers fails the launch clearly": the launch
/// fails within the bound, naming the broker port.
#[test]
#[serial]
fn host_that_never_answers_fails_the_launch_clearly() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    let holder = hold_port_with_foreign_server(sb.port);
    let (out, elapsed) = start_against_a_held_port(&sb);
    holder.store(false, Ordering::Relaxed);

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(!out.status.success(), "start must fail; stderr:\n{stderr}");
    assert!(
        stderr.contains(&sb.port.to_string()),
        "the error names the broker port {}; stderr:\n{stderr}",
        sb.port
    );
    assert!(
        elapsed < Duration::from_secs(30),
        "the launch fails within the bound; took {elapsed:?}"
    );
}

/// Scenario "A fresh launch registers its agents with the host": a fresh
/// host has no receipt, so the agent reaches it through `POST /watch`; its
/// worktree activity then surfaces it in `/status`.
#[test]
#[serial]
fn a_fresh_launch_registers_its_agents_with_the_host() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat/a"]);

    let receipt = sb.receipt().expect("receipt saved");
    let wt = receipt["worktrees"][0]["worktree_path"]
        .as_str()
        .expect("worktree path")
        .to_string();
    fs::write(Path::new(&wt).join("untracked-activity.txt"), "x").expect("dirty worktree");

    let listed = wait_until(Duration::from_secs(15), || {
        status_json(sb.port).is_some_and(|s| {
            s["agents"]
                .as_array()
                .is_some_and(|a| a.iter().any(|r| r["agent_id"] == "feat-a"))
        })
    });
    assert!(
        listed,
        "the registered agent appears in /status from worktree activity; got {:?}",
        status_json(sb.port)
    );
}

// ---------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------

/// Scenario "Recovery recreates the host": after the session dies, `git paw
/// start` recovers it with a `broker` window, and `/status` answers.
#[test]
#[serial]
fn recovery_recreates_the_host() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);
    sb.paw_ok(&["stop"]);
    assert!(
        wait_until(Duration::from_secs(10), || port_refuses(sb.port)),
        "precondition: the stopped host freed the port"
    );

    sb.paw_ok(&["start"]);
    assert!(sb.has_broker_window(), "windows: {:?}", sb.window_names());
    assert!(
        wait_until(Duration::from_secs(10), || status_json(sb.port).is_some()),
        "the recovered host answers /status"
    );
}

// ---------------------------------------------------------------------------
// Pause / restart
// ---------------------------------------------------------------------------

fn dashboard_screen(sb: &Sandbox) -> String {
    sb.tmux(&[
        "capture-pane",
        "-p",
        "-t",
        &format!("{}:0.0", sb.session_name()),
    ])
    .1
}

/// Scenario "Pause stops the host and keeps the dashboard": the `broker`
/// window goes, the port frees, and pane 0 still runs the dashboard, now
/// showing the broker-unreachable state.
#[test]
#[serial]
fn pause_stops_the_host_and_keeps_the_dashboard() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);
    let panes_before = sb.window0_panes().len();

    sb.paw_ok(&["pause"]);

    assert!(
        wait_until(Duration::from_secs(5), || !sb.has_broker_window()),
        "the broker window is gone; windows: {:?}",
        sb.window_names()
    );
    assert!(
        wait_until(Duration::from_secs(5), || port_refuses(sb.port)),
        "the broker port is free after pause"
    );
    assert_eq!(
        sb.window0_panes().len(),
        panes_before,
        "pause leaves the dashboard and agent panes"
    );
    assert!(
        wait_until(Duration::from_secs(8), || dashboard_screen(&sb)
            .contains("broker unreachable")),
        "the dashboard renders the unreachable state; screen:\n{}",
        dashboard_screen(&sb)
    );
}

/// Scenarios "Start against paused session reattaches and restarts broker"
/// and "Existing dashboard pane is reused".
#[test]
#[serial]
fn start_against_paused_session_restarts_the_host_without_a_second_dashboard() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);
    let panes_before = sb.window0_panes().len();
    let pane0 = format!("{}:0.0", sb.session_name());
    let dashboard_pid = sb.display(&pane0, "#{pane_pid}");
    assert!(!dashboard_pid.is_empty(), "pane 0 has a process");
    sb.paw_ok(&["pause"]);
    assert!(wait_until(Duration::from_secs(5), || port_refuses(sb.port)));
    assert_eq!(
        sb.display(&pane0, "#{pane_pid}"),
        dashboard_pid,
        "pause leaves the dashboard pane running"
    );

    sb.paw_ok(&["start"]);

    assert!(
        wait_until(Duration::from_secs(10), || status_json(sb.port).is_some()),
        "the restarted host answers /status"
    );
    assert_eq!(
        sb.window0_panes().len(),
        panes_before,
        "no second dashboard pane is created"
    );
    assert_eq!(
        sb.display(&pane0, "#{pane_pid}"),
        dashboard_pid,
        "the restart reuses the paused dashboard, not a new one"
    );
    assert_eq!(
        sb.receipt().expect("receipt")["status"],
        "active",
        "the session is active again"
    );
    assert!(
        wait_until(Duration::from_secs(8), || !dashboard_screen(&sb)
            .contains("broker unreachable")),
        "the existing dashboard reconnects; screen:\n{}",
        dashboard_screen(&sb)
    );
}

// ---------------------------------------------------------------------------
// Stop / status
// ---------------------------------------------------------------------------

/// Scenario "Stop kills tmux and broker shuts down": the port frees and the
/// host's final flush leaves every accepted message in the session's log.
#[test]
#[serial]
fn stop_kills_the_session_and_the_host_flushes_the_log() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);
    publish_status(sb.port, "flush-probe-agent");
    let log_path = PathBuf::from(
        sb.receipt().expect("receipt")["broker_log_path"]
            .as_str()
            .expect("broker_log_path recorded")
            .to_string(),
    );

    sb.paw_ok(&["stop"]);

    assert!(
        wait_until(Duration::from_secs(5), || port_refuses(sb.port)),
        "the broker port is free after stop"
    );
    let flushed = wait_until(Duration::from_secs(5), || {
        fs::read_to_string(&log_path).is_ok_and(|l| l.contains("flush-probe-agent"))
    });
    assert!(
        flushed,
        "the host's final flush wrote the accepted message to {}: {:?}",
        log_path.display(),
        fs::read_to_string(&log_path).ok()
    );
}

/// Scenario "Status after stop does not report the broker running" (PP-27).
#[test]
#[serial]
fn status_after_stop_does_not_report_the_broker_running() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);
    sb.paw_ok(&["stop"]);

    let stdout = sb.paw_ok(&["status"]);
    assert!(
        !stdout.contains("(running"),
        "a stopped session's broker is never reported running; got:\n{stdout}"
    );
}

/// Scenario "Status shows running broker with agents".
#[test]
#[serial]
fn status_shows_running_broker_with_agents() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat/a,feat/b"]);
    publish_status(sb.port, "feat-a");
    publish_status(sb.port, "feat-b");

    let stdout = sb.paw_ok(&["status"]);
    assert!(stdout.contains(&sb.url()), "got:\n{stdout}");
    assert!(stdout.contains("running"), "got:\n{stdout}");
    assert!(stdout.contains("2 agents"), "got:\n{stdout}");
}

// ---------------------------------------------------------------------------
// Recovery with the saved broker / failed hosts
// ---------------------------------------------------------------------------

/// Starts a session on the sandbox port, stops it, and waits for the port.
fn start_then_stop(sb: &Sandbox) {
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);
    sb.paw_ok(&["stop"]);
    assert!(
        wait_until(Duration::from_secs(10), || port_refuses(sb.port)),
        "precondition: the stopped host freed the port"
    );
}

/// Asserts the recovered session runs its host, dashboard and agents'
/// environment on the SAVED port.
fn assert_recovered_on_the_saved_port(sb: &Sandbox) {
    assert!(sb.has_broker_window(), "windows: {:?}", sb.window_names());
    assert!(
        wait_until(Duration::from_secs(10), || status_json(sb.port).is_some()),
        "the recovered host answers on the saved port {}",
        sb.port
    );
    let pane0 = format!("{}:0.0", sb.session_name());
    assert!(
        wait_until(Duration::from_secs(5), || {
            sb.display(&pane0, "#{pane_current_command}")
                .contains("git-paw")
        }),
        "pane 0 runs the dashboard; got {:?}",
        sb.display(&pane0, "#{pane_current_command}")
    );
    let (_, env) = sb.tmux(&[
        "show-environment",
        "-t",
        &sb.session_name(),
        "GIT_PAW_BROKER_URL",
    ]);
    assert_eq!(
        env.trim(),
        format!("GIT_PAW_BROKER_URL={}", sb.url()),
        "the agents are handed the saved URL"
    );
}

/// session-state "Session recovery recreates the broker host and dashboard
/// pane" + "Session recovery uses original broker config, not current
/// config": the config's port changed after stop.
#[test]
#[serial]
fn recovery_uses_the_saved_broker_port_not_the_current_config() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    start_then_stop(&sb);
    let other = free_port();
    sb.write_broker_config(&format!("enabled = true\nport = {other}"));

    sb.paw_ok(&["start"]);
    assert_recovered_on_the_saved_port(&sb);
    assert!(port_refuses(other), "nothing binds the new config port");
}

/// The same with the broker disabled in the current config.
#[test]
#[serial]
fn recovery_uses_the_saved_broker_when_the_config_disables_it() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    start_then_stop(&sb);
    sb.write_broker_config("enabled = false");

    sb.paw_ok(&["start"]);
    assert_recovered_on_the_saved_port(&sb);
}

/// session-state "Session recovery without original broker creates no
/// dashboard": no broker means no host either.
#[test]
#[serial]
fn session_recovery_without_original_broker_creates_no_host() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.write_broker_config("enabled = false");
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);
    assert!(!sb.has_broker_window(), "no host without a broker");
    sb.paw_ok(&["stop"]);

    sb.paw_ok(&["start"]);
    assert!(
        sb.session_alive(),
        "precondition: the session was recovered"
    );
    assert!(
        !sb.has_broker_window(),
        "recovery of a broker-less session starts no host; windows: {:?}",
        sb.window_names()
    );
}

/// "A recovery whose host fails SHALL close the rebuilt session and restore
/// the receipt it had saved".
#[test]
#[serial]
fn recovery_whose_host_fails_restores_the_receipt() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    start_then_stop(&sb);
    assert_eq!(sb.receipt().expect("receipt")["status"], "stopped");

    let holder = hold_port_with_foreign_server(sb.port);
    let out = sb.paw(&["start"]);
    holder.store(false, Ordering::Relaxed);

    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(
        !out.status.success(),
        "recovery must fail; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(&sb.port.to_string()),
        "the error names the port; stderr:\n{stderr}"
    );
    assert!(!sb.session_alive(), "the rebuilt session is closed");
    assert_eq!(
        sb.receipt().expect("receipt kept")["status"],
        "stopped",
        "the receipt is restored, so the next start recovers again"
    );
}

/// "Existing dashboard pane is reused" — only when it is actually running: a
/// dashboard that quit back to its shell (keeping its `dashboard` title) is
/// recreated on restart.
#[test]
#[serial]
fn restart_from_pause_recreates_a_dashboard_that_quit() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "a"]);
    let pane0 = format!("{}:0.0", sb.session_name());
    assert!(
        wait_until(Duration::from_secs(5), || {
            sb.display(&pane0, "#{pane_current_command}")
                .contains("git-paw")
        }),
        "precondition: pane 0 runs the dashboard"
    );
    sb.paw_ok(&["pause"]);
    let panes_paused = sb.window0_panes().len();

    sb.tmux(&["send-keys", "-t", &pane0, "q"]);
    assert!(
        wait_until(Duration::from_secs(5), || {
            !sb.display(&pane0, "#{pane_current_command}")
                .contains("git-paw")
        }),
        "the dashboard quit back to its shell"
    );
    assert_eq!(
        sb.display(&pane0, "#{pane_title}"),
        "dashboard",
        "precondition: the quit pane keeps its dashboard title"
    );

    sb.paw_ok(&["start"]);

    let dashboard_running = wait_until(Duration::from_secs(5), || {
        let (_, out) = sb.tmux(&[
            "list-panes",
            "-t",
            &format!("{}:0", sb.session_name()),
            "-F",
            "#{pane_current_command}",
        ]);
        out.lines().any(|c| c.contains("git-paw"))
    });
    assert!(
        dashboard_running,
        "a running dashboard pane exists again; panes {:?}",
        sb.window0_panes()
    );
    assert_eq!(
        sb.window0_panes().len(),
        panes_paused + 1,
        "the dashboard is recreated beside the quit pane"
    );
}

/// "Pause of a session started before the split falls back": a session whose
/// pane 0 itself hosts the broker (no `broker` window) is paused by killing
/// that pane, which frees the port.
#[test]
#[serial]
fn pause_of_a_session_started_before_the_split_falls_back() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    let session = sb.session_name();
    let sessions_dir = if cfg!(target_os = "macos") {
        sb.home
            .path()
            .join("Library/Application Support/git-paw/sessions")
    } else {
        sb.home.path().join(".local/share/git-paw/sessions")
    };
    fs::create_dir_all(&sessions_dir).expect("sessions dir");
    let log = sessions_dir.join(format!("{session}.broker.log"));
    let bin = assert_cmd::cargo::cargo_bin("git-paw");
    let host = format!(
        "env -u XDG_DATA_HOME HOME='{}' '{}' __broker --session '{session}' --log-path '{}'",
        sb.home.path().display(),
        bin.display(),
        log.display()
    );
    let repo = sb.repo.path().to_string_lossy().to_string();
    assert!(
        sb.tmux(&[
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-s",
            &session,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            &repo,
            &host,
        ])
        .0,
        "pre-split session created"
    );
    sb.tmux(&["split-window", "-t", &format!("{session}:0"), "sleep 3600"]);
    assert!(
        wait_until(Duration::from_secs(10), || status_json(sb.port).is_some()),
        "precondition: pane 0 hosts the broker"
    );
    let receipt = serde_json::json!({
        "session_name": session,
        "repo_path": canonical(sb.repo.path()),
        "project_name": git_paw::git::project_name(sb.repo.path()),
        "created_at": "2026-05-01T00:00:00Z",
        "status": "active",
        "worktrees": [],
        "broker_port": sb.port,
        "broker_bind": "127.0.0.1",
        "broker_log_path": log.to_string_lossy(),
        "dashboard_pane": 0,
    });
    fs::write(
        sessions_dir.join(format!("{session}.json")),
        serde_json::to_string_pretty(&receipt).expect("receipt json"),
    )
    .expect("write receipt");
    assert!(!sb.has_broker_window(), "precondition: no broker window");

    sb.paw_ok(&["pause"]);

    assert!(
        wait_until(Duration::from_secs(5), || port_refuses(sb.port)),
        "killing the dashboard pane stops the broker it hosts"
    );
    assert_eq!(
        sb.window0_panes().len(),
        1,
        "only the agent pane remains; panes {:?}",
        sb.window0_panes()
    );
}

// ---------------------------------------------------------------------------
// The broker window selected (review finding: session-only pane listings)
// ---------------------------------------------------------------------------

impl Sandbox {
    /// Backdates the receipt's `created_at` past the 30s agent-launch grace
    /// period, so the liveness probe judges the agent panes immediately
    /// instead of reporting a young session alive whatever it sees.
    fn backdate_receipt(&self) {
        let path = self.receipt_path().expect("receipt saved");
        let mut receipt: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("read receipt"))
                .expect("parse receipt");
        receipt["created_at"] = serde_json::json!("2024-03-23T13:20:00Z");
        fs::write(
            &path,
            serde_json::to_string_pretty(&receipt).expect("serialize receipt"),
        )
        .expect("write receipt");
    }

    /// Makes the `broker` window the session's current window, as one click
    /// on the status bar (mouse is on) or `C-b n` does.
    fn select_broker_window(&self) {
        let target = format!("{}:broker", self.session_name());
        assert!(
            self.tmux(&["select-window", "-t", &target]).0,
            "select the broker window"
        );
        assert_eq!(
            self.display(&self.session_name(), "#{window_name}"),
            "broker",
            "precondition: the broker window is the current one"
        );
    }
}

/// With the `broker` window current, a bare `list-panes -t <session>` sees
/// only the host's pane. Before the fix the liveness probe read that as
/// "every agent exited", so `git paw start` took the recovery path and rebuilt
/// — killed — the live session (the agent pane's pid changed). Now the probe
/// lists window 0: start takes the reattach path (non-interactively it
/// refuses, naming the running session), the agent keeps running, and
/// `status` does not claim the agent panes exited.
#[test]
#[serial]
fn start_with_the_broker_window_selected_reattaches_instead_of_recovering() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat/a"]);
    sb.backdate_receipt();
    let agent_pane = format!("{}:0.1", sb.session_name());
    let agent_pid = sb.display(&agent_pane, "#{pane_pid}");
    assert!(!agent_pid.is_empty(), "the agent pane has a process");

    sb.select_broker_window();

    let status = sb.paw_ok(&["status"]);
    assert!(
        !status.contains("agent panes exited"),
        "status must not read the agents as exited with the broker window selected:\n{status}"
    );

    let out = sb.paw(&["start"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stdout.contains("Recovering") && !stderr.contains("Recovering"),
        "start must not recover a live session; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("already running"),
        "start takes the reattach path, which refuses non-interactively; stderr:\n{stderr}"
    );
    assert_eq!(
        sb.display(&agent_pane, "#{pane_pid}"),
        agent_pid,
        "the agent keeps running — the session was not rebuilt"
    );
    assert!(sb.has_broker_window(), "the broker window is untouched");
}

/// `git paw remove` finds the agent's pane even with the `broker` window
/// current. Its owner lookup (`surface_owned_by`) listed a bare
/// `list-panes -t <session>`, which with the broker window selected saw only
/// the host's pane: the agent's surface was "not found" and the remove left
/// the pane running. It now lists window 0, so the pane is found and closed.
#[test]
#[serial]
fn remove_finds_the_agent_pane_with_the_broker_window_selected() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat/a,feat/b"]);
    let before = sb.window0_panes().len();
    assert_eq!(before, 3, "dashboard + 2 agents");

    sb.select_broker_window();

    let out = sb.paw(&["remove", "feat/b", "--keep-worktree"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "remove failed; stderr:\n{stderr}");
    assert!(
        wait_until(Duration::from_secs(5), || sb.window0_panes().len()
            == before - 1),
        "the removed agent's pane is closed; window 0 now: {:?}\nstderr:\n{stderr}",
        sb.window0_panes()
    );
    assert!(sb.has_broker_window(), "the broker window is untouched");
}
