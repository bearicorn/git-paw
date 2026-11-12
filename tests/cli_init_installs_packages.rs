//! `git paw init` installs the bundled skill packages (`skill-packages`
//! *Bundled packages are installed under `.git-paw/skills/`*, *Init installs
//! every package at the root*, *The legacy helper location is a deprecated
//! compatibility copy*, *Installation supersedes the packaging identity pin*,
//! *Installing packages never touches setup grants*) and ignores every piece
//! of runtime state under `.git-paw/` (`core-init` *Init appends logs
//! directory to .gitignore*).

#[path = "helpers/packages.rs"]
mod packages;

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::Duration;

use assert_cmd::Command;
use git_paw::skills::bundled_packages;
use serial_test::serial;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = StdCommand::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn repo() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    git(tmp.path(), &["init", "-q", "-b", "main"]);
    git(tmp.path(), &["config", "user.email", "test@test.com"]);
    git(tmp.path(), &["config", "user.name", "Test"]);
    tmp
}

/// Runs `git paw init` in `root` with an isolated `HOME`; returns stdout.
fn init(root: &Path) -> String {
    let home = TempDir::new().expect("home");
    let out = Command::cargo_bin("git-paw")
        .expect("binary")
        .current_dir(root)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .arg("init")
        .timeout(Duration::from_secs(20))
        .output()
        .expect("run git paw init");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "git paw init failed:\n{stdout}");
    stdout
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 == 0o111)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

// Scenario: Init installs every package at the root.
#[test]
#[serial]
fn init_installs_every_package_at_the_root() {
    let tmp = repo();
    init(tmp.path());
    for package in bundled_packages() {
        let dir = tmp.path().join(package.install_dir());
        assert!(dir.join("SKILL.md").is_file(), "{}", package.name);
        assert!(!dir.join("assets").exists(), "{} assets", package.name);
        for script in package.scripts {
            let path = dir.join(script.path);
            assert!(is_executable(&path), "{}", path.display());
        }
    }
}

// Scenario: Re-install refreshes a stale package.
#[test]
#[serial]
fn re_install_refreshes_a_stale_package() {
    let tmp = repo();
    init(tmp.path());
    let broker = tmp
        .path()
        .join(".git-paw/skills/coordination/scripts/broker.sh");
    fs::write(&broker, "# stale\n").unwrap();

    let stdout = init(tmp.path());

    assert_eq!(
        fs::read_to_string(&broker).unwrap(),
        packages::script("broker.sh")
    );
    assert!(
        stdout.contains("Updated .git-paw/skills/coordination/scripts/broker.sh"),
        "{stdout}"
    );
}

// Scenario: Package directory is gitignored.
#[test]
#[serial]
fn package_directory_is_gitignored() {
    let tmp = repo();
    init(tmp.path());
    let gitignore = fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(
        gitignore.lines().any(|l| l == ".git-paw/skills/"),
        "{gitignore}"
    );
}

// Scenario: The compatibility copy matches the package copy.
#[test]
#[serial]
fn the_compatibility_copy_matches_the_package_copy() {
    let tmp = repo();
    init(tmp.path());
    let compat = tmp.path().join(".git-paw/scripts/sweep.sh");
    assert!(is_executable(&compat));
    assert_eq!(
        fs::read(&compat).unwrap(),
        fs::read(
            tmp.path()
                .join(".git-paw/skills/orchestrator/scripts/sweep.sh")
        )
        .unwrap()
    );
}

// Scenario: No temporary file is left behind.
#[test]
#[serial]
fn no_temporary_file_is_left_behind() {
    let tmp = repo();
    init(tmp.path());
    let mut dirs = vec![tmp.path().join(".git-paw/scripts")];
    for package in bundled_packages() {
        let dir = tmp.path().join(package.install_dir());
        dirs.push(dir.join("scripts"));
        dirs.push(dir);
    }
    for dir in dirs.into_iter().filter(|d| d.is_dir()) {
        for entry in fs::read_dir(&dir).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(!name.contains(".tmp-"), "{}/{name}", dir.display());
        }
    }
}

// Scenario: The exported orchestrator skill names the package helper, and
// Scenario: The identity pin does not constrain installed output.
#[test]
#[serial]
fn the_exported_orchestrator_skill_names_the_package_helper() {
    let tmp = repo();
    init(tmp.path());
    let skill =
        fs::read_to_string(tmp.path().join(".git-paw/skills/orchestrator/SKILL.md")).unwrap();
    assert!(
        skill.contains(".git-paw/skills/orchestrator/scripts/sweep.sh snapshot"),
        "the exported skill must name the package helper"
    );
    assert!(!skill.contains("{{SWEEP_HELPER}}"));
    assert!(
        !skill.contains(".git-paw/scripts/sweep.sh"),
        "the pre-package path is not what the installed skill names"
    );
    for package in bundled_packages() {
        assert!(
            tmp.path().join(package.install_dir()).is_dir(),
            "{}",
            package.name
        );
    }
}

// Scenario: Installing packages leaves setup grants untouched.
#[test]
#[serial]
fn installing_packages_leaves_setup_grants_untouched() {
    let tmp = repo();
    let settings = tmp.path().join(".claude/settings.local.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    let grants = r#"{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"],"deny":["Edit(**/.git-paw/scripts/**)"]}}"#;
    fs::write(&settings, grants).unwrap();
    let record = tmp.path().join(".git/git-paw/cli-setup.json");
    fs::create_dir_all(record.parent().unwrap()).unwrap();
    fs::write(&record, r#"{"schema_version":1,"entries":[]}"#).unwrap();

    init(tmp.path());
    init(tmp.path());

    assert_eq!(fs::read_to_string(&settings).unwrap(), grants);
    assert_eq!(
        fs::read_to_string(&record).unwrap(),
        r#"{"schema_version":1,"entries":[]}"#
    );
}

// core-init / Scenario: Runtime state is not staged by git add .git-paw/
// (PK-19).
#[test]
#[serial]
fn runtime_state_is_not_staged_by_git_add() {
    let tmp = repo();
    init(tmp.path());
    let paw = tmp.path().join(".git-paw");
    fs::create_dir_all(paw.join("sessions")).unwrap();
    fs::write(paw.join("sessions/paw-x.json"), "{}").unwrap();
    fs::write(paw.join("AGENTS.local.md"), "sidecar").unwrap();
    fs::write(paw.join(".sweep-progress"), "1").unwrap();
    fs::write(paw.join(".sweep-stuck-dedup"), "x").unwrap();
    fs::write(paw.join(".add-remove.lock"), "").unwrap();
    fs::write(paw.join("session-summary.md"), "summary").unwrap();
    assert!(paw.join("scripts/sweep.sh").is_file());

    git(tmp.path(), &["add", ".git-paw/"]);
    let staged = git(tmp.path(), &["diff", "--cached", "--name-only"]);
    assert_eq!(staged.trim(), ".git-paw/config.toml", "staged:\n{staged}");
}
