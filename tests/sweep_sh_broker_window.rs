//! `sweep.sh` pane discovery with the broker host's window selected
//! (`broker-host-separation` review finding).
//!
//! A session now has a second tmux window, `broker`, where the broker host
//! runs. `tmux list-panes -t <session>` lists only the CURRENT window, so once
//! someone selects the broker window (one click on the status bar, or
//! `C-b n`) a bare-session listing sees only the host's pane. `sweep.sh`'s
//! coding-pane discovery must list window 0 instead, or `snapshot` and
//! `approve` see no coding panes at all.
//!
//! Runs on a private tmux socket only.

use std::fs;
use std::path::Path;
use std::process::Command;

use serial_test::serial;

mod helpers;
use helpers::{TmuxTestEnv, tmux_test_env};

fn tmux_available() -> bool {
    Command::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git command");
    };
    run(&["init", "-q", "-b", "main"]);
    run(&["config", "user.email", "t@e.st"]);
    run(&["config", "user.name", "Test"]);
    fs::write(dir.join("README.md"), "x").expect("readme");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Installs the bundled sweep.sh (and the preamble it sources) into the repo,
/// as `git paw init` does.
fn install_sweep(repo: &Path) -> std::path::PathBuf {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts");
    let dst_dir = repo.join(".git-paw/scripts");
    fs::create_dir_all(&dst_dir).expect("mk scripts dir");
    fs::copy(
        assets.join("_paw_common.sh"),
        dst_dir.join("_paw_common.sh"),
    )
    .expect("copy _paw_common.sh");
    let dst = dst_dir.join("sweep.sh");
    fs::copy(assets.join("sweep.sh"), &dst).expect("copy sweep.sh");
    dst
}

/// Writes the per-repo discovery file so sweep.sh resolves the session name
/// without `$TMUX`.
fn write_session_json(repo: &Path, session_name: &str) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let body = format!("{{\n  \"session_name\": \"{session_name}\",\n  \"agents\": []\n}}");
    fs::write(dir.join(format!("{session_name}.json")), body).expect("write session json");
}

/// Runs `tmux <args>` on the private socket.
fn tmux(env: &TmuxTestEnv, args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("tmux");
    env.apply(&mut cmd);
    cmd.args(args).output().expect("tmux")
}

/// Builds the launch shape: window 0 with a dashboard pane (title
/// `dashboard`) and one coding pane, plus a detached `broker` window — then
/// selects the broker window, as a click on the status bar would.
fn build_session_with_broker_window_selected(env: &TmuxTestEnv, repo: &Path, name: &str) {
    let root = repo.to_str().expect("utf-8 repo path");
    let st = tmux(
        env,
        &[
            "-f",
            "/dev/null",
            "new-session",
            "-d",
            "-s",
            name,
            "-x",
            "200",
            "-y",
            "50",
            "-c",
            root,
        ],
    );
    assert!(st.status.success(), "tmux new-session failed");
    let pane0 = format!("{name}:0.0");
    assert!(
        tmux(env, &["select-pane", "-t", &pane0, "-T", "dashboard"])
            .status
            .success()
    );
    assert!(
        tmux(env, &["split-window", "-t", &pane0, "-c", root])
            .status
            .success()
    );
    let pane1 = format!("{name}:0.1");
    assert!(
        tmux(env, &["select-pane", "-t", &pane1, "-T", "feat-a"])
            .status
            .success()
    );
    assert!(
        tmux(
            env,
            &[
                "new-window",
                "-d",
                "-n",
                "broker",
                "-t",
                &format!("{name}:"),
                "sleep 3600"
            ]
        )
        .status
        .success()
    );
    assert!(
        tmux(env, &["select-window", "-t", &format!("{name}:broker")])
            .status
            .success()
    );
    let current = tmux(
        env,
        &["display-message", "-p", "-t", name, "#{window_name}"],
    );
    assert_eq!(
        String::from_utf8_lossy(&current.stdout).trim(),
        "broker",
        "precondition: the broker window is the current one"
    );
}

/// With the broker window selected, `sweep.sh snapshot` still finds the coding
/// pane at window 0's index 1 (and not the dashboard).
///
/// Before the fix `discover_coding_panes` ran `tmux list-panes -t
/// "${SESSION}"`, which with the broker window current listed only the
/// broker window's single pane (index 0, not a dashboard and not titled one),
/// so snapshot reported pane 0 of the wrong window — or, had the host pane
/// been filtered, "no coding panes discovered" — and never pane 1.
#[test]
#[serial]
fn snapshot_finds_the_coding_pane_with_the_broker_window_selected() {
    if !tmux_available() {
        return;
    }
    let env = tmux_test_env();
    let repo = tempfile::TempDir::new().expect("repo tempdir");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let name = format!("paw-bw-{nanos}");
    write_session_json(repo.path(), &name);
    build_session_with_broker_window_selected(&env, repo.path(), &name);

    let mut cmd = Command::new("bash");
    env.apply(&mut cmd);
    let out = cmd
        .arg(&sweep)
        .arg("snapshot")
        .current_dir(repo.path())
        .output()
        .expect("run sweep.sh snapshot");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(
        stdout.contains("=== pane 1 ==="),
        "the coding pane in window 0 must be discovered with the broker window \
         selected; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("=== pane 0 ==="),
        "the dashboard pane is not a coding pane; stdout:\n{stdout}"
    );
    assert!(
        !stderr.contains("no coding panes discovered"),
        "stderr:\n{stderr}"
    );
}
