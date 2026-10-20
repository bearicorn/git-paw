//! End-to-end tests for `git paw setup-cli`, through the real binary.
//!
//! `HOME` is redirected into the sandbox and stdin is never a terminal, so a
//! run can only ever touch the throwaway repository, and the confirmation
//! gate's no-terminal branch is the default rather than a special case.
//!
//! The shipped `claude` fact row is verified (see the change's fact gate), so
//! the writing paths are exercised here through the binary itself. The pure
//! planning, undo and apply layers have their own unit tests in
//! `src/cli_setup/`.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as AssertCommand;
use tempfile::TempDir;

/// A throwaway repository with an isolated `HOME`.
struct Sandbox {
    _dir: TempDir,
    repo: PathBuf,
    home: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let dir = TempDir::new().expect("create sandbox");
        let repo = dir.path().join("repo");
        let home = dir.path().join("home");
        std::fs::create_dir_all(&repo).expect("create repo");
        std::fs::create_dir_all(&home).expect("create home");
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@test.invalid"],
            vec!["config", "user.name", "Test"],
            // A developer's global ignore file must not decide whether the
            // exclusion line is planned.
            vec!["config", "core.excludesFile", "/dev/null"],
            vec!["commit", "-q", "--allow-empty", "-m", "initial"],
        ] {
            let out = Command::new("git")
                .current_dir(&repo)
                .args(&args)
                .output()
                .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
            assert!(out.status.success(), "git {args:?} failed");
        }
        Self {
            _dir: dir,
            repo,
            home,
        }
    }

    /// Runs `git-paw` in the sandbox with stdin closed, so it is never a
    /// terminal.
    fn paw(&self, args: &[&str]) -> AssertCommand {
        let mut cmd = AssertCommand::cargo_bin("git-paw").expect("binary exists");
        cmd.args(args)
            .current_dir(&self.repo)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .write_stdin("");
        cmd
    }

    /// Runs `setup-cli` and returns `(exit code, stdout, stderr)`.
    fn setup(&self, args: &[&str]) -> (i32, String, String) {
        let mut full = vec!["setup-cli"];
        full.extend_from_slice(args);
        let out = self.paw(&full).output().expect("run setup-cli");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }

    fn settings(&self) -> PathBuf {
        self.repo.join(".claude/settings.local.json")
    }

    fn record(&self) -> PathBuf {
        self.repo.join(".git/git-paw/cli-setup.json")
    }

    fn write_settings(&self, body: &str) {
        let path = self.settings();
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, body).expect("write settings");
    }

    fn git_status(&self) -> String {
        let out = Command::new("git")
            .current_dir(&self.repo)
            .args(["status", "--porcelain"])
            .output()
            .expect("git status");
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    /// Asserts that neither the settings file nor the record exists.
    fn assert_wrote_nothing(&self) {
        assert!(
            !self.settings().exists(),
            "a refused or unattempted run must create no settings file"
        );
        assert!(
            !self.record().exists(),
            "a refused or unattempted run must create no setup record"
        );
    }
}

/// Every byte under a directory, for a "nothing changed" comparison.
fn byte_snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = entry.metadata() else { continue };
            if meta.is_dir() {
                stack.push(path);
            } else if let Ok(bytes) = std::fs::read(&path) {
                out.insert(path, bytes);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Usage and identity (cli-setup: "The setup-cli command applies named setup
// items for one agent CLI")
// ---------------------------------------------------------------------------

#[test]
fn an_item_flag_is_required() {
    let sandbox = Sandbox::new();
    let (code, _out, err) = sandbox.setup(&["claude"]);
    assert_eq!(code, 2, "{err}");
    for flag in ["--allowlist", "--trust", "--hooks"] {
        assert!(err.contains(flag), "usage error should name {flag}: {err}");
    }
    sandbox.assert_wrote_nothing();
}

#[test]
fn an_unknown_cli_is_rejected_naming_it() {
    let sandbox = Sandbox::new();
    let (code, _out, err) = sandbox.setup(&["no-such-cli", "--allowlist"]);
    assert_ne!(code, 0);
    assert!(err.contains("no-such-cli"), "{err}");
    sandbox.assert_wrote_nothing();
}

#[test]
fn running_from_a_linked_worktree_is_refused() {
    let sandbox = Sandbox::new();
    let wt = sandbox.repo.join(".git-paw/worktrees/feat-x");
    std::fs::create_dir_all(wt.parent().expect("parent")).expect("mkdir");
    let out = Command::new("git")
        .current_dir(&sandbox.repo)
        .args([
            "worktree",
            "add",
            "-q",
            &wt.to_string_lossy(),
            "-b",
            "feat/x",
        ])
        .output()
        .expect("git worktree add");
    assert!(out.status.success());

    let mut cmd = AssertCommand::cargo_bin("git-paw").expect("binary exists");
    let out = cmd
        .args(["setup-cli", "claude", "--allowlist", "--yes"])
        .current_dir(&wt)
        .env("HOME", &sandbox.home)
        .write_stdin("")
        .output()
        .expect("run");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0));
    assert!(err.contains("main checkout"), "{err}");
    assert!(
        !wt.join(".claude").exists() && !sandbox.settings().exists(),
        "nothing is written from a worktree"
    );
}

// ---------------------------------------------------------------------------
// Unsupported items (cli-setup: "Setup items are declared per CLI by a
// compiled fact table")
// ---------------------------------------------------------------------------

#[test]
fn trust_is_not_supported_for_claude() {
    let sandbox = Sandbox::new();
    let (code, out, err) = sandbox.setup(&["claude", "--trust", "--yes"]);
    assert_eq!(code, 2, "{err}");
    let shown = format!("{out}{err}");
    assert!(shown.contains("trust"), "{shown}");
    assert!(shown.contains("not supported"), "{shown}");
    sandbox.assert_wrote_nothing();
}

#[test]
fn a_mixed_request_writes_nothing() {
    let sandbox = Sandbox::new();
    let (code, out, err) = sandbox.setup(&["claude", "--allowlist", "--hooks", "--yes"]);
    assert_eq!(code, 2, "{err}");
    let shown = format!("{out}{err}");
    assert!(shown.contains("hooks"), "{shown}");
    assert!(shown.contains("not supported"), "{shown}");
    sandbox.assert_wrote_nothing();
}

#[test]
fn a_cli_without_a_fact_row_supports_no_item() {
    let sandbox = Sandbox::new();
    let config_dir = sandbox.repo.join(".git-paw");
    std::fs::create_dir_all(&config_dir).expect("mkdir");
    std::fs::write(
        config_dir.join("config.toml"),
        "[clis.my-agent]\ncommand = \"my-agent\"\n",
    )
    .expect("write config");

    let (code, out, err) = sandbox.setup(&["my-agent", "--allowlist", "--yes"]);
    assert_eq!(code, 2, "{err}");
    let shown = format!("{out}{err}");
    assert!(shown.contains("my-agent"), "{shown}");
    assert!(shown.contains("not supported"), "{shown}");
    sandbox.assert_wrote_nothing();
}

// ---------------------------------------------------------------------------
// The plan (cli-setup: "The plan is printed before anything is written",
// "The allowlist target is the main checkout's project-local settings file")
// ---------------------------------------------------------------------------

#[test]
fn the_plan_names_the_main_checkout_file_and_its_entries() {
    let sandbox = Sandbox::new();
    let (code, out, err) = sandbox.setup(&["claude", "--allowlist", "--dry-run"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains(&sandbox.settings().display().to_string()),
        "the plan names the absolute target path: {out}"
    );
    assert!(out.contains("Bash(.git-paw/scripts/broker.sh *)"), "{out}");
    assert!(out.contains("Edit(**/.git-paw/scripts/**)"), "{out}");
    assert!(
        out.contains(&sandbox.record().display().to_string()),
        "the plan names the record file: {out}"
    );
}

#[test]
fn the_plan_names_no_worktree_path() {
    let sandbox = Sandbox::new();
    let wt = sandbox.repo.join(".git-paw/worktrees/feat-x");
    std::fs::create_dir_all(wt.parent().expect("parent")).expect("mkdir");
    Command::new("git")
        .current_dir(&sandbox.repo)
        .args([
            "worktree",
            "add",
            "-q",
            &wt.to_string_lossy(),
            "-b",
            "feat/x",
        ])
        .output()
        .expect("git worktree add");

    let (_code, out, _err) = sandbox.setup(&["claude", "--allowlist", "--dry-run"]);
    assert!(
        !out.contains(".git-paw/worktrees"),
        "a worktree is never a settings target: {out}"
    );
}

#[test]
fn the_configured_settings_path_is_never_a_target() {
    let sandbox = Sandbox::new();
    let config_dir = sandbox.repo.join(".git-paw");
    std::fs::create_dir_all(&config_dir).expect("mkdir");
    std::fs::write(
        config_dir.join("config.toml"),
        "[clis.claude]\ncommand = \"claude\"\nsettings_path = \"~/.claude/settings.json\"\n",
    )
    .expect("write config");

    let (_code, out, _err) = sandbox.setup(&["claude", "--allowlist", "--dry-run"]);
    assert!(
        !out.contains("settings.json"),
        "the configured settings_path is never written: {out}"
    );
    assert!(
        !out.contains(&sandbox.home.display().to_string()),
        "no path under the home directory is a target: {out}"
    );
}

#[test]
fn the_plan_never_echoes_an_existing_secret() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(r#"{"env":{"API_TOKEN":"s3cret"},"permissions":{}}"#);
    let (code, out, err) = sandbox.setup(&["claude", "--allowlist", "--dry-run"]);
    assert_eq!(code, 0, "{err}");
    assert!(!out.contains("s3cret"), "stdout leaked the secret: {out}");
    assert!(!err.contains("s3cret"), "stderr leaked the secret: {err}");
}

#[test]
fn a_dry_run_writes_nothing_even_with_yes() {
    let sandbox = Sandbox::new();
    let (code, _out, err) = sandbox.setup(&["claude", "--allowlist", "--dry-run", "--yes"]);
    assert_eq!(code, 0, "{err}");
    sandbox.assert_wrote_nothing();
}

// ---------------------------------------------------------------------------
// The confirmation gate (cli-setup: "Writing requires confirmation on a
// terminal or an explicit --yes")
// ---------------------------------------------------------------------------

#[test]
fn no_terminal_and_no_yes_writes_nothing_and_says_what_to_do() {
    let sandbox = Sandbox::new();
    let (code, _out, err) = sandbox.setup(&["claude", "--allowlist"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--yes"), "stderr must name --yes: {err}");
    assert_ne!(
        err.trim(),
        "error: Cancelled.",
        "a bare cancellation leaves the user with nothing to act on"
    );
    assert!(!err.contains("Cancelled."), "{err}");
    sandbox.assert_wrote_nothing();
}

#[test]
fn undo_is_gated_the_same_way() {
    let sandbox = Sandbox::new();
    sandbox
        .paw(&["setup-cli", "claude", "--allowlist", "--yes"])
        .assert()
        .success();
    let before = std::fs::read_to_string(sandbox.settings()).expect("settings");

    let (code, _out, err) = sandbox.setup(&["claude", "--undo"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("--yes"), "{err}");
    assert_eq!(
        std::fs::read_to_string(sandbox.settings()).expect("settings"),
        before,
        "every recorded entry stays in place"
    );
}

// ---------------------------------------------------------------------------
// Refusals (cli-setup: "Unsafe targets are refused")
// ---------------------------------------------------------------------------

#[test]
fn a_tracked_target_is_refused() {
    let sandbox = Sandbox::new();
    sandbox.write_settings("{}\n");
    Command::new("git")
        .current_dir(&sandbox.repo)
        .args(["add", "-f", ".claude/settings.local.json"])
        .output()
        .expect("git add");

    let (code, _out, err) = sandbox.setup(&["claude", "--allowlist", "--yes"]);
    assert_ne!(code, 0);
    assert!(err.contains("settings.local.json"), "{err}");
    assert_eq!(
        std::fs::read_to_string(sandbox.settings()).expect("settings"),
        "{}\n",
        "the tracked file is untouched"
    );
    assert!(!sandbox.record().exists());
}

#[cfg(unix)]
#[test]
fn a_symlinked_settings_directory_is_refused() {
    let sandbox = Sandbox::new();
    let elsewhere = sandbox.repo.parent().expect("parent").join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("mkdir");
    std::os::unix::fs::symlink(&elsewhere, sandbox.repo.join(".claude")).expect("symlink");

    let (code, _out, err) = sandbox.setup(&["claude", "--allowlist", "--yes"]);
    assert_ne!(code, 0);
    assert!(err.contains(".claude"), "{err}");
    assert!(
        !elsewhere.join("settings.local.json").exists(),
        "nothing is written through the link"
    );
    assert!(!sandbox.record().exists());
}

#[test]
fn malformed_json_is_refused_naming_the_file() {
    let sandbox = Sandbox::new();
    sandbox.write_settings("{ not json");
    let (code, _out, err) = sandbox.setup(&["claude", "--allowlist", "--yes"]);
    assert_ne!(code, 0);
    assert!(err.contains("settings.local.json"), "{err}");
    assert_eq!(
        std::fs::read_to_string(sandbox.settings()).expect("settings"),
        "{ not json"
    );
    assert!(!sandbox.record().exists());
}

#[test]
fn a_wrong_typed_key_is_refused_naming_file_and_key() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(r#"{"permissions":{"allow":"everything"}}"#);
    let (code, _out, err) = sandbox.setup(&["claude", "--allowlist", "--yes"]);
    assert_ne!(code, 0);
    assert!(err.contains("settings.local.json"), "{err}");
    assert!(err.contains("permissions.allow"), "{err}");
    assert!(!sandbox.record().exists());
}

// ---------------------------------------------------------------------------
// Writing, recording and undo (cli-setup: "Writes add only the planned
// entries", "Every write is recorded", "Undo removes exactly what git-paw
// added")
// ---------------------------------------------------------------------------

#[test]
fn yes_applies_the_plan_records_it_and_leaves_git_status_clean() {
    let sandbox = Sandbox::new();
    let (code, out, err) = sandbox.setup(&["claude", "--allowlist", "--yes"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Applied"), "{out}");

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(sandbox.settings()).expect("settings"))
            .expect("valid json");
    let allow = written["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .clone();
    assert_eq!(allow.len(), 12, "the twelve grant-table entries");
    assert!(
        allow
            .iter()
            .any(|v| v == "Bash(.git-paw/scripts/sweep.sh events *)"),
        "the read-only events grant must be written; got {allow:?}"
    );
    assert!(
        written["permissions"]["deny"]
            .as_array()
            .expect("deny")
            .iter()
            .any(|v| v == "Edit(**/.git-paw/scripts/**)")
    );

    let record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(sandbox.record()).expect("record"))
            .expect("valid json");
    assert!(record["schema_version"].is_number());
    assert!(!record["entries"].as_array().expect("entries").is_empty());

    assert_eq!(
        sandbox.git_status(),
        "",
        "the created file is excluded, so the working tree stays clean"
    );
}

#[test]
fn unrelated_settings_keep_their_value_and_order() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(r#"{"model":"x","permissions":{"deny":["y"],"allow":["z"]}}"#);
    sandbox
        .paw(&["setup-cli", "claude", "--allowlist", "--yes"])
        .assert()
        .success();

    let text = std::fs::read_to_string(sandbox.settings()).expect("settings");
    let written: serde_json::Value = serde_json::from_str(&text).expect("valid json");
    assert_eq!(written["model"], "x");
    assert_eq!(written["permissions"]["deny"][0], "y");
    assert_eq!(written["permissions"]["allow"][0], "z");
    assert_eq!(
        written["permissions"]["allow"][1], "Bash(.git-paw/scripts/broker.sh *)",
        "git-paw's entries follow the user's"
    );
    assert!(
        text.find("\"model\"") < text.find("\"permissions\""),
        "existing key order is preserved:\n{text}"
    );
    // The assertion above is not discriminating on its own - `model` sorts
    // before `permissions` anyway. The nested pair does discriminate: the
    // fixture writes `deny` before `allow`, the reverse of sorted order, so
    // this fails the moment serde_json's preserve_order feature is dropped.
    assert!(
        text.find("\"deny\"") < text.find("\"allow\""),
        "keys keep insertion order, not alphabetical order:\n{text}"
    );
}

#[test]
fn nothing_to_do_exits_zero_without_writing() {
    let sandbox = Sandbox::new();
    sandbox
        .paw(&["setup-cli", "claude", "--allowlist", "--yes"])
        .assert()
        .success();
    let before = byte_snapshot(&sandbox.repo);

    let (code, out, err) = sandbox.setup(&["claude", "--allowlist"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Nothing to do"), "{out}");
    assert_eq!(byte_snapshot(&sandbox.repo), before, "nothing was modified");
}

#[test]
fn undo_removes_what_was_added_and_keeps_what_the_user_had() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(r#"{"permissions":{"allow":["Bash(make test *)"]}}"#);
    sandbox
        .paw(&["setup-cli", "claude", "--allowlist", "--yes"])
        .assert()
        .success();

    let (code, out, err) = sandbox.setup(&["claude", "--undo", "--yes"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Removed"), "{out}");

    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(sandbox.settings()).expect("settings"))
            .expect("valid json");
    let allow: Vec<&str> = left["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert_eq!(allow, vec!["Bash(make test *)"]);
}

#[test]
fn undo_deletes_a_file_it_created_and_leaves_nothing_behind() {
    let sandbox = Sandbox::new();
    sandbox
        .paw(&["setup-cli", "claude", "--allowlist", "--yes"])
        .assert()
        .success();
    sandbox
        .paw(&["setup-cli", "claude", "--undo", "--yes"])
        .assert()
        .success();

    assert!(!sandbox.settings().exists(), "the created file is gone");
    assert!(
        !sandbox.repo.join(".claude").exists(),
        "and the directory git-paw created with it"
    );
    assert_eq!(sandbox.git_status(), "");
}

#[test]
fn an_entry_the_user_already_had_is_never_claimed_by_undo() {
    let sandbox = Sandbox::new();
    sandbox.write_settings(r#"{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"]}}"#);
    sandbox
        .paw(&["setup-cli", "claude", "--allowlist", "--yes"])
        .assert()
        .success();
    sandbox
        .paw(&["setup-cli", "claude", "--undo", "--yes"])
        .assert()
        .success();

    let left: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(sandbox.settings()).expect("settings"))
            .expect("valid json");
    let allow: Vec<&str> = left["permissions"]["allow"]
        .as_array()
        .expect("allow")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert_eq!(
        allow,
        vec!["Bash(.git-paw/scripts/broker.sh *)"],
        "the entry the user wrote themselves survives undo"
    );
}

// ---------------------------------------------------------------------------
// Non-invocation guards (approval-command-safety: "No permission grants are
// seeded into agent CLI settings files")
// ---------------------------------------------------------------------------

#[test]
fn init_creates_no_settings_directory_and_no_record() {
    let sandbox = Sandbox::new();
    sandbox.paw(&["init"]).assert().success();
    assert!(
        !sandbox.repo.join(".claude").exists(),
        "init must create no vendor settings directory"
    );
    assert!(
        !sandbox.record().exists(),
        "init must write no setup record"
    );
}

#[test]
fn adding_a_worktree_does_not_extend_an_earlier_setup() {
    let sandbox = Sandbox::new();
    sandbox.paw(&["init"]).assert().success();
    sandbox
        .paw(&["setup-cli", "claude", "--allowlist", "--yes"])
        .assert()
        .success();

    let settings_before = std::fs::read(sandbox.settings()).expect("settings");
    let record_before = std::fs::read(sandbox.record()).expect("record");

    // `add` needs a live session, so it fails here — the point is that even
    // the attempt writes no settings anywhere.
    let wt = sandbox.repo.join(".git-paw/worktrees/feat-x");
    std::fs::create_dir_all(wt.parent().expect("parent")).expect("mkdir");
    Command::new("git")
        .current_dir(&sandbox.repo)
        .args([
            "worktree",
            "add",
            "-q",
            &wt.to_string_lossy(),
            "-b",
            "feat/x",
        ])
        .output()
        .expect("git worktree add");

    assert!(
        !wt.join(".claude").exists(),
        "no settings directory is created in a worktree"
    );
    assert_eq!(
        std::fs::read(sandbox.settings()).expect("settings"),
        settings_before,
        "the main checkout's file is byte-identical"
    );
    assert_eq!(
        std::fs::read(sandbox.record()).expect("record"),
        record_before,
        "the record is byte-identical"
    );
}

#[test]
fn read_only_commands_leave_the_record_and_its_files_byte_identical() {
    let sandbox = Sandbox::new();
    sandbox.paw(&["init"]).assert().success();
    sandbox
        .paw(&["setup-cli", "claude", "--allowlist", "--yes"])
        .assert()
        .success();

    let before = byte_snapshot(&sandbox.repo);
    for args in [
        vec!["status"],
        vec!["list-clis"],
        vec!["doctor"],
        vec!["setup-cli", "claude", "--allowlist", "--dry-run"],
    ] {
        let _ = sandbox.paw(&args).output();
    }
    assert_eq!(
        byte_snapshot(&sandbox.repo),
        before,
        "no read-only command may touch the setup record or the files it names"
    );
}

// The MCP surface is guarded where it is actually defined, over the live
// tool registry and its frozen allowlist:
// `mcp::server::tests::no_tool_applies_cli_setup_or_writes_cli_settings`.
// A `git paw mcp --help` grep was tried here first and dropped: `--help`
// enumerates no tools, so it would stay green if a setup tool were added.
