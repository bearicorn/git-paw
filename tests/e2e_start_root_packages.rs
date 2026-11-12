//! E2E coverage for `skill-packages`' *Every start installs the bundled
//! packages at the repository root* and *The root package refresh renders
//! from the current config* (DF-06, PK-01): a bare `git paw start` installs the
//! root packages and their compatibility copies before any pane, writes only
//! what is missing or differs from a render of the current config, and never
//! touches `init`'s files.
//!
//! Drives the real binary against an isolated tmux socket and `HOME`, with
//! `cat` as the stand-in CLI. Skips when tmux is unavailable.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use assert_cmd::Command;
use git_paw::skills::bundled_packages;
use serial_test::serial;
use tempfile::TempDir;

mod helpers;
use helpers::*;

fn tmux_available() -> bool {
    let ok = StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success());
    if !ok {
        eprintln!("skipping: tmux not available");
    }
    ok
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A test repository renamed to a unique basename, so its derived tmux
/// session name never collides with another test's.
fn unique_repo(tr: &TestRepo, tag: &str) -> std::path::PathBuf {
    let name = format!(
        "rootpkg-{tag}-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let old = tr.path().to_path_buf();
    let renamed = old.parent().expect("sandbox").join(&name);
    fs::rename(&old, &renamed).expect("rename repo");
    renamed
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("free port")
        .port()
}

fn write_config(repo: &Path, extra: &str) {
    fs::create_dir_all(repo.join(".git-paw")).expect(".git-paw");
    fs::write(
        repo.join(".git-paw/config.toml"),
        format!(
            "default_cli = \"cat\"\n\n[broker]\nenabled = true\nport = {}\n\n[clis.cat]\ncommand = \"cat\"\n{extra}",
            free_port()
        ),
    )
    .expect("config");
}

fn git(repo: &Path, args: &[&str]) {
    let out = StdCommand::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}");
}

fn paw(repo: &Path, env: &TmuxTestEnv, args: &[&str]) -> (bool, String) {
    let home = TempDir::new().expect("home");
    let mut c = Command::cargo_bin("git-paw").expect("binary");
    env.apply_assert(&mut c);
    let out = c
        .current_dir(repo)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .args(args)
        .timeout(Duration::from_secs(40))
        .output()
        .expect("run git paw");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
    )
}

/// Runs `git paw start --branches a` and kills the session it created.
fn bare_start(repo: &Path, env: &TmuxTestEnv) {
    let (ok, out) = paw(repo, env, &["start", "--branches", "a", "--cli", "cat"]);
    let session = format!(
        "paw-{}",
        repo.file_name().unwrap_or_default().to_string_lossy()
    );
    let mut kill = StdCommand::new("tmux");
    env.apply(&mut kill);
    let _ = kill.args(["kill-session", "-t", &session]).status();
    assert!(ok, "bare start failed:\n{out}");
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

/// Every root package file and compatibility copy, relative to the repo.
fn root_files() -> Vec<String> {
    let mut files = Vec::new();
    for package in bundled_packages() {
        files.push(package.skill_md_path());
        for script in package.scripts {
            files.push(format!("{}/{}", package.install_dir(), script.path));
            files.push(format!(".git-paw/scripts/{}", script.file_name()));
        }
    }
    files.sort();
    files.dedup();
    files
}

// Scenario: A bare start after a fresh clone installs the root packages.
#[test]
#[serial]
fn a_bare_start_after_a_fresh_clone_installs_the_root_packages() {
    if !tmux_available() {
        return;
    }
    let tr = setup_test_repo();
    let env = tmux_test_env();
    let repo = unique_repo(&tr, "clone");
    write_config(&repo, "");
    git(&repo, &["add", ".git-paw/config.toml"]);
    git(&repo, &["commit", "-q", "-m", "config"]);
    assert!(!repo.join(".git-paw/skills").exists());
    assert!(!repo.join(".git-paw/scripts").exists());

    bare_start(&repo, &env);

    for rel in [
        ".git-paw/skills/orchestrator/scripts/sweep.sh",
        ".git-paw/scripts/sweep.sh",
    ] {
        assert!(is_executable(&repo.join(rel)), "{rel}");
    }
    for package in bundled_packages() {
        assert!(
            repo.join(package.skill_md_path()).is_file(),
            "{}",
            package.name
        );
    }
}

// Scenario: A start leaves current root packages untouched.
#[test]
#[serial]
fn a_start_leaves_current_root_packages_untouched() {
    if !tmux_available() {
        return;
    }
    let tr = setup_test_repo();
    let env = tmux_test_env();
    let repo = unique_repo(&tr, "current");
    write_config(&repo, "");
    bare_start(&repo, &env);

    let old = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    for rel in root_files() {
        fs::File::options()
            .write(true)
            .open(repo.join(&rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}"))
            .set_modified(old)
            .unwrap();
    }

    bare_start(&repo, &env);

    for rel in root_files() {
        let mtime = fs::metadata(repo.join(&rel)).unwrap().modified().unwrap();
        assert_eq!(mtime, old, "{rel} was rewritten");
    }
}

// Scenario: A start does not take over init's files.
#[test]
#[serial]
fn a_start_does_not_take_over_inits_files() {
    if !tmux_available() {
        return;
    }
    let tr = setup_test_repo();
    let env = tmux_test_env();
    let repo = unique_repo(&tr, "takeover");
    write_config(&repo, "");
    fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    fs::write(repo.join("AGENTS.md"), "# Project rules\n").unwrap();
    git(
        &repo,
        &["add", ".gitignore", "AGENTS.md", ".git-paw/config.toml"],
    );
    git(&repo, &["commit", "-q", "-m", "tracked files"]);
    let before: Vec<Vec<u8>> = [".gitignore", "AGENTS.md", ".git-paw/config.toml"]
        .iter()
        .map(|rel| fs::read(repo.join(rel)).unwrap())
        .collect();

    bare_start(&repo, &env);

    let after: Vec<Vec<u8>> = [".gitignore", "AGENTS.md", ".git-paw/config.toml"]
        .iter()
        .map(|rel| fs::read(repo.join(rel)).unwrap())
        .collect();
    assert_eq!(after, before);
    assert!(repo.join(".git-paw/skills/orchestrator/SKILL.md").is_file());
}

// Scenario: A gate command configured after init reaches the root skill on
// the next start.
#[test]
#[serial]
fn a_gate_command_configured_after_init_reaches_the_root_skill() {
    if !tmux_available() {
        return;
    }
    let tr = setup_test_repo();
    let env = tmux_test_env();
    let repo = unique_repo(&tr, "gate");
    write_config(&repo, "\n[supervisor]\nenabled = false\n");
    let (ok, out) = paw(&repo, &env, &["init"]);
    assert!(ok, "init failed:\n{out}");
    let skill = repo.join(".git-paw/skills/orchestrator/SKILL.md");
    let baseline = |text: &str| {
        text.lines()
            .find(|l| l.contains("**Baseline**"))
            .unwrap_or_default()
            .to_string()
    };
    assert!(
        baseline(&fs::read_to_string(&skill).unwrap()).contains("(not configured)"),
        "init rendered no test command"
    );

    let config = fs::read_to_string(repo.join(".git-paw/config.toml")).unwrap();
    fs::write(
        repo.join(".git-paw/config.toml"),
        config.replace(
            "[supervisor]\nenabled = false\n",
            "[supervisor]\nenabled = false\ntest_command = \"make test\"\n",
        ),
    )
    .unwrap();

    bare_start(&repo, &env);

    let line = baseline(&fs::read_to_string(&skill).unwrap());
    assert!(line.contains("make test"), "{line}");
    assert!(!line.contains("(not configured)"), "{line}");
}

// skill-packages / Scenario: Installing packages leaves setup grants
// untouched — across `git paw init` and a `git paw start` that provisions a
// worktree, the main checkout's settings file and the setup record are
// byte-identical.
#[test]
#[serial]
fn installing_packages_leaves_setup_grants_untouched() {
    if !tmux_available() {
        return;
    }
    let tr = setup_test_repo();
    let env = tmux_test_env();
    let repo = unique_repo(&tr, "grants");
    write_config(&repo, "");
    let settings = repo.join(".claude/settings.local.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::write(
        &settings,
        r#"{"permissions":{"allow":["Bash(.git-paw/scripts/broker.sh *)"],"deny":["Edit(**/.git-paw/scripts/**)"]}}"#,
    )
    .unwrap();
    let record = repo.join(".git/git-paw/cli-setup.json");
    fs::create_dir_all(record.parent().unwrap()).unwrap();
    fs::write(&record, r#"{"schema_version":1,"entries":[]}"#).unwrap();
    let before = (fs::read(&settings).unwrap(), fs::read(&record).unwrap());

    let (ok, out) = paw(&repo, &env, &["init"]);
    assert!(ok, "init failed:\n{out}");
    bare_start(&repo, &env);

    let worktree_skill = fs::read_dir(repo.join(".git-paw/worktrees"))
        .expect("a worktree was provisioned")
        .filter_map(Result::ok)
        .any(|e| {
            e.path()
                .join(".git-paw/skills/coordination/SKILL.md")
                .is_file()
        });
    assert!(worktree_skill, "start provisioned the worktree's packages");
    assert_eq!(
        (fs::read(&settings).unwrap(), fs::read(&record).unwrap()),
        before,
        "the settings file and the setup record must be byte-identical"
    );
}
