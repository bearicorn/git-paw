//! End-to-end tests for `git paw doctor`.
//!
//! These drive the real binary against throwaway git repositories and observe
//! only its public surface — stdout, the `--json` document, and the exit code.
//! Every run is isolated with its own `HOME`/XDG directories so the developer's
//! real user-level config and session receipts cannot leak into a verdict.
//!
//! The per-check ✓/⚠/✗ decisions are unit-tested over injected state in
//! `src/doctor_tests.rs`; what is pinned here is the end-to-end contract the
//! `preflight-diagnostics` capability specifies: the grouped report, the
//! exit-code rule, the `--json` shape, and that a static run mutates nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as AssertCommand;
use serial_test::serial;
use tempfile::TempDir;

/// The eight groups `git paw doctor` reports under.
const GROUPS: [&str; 8] = [
    "Environment",
    "CLIs",
    "Config",
    "Spec system",
    "Bundled scripts",
    "Broker",
    "Supervisor",
    "Hygiene",
];

/// A throwaway sandbox: a git repository plus the isolated `HOME` every
/// `git paw` child in this suite runs under.
struct Sandbox {
    _dir: TempDir,
    repo: PathBuf,
    home: PathBuf,
}

impl Sandbox {
    /// Creates a sandbox whose repository has one commit.
    fn new() -> Self {
        Self::build(true)
    }

    /// Creates a sandbox whose "repository" directory is not a git repository.
    fn without_git() -> Self {
        Self::build(false)
    }

    fn build(git: bool) -> Self {
        let dir = TempDir::new().expect("create sandbox");
        let repo = dir.path().join("repo");
        let home = dir.path().join("home");
        std::fs::create_dir_all(&repo).expect("create repo dir");
        std::fs::create_dir_all(&home).expect("create home dir");

        if git {
            for args in [
                vec!["init", "-b", "main"],
                vec!["config", "user.email", "test@test.com"],
                vec!["config", "user.name", "Test"],
            ] {
                Command::new("git")
                    .current_dir(&repo)
                    .args(&args)
                    .output()
                    .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
            }
            std::fs::write(repo.join("README.md"), "# test").expect("write README");
            for args in [vec!["add", "."], vec!["commit", "-m", "initial"]] {
                Command::new("git")
                    .current_dir(&repo)
                    .args(&args)
                    .output()
                    .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
            }
        }

        Self {
            _dir: dir,
            repo,
            home,
        }
    }

    /// Builds a `git-paw` invocation rooted in the sandbox, with the user-level
    /// config and session directories redirected into the sandbox's `HOME`.
    fn paw(&self, args: &[&str]) -> AssertCommand {
        let mut cmd = AssertCommand::cargo_bin("git-paw").expect("binary exists");
        cmd.args(args)
            .current_dir(&self.repo)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"));
        cmd
    }

    /// Runs `git paw init` so the repository is fully provisioned.
    fn init(&self) {
        self.paw(&["init"]).assert().success();
    }

    /// Runs doctor and returns `(exit code, stdout)`.
    fn doctor(&self, args: &[&str]) -> (i32, String) {
        let mut full = vec!["doctor"];
        full.extend_from_slice(args);
        let out = self.paw(&full).output().expect("run doctor");
        (
            out.status.code().expect("doctor exited via a signal"),
            String::from_utf8_lossy(&out.stdout).to_string(),
        )
    }

    /// Runs `doctor --json` and returns `(exit code, parsed document)`.
    fn doctor_json(&self) -> (i32, serde_json::Value) {
        self.doctor_json_with(&[], &[])
    }

    /// Runs `doctor --json` with extra flags and environment, returning
    /// `(exit code, parsed document)`.
    fn doctor_json_with(&self, args: &[&str], env: &[(&str, &str)]) -> (i32, serde_json::Value) {
        let mut full = vec!["doctor", "--json"];
        full.extend_from_slice(args);
        let mut cmd = self.paw(&full);
        for (key, value) in env {
            cmd.env(key, value);
        }
        let out = cmd.output().expect("run doctor");
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let parsed = serde_json::from_str(&stdout).unwrap_or_else(|e| {
            panic!("--json stdout should be one JSON document ({e}):\n{stdout}")
        });
        (
            out.status.code().expect("doctor exited via a signal"),
            parsed,
        )
    }
}

/// Returns whether a `tmux` binary is callable, so the live arm's assertion
/// can match what the harness will actually do.
fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Returns the single Live-smoke check, or `None` when the group is absent.
fn live_check(document: &serde_json::Value) -> Option<&serde_json::Value> {
    checks(document).iter().find(|c| c["group"] == "Live smoke")
}

/// Snapshots every file under `root` as `path -> (len, modified)`, so a later
/// comparison catches a creation, a deletion, and an in-place rewrite alike.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, (u64, std::time::SystemTime)> {
    let mut out = BTreeMap::new();
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
            } else {
                let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                out.insert(path, (meta.len(), modified));
            }
        }
    }
    out
}

/// Returns every check entry in a parsed `--json` document.
fn checks(document: &serde_json::Value) -> &Vec<serde_json::Value> {
    document["checks"]
        .as_array()
        .unwrap_or_else(|| panic!("the document should carry a checks array: {document}"))
}

// ---------------------------------------------------------------------------
// The grouped report
// ---------------------------------------------------------------------------

#[test]
fn doctor_prints_a_grouped_report_with_a_status_on_every_check() {
    let sandbox = Sandbox::new();
    sandbox.init();
    let (_, stdout) = sandbox.doctor(&[]);

    for group in GROUPS {
        assert!(
            stdout.contains(group),
            "the report should carry the '{group}' group; got:\n{stdout}"
        );
    }

    // Every non-heading, non-summary body line opens with a status glyph.
    let body: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("  ") && !l.trim_start().starts_with('\u{21b3}'))
        .collect();
    assert!(!body.is_empty(), "the report should have checks:\n{stdout}");
    for line in body {
        let glyph = line.trim_start().chars().next().unwrap_or(' ');
        assert!(
            ['\u{2713}', '\u{26a0}', '\u{2717}'].contains(&glyph),
            "every check line should open with ✓/⚠/✗; got: {line:?}"
        );
    }
}

#[test]
fn every_non_passing_check_prints_a_remedy() {
    // A bare repository has no `.git-paw/` at all, so several checks are
    // non-✓ — each one must still say what to do about it.
    let sandbox = Sandbox::new();
    let (_, document) = sandbox.doctor_json();

    let non_passing: Vec<&serde_json::Value> = checks(&document)
        .iter()
        .filter(|c| c["status"] != "ok")
        .collect();
    assert!(
        !non_passing.is_empty(),
        "an unprovisioned repository should produce non-✓ checks: {document}"
    );
    for check in non_passing {
        let remedy = check["remedy"].as_str().unwrap_or("");
        assert!(
            !remedy.trim().is_empty(),
            "every ⚠/✗ check needs a remedy; got: {check}"
        );
    }
}

#[test]
fn doctor_outside_a_git_repository_fails_with_a_remedy() {
    let sandbox = Sandbox::without_git();
    let (code, document) = sandbox.doctor_json();

    let repo_check = checks(&document)
        .iter()
        .find(|c| c["group"] == "Environment" && c["name"] == "git repository")
        .unwrap_or_else(|| panic!("the Environment group should report on the repository"));
    assert_eq!(repo_check["status"], "fail", "check: {repo_check}");
    assert!(
        !repo_check["remedy"].as_str().unwrap_or("").is_empty(),
        "the ✗ needs a remedy; check: {repo_check}"
    );
    assert_ne!(code, 0, "a ✗ means a non-zero exit");
}

// ---------------------------------------------------------------------------
// Exit-code contract
// ---------------------------------------------------------------------------

#[test]
fn exit_code_is_non_zero_exactly_when_a_check_fails() {
    // Checked in both repository states so the rule is exercised from both
    // sides on any machine: unprovisioned (bundled scripts missing → ✗) and
    // provisioned (those ✗ resolved).
    for provisioned in [false, true] {
        let sandbox = Sandbox::new();
        if provisioned {
            sandbox.init();
        }
        let (code, document) = sandbox.doctor_json();
        let any_failed = checks(&document).iter().any(|c| c["status"] == "fail");

        assert_eq!(
            code != 0,
            any_failed,
            "exit code must track the presence of a ✗ (provisioned: {provisioned}); \
             document: {document}"
        );
        assert_eq!(
            document["status"],
            if any_failed {
                "fail"
            } else if checks(&document).iter().any(|c| c["status"] == "warn") {
                "warn"
            } else {
                "ok"
            },
            "the document's overall status is the worst check"
        );
    }
}

#[test]
fn an_unprovisioned_repository_fails_on_the_missing_bundled_scripts() {
    let sandbox = Sandbox::new();
    let (code, document) = sandbox.doctor_json();

    let sweep = checks(&document)
        .iter()
        .find(|c| c["group"] == "Bundled scripts" && c["name"] == "sweep.sh")
        .unwrap_or_else(|| panic!("sweep.sh should be checked: {document}"));
    assert_eq!(sweep["status"], "fail");
    assert!(
        sweep["remedy"]
            .as_str()
            .unwrap_or("")
            .contains("git paw init"),
        "the remedy should point at `git paw init`; check: {sweep}"
    );
    assert_ne!(code, 0);
}

#[test]
fn provisioning_resolves_the_bundled_script_failures_and_warnings_alone_exit_zero() {
    let sandbox = Sandbox::new();
    sandbox.init();
    let (code, document) = sandbox.doctor_json();

    let scripts: Vec<&serde_json::Value> = checks(&document)
        .iter()
        .filter(|c| c["group"] == "Bundled scripts")
        .collect();
    assert!(!scripts.is_empty(), "document: {document}");
    for check in &scripts {
        assert_ne!(
            check["status"], "fail",
            "`git paw init` should resolve every bundled-script ✗; check: {check}"
        );
    }

    // Whatever warnings this machine produces, a ⚠ alone never fails the
    // process — only a ✗ does.
    if !checks(&document).iter().any(|c| c["status"] == "fail") {
        assert_eq!(code, 0, "⚠-only reports exit 0; document: {document}");
    }
}

// ---------------------------------------------------------------------------
// `--json`
// ---------------------------------------------------------------------------

#[test]
fn json_document_carries_the_required_fields_on_every_check() {
    let sandbox = Sandbox::new();
    sandbox.init();
    let (_, document) = sandbox.doctor_json();

    assert!(
        ["ok", "warn", "fail"].contains(&document["status"].as_str().unwrap_or("")),
        "the document needs an overall status; got: {document}"
    );

    let entries = checks(&document);
    assert!(!entries.is_empty(), "document: {document}");
    for entry in entries {
        for field in ["group", "name", "status", "detail", "remedy"] {
            assert!(
                entry.get(field).is_some(),
                "every check needs a '{field}' field; entry: {entry}"
            );
        }
        assert!(
            GROUPS.contains(&entry["group"].as_str().unwrap_or("")),
            "unexpected group in {entry}"
        );
        if entry["status"] != "ok" {
            assert!(
                !entry["remedy"].as_str().unwrap_or("").is_empty(),
                "a non-✓ check needs a remedy string; entry: {entry}"
            );
        }
    }
}

#[test]
fn json_mode_suppresses_the_human_report_and_matches_its_exit_code() {
    let sandbox = Sandbox::new();
    sandbox.init();

    let (human_code, human) = sandbox.doctor(&[]);
    let (json_code, json_stdout) = sandbox.doctor(&["--json"]);

    assert_eq!(
        human_code, json_code,
        "both modes share one exit-code contract"
    );
    assert!(
        !json_stdout.contains('\u{2713}') && !json_stdout.contains('\u{21b3}'),
        "--json should suppress the human rendering; got:\n{json_stdout}"
    );
    assert!(
        human.contains('\u{2713}'),
        "the human report should use glyphs; got:\n{human}"
    );
}

// ---------------------------------------------------------------------------
// Read-only
// ---------------------------------------------------------------------------

#[test]
fn doctor_does_not_mutate_the_repository() {
    let sandbox = Sandbox::new();
    sandbox.init();

    let before = snapshot(&sandbox.repo);
    assert!(
        before.keys().any(|p| p.ends_with("config.toml")),
        "the snapshot should cover .git-paw/ state"
    );

    sandbox.doctor(&[]);
    sandbox.doctor(&["--json"]);

    let after = snapshot(&sandbox.repo);
    assert_eq!(
        before, after,
        "doctor must not create, modify, or delete any file under the repository"
    );
}

// ---------------------------------------------------------------------------
// `--live` smoke arm
// ---------------------------------------------------------------------------

#[test]
fn a_static_run_carries_no_live_smoke_group() {
    let sandbox = Sandbox::new();
    sandbox.init();
    let (_, document) = sandbox.doctor_json();
    assert!(
        live_check(&document).is_none(),
        "the Live-smoke group must only appear under --live; got: {document}"
    );
}

#[test]
#[serial]
fn live_folds_the_lifecycle_verdict_in_and_keeps_json_parseable() {
    // The harness isolates its own tmux socket, broker port, HOME and repo, so
    // this never touches the caller's session. `doctor_json_with` already
    // asserts stdout parsed as one document — the harness's per-step progress
    // output must not leak into it.
    let sandbox = Sandbox::new();
    sandbox.init();
    let (code, document) = sandbox.doctor_json_with(&["--live"], &[]);

    let check = live_check(&document)
        .unwrap_or_else(|| panic!("--live should add a Live-smoke check: {document}"));

    if tmux_available() {
        assert_eq!(
            check["status"], "ok",
            "the lifecycle should complete on a healthy build; check: {check}"
        );
        assert_eq!(code, 0, "document: {document}");
    } else {
        // A run that could not start is ⚠, not ✗ — the Environment group
        // already reports the missing tmux as the hard failure.
        assert_eq!(check["status"], "warn", "check: {check}");
        assert!(!check["remedy"].as_str().unwrap_or("").is_empty());
    }
}

#[test]
#[serial]
fn a_failing_lifecycle_step_is_a_hard_failure_naming_the_step() {
    if !tmux_available() {
        eprintln!("skipping: tmux is not available, so the lifecycle cannot run");
        return;
    }
    let sandbox = Sandbox::new();
    sandbox.init();
    // The harness's forced-failure hook aborts at a named step, which is how
    // the failure path is exercised without breaking a real build.
    let (code, document) =
        sandbox.doctor_json_with(&["--live"], &[("GIT_PAW_SELFTEST_FORCE_FAIL", "pick-port")]);

    let check = live_check(&document)
        .unwrap_or_else(|| panic!("--live should add a Live-smoke check: {document}"));
    assert_eq!(check["status"], "fail", "check: {check}");
    assert!(
        check["detail"].as_str().unwrap_or("").contains("pick-port"),
        "the detail should name the failing step; check: {check}"
    );
    assert!(
        !check["remedy"].as_str().unwrap_or("").is_empty(),
        "a ✗ needs a remedy; check: {check}"
    );
    assert_ne!(code, 0, "a failing lifecycle exits non-zero");
}

// ---------------------------------------------------------------------------
// Diagnose-only
// ---------------------------------------------------------------------------

#[test]
fn doctor_help_does_not_advertise_a_repair_mode() {
    let out = AssertCommand::cargo_bin("git-paw")
        .expect("binary exists")
        .args(["doctor", "--help"])
        .output()
        .expect("run doctor --help");
    let help = String::from_utf8_lossy(&out.stdout).to_string();

    for forbidden in ["--fix", "--repair"] {
        assert!(
            !help.contains(forbidden),
            "doctor is diagnose-only in v0.13.0 and must not advertise {forbidden}; got:\n{help}"
        );
    }
}

#[test]
fn doctor_is_listed_in_the_root_help() {
    let out = AssertCommand::cargo_bin("git-paw")
        .expect("binary exists")
        .arg("--help")
        .output()
        .expect("run --help");
    let help = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        help.contains("doctor"),
        "the root help should surface doctor as the diagnostic entry point; got:\n{help}"
    );
}
