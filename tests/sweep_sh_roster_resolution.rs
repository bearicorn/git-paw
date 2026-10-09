//! Fixture tests for `broker-agent-helper`'s *Bundled helpers read the
//! per-repo discovery file's agent roster* requirement.
//!
//! The bundled `sweep.sh` used to map a pane to its agent by reading
//! `worktrees[].branch` — the shape of the GLOBAL session-state document in
//! the platform data directory, not the per-repo discovery file git-paw
//! actually writes for it (`agents[]` entries carrying `branch_id` /
//! `worktree_path` / `cli` / `pane_index`). Against a real discovery file no
//! pane ever resolved to an agent. Its `status` phantom filter had the
//! matching defect: it recognised only `supervisor` and ids matching
//! `feat[-/]…`, hiding every agent on a differently-prefixed branch (a
//! spec-driven `spec/…` branch, a `fix/…` branch) as a "phantom".
//!
//! These tests drive the bundled asset through its observable subcommands —
//! `detect-stuck` (which names the agent it resolved for each pane) and
//! `status` — against a discovery file written in the real shape.

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

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

/// Copies the bundled `sweep.sh` and the `_paw_common.sh` preamble it sources
/// into `<repo>/.git-paw/scripts/`, mirroring `git paw init`'s co-deployment.
fn install_sweep(repo: &Path) -> PathBuf {
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

/// Writes a per-repo discovery file in the shape `write_repo_discovery_file`
/// produces: a `session_name` plus an `agents` roster of `branch_id` /
/// `worktree_path` / `cli` / `pane_index` entries.
fn write_discovery_file(repo: &Path, session: &str, agents: &[(&str, &Path, usize)]) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let roster: Vec<serde_json::Value> = agents
        .iter()
        .map(|(branch_id, worktree, pane)| {
            serde_json::json!({
                "branch_id": branch_id,
                "worktree_path": worktree.to_string_lossy(),
                "cli": "claude",
                "pane_index": pane,
            })
        })
        .collect();
    let body = serde_json::json!({ "session_name": session, "agents": roster });
    fs::write(
        dir.join(format!("{session}.json")),
        serde_json::to_string_pretty(&body).expect("serialize discovery file"),
    )
    .expect("write discovery file");
}

/// Installs a fake `tmux` into `<repo>/fakebin/` that reports one coding pane
/// (index 1) whose `pane_current_path` is `pane_path`, and prints `capture`
/// for `capture-pane`.
fn install_fake_tmux(repo: &Path, pane_path: &Path, capture: &str) -> PathBuf {
    let bin = repo.join("fakebin");
    fs::create_dir_all(&bin).expect("mk fakebin");
    let tmux = bin.join("tmux");
    let script = format!(
        "#!/bin/sh\n\
         case \"$1\" in\n\
           list-panes) echo '1 bash agentpane' ;;\n\
           display-message) printf '%s\\n' '{path}' ;;\n\
           capture-pane) printf '%s\\n' '{capture}' ;;\n\
         esac\n\
         exit 0\n",
        path = pane_path.display(),
        capture = capture,
    );
    fs::write(&tmux, script).expect("write fake tmux");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmux, fs::Permissions::from_mode(0o755)).expect("chmod fake tmux");
    }
    bin
}

/// Spawns a one-shot HTTP stub answering every GET with `body`.
fn spawn_http_stub(body: &str) -> (u16, Arc<AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let port = listener.local_addr().expect("local addr").port();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_clone = stop.clone();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    thread::spawn(move || {
        listener
            .set_nonblocking(true)
            .expect("non-blocking listener");
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !stop_clone.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            match listener.accept() {
                Ok((mut socket, _)) => {
                    let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
                    let mut buf = [0u8; 2048];
                    let _ = socket.read(&mut buf);
                    let _ = socket.write_all(response.as_bytes());
                    let _ = socket.flush();
                }
                Err(_) => thread::sleep(Duration::from_millis(50)),
            }
        }
    });
    (port, stop)
}

fn write_broker_config(repo: &Path, port: u16) {
    fs::create_dir_all(repo.join(".git-paw")).expect("mk .git-paw");
    fs::write(
        repo.join(".git-paw/config.toml"),
        format!("[broker]\nenabled = true\nport = {port}\n"),
    )
    .expect("write config.toml");
}

/// Spec scenario "A pane path resolves to its agent from the discovery file".
///
/// `detect-stuck` names the agent it resolved for each pane, so a pane whose
/// working directory lies inside the roster's `worktree_path` must produce a
/// line naming that entry's `branch_id`. Before the fix the helper read
/// `worktrees[].branch`, found nothing, and skipped the pane entirely.
#[test]
fn pane_path_resolves_to_its_agent_from_the_discovery_file() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());

    let worktree = repo.path().join(".git-paw/worktrees/feat-x");
    fs::create_dir_all(&worktree).expect("mk worktree dir");
    write_discovery_file(
        repo.path(),
        "paw-roster",
        &[("feat-x", worktree.as_path(), 1)],
    );

    // A pane deeper inside the worktree still resolves to its agent.
    let pane_path = worktree.join("src");
    fs::create_dir_all(&pane_path).expect("mk pane cwd");
    let fakebin = install_fake_tmux(repo.path(), &pane_path, "all quiet");

    // No broker is running: `/status` and `/log` come back empty, which the
    // helper tolerates — the assertion here is purely about pane->agent
    // resolution, which happens before any broker lookup matters.
    write_broker_config(repo.path(), 1);

    let path = format!(
        "{}:{}",
        fakebin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = StdCommand::new("bash")
        .arg(&sweep)
        .arg("detect-stuck")
        .current_dir(repo.path())
        .env("PATH", path)
        .env_remove("TMUX")
        .output()
        .expect("run sweep.sh detect-stuck");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("feat-x"),
        "pane under the roster worktree must resolve to agent feat-x; \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

/// Spec scenario "An agent on a non-feat branch is not a phantom": the roster
/// decides, so a spec-driven `spec-add-auth` agent is shown and only the real
/// phantom `a` is filtered.
#[test]
fn status_shows_a_roster_agent_on_any_branch_prefix() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());

    let worktree = repo.path().join(".git-paw/worktrees/spec-add-auth");
    fs::create_dir_all(&worktree).expect("mk worktree dir");
    write_discovery_file(
        repo.path(),
        "paw-specs",
        &[("spec-add-auth", worktree.as_path(), 1)],
    );

    let body = r#"{"agents":[{"agent_id":"supervisor","status":"working","last_seen_seconds":1},{"agent_id":"spec-add-auth","status":"working","last_seen_seconds":2},{"agent_id":"a","status":"working","last_seen_seconds":3}]}"#;
    let (port, stop) = spawn_http_stub(body);
    write_broker_config(repo.path(), port);

    let out = StdCommand::new("bash")
        .arg(&sweep)
        .arg("status")
        .current_dir(repo.path())
        .env_remove("TMUX")
        .output()
        .expect("run sweep.sh status");
    stop.store(true, Ordering::SeqCst);

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("spec-add-auth"),
        "a roster agent on the spec/ prefix must be shown; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("supervisor"),
        "supervisor must always be shown; stdout:\n{stdout}"
    );
    // The FULL line, not a prefix: `contains` would also pass for
    // "phantoms (use --all to show): a, spec-add-auth".
    let phantom_line = stdout
        .lines()
        .find(|l| l.starts_with("phantoms (use --all to show):"))
        .unwrap_or_else(|| panic!("expected a phantoms line; stdout:\n{stdout}"));
    assert_eq!(
        phantom_line, "phantoms (use --all to show): a",
        "only the true phantom `a` may be listed; stdout:\n{stdout}"
    );
}

/// Spec scenario "Without a discovery file the broker's id rule decides
/// phantoms": `fix-db-timeout` is a well-formed broker agent id and is shown,
/// while the single-letter `a` is not.
#[test]
fn status_without_a_discovery_file_uses_the_broker_id_rule() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    assert!(
        !repo.path().join(".git-paw/sessions").exists(),
        "this test must run with no discovery file present"
    );

    let body = r#"{"agents":[{"agent_id":"supervisor","status":"working","last_seen_seconds":1},{"agent_id":"fix-db-timeout","status":"working","last_seen_seconds":2},{"agent_id":"a","status":"working","last_seen_seconds":3}]}"#;
    let (port, stop) = spawn_http_stub(body);
    write_broker_config(repo.path(), port);

    let out = StdCommand::new("bash")
        .arg(&sweep)
        .arg("status")
        .current_dir(repo.path())
        .env_remove("TMUX")
        .output()
        .expect("run sweep.sh status");
    stop.store(true, Ordering::SeqCst);

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("fix-db-timeout"),
        "a well-formed broker agent id must be shown without a roster; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("supervisor"),
        "supervisor must always be shown; stdout:\n{stdout}"
    );
    let phantom_line = stdout
        .lines()
        .find(|l| l.starts_with("phantoms (use --all to show):"))
        .unwrap_or_else(|| panic!("expected a phantoms line; stdout:\n{stdout}"));
    assert_eq!(
        phantom_line, "phantoms (use --all to show): a",
        "only `a` may be listed as a phantom; stdout:\n{stdout}"
    );
}

/// Spec scenario "No active session is reported explicitly": with no
/// discovery file and no live multiplexer to infer from, a subcommand that
/// needs the session says so and exits non-zero, rather than degrading to an
/// empty result that reads like "nothing to do".
#[test]
fn a_subcommand_needing_the_session_reports_when_none_is_found() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());
    assert!(
        !repo.path().join(".git-paw/sessions").exists(),
        "this test must run with no discovery file present"
    );
    write_broker_config(repo.path(), 1);

    let out = StdCommand::new("bash")
        .arg(&sweep)
        .arg("detect-stuck")
        .current_dir(repo.path())
        .env_remove("TMUX")
        .output()
        .expect("run sweep.sh detect-stuck");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a subcommand that needs the session must not exit 0 when none was \
         found; stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("no session found"),
        "the helper must say no session was found; stderr:\n{stderr}"
    );
}
