//! E2E observable: the deprecated auto-approval config tables warn once.
//!
//! Spec scenarios "An old configuration with both tables loads with one
//! warning" and "A configuration without the tables produces no warning"
//! (`supervisor-config`, requirement *Deprecated auto-approval tables are
//! parsed but have no effect*).
//!
//! `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]` still
//! parse so an older configuration loads and round-trips, but nothing
//! consumes them. A user upgrading needs to be told that, once — not on
//! every config load, and not at all if they never wrote the tables.
//!
//! Driven through the real binary rather than the loader, because "once per
//! process" is a property of the process: several subcommands load the same
//! configuration more than once, and the unit test for
//! `deprecated_table_warning` cannot see that.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Initialises a git repo with `.git-paw/config.toml` holding `config`.
fn repo_with_config(config: &str) -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .current_dir(tmp.path())
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(tmp.path().join("README.md"), "x").expect("readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);

    let paw = tmp.path().join(".git-paw");
    fs::create_dir_all(&paw).expect("create .git-paw");
    fs::write(paw.join("config.toml"), config).expect("write config");
    tmp
}

/// Runs a config-loading subcommand in `repo` and returns its stderr.
///
/// `doctor` is the subcommand of choice here: it fully resolves the
/// repository configuration (so the deprecation warning actually fires),
/// needs no tmux session and no broker, and is guaranteed not to mutate the
/// repository. `status` was the obvious candidate but bails with "No session
/// for this repo." before the config load, so it would make this test pass
/// vacuously.
fn stderr_of_config_load(repo: &Path) -> String {
    let out = cmd()
        .current_dir(repo)
        .arg("doctor")
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("run git paw doctor");
    String::from_utf8_lossy(&out.stderr).to_string()
}

#[test]
fn both_declared_tables_produce_exactly_one_warning_naming_both() {
    let repo = repo_with_config(
        "[supervisor]\n\
         enabled = true\n\
         [supervisor.auto_approve]\n\
         enabled = true\n\
         approval_level = \"safe\"\n\
         [supervisor.common_dev_allowlist]\n\
         stacks = [\"rust\"]\n",
    );
    let stderr = stderr_of_config_load(repo.path());

    let lines: Vec<&str> = stderr
        .lines()
        .filter(|l| l.contains("deprecated") && l.contains("supervisor."))
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "exactly one deprecation line per process, got {lines:?}; full stderr:\n{stderr}"
    );
    let line = lines[0];
    assert!(
        line.contains("[supervisor.auto_approve]")
            && line.contains("[supervisor.common_dev_allowlist]"),
        "the line must name both declared tables: {line}"
    );
    assert!(
        line.contains("no effect"),
        "the line must say the tables do nothing: {line}"
    );
}

#[test]
fn a_config_without_the_tables_produces_no_warning() {
    let repo = repo_with_config(
        "default_cli = \"claude\"\n[supervisor]\nenabled = true\ncli = \"claude\"\n",
    );
    let stderr = stderr_of_config_load(repo.path());
    assert!(
        !stderr.contains("deprecated"),
        "a config declaring neither table must warn about nothing; stderr:\n{stderr}"
    );
}
