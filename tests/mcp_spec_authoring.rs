//! End-to-end tests for the spec-authoring MCP tools
//! (`mcp-spec-authoring-tools`): `create_spec`, `write_spec_artifact` and
//! `validate_spec`, driven over the real `git paw mcp` stdio transport against
//! a real repository in a temp directory.
//!
//! Covers the repository-write consent gate (off by default, repository config
//! only, root session only), the write tools' behavior, write confinement, and
//! the read-only `validate_spec`. No tmux is involved; each server runs with a
//! private `HOME` so no user-level config or session leaks in.

#![allow(clippy::needless_pass_by_value)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;

const WRITE_TOOLS: [&str; 2] = ["create_spec", "write_spec_artifact"];

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?} failed: {out:?}");
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A repository at `<tmp>/repo` with `openspec/changes/` committed, plus a
/// private `HOME`. `repo_config` is written to `.git-paw/config.toml` and
/// committed, so authored files are the only untracked paths.
struct Sandbox {
    tmp: TempDir,
    home: TempDir,
}

impl Sandbox {
    fn new(repo_config: &str) -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let home = TempDir::new().expect("home");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(repo.join("openspec/changes")).unwrap();
        fs::write(repo.join("openspec/changes/.gitkeep"), "").unwrap();
        fs::create_dir_all(repo.join(".git-paw")).unwrap();
        fs::write(repo.join(".git-paw/config.toml"), repo_config).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["config", "user.email", "t@example.com"]);
        git(&repo, &["config", "user.name", "Test"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "init"]);
        Self { tmp, home }
    }

    fn consenting() -> Self {
        Self::new(OPENSPEC_ON)
    }

    fn repo(&self) -> PathBuf {
        self.tmp.path().join("repo").canonicalize().unwrap()
    }

    fn changes(&self) -> PathBuf {
        self.repo().join("openspec/changes")
    }

    /// Writes a user-level config under the private `HOME` (both the macOS
    /// and XDG locations, so the test holds on either platform).
    fn user_config(&self, content: &str) {
        for dir in ["Library/Application Support/git-paw", ".config/git-paw"] {
            let d = self.home.path().join(dir);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("config.toml"), content).unwrap();
        }
    }

    fn mcp(&self, root: &Path) -> McpClient {
        self.mcp_with_log(root, None)
    }

    fn mcp_with_log(&self, root: &Path, log: Option<&Path>) -> McpClient {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_git-paw"));
        cmd.args(["mcp", "--repo", root.to_str().unwrap()]);
        if let Some(log) = log {
            cmd.args(["--log-file", log.to_str().unwrap()]);
        }
        cmd.current_dir(root)
            .env("HOME", self.home.path())
            .env("XDG_CONFIG_HOME", self.home.path().join(".config"))
            .env_remove("XDG_DATA_HOME")
            .env_remove("RUST_LOG")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("TMUX")
            .env_remove("TMUX_PANE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        McpClient::spawn(cmd)
    }

    /// A `git worktree add` worktree at `<tmp>/wt` whose own config sets the
    /// switch on.
    fn worktree_with_switch_on(&self) -> PathBuf {
        let wt = self.tmp.path().join("wt");
        git(
            &self.repo(),
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "feat/x",
                wt.to_str().unwrap(),
            ],
        );
        fs::write(wt.join(".git-paw/config.toml"), OPENSPEC_ON).unwrap();
        wt.canonicalize().unwrap()
    }
}

const OPENSPEC_ON: &str =
    "[specs]\ntype = \"openspec\"\ndir = \"openspec/changes\"\n\n[mcp]\nallow_repo_writes = true\n";
const OPENSPEC_OFF: &str = "[specs]\ntype = \"openspec\"\ndir = \"openspec/changes\"\n";

/// A line-delimited JSON-RPC client driving a spawned MCP server. Every line
/// the server writes to stdout must parse as JSON-RPC.
struct McpClient {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    /// The `initialize` response.
    init: Value,
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
            init: Value::Null,
        };
        client.init = client.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "git-paw-spec-authoring-e2e", "version": "0" },
            }),
        );
        client.send(
            &json!({ "jsonrpc": "2.0", "method": "notifications/initialized", "params": {} }),
        );
        client
    }

    fn send(&mut self, value: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        stdin
            .write_all(serde_json::to_string(value).unwrap().as_bytes())
            .unwrap();
        stdin.write_all(b"\n").unwrap();
        stdin.flush().unwrap();
    }

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

    /// `tools/call` that must fail; returns the error text.
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

    fn tool_names(&mut self) -> Vec<String> {
        self.tools()
            .iter()
            .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_string))
            .collect()
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

/// Every file (relative path → bytes) under `dir`, not following links.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap().flatten() {
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                if path.file_name().is_some_and(|n| n == ".git") {
                    continue;
                }
                stack.push(path);
            } else if kind.is_file() {
                out.push((
                    path.strip_prefix(dir).unwrap().to_path_buf(),
                    fs::read(&path).unwrap(),
                ));
            } else {
                out.push((path.strip_prefix(dir).unwrap().to_path_buf(), Vec::new()));
            }
        }
    }
    out.sort();
    out
}

const PROPOSAL: &str = "## Why\n\nBecause.\n";
const GOOD_DELTA: &str = "## ADDED Requirements\n\n### Requirement: Export\n\nThe system SHALL export data.\n\n#### Scenario: Export works\n\n- **WHEN** asked\n- **THEN** it exports\n";

// --- consent gate -----------------------------------------------------------

// Scenario: Writes are off by default.
#[test]
fn writes_are_off_by_default() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    let mut mcp = sb.mcp(&sb.repo());
    let names = mcp.tool_names();
    for w in WRITE_TOOLS {
        assert!(!names.iter().any(|n| n == w), "{w} listed: {names:?}");
    }
    for read in ["validate_spec", "get_specs", "get_spec", "read_file"] {
        assert!(names.iter().any(|n| n == read), "{read} missing");
    }
    mcp.shutdown();
}

// Scenario: A by-name call while writes are off names the switch.
#[test]
fn a_by_name_call_while_off_names_the_switch_and_creates_nothing() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    let before = snapshot(&sb.repo());
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err("create_spec", json!({ "name": "add-x" }));
    assert!(msg.contains("allow_repo_writes"), "{msg}");
    assert!(msg.contains(".git-paw/config.toml"), "{msg}");
    let msg = mcp.err(
        "write_spec_artifact",
        json!({ "change": "add-x", "path": "proposal.md", "content": "x" }),
    );
    assert!(msg.contains("allow_repo_writes"), "{msg}");
    mcp.shutdown();
    assert!(!sb.changes().join("add-x").exists());
    assert_eq!(snapshot(&sb.repo()), before);
}

// Scenario: Consent enables the write tools for the root session.
#[test]
fn consent_lists_the_write_tools() {
    let sb = Sandbox::consenting();
    let mut mcp = sb.mcp(&sb.repo());
    let names = mcp.tool_names();
    for w in WRITE_TOOLS.iter().chain(&["validate_spec"]) {
        assert!(names.iter().any(|n| n == w), "{w} missing: {names:?}");
    }
    mcp.shutdown();
}

// Scenario: The user-level config cannot consent.
#[test]
fn the_user_level_config_cannot_consent() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    sb.user_config("[mcp]\nname = \"from-user\"\nallow_repo_writes = true\n");
    let mut mcp = sb.mcp(&sb.repo());
    // The user-level file was read (its name applies), yet it cannot consent.
    assert_eq!(
        mcp.init.pointer("/result/serverInfo/name"),
        Some(&json!("from-user")),
        "{}",
        mcp.init
    );
    let names = mcp.tool_names();
    for w in WRITE_TOOLS {
        assert!(!names.iter().any(|n| n == w), "{w} listed: {names:?}");
    }
    let msg = mcp.err("create_spec", json!({ "name": "add-x" }));
    assert!(msg.contains("allow_repo_writes"), "{msg}");
    mcp.shutdown();
    assert!(!sb.changes().join("add-x").exists());
}

// Scenario: A worktree session never gets the write tools.
#[test]
fn a_worktree_session_neither_lists_nor_executes_the_write_tools() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    let wt = sb.worktree_with_switch_on();
    fs::create_dir_all(wt.join("openspec/changes/add-export")).unwrap();
    let before = snapshot(&wt);
    let mut mcp = sb.mcp(&wt);
    let names = mcp.tool_names();
    for w in WRITE_TOOLS {
        assert!(!names.iter().any(|n| n == w), "{w} listed: {names:?}");
    }
    let msg = mcp.err(
        "write_spec_artifact",
        json!({ "change": "add-export", "path": "proposal.md", "content": "x" }),
    );
    assert!(msg.contains("worktree"), "{msg}");
    let msg = mcp.err("create_spec", json!({ "name": "add-y" }));
    assert!(msg.contains("worktree"), "{msg}");
    mcp.shutdown();
    assert_eq!(snapshot(&wt), before);
}

// Scenario: No tool parameter grants consent.
#[test]
fn no_advertised_schema_has_a_consent_parameter() {
    for sb in [Sandbox::consenting(), Sandbox::new(OPENSPEC_OFF)] {
        let mut mcp = sb.mcp(&sb.repo());
        for tool in mcp.tools() {
            let props: Vec<String> = tool
                .pointer("/inputSchema/properties")
                .and_then(Value::as_object)
                .map(|p| p.keys().cloned().collect())
                .unwrap_or_default();
            assert!(
                !props.iter().any(|p| p == "allow_repo_writes"
                    || p.contains("consent")
                    || p.starts_with("allow")),
                "{tool}"
            );
        }
        mcp.shutdown();
    }
}

// --- create_spec ------------------------------------------------------------

// Scenario: create_spec scaffolds an OpenSpec change.
#[test]
fn create_spec_scaffolds_an_openspec_change() {
    let sb = Sandbox::consenting();
    let mut mcp = sb.mcp(&sb.repo());
    let out = mcp.ok("create_spec", json!({ "name": "add-export" }));
    mcp.shutdown();
    assert_eq!(
        out["written"],
        json!(["openspec/changes/add-export/.openspec.yaml"])
    );
    let meta = fs::read_to_string(sb.changes().join("add-export/.openspec.yaml")).unwrap();
    assert!(meta.starts_with("schema: spec-driven\ncreated: "), "{meta}");
    let date = meta.lines().nth(1).unwrap().trim_start_matches("created: ");
    assert!(
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok(),
        "{meta}"
    );
}

// Scenario: create_spec refuses an existing change.
#[test]
fn create_spec_refuses_an_existing_change() {
    let sb = Sandbox::consenting();
    let existing = sb.changes().join("add-export");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("proposal.md"), PROPOSAL).unwrap();
    let before = snapshot(&existing);
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err("create_spec", json!({ "name": "add-export" }));
    mcp.shutdown();
    assert!(msg.contains("exists"), "{msg}");
    assert_eq!(snapshot(&existing), before);
}

// Scenario: create_spec refuses an invalid name.
#[test]
fn create_spec_refuses_invalid_names() {
    let sb = Sandbox::consenting();
    let before = snapshot(&sb.repo());
    let mut mcp = sb.mcp(&sb.repo());
    for name in ["Add_Export", "../x"] {
        let msg = mcp.err("create_spec", json!({ "name": name }));
        assert!(
            msg.contains("lowercase letters, digits and hyphens"),
            "{msg}"
        );
    }
    mcp.shutdown();
    assert_eq!(snapshot(&sb.repo()), before);
    assert!(!sb.tmp.path().join("x").exists());
}

// --- write_spec_artifact ----------------------------------------------------

// Scenario: write_spec_artifact writes a delta spec.
#[test]
fn write_spec_artifact_writes_a_delta_spec() {
    let sb = Sandbox::consenting();
    fs::create_dir_all(sb.changes().join("add-export")).unwrap();
    let mut mcp = sb.mcp(&sb.repo());
    let out = mcp.ok(
        "write_spec_artifact",
        json!({
            "change": "add-export",
            "path": "specs/data-export/spec.md",
            "content": "## ADDED Requirements\n",
        }),
    );
    mcp.shutdown();
    assert_eq!(out["replaced"], json!(false));
    assert_eq!(
        out["written"],
        json!(["openspec/changes/add-export/specs/data-export/spec.md"])
    );
    assert_eq!(
        fs::read_to_string(sb.changes().join("add-export/specs/data-export/spec.md")).unwrap(),
        "## ADDED Requirements\n"
    );
}

// Scenario: write_spec_artifact replaces an existing artifact.
#[test]
fn write_spec_artifact_replaces_an_existing_artifact() {
    let sb = Sandbox::consenting();
    let change = sb.changes().join("add-export");
    fs::create_dir_all(&change).unwrap();
    fs::write(change.join("tasks.md"), "old").unwrap();
    let mut mcp = sb.mcp(&sb.repo());
    let out = mcp.ok(
        "write_spec_artifact",
        json!({ "change": "add-export", "path": "tasks.md", "content": "new" }),
    );
    mcp.shutdown();
    assert_eq!(out["replaced"], json!(true));
    assert_eq!(fs::read_to_string(change.join("tasks.md")).unwrap(), "new");
}

// Scenario: write_spec_artifact needs an existing change.
#[test]
fn write_spec_artifact_needs_an_existing_change() {
    let sb = Sandbox::consenting();
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err(
        "write_spec_artifact",
        json!({ "change": "nope", "path": "proposal.md", "content": "x" }),
    );
    mcp.shutdown();
    assert!(msg.contains("nope"), "{msg}");
    assert!(!sb.changes().join("nope").exists());
}

// Scenario: Writes are logged and never committed.
#[test]
fn writes_are_logged_and_never_committed() {
    let sb = Sandbox::consenting();
    let repo = sb.repo();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let index = git(&repo, &["ls-files", "--stage"]);
    let log = sb.tmp.path().join("mcp.log");
    let mut mcp = sb.mcp_with_log(&repo, Some(&log));
    mcp.ok("create_spec", json!({ "name": "add-export" }));
    mcp.ok(
        "write_spec_artifact",
        json!({ "change": "add-export", "path": "proposal.md", "content": PROPOSAL }),
    );
    mcp.shutdown();
    let logged = fs::read_to_string(&log).unwrap();
    assert!(
        logged.contains("openspec/changes/add-export/proposal.md"),
        "{logged}"
    );
    assert!(logged.contains("repository writes are ENABLED"), "{logged}");
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&repo, &["ls-files", "--stage"]), index);
    let status = git(&repo, &["status", "--porcelain", "--untracked-files=all"]);
    assert!(
        status
            .lines()
            .any(|l| l == "?? openspec/changes/add-export/proposal.md"),
        "{status}"
    );
}

// Scenario: A user-level [specs] section cannot redirect writes.
#[test]
fn a_user_level_specs_section_cannot_redirect_writes() {
    let sb = Sandbox::new("[mcp]\nallow_repo_writes = true\n");
    sb.user_config("[specs]\ntype = \"openspec\"\ndir = \".github\"\n");
    fs::create_dir_all(sb.repo().join(".github/workflows")).unwrap();
    let before = snapshot(&sb.repo());
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err(
        "write_spec_artifact",
        json!({ "change": "workflows", "path": "pwn.yml", "content": "x" }),
    );
    assert!(msg.contains("[specs]"), "{msg}");
    let msg = mcp.err("create_spec", json!({ "name": "add-export" }));
    assert!(msg.contains("[specs]"), "{msg}");
    mcp.shutdown();
    assert_eq!(snapshot(&sb.repo()), before);
}

// With a repository [specs] section, a different user-level dir is ignored.
#[test]
fn the_repository_specs_dir_wins_over_a_user_level_one() {
    let sb = Sandbox::consenting();
    sb.user_config("[specs]\ntype = \"openspec\"\ndir = \"elsewhere\"\n");
    fs::create_dir_all(sb.repo().join("elsewhere")).unwrap();
    let mut mcp = sb.mcp(&sb.repo());
    let out = mcp.ok("create_spec", json!({ "name": "add-export" }));
    mcp.shutdown();
    assert_eq!(
        out["written"],
        json!(["openspec/changes/add-export/.openspec.yaml"])
    );
    assert!(
        fs::read_dir(sb.repo().join("elsewhere"))
            .unwrap()
            .next()
            .is_none()
    );
}

// Scenario: Metadata and oversized content are refused.
#[test]
fn metadata_path_and_oversized_content_are_refused() {
    let sb = Sandbox::consenting();
    let mut mcp = sb.mcp(&sb.repo());
    mcp.ok("create_spec", json!({ "name": "add-export" }));
    let before = snapshot(&sb.repo());
    let msg = mcp.err(
        "write_spec_artifact",
        json!({ "change": "add-export", "path": ".openspec.yaml", "content": "schema: x\n" }),
    );
    assert!(msg.contains(".openspec.yaml"), "{msg}");
    let big = "x".repeat(1024 * 1024 + 1);
    let msg = mcp.err(
        "write_spec_artifact",
        json!({ "change": "add-export", "path": "proposal.md", "content": big }),
    );
    assert!(msg.contains("1 MiB"), "{msg}");
    // Exactly at the limit is accepted.
    mcp.ok(
        "write_spec_artifact",
        json!({ "change": "add-export", "path": "design.md", "content": "x".repeat(1024 * 1024) }),
    );
    mcp.shutdown();
    let after = snapshot(&sb.repo());
    assert_eq!(after.len(), before.len() + 1, "only design.md was added");
    assert!(!sb.changes().join("add-export/proposal.md").exists());
}

// Scenario: Another spec format is not supported.
#[test]
fn another_spec_format_is_not_supported() {
    let sb = Sandbox::new(
        "[specs]\ntype = \"markdown\"\ndir = \"openspec/changes\"\n\n[mcp]\nallow_repo_writes = true\n",
    );
    let before = snapshot(&sb.repo());
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err("create_spec", json!({ "name": "add-export" }));
    assert!(msg.contains("markdown"), "{msg}");
    let msg = mcp.err("validate_spec", json!({ "change": "add-export" }));
    assert!(msg.contains("markdown"), "{msg}");
    mcp.shutdown();
    assert_eq!(snapshot(&sb.repo()), before);
}

// No [specs] section: the error names [specs] and nothing is written.
#[test]
fn no_spec_system_configured_names_specs() {
    let sb = Sandbox::new("[mcp]\nallow_repo_writes = true\n");
    let before = snapshot(&sb.repo());
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err("create_spec", json!({ "name": "add-export" }));
    mcp.shutdown();
    assert!(msg.contains("[specs]"), "{msg}");
    assert_eq!(snapshot(&sb.repo()), before);
}

// --- confinement ------------------------------------------------------------

/// In one session against change `add-export`: a legitimate write succeeds
/// (so the tool is live), then `write_spec_artifact` with `path` is refused
/// with an error containing `names`, and nothing else changes under `watch`.
fn assert_write_refused(sb: &Sandbox, path: &str, names: &str, watch: &[&Path]) {
    let mut mcp = sb.mcp(&sb.repo());
    mcp.ok(
        "write_spec_artifact",
        json!({ "change": "add-export", "path": "proposal.md", "content": PROPOSAL }),
    );
    let before: Vec<_> = watch.iter().map(|d| snapshot(d)).collect();
    let msg = mcp.err(
        "write_spec_artifact",
        json!({ "change": "add-export", "path": path, "content": "x" }),
    );
    mcp.shutdown();
    assert!(
        msg.contains(names),
        "{path}: error should name {names:?}: {msg}"
    );
    let after: Vec<_> = watch.iter().map(|d| snapshot(d)).collect();
    assert_eq!(after, before, "{path} wrote something");
}

// Scenario: Path traversal is refused.
#[test]
fn path_traversal_is_refused() {
    let sb = Sandbox::consenting();
    fs::create_dir_all(sb.changes().join("add-export")).unwrap();
    assert_write_refused(&sb, "../../../src/main.rs", "`..`", &[sb.tmp.path()]);
    assert!(!sb.repo().join("src/main.rs").exists());
}

// Scenario: Absolute paths and .git components are refused.
#[test]
fn absolute_paths_and_git_components_are_refused() {
    let sb = Sandbox::consenting();
    let change = sb.changes().join("add-export");
    fs::create_dir_all(&change).unwrap();
    let outside = TempDir::new().unwrap();
    let abs = outside.path().join("x.md");
    assert_write_refused(
        &sb,
        abs.to_str().unwrap(),
        "relative",
        &[sb.tmp.path(), outside.path()],
    );
    assert!(!abs.exists());
    assert_write_refused(&sb, ".git/hooks/pre-commit", ".git", &[sb.tmp.path()]);
    assert!(!change.join(".git").exists());
    assert!(!sb.repo().join(".git/hooks/pre-commit").exists());
}

// Scenario: A symlinked directory leading out is refused.
#[test]
fn a_symlinked_directory_leading_out_is_refused() {
    let sb = Sandbox::consenting();
    let change = sb.changes().join("add-export");
    fs::create_dir_all(&change).unwrap();
    let outside = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), change.join("specs")).unwrap();
    assert_write_refused(
        &sb,
        "specs/data-export/spec.md",
        "\"specs\"",
        &[outside.path(), sb.tmp.path()],
    );
    assert!(!outside.path().join("data-export").exists());
}

// Scenario: A symlinked target file is refused.
#[test]
fn a_symlinked_target_file_is_refused() {
    let sb = Sandbox::consenting();
    let change = sb.changes().join("add-export");
    fs::create_dir_all(&change).unwrap();
    let outside = TempDir::new().unwrap();
    let target = outside.path().join("victim.md");
    fs::write(&target, "original").unwrap();
    std::os::unix::fs::symlink(&target, change.join("tasks.md")).unwrap();
    assert_write_refused(&sb, "tasks.md", "\"tasks.md\"", &[outside.path()]);
    assert_eq!(fs::read_to_string(&target).unwrap(), "original");
    assert!(
        fs::symlink_metadata(change.join("tasks.md"))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

// Scenario: A spec directory outside the repository is refused.
#[test]
fn a_spec_directory_outside_the_repository_is_refused() {
    let sb = Sandbox::new(
        "[specs]\ntype = \"openspec\"\ndir = \"../outside\"\n\n[mcp]\nallow_repo_writes = true\n",
    );
    let outside = sb.tmp.path().join("outside");
    fs::create_dir_all(&outside).unwrap();
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err("create_spec", json!({ "name": "add-export" }));
    mcp.shutdown();
    assert!(msg.contains("resolves outside the repository"), "{msg}");
    assert!(fs::read_dir(&outside).unwrap().next().is_none());
}

// A spec directory that is a symlink out of the repository is refused too.
#[test]
fn a_symlinked_spec_directory_leading_out_is_refused() {
    let sb = Sandbox::new(
        "[specs]\ntype = \"openspec\"\ndir = \"linked\"\n\n[mcp]\nallow_repo_writes = true\n",
    );
    let outside = TempDir::new().unwrap();
    std::os::unix::fs::symlink(outside.path(), sb.repo().join("linked")).unwrap();
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err("create_spec", json!({ "name": "add-export" }));
    mcp.shutdown();
    assert!(msg.contains("resolves outside the repository"), "{msg}");
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
}

// A spec directory at the repository root (or inside .git-paw) would let a
// "change" name address .git-paw, .github or src: refused, config unchanged.
#[test]
fn a_spec_directory_at_the_root_or_in_git_paw_cannot_reach_the_config() {
    for dir in [".", ".git-paw"] {
        let sb = Sandbox::new(&format!(
            "[specs]\ntype = \"openspec\"\ndir = \"{dir}\"\n\n[mcp]\nallow_repo_writes = true\n"
        ));
        fs::create_dir_all(sb.repo().join(".github")).unwrap();
        let before = snapshot(&sb.repo());
        let mut mcp = sb.mcp(&sb.repo());
        for (change, path) in [
            (".git-paw", "config.toml"),
            ("scripts", "x.sh"),
            (".github", "workflows/ci.yml"),
        ] {
            let msg = mcp.err(
                "write_spec_artifact",
                json!({ "change": change, "path": path, "content": "x" }),
            );
            assert!(!msg.is_empty(), "{dir} {change}");
        }
        let msg = mcp.err("create_spec", json!({ "name": "add-export" }));
        assert!(msg.contains("repository root or .git-paw"), "{dir}: {msg}");
        mcp.shutdown();
        assert_eq!(snapshot(&sb.repo()), before, "{dir}");
    }
}

// A dot-leading or non-kebab `change` is refused even under a valid spec dir.
#[test]
fn a_non_kebab_change_name_is_refused() {
    let sb = Sandbox::consenting();
    fs::create_dir_all(sb.changes().join(".hidden")).unwrap();
    let before = snapshot(&sb.repo());
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err(
        "write_spec_artifact",
        json!({ "change": ".hidden", "path": "proposal.md", "content": "x" }),
    );
    mcp.shutdown();
    assert!(msg.contains(".hidden"), "{msg}");
    assert_eq!(snapshot(&sb.repo()), before);
}

// --- validate_spec ----------------------------------------------------------

fn write_change(sb: &Sandbox, files: &[(&str, &str)]) {
    let change = sb.changes().join("add-export");
    for (path, content) in files {
        let p = change.join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }
}

// Scenario: A well-formed change is valid.
#[test]
fn a_well_formed_change_is_valid() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    write_change(
        &sb,
        &[
            ("proposal.md", PROPOSAL),
            ("specs/data-export/spec.md", GOOD_DELTA),
        ],
    );
    let mut mcp = sb.mcp(&sb.repo());
    let out = mcp.ok("validate_spec", json!({ "change": "add-export" }));
    mcp.shutdown();
    assert_eq!(out, json!({ "valid": true, "findings": [] }));
}

// Scenario: Structural defects are reported.
#[test]
fn structural_defects_are_reported_on_the_delta_file() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    let delta = "## ADDED Requirements\n\n### Requirement: No scenario\n\nThe system SHALL do a.\n\n### Requirement: Late shall\n\nSome intro.\nThe system SHALL do b.\n\n#### Scenario: B\n\n- **WHEN** x\n- **THEN** y\n";
    write_change(
        &sb,
        &[
            ("proposal.md", PROPOSAL),
            ("specs/data-export/spec.md", delta),
        ],
    );
    let mut mcp = sb.mcp(&sb.repo());
    let out = mcp.ok("validate_spec", json!({ "change": "add-export" }));
    mcp.shutdown();
    assert_eq!(out["valid"], json!(false));
    let findings = out["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 2, "{out}");
    for f in findings {
        assert_eq!(f["file"], json!("specs/data-export/spec.md"), "{out}");
    }
    let messages: Vec<&str> = findings
        .iter()
        .map(|f| f["message"].as_str().unwrap())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("No scenario") && m.contains("Scenario"))
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("Late shall") && m.contains("SHALL"))
    );
}

// Scenario: A change without deltas or proposal is reported.
#[test]
fn a_scaffold_only_change_reports_missing_proposal_and_delta() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    write_change(
        &sb,
        &[(
            ".openspec.yaml",
            "schema: spec-driven\ncreated: 2026-10-19\n",
        )],
    );
    let mut mcp = sb.mcp(&sb.repo());
    let out = mcp.ok("validate_spec", json!({ "change": "add-export" }));
    mcp.shutdown();
    assert_eq!(out["valid"], json!(false));
    let findings = out["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 2, "{out}");
    assert_eq!(findings[0]["file"], json!("proposal.md"));
    assert!(
        findings[1]["message"].as_str().unwrap().contains("delta"),
        "{out}"
    );
}

// Scenario: Validation is available without consent and from a worktree.
#[test]
fn validation_is_available_from_a_worktree_without_consent_and_modifies_nothing() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    let wt = sb.tmp.path().join("wt");
    git(
        &sb.repo(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feat/x",
            wt.to_str().unwrap(),
        ],
    );
    let wt = wt.canonicalize().unwrap();
    let change = wt.join("openspec/changes/add-export");
    fs::create_dir_all(change.join("specs/data-export")).unwrap();
    fs::write(change.join("proposal.md"), PROPOSAL).unwrap();
    fs::write(
        change.join("specs/data-export/spec.md"),
        "## ADDED Requirements\n\n### Requirement: X\n\nno keyword\n",
    )
    .unwrap();
    let before = snapshot(&wt);
    let mut mcp = sb.mcp(&wt);
    assert!(mcp.tool_names().iter().any(|n| n == "validate_spec"));
    let out = mcp.ok("validate_spec", json!({ "change": "add-export" }));
    mcp.shutdown();
    assert_eq!(out["valid"], json!(false));
    assert!(!out["findings"].as_array().unwrap().is_empty());
    assert_eq!(snapshot(&wt), before);
}

// An unknown change is an error naming it.
#[test]
fn validate_spec_names_an_unknown_change() {
    let sb = Sandbox::new(OPENSPEC_OFF);
    let mut mcp = sb.mcp(&sb.repo());
    let msg = mcp.err("validate_spec", json!({ "change": "ghost" }));
    mcp.shutdown();
    assert!(msg.contains("ghost"), "{msg}");
}
