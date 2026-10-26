//! E2E tests for the per-session broker authority credential
//! (`broker-verdict-authority`): the launcher mints it and hands it to the
//! broker host through a one-shot `0600` file, the receipt persists the same
//! value, the host enforces it at `/publish`, an older session gains one on
//! recovery or restart from pause, and the credential never reaches argv, the
//! environment, a status surface or the broker log.
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

    fn tmux(&self, args: &[&str]) -> String {
        let out = StdCommand::new("tmux")
            .env("TMUX_TMPDIR", self.tmux.socket_dir())
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .args(args)
            .output()
            .expect("run tmux");
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn session_name(&self) -> String {
        format!("paw-{}", git_paw::git::project_name(self.repo.path()))
    }

    fn receipt_path(&self) -> Option<PathBuf> {
        find_file(self.home.path(), &format!("{}.json", self.session_name()))
    }

    fn receipt(&self) -> serde_json::Value {
        let path = self.receipt_path().expect("a saved receipt");
        serde_json::from_str(&fs::read_to_string(path).expect("read receipt")).expect("json")
    }

    fn credential(&self) -> String {
        self.receipt()["broker_authority"]
            .as_str()
            .expect("the receipt records broker_authority")
            .to_string()
    }

    /// Removes `broker_authority` from the saved receipt, as a session saved
    /// by an earlier binary would look.
    fn strip_credential(&self) {
        let path = self.receipt_path().expect("a saved receipt");
        let mut receipt = self.receipt();
        receipt
            .as_object_mut()
            .expect("object")
            .remove("broker_authority");
        fs::write(path, receipt.to_string()).expect("rewrite receipt");
    }

    /// Every file under `HOME` whose name ends in `.authority`.
    fn handoff_files(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();
        collect(self.home.path(), &mut |p| {
            if p.extension().is_some_and(|e| e == "authority") {
                found.push(p.to_path_buf());
            }
        });
        found
    }

    /// `POST /publish` of an `agent.verified` for `agent`, with an optional
    /// `X-Paw-Authority`.
    fn publish_verified(&self, agent: &str, authority: Option<&str>) -> u16 {
        let body = format!(
            r#"{{"type":"agent.verified","agent_id":"{agent}","payload":{{"verified_by":"supervisor","message":"ok"}}}}"#
        );
        let header = authority.map(|a| ("X-Paw-Authority", a));
        http(self.port, "POST", "/publish", &body, header)
            .expect("broker answers /publish")
            .0
    }
}

fn collect(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, f);
        } else {
            f(&path);
        }
    }
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut hit = None;
    collect(dir, &mut |p| {
        if hit.is_none() && p.file_name().is_some_and(|n| n == name) {
            hit = Some(p.to_path_buf());
        }
    });
    hit
}

/// Raw HTTP/1.1 exchange; `None` when nothing answers.
fn http(
    port: u16,
    method: &str,
    path: &str,
    body: &str,
    header: Option<(&str, &str)>,
) -> Option<(u16, String)> {
    let mut stream = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().expect("addr"),
        Duration::from_millis(500),
    )
    .ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok();
    let extra = header.map_or(String::new(), |(k, v)| format!("{k}: {v}\r\n"));
    let req = if body.is_empty() {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{extra}Connection: close\r\n\r\n"
        )
    } else {
        format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
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

fn assert_hex_credential(credential: &str) {
    assert!(
        credential.len() >= 64 && credential.chars().all(|c| c.is_ascii_hexdigit()),
        "the credential is at least 64 hex characters: {credential:?}"
    );
}

/// The launcher's stdout and stderr (its `Output` sink) never show the
/// credential.
fn assert_output_lacks(out: &std::process::Output, credential: &str) {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !text.contains(credential),
        "launcher output leaks it:\n{text}"
    );
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

// ---------------------------------------------------------------------------
// Launch
// ---------------------------------------------------------------------------

/// broker-runtime "Bare start with the broker also records a credential",
/// "The host and the receipt hold the same credential", "Host enforces the
/// launcher's credential" (the hand-off is consumed), and broker-protocol "A
/// worker cannot self-verify" end to end.
#[test]
#[serial]
fn bare_start_records_the_credential_the_host_enforces() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    let out = sb.paw(&["start", "--cli", "cat", "--branches", "feat-a"]);
    assert!(out.status.success(), "start failed");

    let credential = sb.credential();
    assert_hex_credential(&credential);
    assert_output_lacks(&out, &credential);
    assert!(
        sb.handoff_files().is_empty(),
        "the host consumed the hand-off file: {:?}",
        sb.handoff_files()
    );

    // A worktree-side publisher has no credential: refused, and not verified.
    assert_eq!(sb.publish_verified("feat-a", None), 403);
    assert_eq!(
        sb.publish_verified("feat-a", Some("not-the-credential")),
        403
    );
    let (_, log) = http(sb.port, "GET", "/log", "", None).expect("/log answers");
    assert!(
        !log.contains("agent.verified"),
        "a refused verdict never reaches the log: {log}"
    );

    // The root holds the receipt's credential: accepted.
    assert_eq!(sb.publish_verified("feat-a", Some(&credential)), 202);
}

/// broker-runtime "Supervised start records a credential".
#[test]
#[serial]
fn supervised_start_records_a_credential() {
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
        "feat-a",
    ]);

    let credential = sb.credential();
    assert_hex_credential(&credential);
    assert_eq!(sb.publish_verified("feat-a", Some(&credential)), 202);
}

/// broker-runtime: the spec-driven start path mints and records a
/// credential too, and the host enforces it.
#[test]
#[serial]
fn spec_driven_start_records_a_credential() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("[specs]\ntype = \"openspec\"\ndir = \"specs\"\n");
    let _proc = sb.tmux.apply_to_process();
    let spec = sb.repo.path().join("specs").join("feat-spec");
    fs::create_dir_all(&spec).expect("create spec dir");
    fs::write(spec.join("tasks.md"), "Implement the spec.\n").expect("write tasks.md");
    for args in [&["add", "."][..], &["commit", "-q", "-m", "add spec"][..]] {
        let ok = StdCommand::new("git")
            .args(args)
            .current_dir(sb.repo.path())
            .status()
            .expect("run git")
            .success();
        assert!(ok, "git {args:?}");
    }

    let out = sb.paw(&["start", "--from-specs", "--cli", "cat"]);
    assert!(
        out.status.success(),
        "spec-driven start failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let credential = sb.credential();
    assert_hex_credential(&credential);
    assert_output_lacks(&out, &credential);
    let agent = git_paw::broker::messages::slugify_branch(
        sb.receipt()["worktrees"][0]["branch"]
            .as_str()
            .expect("a spec branch"),
    );
    assert_eq!(sb.publish_verified(&agent, None), 403);
    assert_eq!(sb.publish_verified(&agent, Some(&credential)), 202);
}

/// broker-runtime "A failed start persists no credential": the host cannot
/// serve, the launch fails, no receipt is saved and no hand-off is left.
#[test]
#[serial]
fn a_failed_start_persists_no_credential() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    let holder = hold_port_with_foreign_server(sb.port);
    let out = sb.paw(&["start", "--cli", "cat", "--branches", "feat-a"]);
    holder.store(false, Ordering::Relaxed);

    assert!(!out.status.success(), "start must fail");
    assert!(sb.receipt_path().is_none(), "no receipt for a failed start");
    assert!(
        sb.handoff_files().is_empty(),
        "no hand-off file may outlive a failed start: {:?}",
        sb.handoff_files()
    );
}

/// broker-runtime "Credential never appears on a command line or in the
/// environment": no pane's start command in any window (the `broker` window
/// included) and no session or global tmux environment carries it.
#[test]
#[serial]
fn credential_never_appears_on_a_command_line_or_in_the_environment() {
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
        "feat-a",
    ]);
    let credential = sb.credential();

    let commands = sb.tmux(&[
        "list-panes",
        "-s",
        "-t",
        &sb.session_name(),
        "-F",
        "#{window_name}|#{pane_start_command}",
    ]);
    assert!(
        commands.contains("broker|"),
        "the listing covers the broker window: {commands}"
    );
    assert!(!commands.contains(&credential), "{commands}");
    let session_env = sb.tmux(&["show-environment", "-t", &sb.session_name()]);
    let global_env = sb.tmux(&["show-environment", "-g"]);
    assert!(!session_env.contains(&credential));
    assert!(!global_env.contains(&credential));
}

// ---------------------------------------------------------------------------
// Back-fill
// ---------------------------------------------------------------------------

/// broker-runtime "Restart from pause keeps a back-filled credential".
#[test]
#[serial]
fn restart_from_pause_keeps_a_backfilled_credential() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat-a"]);
    let launched = sb.credential();
    sb.paw_ok(&["pause"]);
    assert!(wait_until(Duration::from_secs(10), || port_refuses(
        sb.port
    )));
    sb.strip_credential();

    let out = sb.paw(&["start"]);
    assert!(out.status.success(), "restart from pause failed");
    let backfilled = sb.credential();
    assert_hex_credential(&backfilled);
    assert_output_lacks(&out, &backfilled);
    assert_ne!(backfilled, launched, "the host minted a fresh credential");
    assert_eq!(sb.publish_verified("feat-a", Some(&launched)), 403);
    assert_eq!(sb.publish_verified("feat-a", Some(&backfilled)), 202);
}

/// broker-runtime "A session without a credential gains one", through
/// recovery after `stop`.
#[test]
#[serial]
fn recovery_backfills_a_session_without_a_credential() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat-a"]);
    sb.paw_ok(&["stop"]);
    assert!(wait_until(Duration::from_secs(10), || port_refuses(
        sb.port
    )));
    sb.strip_credential();

    let out = sb.paw(&["start"]);
    assert!(out.status.success(), "recovery failed");
    let backfilled = sb.credential();
    assert_hex_credential(&backfilled);
    assert_output_lacks(&out, &backfilled);
    assert_eq!(sb.publish_verified("feat-a", None), 403);
    assert_eq!(sb.publish_verified("feat-a", Some(&backfilled)), 202);
}

/// Recovery of a session that already holds a credential hands the host the
/// saved one: the receipt's value keeps working.
#[test]
#[serial]
fn recovery_keeps_the_saved_credential() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat-a"]);
    let launched = sb.credential();
    sb.paw_ok(&["stop"]);
    assert!(wait_until(Duration::from_secs(10), || port_refuses(
        sb.port
    )));

    sb.paw_ok(&["start"]);
    assert_eq!(sb.credential(), launched);
    assert_eq!(sb.publish_verified("feat-a", Some(&launched)), 202);
}

// ---------------------------------------------------------------------------
// Confinement
// ---------------------------------------------------------------------------

/// broker-runtime "Discovery file carries no credential", "Status surfaces
/// carry no credential" (`/status`, `git paw status --json`) and "Broker log
/// carries no credential".
#[test]
#[serial]
fn credential_is_confined_to_root_side_surfaces() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat-a"]);
    let credential = sb.credential();
    assert_eq!(sb.publish_verified("feat-a", Some(&credential)), 202);

    let discovery = sb
        .repo
        .path()
        .join(".git-paw")
        .join("sessions")
        .join(format!("{}.json", sb.session_name()));
    let discovery = fs::read_to_string(discovery).expect("discovery file");
    assert!(!discovery.contains(&credential), "{discovery}");
    assert!(!discovery.contains("broker_authority"), "{discovery}");

    let (_, status) = http(sb.port, "GET", "/status", "", None).expect("/status");
    assert!(!status.contains(&credential), "{status}");
    let (_, log) = http(sb.port, "GET", "/log", "", None).expect("/log");
    assert!(log.contains("agent.verified"), "the verdict was logged");
    assert!(!log.contains(&credential), "{log}");
    let status_json = sb.paw_ok(&["status", "--json"]);
    assert!(!status_json.contains(&credential), "{status_json}");

    let log_path = PathBuf::from(
        sb.receipt()["broker_log_path"]
            .as_str()
            .expect("broker_log_path"),
    );
    sb.paw_ok(&["stop"]);
    assert!(wait_until(Duration::from_secs(10), || {
        fs::read_to_string(&log_path).is_ok_and(|l| l.contains("verified by supervisor"))
    }));
    let broker_log = fs::read_to_string(&log_path).expect("broker log");
    assert!(!broker_log.contains(&credential));
}

/// broker-protocol "Branch-guard feedback from a worktree is still accepted",
/// end to end: the worktree's real post-commit hook, fired by a commit on a
/// branch other than the worktree's, publishes `agent.feedback` from
/// `branch-guard` with no credential, and the enforcing broker delivers it.
#[test]
#[serial]
fn branch_guard_hook_feedback_is_still_accepted() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new("[supervisor]\nstrict_branch_guard = false\n");
    let _proc = sb.tmux.apply_to_process();
    sb.paw_ok(&["start", "--cli", "cat", "--branches", "feat-a"]);
    let worktree = PathBuf::from(
        sb.receipt()["worktrees"][0]["worktree_path"]
            .as_str()
            .expect("worktree path"),
    );
    let git = |args: &[&str]| {
        let out = StdCommand::new("git")
            .args(args)
            .current_dir(&worktree)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    git(&["checkout", "-b", "feat-elsewhere"]);
    fs::write(worktree.join("guard.txt"), "guard\n").expect("write file");
    git(&["add", "guard.txt"]);
    git(&["commit", "-m", "commit on the wrong branch"]);

    assert!(
        wait_until(Duration::from_secs(10), || {
            http(sb.port, "GET", "/messages/feat-a?since=0", "", None)
                .is_some_and(|(_, body)| body.contains("branch-guard"))
        }),
        "the branch-guard feedback reaches the worktree's inbox: {:?}",
        http(sb.port, "GET", "/messages/feat-a?since=0", "", None)
    );
}
