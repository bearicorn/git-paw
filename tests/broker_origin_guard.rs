//! Integration tests for the broker listener's browser-exposure guard
//! (`broker-listener-origin-guard`).
//!
//! The router-level `oneshot` tests live beside the decision in
//! `src/broker/origin.rs` and `src/broker/server.rs`. These cover what only a
//! real listener can show: the non-loopback-bind exception and its start-time
//! warning, and the regression guarantee that every in-tree client — the
//! bundled shell helpers, the post-commit hook, and the Rust HTTP clients —
//! still passes the guard.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use git_paw::broker::messages::{BrokerMessage, StatusPayload};
use git_paw::broker::{BrokerHandle, BrokerState, ProbeResult, start_broker};
use git_paw::config::BrokerConfig;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Allocates an OS-assigned ephemeral port and releases it again.
fn pick_broker_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

/// Starts a broker on `bind` and a free port.
///
/// Returns the handle plus the loopback URL a client uses to reach it — for a
/// `0.0.0.0` bind the listener answers on `127.0.0.1` too, which is how the
/// MCP server already addresses a wildcard-bound broker.
fn spawn_broker_on(bind: &str) -> (BrokerHandle, String, u16) {
    let mut attempts = 0;
    loop {
        let port = pick_broker_port();
        let config = BrokerConfig {
            enabled: true,
            port,
            bind: bind.to_string(),
            ..Default::default()
        };
        match start_broker(&config, BrokerState::new(None), Vec::new()) {
            Ok(handle) => return (handle, format!("http://127.0.0.1:{port}"), port),
            Err(_) if attempts < 10 => attempts += 1,
            Err(e) => panic!("failed to start test broker on {bind}: {e}"),
        }
    }
}

/// Sends a raw HTTP/1.1 request with an explicit `Host` (and optional
/// `Origin`), returning `(status, body)`. Chunked bodies are decoded.
fn raw_req(
    port: u16,
    method: &str,
    path: &str,
    host: &str,
    origin: Option<&str>,
    body: &str,
) -> (u16, String) {
    use std::fmt::Write as _;

    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("failed to connect to test broker");
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    let mut request = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    if let Some(origin) = origin {
        let _ = write!(request, "Origin: {origin}\r\n");
    }
    if !body.is_empty() {
        let _ = write!(
            request,
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    request.push_str("\r\n");
    request.push_str(body);

    stream
        .write_all(request.as_bytes())
        .expect("failed to write request");

    let mut response = String::new();
    stream.read_to_string(&mut response).ok();

    let header_section = response.split("\r\n\r\n").next().unwrap_or("");
    let status: u16 = header_section
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let raw_body = response
        .split_once("\r\n\r\n")
        .map_or_else(String::new, |(_, b)| b.to_string());
    let body = if header_section
        .to_lowercase()
        .contains("transfer-encoding: chunked")
    {
        decode_chunked(&raw_body)
    } else {
        raw_body
    };
    (status, body)
}

fn decode_chunked(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    loop {
        let line_end = rest.find("\r\n").unwrap_or(rest.len());
        let Ok(size) = usize::from_str_radix(rest[..line_end].trim(), 16) else {
            break;
        };
        if size == 0 || rest.len() < line_end + 2 + size {
            break;
        }
        rest = &rest[line_end + 2..];
        out.push_str(&rest[..size]);
        rest = &rest[size..];
        rest = rest.strip_prefix("\r\n").unwrap_or(rest);
    }
    out
}

/// The repository root this test binary was compiled from.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Returns `true` when every external tool the shell-client tests need is on
/// PATH. Those tests report success rather than fail on a host without them,
/// matching how the other shell-touching suites in this repo degrade.
fn shell_tools_available() -> bool {
    ["bash", "curl", "git", "python3"]
        .iter()
        .all(|tool| which::which(tool).is_ok())
}

/// Builds a throwaway git repo wired to talk to the broker on `port`: the
/// bundled helper scripts under `.git-paw/scripts/`, a `.git-paw/config.toml`
/// naming the port, and a session file so `sweep.sh` resolves a broker URL.
fn fixture_repo(dir: &Path, port: u16) {
    let run = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    };
    run(&["init", "--initial-branch=feat/guard-fixture"]);
    run(&["config", "user.email", "fixture@example.invalid"]);
    run(&["config", "user.name", "fixture"]);

    let paw = dir.join(".git-paw");
    fs::create_dir_all(paw.join("scripts")).expect("create .git-paw/scripts");
    fs::write(
        paw.join("config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\nbind = \"127.0.0.1\"\n"),
    )
    .expect("write config.toml");

    for script in ["_paw_common.sh", "broker.sh", "sweep.sh"] {
        let src = repo_root().join("assets/scripts").join(script);
        let dst = paw.join("scripts").join(script);
        fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copy {script}: {e}"));
        make_executable(&dst);
    }

    fs::write(dir.join("seed.txt"), "seed\n").expect("write seed file");
    run(&["add", "seed.txt"]);
    run(&["commit", "-m", "seed"]);
}

fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path).expect("stat script").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod script");
}

/// Runs a bundled helper inside the fixture repo and returns its combined
/// output.
fn run_helper(dir: &Path, script: &str, args: &[&str]) -> String {
    let out = Command::new("bash")
        .arg(format!(".git-paw/scripts/{script}"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("run {script}: {e}"));
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Asserts a helper's output shows it was not refused by the guard.
fn assert_not_forbidden(label: &str, output: &str) {
    assert!(
        !output.contains("forbidden"),
        "{label} was refused by the listener guard: {output}"
    );
}

fn status_message(agent_id: &str, message: &str) -> BrokerMessage {
    BrokerMessage::Status {
        agent_id: agent_id.to_string(),
        payload: StatusPayload {
            status: "working".to_string(),
            message: Some(message.to_string()),
            ..Default::default()
        },
    }
}

// ---------------------------------------------------------------------------
// 2.3 / 3.4 — Non-loopback bind: skips the Host check, warns, clients still work
// ---------------------------------------------------------------------------
//
// NOTE ON THE `0.0.0.0` BIND BELOW. A live non-loopback listener cannot be
// faked: `start_broker_with` parses `[broker] bind` as an IP literal, and the
// only bindable literals that `is_loopback_bind` calls non-loopback are
// `0.0.0.0` and a real interface address — both of which put an
// unauthenticated broker on the LAN for as long as the listener is up. The
// suite therefore does this exactly ONCE, in a child process, on an ephemeral
// port, dropping the handle as soon as the assertions finish. Every other
// non-loopback assertion runs against the pure decision table
// (`broker::origin::tests`) or `router(state, false)`, which bind nothing.

/// Scenario: Non-loopback bind skips the Host check and warns.
/// Scenario (3.4): the same clients work when `[broker] bind = "0.0.0.0"`.
///
/// Driven from a child process because the warning goes to the process's real
/// stderr, which an in-process test cannot read back.
#[test]
fn non_loopback_bind_skips_the_host_check_and_warns() {
    let exe = std::env::current_exe().expect("current test binary");
    let out = Command::new(exe)
        .args([
            "--exact",
            "non_loopback_bind_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("GIT_PAW_GUARD_NON_LOOPBACK_CHILD", "1")
        .env("GIT_PAW_ALLOW_LIVE_SESSION", "1")
        .output()
        .expect("re-run test binary as child");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the non-loopback child failed.\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        stderr.contains("0.0.0.0"),
        "the start-time warning must name the bind address; stderr was: {stderr}"
    );
    assert!(
        stderr.contains("reachable from the network without authentication"),
        "the start-time warning must state the exposure; stderr was: {stderr}"
    );
}

/// Child half of [`non_loopback_bind_skips_the_host_check_and_warns`]. Ignored
/// by default so a normal run does not bind `0.0.0.0` a second time.
#[test]
#[ignore = "child process harness for non_loopback_bind_skips_the_host_check_and_warns"]
fn non_loopback_bind_child() {
    if std::env::var_os("GIT_PAW_GUARD_NON_LOOPBACK_CHILD").is_none() {
        return;
    }
    let (handle, _loopback_url, port) = spawn_broker_on("0.0.0.0");

    // The Host check is skipped: a foreign Host is served.
    let (status, _body) = raw_req(port, "GET", "/status", "192.168.1.5:9119", None, "");
    assert_eq!(
        status, 200,
        "a foreign Host must be served on a non-loopback bind"
    );

    // The Origin check still applies on every bind.
    let (status, body) = raw_req(
        port,
        "GET",
        "/status",
        "192.168.1.5:9119",
        Some("http://attacker.example"),
        "",
    );
    assert_eq!(status, 403);
    assert!(body.contains("forbidden origin"), "got {body}");

    // 3.4 — a Rust client addressing the broker exactly as `BrokerConfig::url()`
    // renders it for a wildcard bind, so it sends `Host: 0.0.0.0:<port>`.
    let wildcard_url = format!("http://0.0.0.0:{port}");
    git_paw::broker::publish::publish_to_broker_http(
        &wildcard_url,
        &status_message("feat-guard-wildcard", "wildcard"),
    )
    .expect("publish_to_broker_http must pass on a wildcard bind");

    // 3.4 — and a bundled shell client, with the wildcard bind in its config.
    if shell_tools_available() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        fixture_repo(dir, port);
        fs::write(
            dir.join(".git-paw/config.toml"),
            format!("[broker]\nenabled = true\nport = {port}\nbind = \"0.0.0.0\"\n"),
        )
        .expect("rewrite config for a wildcard bind");
        let out = run_helper(
            dir,
            "broker.sh",
            &[
                "--agent",
                "feat-guard-wildcard",
                "status",
                "shell-on-wildcard",
            ],
        );
        assert_not_forbidden("broker.sh status on a wildcard bind", &out);
    }

    let (status, log) = raw_req(port, "GET", "/log", &format!("0.0.0.0:{port}"), None, "");
    assert_eq!(status, 200);
    assert!(
        log.contains("feat-guard-wildcard"),
        "the wildcard-bind publishes must have landed: {log}"
    );

    drop(handle);
}

/// A loopback bind must NOT print the warning, and must still look like a live
/// broker to the stale-broker probe.
#[test]
fn loopback_bind_does_not_warn() {
    assert!(
        git_paw::broker::origin::is_loopback_bind("127.0.0.1"),
        "the default bind must be classified loopback, so no warning is emitted"
    );
    let (handle, url, port) = spawn_broker_on("127.0.0.1");
    let (status, _) = raw_req(
        port,
        "GET",
        "/status",
        &format!("127.0.0.1:{port}"),
        None,
        "",
    );
    assert_eq!(status, 200);
    assert_eq!(
        git_paw::broker::probe_broker(&url),
        ProbeResult::LiveBroker,
        "the default bind must still look like a live broker"
    );
    drop(handle);
}

/// Every path that brings a session's broker up must print the exposure
/// warning first — the broker's own copy lands in the dashboard pane just
/// before it switches to tmux's alternate screen, where nobody reads it.
///
/// Five paths qualify, not three: besides the fresh launches (`cmd_start`,
/// `cmd_start_with_specs`, the supervisor launch), a Stopped session is
/// rebuilt by `recover_session` — the common path after a reboot or a tmux
/// crash — and a Paused one re-spawns the dashboard pane in
/// `restart_from_pause`.
///
/// The last two are matched on their `_detached` halves: `drive-loop-reattach`
/// split the terminal attach off each one, so the body that raises the broker —
/// and must carry the warning — is now `recover_session_detached` /
/// `restart_from_pause_detached`, which the attaching wrappers delegate to.
///
/// Each resume/recovery site warns on the address its host will bind (the
/// saved one), not the current config's. Each site's trigger is
/// `start_broker_host(`: since `broker-host-separation`
/// the broker is raised by starting its host process in the session's
/// detached service surface, not by the dashboard pane. A path that raises
/// the broker without that call does not raise it at all.
///
/// Matching is by function scope rather than a byte window: the window would
/// both miss a warning placed further away and risk slicing a multi-byte
/// character (these files are full of em dashes).
#[test]
fn every_launch_path_warns_about_an_exposed_bind() {
    /// One broker-raising function: the `fn` that opens it, the statement
    /// inside it that brings the broker up, and the warning that must appear
    /// in the same body before that statement.
    struct Site {
        file: &'static str,
        function: &'static str,
        trigger: &'static str,
        warning: &'static str,
    }

    let sites = [
        Site {
            file: "src/commands/start.rs",
            function: "pub fn start(",
            trigger: "start_broker_host(",
            warning: "warn_if_broker_is_exposed(&broker_config, out)",
        },
        Site {
            file: "src/commands/start.rs",
            function: "fn launch_spec_session(",
            trigger: "start_broker_host(",
            warning: "warn_if_broker_is_exposed(&broker_config, out)",
        },
        Site {
            file: "src/commands/start.rs",
            function: "pub(crate) fn restart_from_pause_detached(",
            trigger: "start_broker_host(",
            warning: "warn_if_broker_is_exposed(&recorded, out)",
        },
        Site {
            file: "src/commands/supervisor.rs",
            function: "pub fn supervisor(",
            trigger: "start_broker_host(",
            warning: "warn_if_broker_is_exposed(&broker_config, out)",
        },
        Site {
            file: "src/commands/recover.rs",
            function: "pub(crate) fn recover_session_detached(",
            trigger: "start_broker_host(",
            warning: "warn_if_broker_is_exposed(&recovery_broker_config, out)",
        },
    ];
    assert_eq!(
        sites.len(),
        5,
        "the five broker-raising paths: start, start --from-specs, supervisor, recover_session, \
         restart_from_pause_detached"
    );

    // Every place that starts a broker host must be one of the sites above,
    // so a sixth broker-raising path cannot slip in without a warning check.
    let mut raising = 0;
    for entry in fs::read_dir(repo_root().join("src/commands")).expect("read src/commands") {
        let path = entry.expect("dir entry").path();
        if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).expect("read command module");
            raising += text.matches("start_broker_host(").count()
                - text.matches("fn start_broker_host(").count();
        }
    }
    assert_eq!(
        raising,
        sites.len(),
        "every `start_broker_host(` call must be a guarded launch path listed above"
    );

    for site in &sites {
        let text = fs::read_to_string(repo_root().join(site.file)).expect("read launch command");
        let body = function_body(&text, site.function)
            .unwrap_or_else(|| panic!("{}: no function `{}`", site.file, site.function));
        let trigger_at = body.find(site.trigger).unwrap_or_else(|| {
            panic!(
                "{}: `{}` no longer contains `{}` — the guard needs retargeting",
                site.file, site.function, site.trigger
            )
        });
        let warning_at = body.find(site.warning).unwrap_or_else(|| {
            panic!(
                "{} `{}` brings the broker up without warning about a non-loopback bind",
                site.file, site.function
            )
        });
        assert!(
            warning_at < trigger_at,
            "{} `{}` warns about a non-loopback bind only AFTER bringing the broker up",
            site.file,
            site.function
        );
    }
}

/// The body of the function whose signature starts with `signature`, found by
/// brace matching from the signature's opening `{`.
///
/// Slices only at indices returned by `find`/`char_indices`, so it can never
/// split a multi-byte character. Brace counting is good enough here: these
/// bodies contain no unbalanced brace inside a string or char literal, and the
/// caller asserts on content it expects to find, so a bad slice surfaces as a
/// panic naming the function rather than a silent pass.
fn function_body<'a>(text: &'a str, signature: &str) -> Option<&'a str> {
    let start = text.find(signature)?;
    let open = start + text[start..].find('{')?;
    let mut depth = 0usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open..=open + offset]);
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// 3.1 — Bundled shell clients are unaffected
// ---------------------------------------------------------------------------

/// Scenario: Bundled shell clients are unaffected.
#[test]
fn bundled_shell_clients_are_unaffected() {
    if !shell_tools_available() {
        eprintln!("skipping: bash/curl/git/python3 not all on PATH");
        return;
    }
    let (handle, url, port) = spawn_broker_on("127.0.0.1");
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path();
    fixture_repo(dir, port);

    // broker.sh status — a publish. Also seeds the agent into the /status
    // roster, which is what `sweep.sh status` must then be able to read back.
    let out = run_helper(
        dir,
        "broker.sh",
        &["--agent", "feat-guard-fixture", "status", "x"],
    );
    assert_not_forbidden("broker.sh status", &out);

    // Seed a real message into the fixture's inbox before polling. Without
    // this, a refused read is indistinguishable from an accepted empty one:
    // `broker.sh poll` prints "(no messages)" and exits 0 either way.
    let (status, _) = raw_req(
        port,
        "POST",
        "/publish",
        &format!("127.0.0.1:{port}"),
        None,
        r#"{"type":"agent.feedback","agent_id":"feat-guard-fixture","payload":{"from":"supervisor","errors":["guard-probe-marker"]}}"#,
    );
    assert_eq!(status, 202, "seeding the inbox must be accepted");

    // broker.sh poll — a read. It must return the seeded message, not merely
    // fail to say "forbidden".
    let out = run_helper(dir, "broker.sh", &["--agent", "feat-guard-fixture", "poll"]);
    assert_not_forbidden("broker.sh poll", &out);
    assert!(
        out.contains("guard-probe-marker"),
        "broker.sh poll must return the seeded inbox message, got: {out}"
    );

    // sweep.sh status — a read. It must name the agent the roster holds.
    let out = run_helper(dir, "sweep.sh", &["status"]);
    assert_not_forbidden("sweep.sh status", &out);
    assert!(
        out.contains("feat-guard-fixture"),
        "sweep.sh status must list the roster agent, got: {out}"
    );

    // sweep.sh status-publish — a publish.
    let out = run_helper(dir, "sweep.sh", &["status-publish", "x"]);
    assert_not_forbidden("sweep.sh status-publish", &out);

    // The bundled post-commit hook — a publish, driven by a real commit.
    install_post_commit_hook(dir, &url, "feat-guard-fixture");
    fs::write(dir.join("hooked.txt"), "hooked\n").expect("write file");
    let out = Command::new("git")
        .args(["add", "hooked.txt"])
        .current_dir(dir)
        .output()
        .expect("git add");
    assert!(out.status.success());
    let out = Command::new("git")
        .args(["commit", "-m", "hooked"])
        .current_dir(dir)
        .output()
        .expect("git commit");
    assert!(
        out.status.success(),
        "commit failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Every publish above must have landed in the log — proof the guard let
    // them through rather than silently refusing them.
    let (status, log) = raw_req(port, "GET", "/log", &format!("127.0.0.1:{port}"), None, "");
    assert_eq!(status, 200, "GET /log must be served");
    for expected in [
        "agent.status",
        "agent.artifact",
        "feat-guard-fixture",
        "supervisor",
    ] {
        assert!(
            log.contains(expected),
            "{expected} published by a bundled client is missing from /log: {log}"
        );
    }

    drop(handle);
}

/// Writes the bundled `post-commit` hook plus the `paw-agent-id` marker it
/// reads, into the fixture repo's gitdir.
fn install_post_commit_hook(dir: &Path, broker_url: &str, agent_id: &str) {
    let git_dir = dir.join(".git");
    let hook = git_dir.join("hooks/post-commit");
    fs::create_dir_all(git_dir.join("hooks")).expect("create hooks dir");
    let body = fs::read_to_string(repo_root().join("assets/hooks/post-commit.sh"))
        .expect("read bundled post-commit hook");
    fs::write(&hook, body).expect("write post-commit hook");
    make_executable(&hook);
    fs::write(
        git_dir.join("paw-agent-id"),
        format!("PAW_AGENT_ID={agent_id}\nPAW_BROKER_URL={broker_url}\n"),
    )
    .expect("write paw-agent-id marker");
}

// ---------------------------------------------------------------------------
// 3.2 — In-tree Rust clients are unaffected
// ---------------------------------------------------------------------------

/// Scenario: In-tree Rust clients are unaffected.
#[test]
fn in_tree_rust_clients_are_unaffected() {
    let (handle, url, _port) = spawn_broker_on("127.0.0.1");
    let tmp = tempfile::tempdir().expect("tempdir");

    git_paw::broker::publish::publish_to_broker_http(
        &url,
        &status_message("feat-guard-client", "hello"),
    )
    .expect("publish_to_broker_http must pass the guard");

    git_paw::broker::publish::register_watch_target_http(
        &url,
        "feat-guard-client",
        tmp.path(),
        "claude",
        Some("003-guard"),
    )
    .expect("register_watch_target_http must pass the guard");

    let entries = git_paw::broker::publish::fetch_log_entries_over_http(&url, 0, None)
        .expect("fetch_log_entries_over_http must pass the guard");
    assert!(
        !entries.is_empty(),
        "the log read must return the published message, not a 403 body"
    );

    let messages = git_paw::broker::publish::fetch_log_over_http(&url)
        .expect("fetch_log_over_http must pass the guard");
    assert!(!messages.is_empty());

    git_paw::coordination::inventory::fetch_status_agents_over_http(&url)
        .expect("fetch_status_agents_over_http must pass the guard");

    git_paw::supervisor::drive::fetch_status_over_http(&url)
        .expect("fetch_status_over_http must pass the guard");

    assert_eq!(
        git_paw::broker::probe_broker(&url),
        ProbeResult::LiveBroker,
        "the stale-broker probe must still recognise a live git-paw broker"
    );

    drop(handle);
}

// ---------------------------------------------------------------------------
// 3.3 — Source guard: no in-tree client can drift out of the accepted set
// ---------------------------------------------------------------------------

/// Collects every `.rs` file under `src/`.
fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("read src dir").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&repo_root().join("src"), &mut out);
    out
}

/// Every raw `Host:` header written under `src/` must interpolate the address
/// derived from the broker URL, so no client can hard-code a host the guard
/// would refuse.
#[test]
fn rust_clients_derive_the_host_header_from_the_broker_url() {
    let mut checked = 0;
    for path in rust_sources() {
        let text = fs::read_to_string(&path).expect("read source");
        for (index, line) in text.lines().enumerate() {
            if !line.contains("Host: ") {
                continue;
            }
            checked += 1;
            assert!(
                line.contains("{addr}"),
                "{}:{} writes a Host header that is not derived from the broker URL: {}",
                path.display(),
                index + 1,
                line.trim()
            );
        }
    }
    // `broker-host-separation` consolidated the hand-rolled clients into
    // `BrokerClient`, so the census is small; what matters is that the shared
    // client is among those checked.
    assert!(
        checked >= 3,
        "expected to find the in-tree raw-HTTP clients, found {checked} Host headers"
    );
    let client = fs::read_to_string(repo_root().join("src/broker/client.rs"))
        .expect("read the shared broker client");
    assert!(
        client.contains("Host: {addr}"),
        "the shared BrokerClient must derive its Host header from the broker URL"
    );
}

/// No in-tree client — Rust or shell — may set an `Origin` header; the guard
/// accepts an absent `Origin` and refuses every non-loopback one.
#[test]
fn no_in_tree_client_sets_an_origin_header() {
    let mut files: Vec<PathBuf> = rust_sources();
    for dir in ["assets/scripts", "assets/hooks"] {
        for entry in fs::read_dir(repo_root().join(dir))
            .expect("read assets dir")
            .flatten()
        {
            files.push(entry.path());
        }
    }
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            assert!(
                !line.contains("Origin: "),
                "{}:{} sets an Origin header, which the listener guard refuses: {}",
                path.display(),
                index + 1,
                line.trim()
            );
        }
    }
}

/// No bundled shell client may override `Host`; `curl` derives it from the
/// broker URL, which is what keeps the helpers inside the accepted set.
#[test]
fn bundled_shell_clients_do_not_override_the_host_header() {
    for dir in ["assets/scripts", "assets/hooks"] {
        for entry in fs::read_dir(repo_root().join(dir))
            .expect("read assets dir")
            .flatten()
        {
            let Ok(text) = fs::read_to_string(entry.path()) else {
                continue;
            };
            assert!(
                !text.contains("Host:"),
                "{} overrides the Host header",
                entry.path().display()
            );
        }
    }
}
