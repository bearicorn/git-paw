//! End-to-end tests for `git paw doctor`'s sandbox-mode check
//! (`preflight-diagnostics`, "Doctor reports each worker CLI's declared
//! sandbox mode").
//!
//! Each test drives the real binary against a throwaway repository with its
//! own isolated `HOME`, and observes only the report and the `--json`
//! document.

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::Command as AssertCommand;
use tempfile::TempDir;

/// A throwaway git repository plus the isolated `HOME` doctor runs under.
struct Repo {
    _dir: TempDir,
    root: PathBuf,
    home: PathBuf,
}

impl Repo {
    /// A repository with one commit and `config` as `.git-paw/config.toml`.
    fn with_config(config: &str) -> Self {
        let dir = TempDir::new().expect("tempdir");
        let root = dir.path().join("repo");
        let home = dir.path().join("home");
        std::fs::create_dir_all(root.join(".git-paw")).expect("create repo");
        std::fs::create_dir_all(&home).expect("create home");
        for args in [
            vec!["init", "-b", "main"],
            vec!["config", "user.email", "test@test.com"],
            vec!["config", "user.name", "Test"],
        ] {
            Command::new("git")
                .current_dir(&root)
                .args(&args)
                .output()
                .expect("git");
        }
        std::fs::write(root.join("README.md"), "# test").expect("write README");
        for args in [vec!["add", "README.md"], vec!["commit", "-m", "initial"]] {
            Command::new("git")
                .current_dir(&root)
                .args(&args)
                .output()
                .expect("git");
        }
        std::fs::write(root.join(".git-paw/config.toml"), config).expect("write config");
        Self {
            _dir: dir,
            root,
            home,
        }
    }

    fn doctor(&self, args: &[&str]) -> std::process::Output {
        AssertCommand::cargo_bin("git-paw")
            .expect("binary")
            .arg("doctor")
            .args(args)
            .current_dir(&self.root)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .output()
            .expect("run doctor")
    }

    /// The sandbox-mode check rows of `doctor --json`.
    fn sandbox_mode_rows(&self) -> Vec<serde_json::Value> {
        let out = self.doctor(&["--json"]);
        let doc: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("doctor --json is JSON");
        doc["checks"]
            .as_array()
            .expect("checks array")
            .iter()
            .filter(|c| {
                c["name"]
                    .as_str()
                    .is_some_and(|n| n.starts_with("sandbox mode"))
            })
            .cloned()
            .collect()
    }
}

/// Scenario "A declared sandbox is reported".
#[test]
fn a_declared_wrapper_sandbox_is_reported_as_declared() {
    let repo = Repo::with_config(
        "default_cli = \"sbx\"\n\n[clis.sbx]\ncommand = \"/bin/sh\"\nsandbox = \"wrapper\"\n",
    );
    let stdout = String::from_utf8_lossy(&repo.doctor(&[]).stdout).to_string();
    assert!(stdout.contains("sbx: wrapper (declared)"), "{stdout}");

    let rows = repo.sandbox_mode_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["status"], "ok");
    assert!(
        rows[0]["detail"]
            .as_str()
            .is_some_and(|d| d.contains("wrapper (declared)")),
        "{rows:?}"
    );
}

/// Scenario "Unsandboxed full-auto warns".
#[test]
fn unsandboxed_full_auto_warns_naming_the_cli_and_does_not_fail() {
    let repo = Repo::with_config(
        "default_cli = \"claude\"\n\n[supervisor]\nagent_approval = \"full-auto\"\n",
    );
    let rows = repo.sandbox_mode_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row["status"], "warn", "{row}");
    assert!(
        row["detail"].as_str().is_some_and(|d| d.contains("claude")),
        "{row}"
    );
    assert!(
        row["remedy"]
            .as_str()
            .is_some_and(|r| r.contains("sandbox guide")),
        "the remedy must point at the sandbox documentation: {row}"
    );

    let out = repo.doctor(&["--json"]);
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("JSON");
    assert!(
        doc["checks"]
            .as_array()
            .expect("checks")
            .iter()
            .filter(|c| c["name"]
                .as_str()
                .is_some_and(|n| n.starts_with("sandbox mode")))
            .all(|c| c["status"] != "fail"),
        "an unsandboxed full-auto worker is a ⚠, not a ✗"
    );
}

/// Scenario "A declared sandbox at full-auto is not shown as verified".
#[test]
fn a_declared_sandbox_at_full_auto_warns_declared_not_verified() {
    let repo = Repo::with_config(
        "default_cli = \"sbx\"\n\n[supervisor]\nagent_approval = \"full-auto\"\n\n\
         [clis.sbx]\ncommand = \"/bin/sh\"\nsandbox = \"wrapper\"\n",
    );
    let rows = repo.sandbox_mode_rows();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["status"], "warn", "{rows:?}");
    assert!(
        rows[0]["detail"]
            .as_str()
            .is_some_and(|d| d.contains("declared, not verified")),
        "{rows:?}"
    );
}

/// Scenario "A wrapper that is the agent CLI itself warns".
#[test]
fn a_wrapper_declaration_on_the_agent_cli_itself_warns() {
    let repo = Repo::with_config(
        "default_cli = \"sbx\"\n\n[clis.sbx]\ncommand = \"claude\"\nfamily = \"claude\"\n\
         sandbox = \"wrapper\"\n",
    );
    let rows = repo.sandbox_mode_rows();
    assert_eq!(rows[0]["status"], "warn", "{rows:?}");
    assert!(
        rows[0]["detail"]
            .as_str()
            .is_some_and(|d| d.contains("`claude`")),
        "{rows:?}"
    );
}

/// Scenario "Doctor reports a launch the sandbox rule would refuse".
#[test]
fn a_native_sandbox_full_auto_would_disable_fails_doctor() {
    let repo = Repo::with_config(
        "default_cli = \"codex\"\n\n[supervisor]\nagent_approval = \"full-auto\"\n\n\
         [clis.codex]\ncommand = \"codex\"\nsandbox = \"native\"\n",
    );
    let rows = repo.sandbox_mode_rows();
    assert_eq!(rows[0]["status"], "fail", "{rows:?}");
    assert!(
        rows[0]["detail"]
            .as_str()
            .is_some_and(|d| d.contains("[clis.codex].approval_args")),
        "{rows:?}"
    );
    assert!(!repo.doctor(&[]).status.success(), "a ✗ fails doctor");
}

/// Scenario "A full-auto worker profile on an unsandboxed CLI warns"
/// (agent profiles): the session level is `manual`, the profile's own level
/// is `full-auto`, so the profile's row warns — and nothing fails.
#[test]
fn a_full_auto_worker_profile_on_an_unsandboxed_cli_warns() {
    let repo = Repo::with_config(
        "[supervisor]\nagent_approval = \"manual\"\n\n\
         [profiles.trusted]\nkind = \"worker\"\ndescription = \"Trusted\"\n\
         cli = \"claude\"\napproval = \"full-auto\"\n",
    );
    let rows = repo.sandbox_mode_rows();
    let row = rows
        .iter()
        .find(|r| r["name"] == "sandbox mode: claude (profile trusted)")
        .unwrap_or_else(|| panic!("no profile row in {rows:?}"));
    assert_eq!(row["status"], "warn", "{row}");
    assert!(
        row["remedy"]
            .as_str()
            .is_some_and(|r| r.contains("sandbox guide")),
        "{row}"
    );
    assert!(rows.iter().all(|r| r["status"] != "fail"), "{rows:?}");
}

/// Scenario "A spec's profile resolves to its CLI": the spec-worker check
/// reports the profile's CLI for a spec whose `paw_profile` names it.
#[test]
fn a_specs_profile_resolves_to_its_cli() {
    let repo = Repo::with_config(
        "default_spec_cli = \"alpha\"\n\n[specs]\ntype = \"markdown\"\ndir = \"specs\"\n\n\
         [profiles.routine]\nkind = \"worker\"\ndescription = \"Routine\"\ncli = \"beta\"\n",
    );
    std::fs::create_dir_all(repo.root.join("specs")).expect("specs dir");
    std::fs::write(
        repo.root.join("specs/add-x.md"),
        "---\npaw_status: pending\npaw_profile: routine\n---\nbody\n",
    )
    .expect("write spec");
    let out = repo.doctor(&["--json"]);
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let row = doc["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["name"] == "spec-worker CLI")
        .cloned()
        .expect("spec-worker CLI row");
    assert!(
        row["detail"].as_str().is_some_and(|d| d.contains("beta")),
        "{row}"
    );
}
