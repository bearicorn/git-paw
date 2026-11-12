//! `git paw delegate` end to end, with a stub agent CLI script in a
//! throwaway repository (`cli-agent-profiles`). No tmux is involved.
//!
//! The stub's behaviour is chosen per test through environment variables the
//! delegate inherits: `STUB_MODE` picks what it prints and how it exits,
//! `STUB_DIR` is where it records its stdin and the processes it starts.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use assert_cmd::Command;

mod helpers;
use helpers::*;

const STUB: &str = r#"#!/bin/sh
cat > "$STUB_DIR/stdin.txt"
echo ran > "$STUB_DIR/ran"
case "$STUB_MODE" in
  ok) echo "MAP: src/config" ;;
  marker) echo "RESULT-MARKER" ;;
  big) head -c 200000 /dev/zero | tr '\0' 'x' ;;
  fail) echo "partial"; exit 3 ;;
  hang)
    sleep 30 &
    echo $! > "$STUB_DIR/child.pid"
    sleep 30
    ;;
esac
exit 0
"#;

struct Fixture {
    tr: TestRepo,
    stub_dir: tempfile::TempDir,
}

impl Fixture {
    fn new(extra_config: &str) -> Self {
        let tr = setup_test_repo();
        let stub_dir = tempfile::TempDir::new().expect("stub dir");
        let stub = stub_dir.path().join("stub-agent");
        fs::write(&stub, STUB).expect("write stub");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).expect("chmod");
        }
        let skill = tr.path().join(".agents/skills/probe-role");
        fs::create_dir_all(&skill).expect("skill dir");
        fs::write(
            skill.join("SKILL.md"),
            "---\nname: probe-role\ndescription: Test role\n---\nPROBE-ROLE\n",
        )
        .expect("write skill");
        let paw = tr.path().join(".git-paw");
        fs::create_dir_all(&paw).expect(".git-paw");
        fs::write(
            paw.join("config.toml"),
            format!(
                "[clis.stub]\ncommand = \"{}\"\nheadless_args = \"--batch\"\n\n\
                 [profiles.explorer]\nkind = \"delegate\"\ndescription = \"Read-only sweep\"\n\
                 cli = \"stub\"\nskill = \"probe-role\"\n{extra_config}",
                stub.display()
            ),
        )
        .expect("write config");
        Self { tr, stub_dir }
    }

    fn root(&self) -> &Path {
        self.tr.path()
    }

    fn stub_file(&self, name: &str) -> PathBuf {
        self.stub_dir.path().join(name)
    }

    fn delegate_in(
        &self,
        dir: &Path,
        mode: &str,
        args: &[&str],
        stdin: Option<&str>,
        env: &[(&str, &str)],
    ) -> std::process::Output {
        let home = tempfile::TempDir::new().expect("home");
        let mut cmd = Command::cargo_bin("git-paw").expect("binary");
        cmd.current_dir(dir)
            .env("HOME", home.path())
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_DATA_HOME")
            .env_remove("GIT_PAW_DELEGATE")
            .env("STUB_MODE", mode)
            .env("STUB_DIR", self.stub_dir.path())
            .arg("delegate")
            .args(args);
        for (k, v) in env {
            cmd.env(k, v);
        }
        if let Some(input) = stdin {
            cmd.write_stdin(input);
        }
        cmd.output().expect("run delegate")
    }

    fn delegate(&self, mode: &str, args: &[&str]) -> std::process::Output {
        self.delegate_in(self.root(), mode, args, None, &[])
    }

    fn ledger(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(self.root().join(".git-paw/logs/delegates/runs.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).expect("ledger line is JSON"))
            .collect()
    }

    fn run_logs(&self) -> Vec<String> {
        fs::read_dir(self.root().join(".git-paw/logs/delegates"))
            .expect("delegates dir")
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
            .map(|e| fs::read_to_string(e.path()).expect("read log"))
            .collect()
    }
}

/// Scenario "Refused from a linked worktree".
#[test]
fn refused_from_a_linked_worktree() {
    let fx = Fixture::new("");
    let wt = fx.stub_dir.path().join("wt");
    let added = std::process::Command::new("git")
        .current_dir(fx.root())
        .args(["worktree", "add", "-q", "-b", "feat-x"])
        .arg(&wt)
        .output()
        .expect("git worktree add");
    assert!(added.status.success(), "{added:?}");
    let out = fx.delegate_in(&wt, "ok", &["explorer", "--task", "x"], None, &[]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("only from the main working tree"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!fx.stub_file("ran").exists(), "no process spawned");
}

/// Scenario "No nested delegates".
#[test]
fn no_nested_delegates() {
    let fx = Fixture::new("");
    let out = fx.delegate_in(
        fx.root(),
        "ok",
        &["explorer", "--task", "x"],
        None,
        &[("GIT_PAW_DELEGATE", "explorer")],
    );
    assert!(!out.status.success());
    assert!(!fx.stub_file("ran").exists(), "no process spawned");
}

/// Scenario "The task may arrive on stdin".
#[test]
fn the_task_may_arrive_on_stdin() {
    let fx = Fixture::new("");
    let out = fx.delegate_in(fx.root(), "ok", &["explorer"], Some("map the broker"), &[]);
    assert!(out.status.success(), "{out:?}");
    let doc = fs::read_to_string(fx.stub_file("stdin.txt")).expect("stub read stdin");
    assert!(doc.trim_end().ends_with("map the broker"), "{doc}");
    assert!(doc.contains("PROBE-ROLE"), "role skill in the document");
}

/// Scenario "A successful run prints its result".
#[test]
fn a_successful_run_prints_its_result() {
    let fx = Fixture::new("");
    let out = fx.delegate("ok", &["explorer", "--task", "x"]);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "MAP: src/config\n");
    let ledger = fx.ledger();
    assert_eq!(ledger.len(), 1);
    assert_eq!(ledger[0]["outcome"], "ok");
    assert_eq!(ledger[0]["profile"], "explorer");
}

/// Scenario "Oversized output is truncated": the printed result is capped
/// and the run log holds the full output.
#[test]
fn oversized_output_is_truncated() {
    let fx = Fixture::new("");
    let out = fx.delegate("big", &["explorer", "--task", "x"]);
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let (body, last) = stdout
        .trim_end()
        .rsplit_once('\n')
        .expect("truncation line");
    assert!(body.len() <= 65_536, "{}", body.len());
    assert!(
        last.contains("truncated") && last.contains(".git-paw/logs/delegates/"),
        "{last}"
    );
    let logs = fx.run_logs();
    assert!(
        logs.iter().any(|l| l.matches('x').count() >= 200_000),
        "the run log holds the full output"
    );
}

/// Scenario "A failing delegate exits non-zero".
#[test]
fn a_failing_delegate_exits_non_zero() {
    let fx = Fixture::new("");
    let out = fx.delegate("fail", &["explorer", "--task", "x"]);
    assert!(!out.status.success());
    let ledger = fx.ledger();
    assert_eq!(ledger[0]["outcome"], "failed");
    assert_eq!(ledger[0]["exit_code"], 3);
}

/// Scenario "The ledger holds metadata only".
#[test]
fn the_ledger_holds_metadata_only() {
    let fx = Fixture::new("");
    let out = fx.delegate("marker", &["explorer", "--task", "TASK-MARKER"]);
    assert!(out.status.success(), "{out:?}");
    let raw =
        fs::read_to_string(fx.root().join(".git-paw/logs/delegates/runs.jsonl")).expect("ledger");
    assert!(
        !raw.contains("TASK-MARKER") && !raw.contains("RESULT-MARKER"),
        "{raw}"
    );
    let line = &fx.ledger()[0];
    assert_eq!(line["profile"], "explorer");
    assert_eq!(line["cli"], "stub");
    assert_eq!(line["outcome"], "ok");
    assert!(fx.run_logs().iter().any(|l| l.contains("RESULT-MARKER")));
}

/// Scenario "Timeout kills the process group": the stub and the child it
/// starts are both gone after about the profile's timeout.
#[test]
fn timeout_kills_the_process_group() {
    let fx = Fixture::new("timeout_seconds = 2\n");
    let started = Instant::now();
    let out = fx.delegate("hang", &["explorer", "--task", "x"]);
    let elapsed = started.elapsed();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("timeout"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(elapsed < Duration::from_secs(20), "{elapsed:?}");
    let pid = fs::read_to_string(fx.stub_file("child.pid")).expect("child pid");
    let alive = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .output()
        .expect("kill -0")
        .status
        .success();
    assert!(!alive, "the child in the delegate's group was killed");
    assert_eq!(fx.ledger()[0]["outcome"], "timeout");
}

/// Scenario "Listing shows delegates only".
#[test]
fn listing_shows_delegates_only() {
    let fx = Fixture::new(
        "\n[clis.plain]\ncommand = \"echo\"\n\n\
         [profiles.audit]\nkind = \"delegate\"\ndescription = \"Audit the spec\"\n\
         cli = \"plain\"\nskill = \"probe-role\"\n\n\
         [profiles.routine]\nkind = \"worker\"\ndescription = \"Routine work\"\ncli = \"stub\"\n",
    );
    let out = fx.delegate("ok", &["--list"]);
    assert!(out.status.success(), "{out:?}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("explorer \u{2014} Read-only sweep"),
        "{stdout}"
    );
    let audit = stdout
        .lines()
        .find(|l| l.starts_with("audit "))
        .unwrap_or_else(|| panic!("no audit row:\n{stdout}"));
    assert!(
        audit.contains("Audit the spec") && audit.contains("no headless form"),
        "{audit}"
    );
    assert!(!stdout.contains("routine"), "{stdout}");
    assert!(!fx.stub_file("ran").exists());

    let empty = setup_test_repo();
    let out = Command::cargo_bin("git-paw")
        .expect("binary")
        .current_dir(empty.path())
        .env("HOME", fx.stub_dir.path())
        .env_remove("GIT_PAW_DELEGATE")
        .args(["delegate", "--list"])
        .output()
        .expect("run");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("No delegate profiles configured"));
}
