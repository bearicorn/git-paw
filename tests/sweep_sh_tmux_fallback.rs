//! Asserts the bundled `sweep.sh` resolves the session name from the live
//! tmux session (`$TMUX` / `tmux display-message -p '#S'`) when no per-repo
//! `.git-paw/sessions/*.json` discovery file is present — capability
//! `session-json-location`, scenario "Helper discovers the session with no
//! JSON present".
//!
//! The test runs entirely on a DEDICATED tmux socket (`tmux -L <socket>`) so
//! it is fully isolated from the default-socket session this dogfood runs in:
//! a process spawned inside that session inherits `$TMUX` pointing at the
//! dedicated server, and sweep.sh's bare `tmux display-message` follows
//! `$TMUX`, so it never touches the default socket.

use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;
use std::time::{Duration, Instant};

fn tmux_available() -> bool {
    StdCommand::new("tmux")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn init_git_repo(dir: &Path) {
    let run = |args: &[&str]| {
        StdCommand::new("git")
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

/// Copies the bundled sweep.sh asset into `<repo>/.git-paw/scripts/`.
fn install_sweep(repo: &Path) -> std::path::PathBuf {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh");
    let dst_dir = repo.join(".git-paw/scripts");
    fs::create_dir_all(&dst_dir).expect("mk scripts dir");
    let dst = dst_dir.join("sweep.sh");
    fs::copy(&src, &dst).expect("copy sweep.sh");
    dst
}

/// Writes a per-repo discovery JSON, optionally carrying an unknown extra
/// top-level field to model a forward-compatible schema addition.
fn write_repo_json(repo: &Path, session_name: &str, extra_field: bool) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let extra = if extra_field {
        ",\n  \"future_field\": {\"nested\": true}"
    } else {
        ""
    };
    let body = format!(
        "{{\n  \"session_name\": \"{session_name}\",\n  \"agents\": [\n    \
         {{\"branch_id\": \"feat-x\", \"worktree_path\": \"/wt/x\", \"cli\": \"claude\", \
         \"pane_index\": 2}}\n  ]{extra}\n}}"
    );
    fs::write(dir.join(format!("{session_name}.json")), body).expect("write repo json");
}

/// Runs sweep.sh with no subcommand (the usage path needs no broker or tmux)
/// and returns the combined stdout+stderr, which includes the discovered
/// `session:` line.
fn sweep_usage_output(repo: &Path, sweep: &Path) -> String {
    let out = StdCommand::new("bash")
        .arg(sweep)
        .current_dir(repo)
        .env_remove("TMUX")
        .output()
        .expect("run sweep.sh");
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn sweep_sh_reads_documented_fields_and_ignores_unknown_extra() {
    // Forward-compat: a future field on the per-repo JSON must not stop the
    // current sweep.sh from resolving the documented `session_name`.
    let repo = tempfile::TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    write_repo_json(repo.path(), "paw-extrafield", true);

    let out = sweep_usage_output(repo.path(), &sweep);
    assert!(
        out.contains("paw-extrafield"),
        "sweep.sh must resolve session_name despite an unknown extra field;\n{out}"
    );
}

#[test]
fn sweep_sh_prefers_per_repo_json_over_live_tmux_session() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }
    let repo = tempfile::TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    // Both a per-repo JSON (session "paw-fromjson") AND a live tmux session
    // (named differently) are present — the JSON must win.
    write_repo_json(repo.path(), "paw-fromjson", false);

    let socket = "git-paw-sweep-precedence-test";
    let session = "livetmuxsession";
    let out_file = repo.path().join("sweep-usage.out");
    let inner = format!(
        "cd '{repo}' && bash '{sweep}' > '{out}' 2>&1",
        repo = repo.path().display(),
        sweep = sweep.display(),
        out = out_file.display(),
    );
    let status = StdCommand::new("tmux")
        .args([
            "-L",
            socket,
            "new-session",
            "-d",
            "-s",
            session,
            "-x",
            "200",
            "-y",
            "50",
            "bash",
            "-lc",
            &inner,
        ])
        .status()
        .expect("tmux new-session on dedicated socket");
    assert!(status.success(), "dedicated-socket session should start");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut contents = String::new();
    while Instant::now() < deadline {
        if let Ok(s) = fs::read_to_string(&out_file)
            && s.contains("session:")
        {
            contents = s;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = StdCommand::new("tmux")
        .args(["-L", socket, "kill-server"])
        .status();

    assert!(
        contents.contains("paw-fromjson"),
        "the per-repo JSON's session_name must take precedence over the live tmux session;\n{contents}"
    );
    assert!(
        !contents.contains(session),
        "must NOT fall back to the live tmux session name when the JSON is present;\n{contents}"
    );
}

#[test]
fn sweep_sh_falls_back_to_live_tmux_session_name() {
    if !tmux_available() {
        eprintln!("skipping: tmux not available");
        return;
    }

    let repo = tempfile::TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());

    // Deliberately NO .git-paw/sessions/*.json — force the $TMUX fallback.
    assert!(
        !repo.path().join(".git-paw/sessions").exists(),
        "test must run with no per-repo discovery JSON present"
    );

    // Dedicated socket + session name, isolated from the default socket.
    let socket = "git-paw-sweep-fallback-test";
    let session = "sweepfallbacksess";
    let out_file = repo.path().join("sweep-usage.out");

    // Run sweep.sh (usage path — needs no broker) INSIDE the dedicated
    // session so it inherits $TMUX. `usage` prints "session: <name>" derived
    // from discover_session_name, which must resolve via the tmux fallback.
    let inner = format!(
        "cd '{repo}' && bash '{sweep}' > '{out}' 2>&1",
        repo = repo.path().display(),
        sweep = sweep.display(),
        out = out_file.display(),
    );
    let status = StdCommand::new("tmux")
        .args([
            "-L",
            socket,
            "new-session",
            "-d",
            "-s",
            session,
            "-x",
            "200",
            "-y",
            "50",
            "bash",
            "-lc",
            &inner,
        ])
        .status()
        .expect("tmux new-session on dedicated socket");
    assert!(status.success(), "dedicated-socket session should start");

    // Poll for the output file the in-session command writes.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut contents = String::new();
    while Instant::now() < deadline {
        if let Ok(s) = fs::read_to_string(&out_file)
            && s.contains("session:")
        {
            contents = s;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Tear down the dedicated server regardless of assertion outcome.
    let _ = StdCommand::new("tmux")
        .args(["-L", socket, "kill-server"])
        .status();

    assert!(
        contents.contains(&format!("session:        {session}"))
            || contents.contains(&format!("session: {session}")),
        "sweep.sh should resolve the session name via the $TMUX fallback; got:\n{contents}"
    );
}
