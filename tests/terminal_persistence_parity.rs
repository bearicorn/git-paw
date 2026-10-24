//! Persisted surface handles stay backward compatible (`code-architecture`,
//! "Persisted surface handles stay backward compatible"), and the bundled
//! `sweep.sh` owner matcher agrees with the core `resolve_agent` on surfaces.
//!
//! The fixtures under `tests/fixtures/terminal_port/` are a `Session` and a
//! per-repo discovery file in the exact bytes the pre-refactor binary wrote
//! (`src/session.rs` serialisation is untouched by the terminal port); the
//! terminal port's tmux adapter encodes a surface as the `pane_index` /
//! `dashboard_pane` integers they carry.

use std::path::{Path, PathBuf};
use std::process::Command;

use git_paw::session::{
    RepoSessionFile, load_session_from, read_repo_session_file, save_session_in,
    write_repo_session_file,
};
use git_paw::surface::resolve_agent;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/terminal_port")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("read fixture");
    // The files end in a newline for the editor; the writers emit none.
    text.strip_suffix('\n').unwrap_or(&text).to_string()
}

/// Scenario "Session files round-trip byte-identically" — the user-data
/// `Session`, `dashboard_pane` included.
#[test]
fn a_saved_session_round_trips_byte_identically() {
    let original = fixture("session.json");
    let src = tempfile::TempDir::new().unwrap();
    std::fs::write(src.path().join("paw-proj.json"), &original).unwrap();
    let session = load_session_from("paw-proj", src.path())
        .unwrap()
        .expect("the pre-refactor session loads");
    assert_eq!(session.dashboard_pane, Some(0));

    let dst = tempfile::TempDir::new().unwrap();
    save_session_in(&session, dst.path()).unwrap();
    let resaved = std::fs::read_to_string(dst.path().join("paw-proj.json")).unwrap();
    assert_eq!(resaved, original);
}

/// Scenario "Session files round-trip byte-identically" — the per-repo
/// discovery file, `pane_index` included.
#[test]
fn a_discovery_file_round_trips_byte_identically() {
    let original = fixture("repo_session.json");
    let repo = tempfile::TempDir::new().unwrap();
    let dir = repo.path().join(".git-paw/sessions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("paw-proj.json"), &original).unwrap();

    let file = read_repo_session_file(repo.path(), "paw-proj")
        .unwrap()
        .expect("the pre-refactor discovery file loads");
    let indices: Vec<usize> = file.agents.iter().map(|a| a.pane_index).collect();
    assert_eq!(indices, vec![1, 2]);

    write_repo_session_file(repo.path(), &file).unwrap();
    let resaved = std::fs::read_to_string(dir.join("paw-proj.json")).unwrap();
    assert_eq!(resaved, original);
}

/// Runs `sweep.sh`'s `resolve_agent_for_path` — extracted verbatim from the
/// bundled script — against `session_file` for `path`.
fn sweep_resolve(session_file: &Path, path: &Path) -> String {
    let script = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh"),
    )
    .unwrap();
    let start = script
        .find("resolve_agent_for_path() {")
        .expect("sweep.sh defines resolve_agent_for_path");
    let end = start + script[start..].find("\n}\n").expect("function ends") + 3;
    let harness = format!(
        "PY=python3\nSESSION_FILE=\"$1\"\n{}\nresolve_agent_for_path \"$2\"\n",
        &script[start..end]
    );
    let out = Command::new("bash")
        .args(["-c", &harness, "harness"])
        .arg(session_file)
        .arg(path)
        .output()
        .expect("run bash");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

/// Scenario "sweep.sh discovery contract is unchanged", plus the parity the
/// change pins: for an owner equal to a worktree root (the surface case),
/// `sweep.sh` and the core rule name the same agent. Below a root they
/// deliberately differ — `sweep.sh` maps arbitrary paths by longest realpath
/// prefix, the core rule matches surfaces exactly.
#[test]
fn sweep_sh_and_the_core_rule_name_the_same_agent_for_a_surface_owner() {
    let repo = tempfile::TempDir::new().unwrap();
    let wt = |x: &str| -> PathBuf {
        let p = repo.path().join(format!(".git-paw/worktrees/feat-{x}"));
        std::fs::create_dir_all(p.join("src")).unwrap();
        p
    };
    let (a, b) = (wt("a"), wt("b"));
    let mut file: RepoSessionFile = serde_json::from_str(&fixture("repo_session.json")).unwrap();
    file.agents[0].worktree_path.clone_from(&a);
    file.agents[1].worktree_path.clone_from(&b);
    write_repo_session_file(repo.path(), &file).unwrap();
    let discovery = repo.path().join(".git-paw/sessions/paw-proj.json");

    let roster: Vec<(&str, &Path)> = file
        .agents
        .iter()
        .map(|e| (e.branch_id.as_str(), e.worktree_path.as_path()))
        .collect();

    for (owner, agent) in [(&a, "feat-a"), (&b, "feat-b")] {
        assert_eq!(sweep_resolve(&discovery, owner), agent);
        assert_eq!(resolve_agent(owner, roster.iter().copied()), Some(agent));
    }

    let below = a.join("src");
    assert_eq!(
        sweep_resolve(&discovery, &below),
        "feat-a",
        "sweep.sh: prefix"
    );
    assert_eq!(
        resolve_agent(&below, roster.iter().copied()),
        None,
        "core: exact surfaces only"
    );
}

/// `sweep.sh` still parses after the documentation comment.
#[test]
fn sweep_sh_is_syntactically_valid() {
    let status = Command::new("bash")
        .args(["-n"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh"))
        .status()
        .unwrap();
    assert!(status.success());
}
