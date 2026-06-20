//! End-to-end tests for the `git paw mcp` stdio server.
//!
//! Layer 3 of design D6: spawn the real binary, drive a JSON-RPC lifecycle
//! over stdin/stdout, and assert framing + degradation behaviour. Also covers
//! the live-broker read path (intents/conflicts) against a real broker, and a
//! source-level audit that no agent CLI is ever spawned.
//!
//! No tmux is required — the MCP server never touches tmux.

// Test ergonomics: JSON-RPC helpers take owned `serde_json::Value` params (the
// `json!` call sites build owned values), and the lifecycle test is a single
// linear script.
#![allow(clippy::needless_pass_by_value, clippy::too_many_lines)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// Initialises a throwaway git repo at `dir` with one commit on `main`.
fn git_init_with_commit(dir: &Path) {
    run_git(dir, &["init", "-q", "-b", "main"]);
    run_git(dir, &["config", "user.email", "t@example.com"]);
    run_git(dir, &["config", "user.name", "Test"]);
    std::fs::write(dir.join("README.md"), "hello\n").unwrap();
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-q", "-m", "init"]);
}

fn run_git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git runs")
        .success();
    assert!(ok, "git {args:?} failed");
}

/// A line-delimited JSON-RPC client driving the spawned MCP server.
struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    /// Every non-empty line received on stdout (for the "JSON-only" assertion).
    received: Vec<String>,
}

impl McpClient {
    fn spawn(repo: &Path) -> Self {
        let bin = env!("CARGO_BIN_EXE_git-paw");
        let mut child = Command::new(bin)
            .args(["mcp", "--repo", repo.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn git paw mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
            received: Vec::new(),
        }
    }

    fn send_line(&mut self, value: &Value) {
        let line = serde_json::to_string(value).unwrap();
        self.stdin.write_all(line.as_bytes()).unwrap();
        self.stdin.write_all(b"\n").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Reads the next non-empty stdout line, asserting it is valid JSON, and
    /// returns the parsed value.
    fn read_message(&mut self) -> Value {
        loop {
            let mut line = String::new();
            let n = self.stdout.read_line(&mut line).expect("read stdout");
            assert!(n != 0, "server closed stdout before a response was read");
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            self.received.push(trimmed.to_string());
            return serde_json::from_str(trimmed)
                .unwrap_or_else(|e| panic!("stdout line is not valid JSON ({e}): {trimmed}"));
        }
    }

    /// Sends a request and reads messages until the matching id response
    /// arrives (skipping any notifications).
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send_line(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));
        loop {
            let msg = self.read_message();
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                return msg;
            }
            // Otherwise it is a notification; keep reading.
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send_line(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Performs the MCP initialize handshake and returns the result object.
    fn initialize(&mut self) -> Value {
        let resp = self.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "git-paw-e2e", "version": "0" },
            }),
        );
        self.notify("notifications/initialized", json!({}));
        resp.get("result").cloned().expect("initialize result")
    }

    /// Calls a tool and returns the `result` object.
    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        let resp = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        resp.get("result")
            .cloned()
            .unwrap_or_else(|| panic!("tools/call {name} returned no result: {resp}"))
    }

    /// Closes stdin and asserts the process exits 0 within `timeout`.
    fn shutdown_and_assert_clean_exit(mut self, timeout: Duration) {
        // Dropping stdin closes the server's input → EOF → clean exit.
        drop(self.stdin);
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().expect("try_wait") {
                assert!(
                    status.success(),
                    "server exited non-zero on stdin EOF: {status}"
                );
                return;
            }
            if start.elapsed() > timeout {
                let _ = self.child.kill();
                panic!("server did not exit within {timeout:?} after stdin EOF");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn full_lifecycle_initialize_list_call_shutdown() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("proj");
    std::fs::create_dir(&repo).unwrap();
    git_init_with_commit(&repo);

    let mut client = McpClient::spawn(&repo);

    // initialize → advertises protocol version + tools capability.
    let init = client.initialize();
    assert!(
        init.get("protocolVersion").is_some(),
        "missing protocolVersion: {init}"
    );
    assert!(
        init.pointer("/capabilities/tools").is_some(),
        "server must advertise tools capability: {init}"
    );
    assert!(
        init.pointer("/serverInfo/name").is_some(),
        "missing serverInfo: {init}"
    );

    // tools/list → every tool carries a name + inputSchema (object root).
    let list = client.request("tools/list", json!({}));
    let tools = list
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("tools array");
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(Value::as_str))
        .collect();
    for expected in [
        "get_intents",
        "get_intent",
        "get_conflicts",
        "get_adrs",
        "get_adr",
        "get_test_strategy",
        "get_security_checklist",
        "get_dod",
        "check_dod",
        "get_constitution",
        "get_specs",
        "get_spec",
        "get_tasks",
        "get_task",
        "get_dependency_graph",
        "get_skill",
        "get_session_status",
        "get_session_summary",
        "get_learnings",
        "get_branches",
        "get_recent_commits",
        "get_diff",
    ] {
        assert!(
            names.contains(&expected),
            "tools/list missing {expected}: {names:?}"
        );
    }
    for tool in tools {
        let schema = tool.get("inputSchema").expect("inputSchema present");
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("object"),
            "inputSchema root type must be object for {:?}",
            tool.get("name")
        );
    }

    // Cold repo: coordination + session degrade to empty/null.
    let intents = client.call_tool("get_intents", json!({}));
    assert_eq!(
        intents.pointer("/structuredContent/intents"),
        Some(&json!([])),
        "cold repo get_intents must be empty: {intents}"
    );
    let conflicts = client.call_tool("get_conflicts", json!({}));
    assert_eq!(
        conflicts.pointer("/structuredContent/conflicts"),
        Some(&json!([]))
    );
    let session = client.call_tool("get_session_status", json!({}));
    assert_eq!(
        session.pointer("/structuredContent/session"),
        Some(&json!(null)),
        "cold repo session must be null: {session}"
    );
    let dod = client.call_tool("get_dod", json!({}));
    assert_eq!(
        dod.pointer("/structuredContent/content"),
        Some(&json!(null))
    );

    // Git tools work without any session.
    let branches = client.call_tool("get_branches", json!({}));
    let bs = branches
        .pointer("/structuredContent/branches")
        .and_then(Value::as_array)
        .expect("branches array");
    assert!(
        bs.iter()
            .any(|b| b.get("name").and_then(Value::as_str) == Some("main"))
    );

    // get_skill renders the embedded coordination skill.
    let skill = client.call_tool("get_skill", json!({ "name": "coordination" }));
    let content = skill
        .pointer("/structuredContent/skill/content")
        .and_then(Value::as_str)
        .expect("rendered skill content");
    assert!(!content.is_empty());
    assert!(
        !content.contains("{{BRANCH_ID}}"),
        "placeholders should be substituted in rendered skill"
    );

    // Unknown skill → null payload + message, NOT a transport error.
    let unknown_skill = client.call_tool("get_skill", json!({ "name": "does-not-exist" }));
    assert_eq!(
        unknown_skill.pointer("/structuredContent/skill"),
        Some(&json!(null))
    );

    // Unknown tool → JSON-RPC error, server keeps running.
    let resp = client.request(
        "tools/call",
        json!({ "name": "no_such_tool", "arguments": {} }),
    );
    assert!(
        resp.get("error").is_some(),
        "unknown tool must yield an error: {resp}"
    );
    // Server still alive: a follow-up request succeeds.
    let after = client.call_tool("get_branches", json!({}));
    assert!(after.pointer("/structuredContent/branches").is_some());

    // Every stdout line we observed was valid JSON (asserted in read_message).
    assert!(!client.received.is_empty());

    client.shutdown_and_assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn cold_repo_specs_and_learnings_are_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("proj");
    std::fs::create_dir(&repo).unwrap();
    git_init_with_commit(&repo);

    let mut client = McpClient::spawn(&repo);
    client.initialize();

    let specs = client.call_tool("get_specs", json!({}));
    assert_eq!(specs.pointer("/structuredContent/specs"), Some(&json!([])));

    let learnings = client.call_tool("get_learnings", json!({}));
    let sections = learnings
        .pointer("/structuredContent/sections")
        .and_then(Value::as_array)
        .expect("sections array");
    assert!(
        sections.iter().all(|s| s
            .pointer("/entries")
            .and_then(Value::as_array)
            .unwrap()
            .is_empty()),
        "cold repo learnings sections must all be empty"
    );

    client.shutdown_and_assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn invalid_spec_type_exits_nonzero_with_clear_message() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("proj");
    std::fs::create_dir(&repo).unwrap();
    git_init_with_commit(&repo);
    std::fs::create_dir(repo.join(".git-paw")).unwrap();
    std::fs::write(
        repo.join(".git-paw/config.toml"),
        "[specs]\ntype = \"unrecognised\"\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_git-paw");
    let output = Command::new(bin)
        .args(["mcp", "--repo", repo.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .expect("run git paw mcp");

    assert!(
        !output.status.success(),
        "invalid [specs].type must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("invalid") && stderr.contains("specs"),
        "stderr should identify the invalid spec type, got: {stderr}"
    );
}

#[test]
fn optional_parameters_are_marked_optional_in_schema() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("proj");
    std::fs::create_dir(&repo).unwrap();
    git_init_with_commit(&repo);

    let mut client = McpClient::spawn(&repo);
    client.initialize();
    let list = client.request("tools/list", json!({}));
    let tools = list
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .expect("tools array");
    let commits = tools
        .iter()
        .find(|t| t.get("name").and_then(Value::as_str) == Some("get_recent_commits"))
        .expect("get_recent_commits tool");
    let required: Vec<&str> = commits
        .pointer("/inputSchema/required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    assert!(
        required.contains(&"branch"),
        "branch must be required: {required:?}"
    );
    assert!(
        !required.contains(&"limit"),
        "limit (with a default) must be optional: {required:?}"
    );

    client.shutdown_and_assert_clean_exit(Duration::from_secs(2));
}

#[test]
fn non_git_repo_exits_nonzero_with_clear_message() {
    let tmp = tempfile::tempdir().unwrap();
    let not_repo = tmp.path().join("plain");
    std::fs::create_dir(&not_repo).unwrap();

    let bin = env!("CARGO_BIN_EXE_git-paw");
    let output = Command::new(bin)
        .args(["mcp", "--repo", not_repo.to_str().unwrap()])
        .stdin(Stdio::null())
        .output()
        .expect("run git paw mcp");

    assert!(
        !output.status.success(),
        "non-git --repo must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("not a git repository"),
        "stderr should explain the failure, got: {stderr}"
    );
}

#[test]
fn no_agent_cli_binary_is_spawned_anywhere_in_mcp_module() {
    // Guardrail (spec: no agent CLI as inference backend): the only external
    // process `src/mcp/` may spawn is `git`. Assert no Command::new(...) targets
    // an agent CLI binary across the module tree.
    let mcp_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("mcp");
    let forbidden = [
        "claude",
        "claude-oss",
        "gemini",
        "codex",
        "aider",
        "opencode",
        "vibe",
        "amp",
        "qwen",
    ];
    let mut offenders = Vec::new();
    visit_rs(&mcp_dir, &mut |path, contents| {
        for bin in forbidden {
            let needle = format!("Command::new(\"{bin}\"");
            if contents.contains(&needle) {
                offenders.push(format!("{}: spawns {bin}", path.display()));
            }
        }
    });
    assert!(
        offenders.is_empty(),
        "agent CLI spawn(s) found under src/mcp/: {offenders:?}"
    );
}

fn visit_rs(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rs(&path, f);
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Ok(contents) = std::fs::read_to_string(&path)
        {
            f(&path, &contents);
        }
    }
}

// --- Active-session read path against a real broker -----------------------

use git_paw::broker::delivery::publish_message;
use git_paw::broker::messages::{BrokerMessage, FileIntent, IntentPayload};
use git_paw::broker::{BrokerState, WatchTarget, start_broker_with};
use git_paw::config::BrokerConfig;
use git_paw::mcp::RepoContext;
use git_paw::mcp::query;

fn intent_msg(agent: &str, files: &[&str], summary: &str) -> BrokerMessage {
    BrokerMessage::Intent {
        agent_id: agent.to_string(),
        payload: IntentPayload {
            files: files
                .iter()
                .map(|f| FileIntent::Path((*f).to_string()))
                .collect(),
            summary: summary.to_string(),
            valid_for_seconds: 600,
        },
    }
}

#[test]
fn active_broker_populates_intents_and_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    #[allow(clippy::cast_possible_truncation)]
    let config = BrokerConfig {
        enabled: true,
        port: 21_000 + (std::process::id() as u16 % 500),
        bind: "127.0.0.1".to_string(),
        ..Default::default()
    };
    let state = BrokerState::new(None);
    let wt = |a: &str| WatchTarget {
        agent_id: a.to_string(),
        cli: "claude".to_string(),
        worktree_path: tmp.path().to_path_buf(),
    };
    // Skip gracefully if the port is busy locally.
    let Ok(handle) =
        start_broker_with(&config, state, vec![wt("feat-a"), wt("feat-b")], None, 3600)
    else {
        return;
    };

    // Two agents declaring overlapping intent on the same file → forward conflict.
    publish_message(
        &handle.state,
        &intent_msg("feat-a", &["src/shared.rs"], "edit shared"),
    );
    publish_message(
        &handle.state,
        &intent_msg("feat-b", &["src/shared.rs"], "also edit shared"),
    );

    let ctx = RepoContext {
        root: tmp.path().to_path_buf(),
        git_paw_dir: None,
        broker_url: Some(handle.url.clone()),
    };

    let intents = query::intents::active_intents(&ctx);
    assert_eq!(
        intents.len(),
        2,
        "both active intents should be read over HTTP"
    );
    assert!(intents.iter().any(|i| i.branch_id == "feat-a"));
    assert!(
        intents
            .iter()
            .any(|i| i.files.contains(&"src/shared.rs".to_string()))
    );

    // Single-agent lookup.
    let one = query::intents::intent_for(&ctx, "feat-b").expect("feat-b intent");
    assert_eq!(one.summary, "also edit shared");

    // Conflict reconstruction detects the overlap.
    let conflicts = query::conflicts::conflicts(&ctx);
    assert_eq!(
        conflicts.len(),
        1,
        "overlapping intents should yield one conflict"
    );
    assert_eq!(
        conflicts[0].branches,
        ["feat-a".to_string(), "feat-b".to_string()]
    );
    assert!(
        conflicts[0]
            .files
            .iter()
            .any(|f| f.contains("src/shared.rs"))
    );
}
