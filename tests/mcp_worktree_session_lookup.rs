//! `session-state`'s scenario "The MCP server started in a worktree finds the
//! broker": `git paw mcp` spawned by an agent's CLI inherits that agent's
//! worktree as its working directory and is given no `--repo`, so its
//! repository context is resolved from inside a LINKED worktree. A worktree
//! root never equals the repository root, so the session lookup used to miss
//! and the server came up with no broker URL — its publish tools had nowhere
//! to publish.
//!
//! Driven as a CHILD process (the real `git paw mcp` binary over stdio) rather
//! than an in-process library call, so `HOME` / `XDG_DATA_HOME` / `GIT_DIR` are
//! set on that child alone. Mutating this test process's environment would
//! leak into every non-serial test running in parallel — `#[serial]` only
//! orders a test against other `#[serial]` tests — and any of them that shells
//! out to git could then operate on the wrong repository.
//!
//! The second test pins the complementary hardening: an inherited `GIT_DIR`
//! (a tmux pane or a git hook can export one) must not redirect the owner
//! resolution at a different repository.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

fn git(args: &[&str], cwd: &Path) {
    let out = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Mirrors `crate::dirs::data_dir()` for a given fake `HOME`.
fn sessions_dir_under(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support")
            .join("git-paw")
            .join("sessions")
    }
    #[cfg(not(target_os = "macos"))]
    {
        home.join(".local/share").join("git-paw").join("sessions")
    }
}

/// A line-delimited JSON-RPC client driving a spawned `git paw mcp` server.
struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpClient {
    /// Spawns the server with `cwd` as its working directory and NO `--repo`
    /// — exactly how an agent's CLI starts it — with `extra_env` applied to
    /// the child only.
    fn spawn_in(cwd: &Path, home: &Path, extra_env: &[(&str, PathBuf)]) -> Self {
        let bin = env!("CARGO_BIN_EXE_git-paw");
        let mut command = Command::new(bin);
        command
            .arg("mcp")
            .current_dir(cwd)
            .env("HOME", home)
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("spawn git paw mcp");
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = BufReader::new(child.stdout.take().expect("child stdout"));
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    fn request(&mut self, method: &str, params: &Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        let line = serde_json::to_string(&json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params,
        }))
        .expect("serialize request");
        self.stdin.write_all(line.as_bytes()).expect("write stdin");
        self.stdin.write_all(b"\n").expect("write newline");
        self.stdin.flush().expect("flush stdin");
        loop {
            let mut raw = String::new();
            let n = self.stdout.read_line(&mut raw).expect("read stdout");
            assert!(n != 0, "server closed stdout before responding");
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }
            let msg: Value = serde_json::from_str(trimmed)
                .unwrap_or_else(|e| panic!("stdout line is not JSON ({e}): {trimmed}"));
            if msg.get("id").and_then(Value::as_i64) == Some(id) {
                return msg;
            }
        }
    }

    fn initialize(&mut self) {
        self.request(
            "initialize",
            &json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "git-paw-worktree-lookup", "version": "0" },
            }),
        );
        let line = serde_json::to_string(&json!({
            "jsonrpc": "2.0", "method": "notifications/initialized", "params": {},
        }))
        .expect("serialize notification");
        self.stdin.write_all(line.as_bytes()).expect("write stdin");
        self.stdin.write_all(b"\n").expect("write newline");
        self.stdin.flush().expect("flush stdin");
    }

    fn call_tool(&mut self, name: &str, arguments: &Value) -> Value {
        let resp = self.request(
            "tools/call",
            &json!({ "name": name, "arguments": arguments }),
        );
        resp.get("result")
            .cloned()
            .unwrap_or_else(|| panic!("tools/call {name} returned no result: {resp}"))
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Builds a repository with one embedded agent worktree plus an ACTIVE
/// broker-enabled session receipt under `home`, and returns the worktree root.
fn repo_with_worktree_and_session(sandbox: &Path, home: &Path) -> PathBuf {
    let repo = sandbox.join("repo");
    fs::create_dir_all(&repo).expect("mk repo");

    git(&["init", "-q", "-b", "main"], &repo);
    git(&["config", "user.email", "t@e.st"], &repo);
    git(&["config", "user.name", "Test"], &repo);
    fs::write(repo.join("README.md"), "x").expect("readme");
    git(&["add", "."], &repo);
    git(&["commit", "-q", "-m", "init"], &repo);

    // An embedded agent worktree, as `git paw start` creates.
    let worktree = repo.join(".git-paw").join("worktrees").join("feat-x");
    git(
        &[
            "worktree",
            "add",
            "-b",
            "feat/x",
            worktree.to_str().expect("utf-8 worktree path"),
        ],
        &repo,
    );

    let sessions = sessions_dir_under(home);
    fs::create_dir_all(&sessions).expect("mk sessions dir");
    let session_json = json!({
        "session_name": "paw-repo",
        "repo_path": repo.canonicalize().expect("canonical repo").to_string_lossy(),
        "project_name": "repo",
        "created_at": "2026-05-01T00:00:00Z",
        "status": "active",
        "mode": "bare",
        "worktrees": [{
            "branch": "feat/x",
            "worktree_path": worktree.to_string_lossy(),
            "cli": "claude",
            "branch_created": true,
        }],
        "broker_port": 9119,
        "broker_bind": "127.0.0.1",
        "dashboard_pane": 0,
    });
    fs::write(
        sessions.join("paw-repo.json"),
        serde_json::to_string_pretty(&session_json).expect("serialize session"),
    )
    .expect("write session receipt");

    worktree
}

/// Asserts `get_session_status` reports the owning repository's session with
/// its broker URL.
fn assert_sees_the_owning_session(client: &mut McpClient) {
    client.initialize();
    let status = client.call_tool("get_session_status", &json!({}));
    let session = status
        .pointer("/structuredContent/session")
        .unwrap_or_else(|| panic!("no session in get_session_status result: {status}"));
    assert_eq!(
        session.get("name").and_then(Value::as_str),
        Some("paw-repo"),
        "the MCP server started inside an agent worktree must see the owning \
         repository's session: {status}"
    );
    assert_eq!(
        session.get("broker_url").and_then(Value::as_str),
        Some("http://127.0.0.1:9119"),
        "its broker URL must be the session's, not absent: {status}"
    );
}

#[test]
fn mcp_started_inside_a_worktree_sees_the_owning_session_and_its_broker() {
    let sandbox = TempDir::new().expect("sandbox");
    let home = TempDir::new().expect("home");
    let worktree = repo_with_worktree_and_session(sandbox.path(), home.path());

    let mut client = McpClient::spawn_in(&worktree, home.path(), &[]);
    assert_sees_the_owning_session(&mut client);
}

/// An inherited `GIT_DIR` — a tmux pane or a git hook can export one — must
/// not redirect the owner resolution at a different repository.
#[test]
fn an_inherited_git_dir_does_not_redirect_the_owner_resolution() {
    let sandbox = TempDir::new().expect("sandbox");
    let home = TempDir::new().expect("home");
    let worktree = repo_with_worktree_and_session(sandbox.path(), home.path());

    // A second, unrelated repository whose `.git` the child will inherit as
    // `GIT_DIR`. It has no session receipt, so a redirected lookup finds none.
    let other = sandbox.path().join("other");
    fs::create_dir_all(&other).expect("mk other repo");
    git(&["init", "-q", "-b", "main"], &other);

    let mut client =
        McpClient::spawn_in(&worktree, home.path(), &[("GIT_DIR", other.join(".git"))]);
    assert_sees_the_owning_session(&mut client);
}
