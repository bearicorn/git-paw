//! Integration tests for `git paw approvals` and the manual-decision
//! recorder (the `approval-pattern-surfacing` change, §9.1–§9.4).
//!
//! §9.1/§9.2 drive the `git-paw` binary against a prepared per-session
//! manual-decision log (using `--session` to bypass the global session
//! receipt). §9.3/§9.4 exercise the recording path end-to-end through the
//! public library API — recording happens on a background poll thread that a
//! deterministic test cannot drive through tmux, so the library-level
//! integration is the faithful, non-flaky way to assert the opt-out and the
//! first-seen learning emission.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use std::process::Command as StdCommand;
use tempfile::TempDir;

use git_paw::broker::messages::BrokerMessage;
use git_paw::supervisor::manual_approvals::{self, ManualApproval, ManualDecisionRecorder};

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// A minimal committed git repo in a temp dir. These tests exercise
/// `git paw approvals` (which only reads `git::validate_repo` + the JSONL log)
/// and the recorder library — none of them touch tmux, so we deliberately do
/// NOT use the shared `setup_test_repo` helper (its live-session guard would
/// trip under the dogfood supervisor session, and it is irrelevant here).
struct TestRepo {
    _dir: TempDir,
    path: std::path::PathBuf,
}

impl TestRepo {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn setup_repo() -> TestRepo {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("repo");
    std::fs::create_dir(&path).unwrap();
    let git = |args: &[&str]| {
        StdCommand::new("git")
            .current_dir(&path)
            .args(args)
            .output()
            .expect("git");
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.email", "test@test.com"]);
    git(&["config", "user.name", "Test"]);
    std::fs::write(path.join("README.md"), "# test").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "init"]);
    TestRepo { _dir: dir, path }
}

/// Writes a manual-decision JSONL log for `session` under `repo_root` with the
/// given `(agent, pattern, first_seen, timestamp)` entries.
fn write_log(repo_root: &Path, session: &str, entries: &[(&str, &str, bool, &str)]) {
    let path = manual_approvals::log_path(repo_root, session);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut body = String::new();
    for (agent, pattern, first_seen, ts) in entries {
        let appr = ManualApproval {
            timestamp: (*ts).to_string(),
            agent_id: (*agent).to_string(),
            pattern: (*pattern).to_string(),
            first_seen: *first_seen,
        };
        body.push_str(&serde_json::to_string(&appr).unwrap());
        body.push('\n');
    }
    std::fs::write(&path, body).unwrap();
}

// --- §9.1: lists patterns with counts + suggested targets ---

#[test]
fn approvals_lists_three_patterns_with_counts_and_suggestions() {
    let tr = setup_repo();
    write_log(
        tr.path(),
        "paw-test",
        &[
            (
                "feat/a",
                "make integration-test",
                true,
                "2026-05-29T12:00:00Z",
            ),
            (
                "feat/a",
                "make integration-test",
                false,
                "2026-05-29T12:01:00Z",
            ),
            (
                "feat/a",
                "make integration-test",
                false,
                "2026-05-29T12:02:00Z",
            ),
            (
                "feat/a",
                "./scripts/deploy-staging.sh",
                true,
                "2026-05-29T12:03:00Z",
            ),
            (
                "feat/b",
                "podman build -t paw-ci .",
                true,
                "2026-05-29T12:04:00Z",
            ),
            (
                "feat/b",
                "podman build -t paw-ci .",
                false,
                "2026-05-29T12:05:00Z",
            ),
        ],
    );

    cmd()
        .args(["approvals", "--session", "paw-test"])
        .current_dir(tr.path())
        .assert()
        .success()
        // Highest count first; all three patterns present.
        .stdout(
            predicate::str::contains("make integration-test")
                .and(predicate::str::contains("./scripts/deploy-staging.sh"))
                .and(predicate::str::contains("podman build -t paw-ci ."))
                // Promotion hints: a `./` path → project allowlist; generics → preset.
                .and(predicate::str::contains("project allowlist"))
                .and(predicate::str::contains("bundled preset candidate"))
                .and(predicate::str::contains("PATTERN"))
                .and(predicate::str::contains("COUNT"))
                .and(predicate::str::contains("SUGGEST")),
        );
}

// --- §9.2: --json produces the documented shape ---

#[test]
fn approvals_json_has_documented_shape() {
    let tr = setup_repo();
    write_log(
        tr.path(),
        "paw-test",
        &[
            (
                "feat/a",
                "make integration-test",
                true,
                "2026-05-29T12:00:00Z",
            ),
            (
                "feat/a",
                "make integration-test",
                false,
                "2026-05-29T12:30:00Z",
            ),
            ("feat/a", "./deploy.sh", true, "2026-05-29T12:10:00Z"),
        ],
    );

    let out = cmd()
        .args(["approvals", "--session", "paw-test", "--json"])
        .current_dir(tr.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");

    assert_eq!(v["session"], "paw-test");
    let approvals = v["approvals"].as_array().expect("approvals array");
    assert_eq!(approvals.len(), 2);
    // Sorted by descending count → `make integration-test` (2) first.
    let top = &approvals[0];
    assert_eq!(top["pattern"], "make integration-test");
    assert_eq!(top["count"], 2);
    assert_eq!(top["suggested_target"], "bundled-preset");
    assert_eq!(top["first_seen"], "2026-05-29T12:00:00Z");
    assert_eq!(top["last_seen"], "2026-05-29T12:30:00Z");
    // Each entry carries every documented field.
    for entry in approvals {
        for key in [
            "pattern",
            "count",
            "suggested_target",
            "first_seen",
            "last_seen",
        ] {
            assert!(entry.get(key).is_some(), "missing {key} in {entry}");
        }
    }
    // The project-local path is suggested for the project allowlist.
    let deploy = approvals
        .iter()
        .find(|e| e["pattern"] == "./deploy.sh")
        .unwrap();
    assert_eq!(deploy["suggested_target"], "project-allowlist");
}

#[test]
fn approvals_empty_log_text_and_json() {
    let tr = setup_repo();
    // No log written for this session.
    cmd()
        .args(["approvals", "--session", "paw-empty"])
        .current_dir(tr.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no manual approvals recorded"));

    let out = cmd()
        .args(["approvals", "--session", "paw-empty", "--json"])
        .current_dir(tr.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["session"], "paw-empty");
    assert_eq!(v["approvals"].as_array().unwrap().len(), 0);
}

#[test]
fn approvals_limit_caps_output() {
    let tr = setup_repo();
    write_log(
        tr.path(),
        "paw-test",
        &[
            ("a", "cmd-one", true, "2026-05-29T12:00:00Z"),
            ("a", "cmd-one", false, "2026-05-29T12:01:00Z"),
            ("a", "cmd-one", false, "2026-05-29T12:02:00Z"),
            ("a", "cmd-two", true, "2026-05-29T12:03:00Z"),
            ("a", "cmd-two", false, "2026-05-29T12:04:00Z"),
            ("a", "cmd-three", true, "2026-05-29T12:05:00Z"),
        ],
    );

    let out = cmd()
        .args([
            "approvals",
            "--session",
            "paw-test",
            "--limit",
            "1",
            "--json",
        ])
        .current_dir(tr.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let approvals = v["approvals"].as_array().unwrap();
    assert_eq!(approvals.len(), 1, "--limit 1 caps to the top pattern");
    assert_eq!(approvals[0]["pattern"], "cmd-one");
}

// --- §9.3: opt-out produces no log file writes ---

#[test]
fn opt_out_recorder_writes_no_log_file() {
    let tr = setup_repo();
    let session = "paw-optout";
    let log = manual_approvals::log_path(tr.path(), session);

    // `manual_approvals_log = false` → recorder constructed disabled.
    let mut recorder = ManualDecisionRecorder::new(
        log.clone(),
        false, // disabled
        true,  // learnings on — must still be suppressed by the opt-out
        "repo".to_string(),
        Some("claude".to_string()),
    );
    let learning = recorder.record_forwarded("feat/a", "Bash command:\nmake foo\n[y/N]");

    assert!(learning.is_none(), "opt-out must suppress the learning");
    assert!(
        !log.exists(),
        "opt-out must not create the manual-approvals log at {}",
        log.display()
    );
}

// --- §9.4: first-seen pattern emits exactly one permission_pattern learning ---

#[test]
fn first_seen_emits_exactly_one_permission_pattern_learning() {
    let tr = setup_repo();
    let session = "paw-learn";
    let log = manual_approvals::log_path(tr.path(), session);

    let mut recorder = ManualDecisionRecorder::new(
        log.clone(),
        true, // enabled
        true, // learnings on
        "repo".to_string(),
        Some("claude".to_string()),
    );

    let captured = "Bash command:\n./scripts/migrate.sh\nrequires approval [y/N]";
    let first = recorder.record_forwarded("feat/a", captured);
    let second = recorder.record_forwarded("feat/a", captured);
    let third = recorder.record_forwarded("feat/a", captured);

    // Exactly one learning across three forwards of the same pattern.
    let learnings: Vec<&BrokerMessage> = [&first, &second, &third]
        .into_iter()
        .filter_map(|o| o.as_ref())
        .collect();
    assert_eq!(
        learnings.len(),
        1,
        "only the first sighting emits a learning"
    );

    let BrokerMessage::Learning { payload } = learnings[0] else {
        panic!("expected a Learning message");
    };
    assert_eq!(payload.category, "permission_pattern");
    assert_eq!(payload.body["pattern"], "./scripts/migrate.sh");
    assert_eq!(payload.body["suggested_target"], "project-allowlist");

    // All three forwards are still logged (the count signal), with first_seen
    // true exactly once.
    let rows = manual_approvals::aggregate(&log).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].count, 3);
}
