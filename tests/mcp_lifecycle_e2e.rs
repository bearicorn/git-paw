//! End-to-end tests for the root-session MCP tools
//! (`mcp-orchestrator-write-tools`): the session lifecycle tools
//! (`start_session`, `pause_session`, `stop_session`, `purge_session`) and the
//! verdict tools (`publish_verified`, `publish_feedback`), driven over the
//! real `git paw mcp` stdio transport — so a stray write to the server's
//! stdout from anything a tool runs would break the JSON-RPC framing here.
//!
//! Every tmux invocation — direct, or through the `git-paw` binary — goes
//! through a [`helpers::TmuxTestEnv`] private socket, and session state lives
//! under a private `HOME`; nothing here touches the default tmux server or the
//! user's sessions.

// The lifecycle scenario is one linear script; JSON-RPC helpers take owned
// `serde_json::Value` params built by `json!`.
#![allow(clippy::needless_pass_by_value, clippy::too_many_lines)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;

mod helpers;
use helpers::{TestRepo, TmuxTestEnv, setup_test_repo, tmux_test_env};

use git_paw::broker::messages::BrokerMessage;
use git_paw::broker::publish::fetch_log_over_http;

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// An OS-assigned port that refuses connections before it is handed out.
fn free_port() -> u16 {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port();
    let deadline = Instant::now() + Duration::from_secs(2);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    port
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
    fn new() -> Self {
        let repo = setup_test_repo();
        let tmux = tmux_test_env();
        let home = TempDir::new().expect("home tempdir");
        let port = free_port();
        let paw_dir = repo.path().join(".git-paw");
        fs::create_dir_all(&paw_dir).expect("create .git-paw");
        fs::write(
            paw_dir.join("config.toml"),
            format!("default_cli = \"cat\"\n[clis.cat]\ncommand = \"cat\"\n[broker]\nenabled = true\nport = {port}\n"),
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

    fn session_name(&self) -> String {
        format!("paw-{}", git_paw::git::project_name(self.repo.path()))
    }

    /// Runs `tmux <args>` on the private socket, returning success and stdout.
    fn tmux(&self, args: &[&str]) -> (bool, String) {
        let out = Command::new("tmux")
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

    fn session_alive(&self) -> bool {
        self.tmux(&["has-session", "-t", &self.session_name()]).0
    }

    /// Spawns `git paw mcp --repo <root>`, isolated to this sandbox.
    fn mcp(&self, root: &Path) -> McpClient {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_git-paw"));
        self.tmux.apply(&mut cmd);
        cmd.args(["mcp", "--repo", root.to_str().expect("utf-8 path")])
            .current_dir(root)
            .env("HOME", self.home.path())
            .env_remove("XDG_DATA_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        McpClient::spawn(cmd)
    }

    /// The worktree `git paw` created for `branch`.
    fn worktree_of(&self, branch: &str) -> PathBuf {
        let out = Command::new("git")
            .current_dir(self.repo.path())
            .args(["worktree", "list", "--porcelain"])
            .output()
            .expect("git worktree list");
        let listing = String::from_utf8_lossy(&out.stdout);
        let mut path = None;
        for line in listing.lines() {
            if let Some(p) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(p));
            } else if line == format!("branch refs/heads/{branch}") {
                return path.expect("a worktree line precedes its branch line");
            }
        }
        panic!("no worktree for {branch}; listing:\n{listing}");
    }

    fn verified_for(&self, agent: &str) -> usize {
        fetch_log_over_http(&self.url())
            .expect("fetch broker log")
            .iter()
            .filter(|m| matches!(m, BrokerMessage::Verified { agent_id, .. } if agent_id == agent))
            .count()
    }

    fn port_free_within(&self, budget: Duration) -> bool {
        let deadline = Instant::now() + budget;
        loop {
            if std::net::TcpListener::bind(("127.0.0.1", self.port)).is_ok() {
                return true;
            }
            if Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?} failed: {out:?}");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A line-delimited JSON-RPC client driving a spawned MCP server.
struct McpClient {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpClient {
    fn spawn(mut cmd: Command) -> Self {
        let mut child = cmd.spawn().expect("spawn git paw mcp");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut client = Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        };
        client.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "git-paw-lifecycle-e2e", "version": "0" },
            }),
        );
        client.send(
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized", "params": {} }),
        );
        client
    }

    fn send(&mut self, value: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        let line = serde_json::to_string(value).unwrap();
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

    /// Sends a request and returns the response with the matching id. Every
    /// line the server writes must be JSON: a stray stdout write from a tool
    /// fails here.
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).expect("read stdout");
            assert!(n != 0, "server closed stdout before answering {method}");
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let msg: Value = serde_json::from_str(trimmed)
                .unwrap_or_else(|e| panic!("stdout line is not JSON-RPC ({e}): {trimmed}"));
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                return msg;
            }
        }
    }

    /// `tools/call`, returning the whole response (a `result` or an `error`).
    fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        )
    }

    /// `tools/call` that must succeed; returns its structured content.
    fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let resp = self.call(name, arguments);
        let result = resp
            .get("result")
            .unwrap_or_else(|| panic!("{name} failed: {resp}"));
        assert_ne!(
            result.get("isError").and_then(Value::as_bool),
            Some(true),
            "{name} reported an error: {resp}"
        );
        result
            .get("structuredContent")
            .cloned()
            .unwrap_or_else(|| panic!("{name} returned no structured content: {resp}"))
    }

    /// `tools/call` that must fail; returns the error message.
    fn err(&mut self, name: &str, arguments: Value) -> String {
        let resp = self.call(name, arguments);
        if let Some(error) = resp.get("error") {
            return error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
        }
        let result = resp.get("result").cloned().unwrap_or_default();
        assert_eq!(
            result.get("isError").and_then(Value::as_bool),
            Some(true),
            "{name} should have failed: {resp}"
        );
        result.to_string()
    }

    fn tools(&mut self) -> Vec<Value> {
        let resp = self.request("tools/list", json!({}));
        resp.pointer("/result/tools")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_else(|| panic!("tools/list returned no tools: {resp}"))
    }

    fn shutdown(mut self) {
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn tool_named<'a>(tools: &'a [Value], name: &str) -> Option<&'a Value> {
    tools
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some(name))
}

fn properties(tool: &Value) -> Vec<String> {
    tool.pointer("/inputSchema/properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default()
}

const ROOT_ONLY: [&str; 6] = [
    "start_session",
    "pause_session",
    "stop_session",
    "purge_session",
    "publish_verified",
    "publish_feedback",
];

/// `mcp-server` "Session lifecycle tools for the root session" and
/// "Root-session detection", `mcp-agent-publish` "Supervisor authority verbs
/// are excluded from the MCP write surface" — one session, driven from a root
/// MCP server end to end, with a worktree server alongside it.
#[test]
fn a_root_session_drives_the_lifecycle_and_verdicts_and_a_worktree_cannot() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new();
    let mut root = sb.mcp(sb.repo.path());

    // "Root session sees root-only tools", "start_session never runs the
    // drive loop", "stop_session takes no confirmation or force parameter",
    // "Purge is annotated destructive".
    let tools = root.tools();
    for name in ROOT_ONLY {
        assert!(
            tool_named(&tools, name).is_some(),
            "{name} missing: {tools:?}"
        );
    }
    let start_props = properties(tool_named(&tools, "start_session").unwrap());
    assert!(
        !start_props.iter().any(|p| p == "unattended"),
        "{start_props:?}"
    );
    let stop_props = properties(tool_named(&tools, "stop_session").unwrap());
    assert!(
        !stop_props.iter().any(|p| p == "confirm" || p == "force"),
        "{stop_props:?}"
    );
    assert_eq!(
        tool_named(&tools, "purge_session")
            .unwrap()
            .pointer("/annotations/destructiveHint"),
        Some(&json!(true))
    );

    // "start_session launches detached".
    let started = root.ok(
        "start_session",
        json!({ "branches": ["feat/a", "feat/b"], "supervisor": true }),
    );
    assert_eq!(started["outcome"], "launched", "{started}");
    assert_eq!(started["session_name"], sb.session_name().as_str());
    assert!(
        started["attach_command"]
            .as_str()
            .is_some_and(|c| c.contains(&sb.session_name())),
        "{started}"
    );
    assert!(
        started["orchestrator_context"]
            .as_array()
            .is_some_and(|paths| paths.iter().any(|p| p == ".git-paw/AGENTS.local.md")),
        "{started}"
    );
    let (_, panes) = sb.tmux(&[
        "list-panes",
        "-t",
        &format!("{}:0", sb.session_name()),
        "-F",
        "#{pane_index}",
    ]);
    assert_eq!(
        panes.lines().count(),
        3,
        "dashboard + two agents: {panes:?}"
    );
    // The server is still responsive: no blocking loop.
    let status = root.ok("get_session_status", json!({}));
    assert_eq!(status["session"]["name"], sb.session_name().as_str());

    // "Root session verdict is accepted by the broker", from the same server
    // that started the session (the credential is read at call time).
    let verified = root.ok(
        "publish_verified",
        json!({ "agent": "feat-a", "message": "gates green" }),
    );
    assert_eq!(verified["published"], true);
    assert_eq!(sb.verified_for("feat-a"), 1);
    let feedback = root.ok(
        "publish_feedback",
        json!({ "agent": "feat-b", "gate": "testing", "errors": ["one test fails"] }),
    );
    assert_eq!(feedback["published"], true);

    // "Worktree session does not see root-only tools", "Calling a root-only
    // tool from a worktree errors cleanly", "Agent cannot self-verify via MCP".
    let mut worktree = sb.mcp(&sb.worktree_of("feat/b"));
    let wt_tools = worktree.tools();
    for name in ROOT_ONLY {
        assert!(
            tool_named(&wt_tools, name).is_none(),
            "{name} must not be listed in a worktree"
        );
    }
    assert!(tool_named(&wt_tools, "publish_status").is_some());
    let refused = worktree.err("pause_session", json!({}));
    assert!(refused.contains("worktree"), "{refused}");
    let refused = worktree.err(
        "publish_verified",
        json!({ "agent": "feat-b", "message": "self-verified" }),
    );
    assert!(refused.contains("worktree"), "{refused}");
    assert_eq!(sb.verified_for("feat-b"), 0, "nothing published");
    assert!(sb.session_alive(), "a refused call changes nothing");
    let branches = worktree.ok("get_branches", json!({}));
    assert!(
        branches.get("branches").is_some(),
        "the worktree server keeps running"
    );
    worktree.shutdown();

    // "Purge requires confirmation".
    let missing = root.err("purge_session", json!({}));
    assert!(missing.contains("confirm"), "{missing}");
    let declined = root.err("purge_session", json!({ "confirm": false }));
    assert!(declined.contains("confirm"), "{declined}");
    assert!(sb.session_alive(), "an unconfirmed purge changes nothing");

    // "pause and stop round-trip".
    let paused = root.ok("pause_session", json!({}));
    assert_eq!(paused["outcome"], "paused", "{paused}");
    assert_eq!(paused["no_session"], false);
    assert!(
        sb.port_free_within(Duration::from_secs(10)),
        "the broker port is free after pause"
    );
    assert_eq!(
        root.ok("get_session_status", json!({}))["session"]["status"],
        "paused"
    );
    let stopped = root.ok("stop_session", json!({}));
    assert_eq!(stopped["outcome"], "stopped", "{stopped}");
    assert!(!sb.session_alive(), "the tmux session is gone after stop");
    assert_eq!(
        root.ok("get_session_status", json!({}))["session"]["status"],
        "stopped"
    );

    // "Purge keeps unmerged branches".
    let wt_a = sb.worktree_of("feat/a");
    fs::write(wt_a.join("work.txt"), "unmerged\n").unwrap();
    git(&wt_a, &["add", "work.txt"]);
    git(&wt_a, &["commit", "-q", "-m", "unmerged work"]);
    let purged = root.ok("purge_session", json!({ "confirm": true }));
    assert_eq!(purged["no_session"], false, "{purged}");
    let retained = purged["retained"].as_array().expect("retained list");
    let kept = retained
        .iter()
        .find(|b| b["branch"] == "feat/a")
        .unwrap_or_else(|| panic!("feat/a must be retained: {purged}"));
    assert!(
        kept["tip"]
            .as_str()
            .is_some_and(|t| !t.is_empty() && t != "unknown"),
        "{purged}"
    );
    assert!(
        !git(sb.repo.path(), &["branch", "--list", "feat/a"])
            .trim()
            .is_empty(),
        "feat/a still exists"
    );

    root.shutdown();
}

/// `mcp-server` "No session yields a graceful result".
#[test]
fn lifecycle_tools_without_a_session_answer_no_session() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new();
    let mut root = sb.mcp(sb.repo.path());
    for (name, args) in [
        ("pause_session", json!({})),
        ("stop_session", json!({})),
        ("purge_session", json!({ "confirm": true })),
    ] {
        let resp = root.ok(name, args);
        assert_eq!(resp["no_session"], true, "{name}: {resp}");
    }
    root.shutdown();
}

/// `start_session` never asks: a choice the command would put to a picker is
/// refused with the parameter that answers it, and the server keeps serving
/// (an interactive prompt would read the JSON-RPC stream as its input). A
/// dry run reports the plan and creates nothing.
#[test]
fn start_session_never_prompts_and_a_dry_run_creates_nothing() {
    if !tmux_available() {
        return;
    }
    let sb = Sandbox::new();
    let mut root = sb.mcp(sb.repo.path());

    let refused = root.err("start_session", json!({ "dry_run": true }));
    assert!(refused.contains("`branches`"), "{refused}");

    let planned = root.ok(
        "start_session",
        json!({ "branches": ["feat/a"], "cli": "cat", "dry_run": true }),
    );
    assert_eq!(planned["outcome"], "dry_run", "{planned}");
    assert!(planned["session_name"].is_null(), "{planned}");
    assert!(!sb.session_alive(), "a dry run creates no session");

    let branches = root.ok("get_branches", json!({}));
    assert!(
        branches.get("branches").is_some(),
        "the server keeps running"
    );
    root.shutdown();
}

/// Review finding: root detection must describe the directory the server
/// serves, not a repository an inherited git environment points at. A server
/// started in an agent's worktree with `GIT_DIR` / `GIT_COMMON_DIR` naming the
/// main repository's git directory (and `GIT_WORK_TREE` naming the worktree)
/// is still a worktree session: no root-only tool is listed.
#[test]
fn inherited_git_location_env_does_not_make_a_worktree_a_root_session() {
    let repo = setup_test_repo();
    let worktree = repo.path().parent().expect("sandbox").join("wt");
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feat/env",
            worktree.to_str().unwrap(),
        ],
    );
    let worktree = worktree.canonicalize().unwrap();
    let main_git_dir = repo.path().join(".git").canonicalize().unwrap();
    let home = TempDir::new().expect("home tempdir");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_git-paw"));
    cmd.args(["mcp", "--repo", worktree.to_str().unwrap()])
        .current_dir(&worktree)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env("GIT_DIR", &main_git_dir)
        .env("GIT_COMMON_DIR", &main_git_dir)
        .env("GIT_WORK_TREE", &worktree)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut client = McpClient::spawn(cmd);

    let tools = client.tools();
    assert!(
        tool_named(&tools, "publish_status").is_some(),
        "the server answers: {tools:?}"
    );
    for name in ROOT_ONLY {
        assert!(
            tool_named(&tools, name).is_none(),
            "{name} listed for a worktree server under an inherited GIT_DIR"
        );
    }
    client.shutdown();
}
