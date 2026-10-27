//! `git paw init` exports for `orchestrator-skill-export`: the standardized
//! orchestrator and `cli-setup` skills under `.git-paw/skills/`, the
//! `.gitignore` entry, the root `AGENTS.md` pointer section, and an init
//! report whose `Updated` lines and closing line agree with what was written
//! (PP-30).
//!
//! Maps to the `core-init`, `skill-standardized` and `core-lang-agnostic`
//! deltas' init-side scenarios.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use git_paw::skills::{resolve, validate_skill_dir};
use serial_test::serial;
use tempfile::TempDir;

const ORCHESTRATOR_MD: &str = ".git-paw/skills/orchestrator/SKILL.md";
const CLI_SETUP_MD: &str = ".git-paw/skills/cli-setup/SKILL.md";
const POINTER_START: &str = "<!-- git-paw-orchestrator:start -->";
const POINTER_END: &str = "<!-- git-paw-orchestrator:end -->";

fn init_git_repo(dir: &Path) {
    for args in [
        &["init", "-b", "main"][..],
        &["config", "user.email", "test@test.com"],
        &["config", "user.name", "Test"],
    ] {
        let st = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git");
        assert!(st.success(), "git {args:?}");
    }
}

/// A fresh git repository, optionally with a pre-written `.git-paw/config.toml`.
fn repo(config: Option<&str>) -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    if let Some(config) = config {
        fs::create_dir_all(tmp.path().join(".git-paw")).expect("mkdir");
        fs::write(tmp.path().join(".git-paw/config.toml"), config).expect("config");
    }
    tmp
}

/// Runs `git paw <args>` in `root` with an isolated `HOME`; returns stdout.
fn paw(root: &Path, home: &Path, args: &[&str]) -> (bool, String) {
    let out = Command::cargo_bin("git-paw")
        .expect("binary")
        .current_dir(root)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .args(args)
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run git paw");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

fn init(root: &Path) -> String {
    let home = TempDir::new().expect("home");
    let (ok, stdout) = paw(root, home.path(), &["init"]);
    assert!(ok, "git paw init failed:\n{stdout}");
    stdout
}

fn read(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

fn closing_line(stdout: &str) -> &str {
    stdout.lines().last().unwrap_or("")
}

// core-init / Scenario: Init installs the skill in a fresh repo
// skill-standardized / Scenario: Exported skill passes standardized validation
#[test]
#[serial]
fn init_installs_the_orchestrator_skill_in_a_fresh_repo() {
    let tmp = repo(None);
    let stdout = init(tmp.path());

    let md = read(tmp.path(), ORCHESTRATOR_MD);
    assert_eq!(md.lines().next(), Some("---"));
    let frontmatter_end = md[3..].find("\n---").expect("closed frontmatter") + 3;
    assert!(md[..frontmatter_end].contains("name: orchestrator"));
    assert!(
        stdout.contains(&format!("Created {ORCHESTRATOR_MD}")),
        "{stdout}"
    );

    let meta = validate_skill_dir(&tmp.path().join(".git-paw/skills/orchestrator"))
        .expect("exported orchestrator skill validates");
    assert_eq!(meta.name, "orchestrator");
}

// core-init / Scenario: Skill renders configured gate commands
#[test]
#[serial]
fn init_renders_configured_gate_commands() {
    let tmp = repo(Some(
        "[supervisor]\nenabled = true\ntest_command = \"make test\"\n",
    ));
    init(tmp.path());
    let md = read(tmp.path(), ORCHESTRATOR_MD);
    assert!(md.contains("make test"));
    assert!(
        md.contains("(not configured)"),
        "an unset gate renders as (not configured)"
    );
}

// core-init / Scenario: Init renders without a session
#[test]
#[serial]
fn init_renders_the_skill_without_a_session() {
    let tmp = repo(Some("[specs]\ntype = \"openspec\"\n"));
    init(tmp.path());
    let md = read(tmp.path(), ORCHESTRATOR_MD);
    let doctrine = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/spec-path-doctrine/openspec.md"),
    )
    .expect("openspec doctrine");
    assert!(md.contains(doctrine.trim()), "OpenSpec doctrine rendered");
    assert!(!md.contains("{{GIT_PAW_BROKER_URL}}"));
    assert!(!md.contains("http://"));
    assert!(!md.contains("paw-"), "no session name");
    assert!(md.contains("{{CHANGE_ID}}"));
    assert_eq!(
        md.matches("{{").count(),
        md.matches("{{CHANGE_ID}}").count(),
        "no other placeholder may remain"
    );
}

// core-init / Scenario: No spec system renders the doctrine sentinel
#[test]
#[serial]
fn init_without_a_spec_system_renders_the_sentinel() {
    let tmp = repo(Some("default_cli = \"echo\"\n"));
    init(tmp.path());
    // The no-backend sentinel sentence.
    assert!(
        read(tmp.path(), ORCHESTRATOR_MD).contains("no spec backend resolved for this session")
    );
}

// core-init / Scenarios: Init overwrites a stale skill file; Init overwrites
// a stale cli-setup skill file
#[test]
#[serial]
fn init_overwrites_stale_exported_skills_and_reports_updates() {
    let tmp = repo(None);
    init(tmp.path());
    let fresh_orchestrator = read(tmp.path(), ORCHESTRATOR_MD);
    let fresh_cli_setup = read(tmp.path(), CLI_SETUP_MD);
    fs::write(tmp.path().join(ORCHESTRATOR_MD), "stale local content\n").unwrap();
    fs::write(tmp.path().join(CLI_SETUP_MD), "stale local content\n").unwrap();

    let stdout = init(tmp.path());
    assert!(
        stdout.contains(&format!("Updated {ORCHESTRATOR_MD}")),
        "{stdout}"
    );
    assert!(
        stdout.contains(&format!("Updated {CLI_SETUP_MD}")),
        "{stdout}"
    );
    assert_eq!(read(tmp.path(), ORCHESTRATOR_MD), fresh_orchestrator);
    assert_eq!(read(tmp.path(), CLI_SETUP_MD), fresh_cli_setup);
    assert_eq!(closing_line(&stdout), "Initialized git-paw.");
}

// core-init / Scenario: Skill directory is gitignored
#[test]
#[serial]
fn init_gitignores_the_skill_directory() {
    let tmp = repo(None);
    init(tmp.path());
    assert!(
        read(tmp.path(), ".gitignore")
            .lines()
            .any(|l| l.trim() == ".git-paw/skills/")
    );
}

// core-init / Scenario: Init installs the cli-setup skill beside the
// orchestrator skill
// skill-standardized / Scenario: Exported cli-setup skill passes
// standardized validation
#[test]
#[serial]
fn init_installs_the_cli_setup_skill() {
    let tmp = repo(None);
    let stdout = init(tmp.path());
    assert!(
        stdout.contains(&format!("Created {CLI_SETUP_MD}")),
        "{stdout}"
    );

    let meta = validate_skill_dir(&tmp.path().join(".git-paw/skills/cli-setup"))
        .expect("exported cli-setup skill validates");
    assert_eq!(meta.name, "cli-setup");
    let resolved = resolve("cli-setup").expect("cli-setup resolves");
    assert_eq!(
        meta.description,
        resolved.metadata.expect("frontmatter").description
    );
    let exported = read(tmp.path(), CLI_SETUP_MD);
    let body = exported
        .splitn(3, "---\n")
        .nth(2)
        .expect("body after frontmatter");
    assert_eq!(
        body.trim_end(),
        resolved.content.trim_end(),
        "the export carries the resolved skill body"
    );
}

// core-init / Scenario: Init injects the pointer into an existing AGENTS.md
#[test]
#[serial]
fn init_injects_the_pointer_into_an_existing_agents_md() {
    let tmp = repo(None);
    fs::write(
        tmp.path().join("AGENTS.md"),
        "# My project\n\nHouse rules.\n",
    )
    .unwrap();
    init(tmp.path());

    let agents = read(tmp.path(), "AGENTS.md");
    assert!(agents.starts_with("# My project"));
    assert_eq!(agents.matches(POINTER_START).count(), 1);
    assert_eq!(agents.matches(POINTER_END).count(), 1);
    let section = &agents[agents.find(POINTER_START).unwrap()..agents.find(POINTER_END).unwrap()];
    assert!(section.contains(ORCHESTRATOR_MD));
    assert!(section.contains(".git-paw/AGENTS.local.md"));
    assert!(!section.contains("http://"), "no broker URL in the pointer");
}

// core-init / Scenario: Init creates AGENTS.md when absent
#[test]
#[serial]
fn init_creates_agents_md_when_absent() {
    let tmp = repo(None);
    let stdout = init(tmp.path());
    let agents = read(tmp.path(), "AGENTS.md");
    assert!(agents.contains(POINTER_START) && agents.contains(ORCHESTRATOR_MD));
    assert!(stdout.contains("Created AGENTS.md"), "{stdout}");
}

// core-init / Scenario: Re-running init replaces the section in place
#[test]
#[serial]
fn rerunning_init_replaces_the_pointer_in_place() {
    let tmp = repo(None);
    init(tmp.path());
    let path = tmp.path().join("AGENTS.md");
    let mut agents = read(tmp.path(), "AGENTS.md");
    agents.push_str("\n## User section\n\nkept verbatim\n");
    // Make the section stale so the re-run must rewrite it.
    let stale = agents.replacen(ORCHESTRATOR_MD, ".git-paw/skills/stale/SKILL.md", 1);
    fs::write(&path, &stale).unwrap();
    let outside = |s: &str| {
        let start = s.find(POINTER_START).unwrap();
        let end = s.find(POINTER_END).unwrap() + POINTER_END.len();
        (s[..start].to_string(), s[end..].to_string())
    };

    init(tmp.path());
    let after = read(tmp.path(), "AGENTS.md");
    assert_eq!(after.matches(POINTER_START).count(), 1);
    assert_eq!(
        outside(&after),
        outside(&stale),
        "content outside the markers is byte-identical"
    );
    assert!(after.contains(ORCHESTRATOR_MD));
}

// core-init / Scenario: Stop and purge leave the pointer
#[test]
#[serial]
fn stop_and_purge_leave_the_pointer() {
    let tmp = repo(None);
    init(tmp.path());
    let before = read(tmp.path(), "AGENTS.md");
    let home = TempDir::new().expect("home");
    let _ = paw(tmp.path(), home.path(), &["stop"]);
    let _ = paw(tmp.path(), home.path(), &["purge", "--force"]);
    let after = read(tmp.path(), "AGENTS.md");
    assert!(after.contains(POINTER_START) && after.contains(POINTER_END));
    assert_eq!(after, before);
}

// core-init / Scenario: A re-run with current content reports nothing to do
#[test]
#[serial]
fn rerun_with_current_content_reports_nothing_to_do() {
    let tmp = repo(None);
    init(tmp.path());
    let stdout = init(tmp.path());
    assert!(!stdout.contains("Updated"), "{stdout}");
    assert_eq!(closing_line(&stdout), "Already initialized. Nothing to do.");
}

// core-init / Scenario: A stale helper is updated and the closing line agrees
#[test]
#[serial]
fn stale_helper_is_updated_and_the_closing_line_agrees() {
    let tmp = repo(None);
    init(tmp.path());
    fs::write(tmp.path().join(".git-paw/scripts/sweep.sh"), "# stale\n").unwrap();
    let stdout = init(tmp.path());
    assert!(
        stdout.contains("Updated .git-paw/scripts/sweep.sh"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("Updated .git-paw/scripts/broker.sh"),
        "{stdout}"
    );
    assert_eq!(closing_line(&stdout), "Initialized git-paw.");
    assert!(!stdout.contains("Already initialized. Nothing to do."));
}

// The exports stay out of `git add -A` even when the root `.gitignore` lacks
// the `.git-paw/skills/` entry (a repository whose ignore file predates it):
// the skills directory ignores itself.
#[test]
#[serial]
fn exports_stay_untracked_without_the_root_gitignore_entry() {
    let tmp = repo(None);
    init(tmp.path());
    let gitignore = read(tmp.path(), ".gitignore");
    let stripped: Vec<&str> = gitignore
        .lines()
        .filter(|l| l.trim() != ".git-paw/skills/")
        .collect();
    let stripped = stripped.join("\n") + "\n";
    fs::write(tmp.path().join(".gitignore"), stripped).unwrap();

    let status = StdCommand::new("git")
        .current_dir(tmp.path())
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .expect("git status");
    let listing = String::from_utf8_lossy(&status.stdout);
    assert!(
        !listing.contains(".git-paw/skills"),
        "exported skills must stay untracked:\n{listing}"
    );
}
