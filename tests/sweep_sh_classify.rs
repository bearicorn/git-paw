//! Fixture tests for the `classify` subcommand of the bundled
//! `<repo>/.git-paw/scripts/sweep.sh`, verifying parity with the Rust
//! auto-approve classifier (`src/supervisor/auto_approve.rs`).
//!
//! Each test runs `git paw init` (which writes the helper), then pipes a
//! scripted pane capture into `sweep.sh classify` and asserts the printed
//! decision. Coverage mirrors the four §8.2 cases: a danger pattern escalates,
//! the rm -rf scratch exception approves, a worktree-confined `git commit`
//! pre-approves, and a non-live capture is a no-op.
//!
//! Maps to openspec/changes/auto-approve-classifier/tasks.md §8.

use std::fs;
use std::io::Write;
use std::process::{Command as StdCommand, Stdio};
use std::time::Duration;

use assert_cmd::Command;
use serial_test::serial;
use tempfile::TempDir;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

fn init_git_repo(dir: &std::path::Path) {
    for args in [
        &["init", "-b", "main"][..],
        &["config", "user.email", "test@test.com"][..],
        &["config", "user.name", "Test"][..],
    ] {
        let st = StdCommand::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("git");
        assert!(st.success());
    }
    fs::write(dir.join("README.md"), "# test").expect("write readme");
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["add", "."])
        .status();
    let _ = StdCommand::new("git")
        .current_dir(dir)
        .args(["commit", "-m", "initial"])
        .status();
}

struct Fixture {
    _tmp: TempDir,
    sweep: std::path::PathBuf,
    root: std::path::PathBuf,
}

fn setup() -> Fixture {
    let tmp = TempDir::new().expect("tempdir");
    init_git_repo(tmp.path());
    let init_out = cmd()
        .current_dir(tmp.path())
        .arg("init")
        .timeout(Duration::from_secs(10))
        .output()
        .expect("git paw init");
    assert!(init_out.status.success(), "git paw init must succeed");
    let sweep = tmp.path().join(".git-paw/scripts/sweep.sh");
    assert!(sweep.exists(), "init must write sweep.sh");
    let root = tmp.path().to_path_buf();
    Fixture {
        _tmp: tmp,
        sweep,
        root,
    }
}

fn classify(fx: &Fixture, capture: &str, root_arg: Option<&str>) -> String {
    let mut c = StdCommand::new("bash");
    c.arg(&fx.sweep).arg("classify");
    if let Some(r) = root_arg {
        c.arg(r);
    }
    let mut child = c
        .current_dir(&fx.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn sweep.sh classify");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(capture.as_bytes())
        .unwrap();
    let out = child.wait_with_output().expect("wait");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
#[serial]
fn danger_pattern_escalates() {
    let fx = setup();
    let out = classify(
        &fx,
        "Bash command\n  git push --force origin main\nDo you want to proceed?\nEsc to cancel",
        None,
    );
    assert!(
        out.contains("escalate") && out.contains("danger"),
        "force-push must escalate, got: {out}"
    );
}

#[test]
#[serial]
fn scratch_rm_approves() {
    let fx = setup();
    let out = classify(
        &fx,
        "Bash command\n  rm -rf /tmp/paw-build-1\nDo you want to proceed?\nEsc to cancel",
        None,
    );
    assert!(
        out.contains("approve") && out.contains("scratch-rm"),
        "scratch delete must approve, got: {out}"
    );
}

#[test]
#[serial]
fn worktree_commit_approves() {
    let fx = setup();
    let root = fx.root.to_string_lossy().to_string();
    let out = classify(
        &fx,
        "Bash command\n  git commit -m \"feat: x\"\nDo you want to proceed?\nEsc to cancel",
        Some(&root),
    );
    assert!(
        out.contains("approve") && out.contains("worktree-git"),
        "worktree-confined commit must approve, got: {out}"
    );
}

#[test]
#[serial]
fn non_live_capture_is_noop() {
    let fx = setup();
    let out = classify(&fx, "I might run cargo test later\njust narration", None);
    assert!(
        out.contains("no-op") && out.contains("not live"),
        "non-live capture must be a no-op, got: {out}"
    );
}

/// Appends a raw TOML snippet to the fixture repo's `.git-paw/config.toml`.
fn append_config(fx: &Fixture, snippet: &str) {
    use std::io::Write as _;
    let path = fx.root.join(".git-paw/config.toml");
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("open config.toml for append");
    writeln!(f, "{snippet}").expect("append config snippet");
}

// --- stack-driven composition (classifier-stack-de-opinionation) ---

/// Spec scenario "sweep.sh composes the same whitelist": with
/// `stacks = ["rust"]` declared in `.git-paw/config.toml`, a `cargo fmt`
/// prompt approves via the whitelist — agreeing with the Rust classifier.
#[test]
#[serial]
fn declared_rust_stack_approves_cargo_via_config() {
    let fx = setup();
    append_config(
        &fx,
        "\n[supervisor.common_dev_allowlist]\nstacks = [\"rust\"]\n",
    );
    let out = classify(
        &fx,
        "Bash command\n  cargo fmt --check\nDo you want to proceed?\nEsc to cancel",
        None,
    );
    assert!(
        out.contains("approve") && out.contains("whitelist"),
        "declared rust stack must approve cargo fmt, got: {out}"
    );
}

/// Spec scenario "Default whitelist is stack-neutral" through the helper:
/// without a declared stack, a cargo prompt escalates as unknown.
#[test]
#[serial]
fn undeclared_stack_cargo_escalates_unknown() {
    let fx = setup();
    let out = classify(
        &fx,
        "Bash command\n  cargo test --workspace\nDo you want to proceed?\nEsc to cancel",
        None,
    );
    assert!(
        out.contains("escalate") && out.contains("unknown"),
        "cargo without a declared stack must escalate, got: {out}"
    );
}

/// The third whitelist source: `[supervisor.auto_approve] safe_commands`
/// entries read from config.toml extend the helper's whitelist.
#[test]
#[serial]
fn safe_commands_extension_approves_via_config() {
    let fx = setup();
    append_config(
        &fx,
        "\n[supervisor.auto_approve]\nsafe_commands = [\"just smoke\"]\n",
    );
    let out = classify(
        &fx,
        "Bash command\n  just smoke -v\nDo you want to proceed?\nEsc to cancel",
        None,
    );
    assert!(
        out.contains("approve") && out.contains("whitelist"),
        "safe_commands extension must approve, got: {out}"
    );
}

// --- worktree-confined dev-test shapes (rider mirror) ---

/// Rider scenario "bash -n on a worktree script is safe" through the helper.
#[test]
#[serial]
fn worktree_dev_test_bash_n_approves() {
    let fx = setup();
    fs::create_dir_all(fx.root.join("scripts")).expect("mkdir scripts");
    fs::write(fx.root.join("scripts/helper.sh"), "echo hi\n").expect("write helper");
    let root = fx.root.to_string_lossy().to_string();
    let out = classify(
        &fx,
        "Bash command\n  bash -n scripts/helper.sh\nDo you want to proceed?\nEsc to cancel",
        Some(&root),
    );
    assert!(
        out.contains("approve") && out.contains("worktree-dev-test"),
        "bash -n on a worktree script must approve, got: {out}"
    );
}

/// Rider scenario "Inline code strings do not match" through the helper.
#[test]
#[serial]
fn inline_code_string_escalates_unknown() {
    let fx = setup();
    let root = fx.root.to_string_lossy().to_string();
    let out = classify(
        &fx,
        "Bash command\n  python3 -c \"import os\"\nDo you want to proceed?\nEsc to cancel",
        Some(&root),
    );
    assert!(
        out.contains("escalate") && out.contains("unknown"),
        "inline -c code string must escalate, got: {out}"
    );
}

/// Rider scenario "Out-of-worktree script does not match" through the helper.
#[test]
#[serial]
fn out_of_worktree_script_escalates_unknown() {
    let fx = setup();
    let root = fx.root.to_string_lossy().to_string();
    let out = classify(
        &fx,
        "Bash command\n  bash /etc/init.d/thing\nDo you want to proceed?\nEsc to cancel",
        Some(&root),
    );
    assert!(
        out.contains("escalate") && out.contains("unknown"),
        "out-of-worktree script must escalate, got: {out}"
    );
}

// --- protected-path rule (agent-memory-isolation mirror) ---

/// Appends a `[clis.myvariant]` entry whose `settings_path` points into
/// `op_home`, arming the helper's protected-path set from config.
fn append_settings_path_config(fx: &Fixture, op_home: &std::path::Path) {
    let settings = op_home.join(".myvariant/settings.json");
    append_config(
        fx,
        &format!(
            "\n[clis.myvariant]\ncommand = \"myvariant\"\nsettings_path = \"{}\"\n",
            settings.to_string_lossy()
        ),
    );
}

/// Spec scenario "Shell append to a configured settings file escalates"
/// through the helper: the write target inside a configured settings dir
/// escalates as danger even though `echo` is a whitelisted verb.
#[test]
#[serial]
fn protected_settings_append_escalates_danger() {
    let fx = setup();
    let op_home = TempDir::new().expect("op home");
    append_settings_path_config(&fx, op_home.path());
    let settings = op_home.path().join(".myvariant/settings.json");
    let out = classify(
        &fx,
        &format!(
            "Bash command\n  echo '{{}}' >> {}\nDo you want to proceed?\nEsc to cancel",
            settings.to_string_lossy()
        ),
        None,
    );
    assert!(
        out.contains("escalate") && out.contains("danger"),
        "settings append must escalate as danger, got: {out}"
    );
}

/// Spec scenario "Write to operator memory escalates as danger" through the
/// helper: a filesystem write prompt targeting a protected memory subtree
/// escalates.
#[test]
#[serial]
fn protected_memory_write_prompt_escalates_danger() {
    let fx = setup();
    let op_home = TempDir::new().expect("op home");
    let memory = op_home.path().join(".myvariant/projects/-x-repo/memory");
    fs::create_dir_all(&memory).expect("mkdir memory");
    append_settings_path_config(&fx, op_home.path());
    let out = classify(
        &fx,
        &format!(
            "Do you want to allow this write to {}/MEMORY.md?\nEsc to cancel",
            memory.to_string_lossy()
        ),
        None,
    );
    assert!(
        out.contains("escalate") && out.contains("danger"),
        "operator-memory write prompt must escalate as danger, got: {out}"
    );
}

/// Spec scenario "Reads of operator config are not matched by this rule"
/// through the helper: a `cat` of the protected settings file still approves
/// via the read-mostly whitelist.
#[test]
#[serial]
fn protected_config_read_still_approves() {
    let fx = setup();
    let op_home = TempDir::new().expect("op home");
    append_settings_path_config(&fx, op_home.path());
    let settings = op_home.path().join(".myvariant/settings.json");
    let out = classify(
        &fx,
        &format!(
            "Bash command\n  cat {}\nDo you want to proceed?\nEsc to cancel",
            settings.to_string_lossy()
        ),
        None,
    );
    assert!(
        out.contains("approve") && out.contains("whitelist"),
        "read of operator config must stay whitelisted, got: {out}"
    );
}

/// Spec scenarios "Repo-root control dirs are protected for embedded
/// worktrees" + "Path-escape into the protected set is caught" + "In-worktree
/// writes are unaffected" through the helper: from an embedded worktree, a
/// `..`-escape into `.git-paw/` escalates while an in-worktree write approves.
#[test]
#[serial]
fn repo_control_dir_write_from_embedded_worktree_escalates() {
    let fx = setup();
    let worktree = fx.root.join(".git-paw/worktrees/feat-x");
    fs::create_dir_all(&worktree).expect("mkdir embedded worktree");
    let root = worktree.to_string_lossy().to_string();
    let out = classify(
        &fx,
        "Bash command\n  echo x >> ../../config.toml\nDo you want to proceed?\nEsc to cancel",
        Some(&root),
    );
    assert!(
        out.contains("escalate") && out.contains("danger"),
        "..-escape into repo .git-paw must escalate as danger, got: {out}"
    );
    let out = classify(
        &fx,
        "Bash command\n  echo x >> notes.md\nDo you want to proceed?\nEsc to cancel",
        Some(&root),
    );
    assert!(
        out.contains("approve") && out.contains("whitelist"),
        "in-worktree write must stay approved, got: {out}"
    );
}

// --- list-parity guard (Rust ↔ sweep.sh lockstep) ---

/// Extracts the string items of a Python list literal `NAME = [...]` from the
/// sweep.sh source. Items are plain double-quoted command prefixes (no
/// escaped quotes), so splitting on `"` yields the contents at odd indices.
fn extract_py_array(src: &str, name: &str) -> Vec<String> {
    let marker = format!("{name} = [");
    let start = src
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} array not found in sweep.sh"));
    let body = &src[start + marker.len()..];
    let end = body.find(']').expect("closing bracket");
    body[..end]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(String::from)
        .collect()
}

fn to_strings(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

/// Spec scenario "sweep.sh composes the same whitelist" (parity clause): the
/// helper's built-in verb arrays equal the Rust classifier's constants
/// byte-for-byte, closing the audit-flagged behavioural-only gap.
#[test]
fn sweep_sh_verb_lists_match_rust_constants() {
    use git_paw::supervisor::auto_approve::{READ_MOSTLY_VERBS, default_safe_commands};
    use git_paw::supervisor::dev_allowlist::{
        DEV_ALLOWLIST_PRESET, GO_STACK_PRESET, NODE_STACK_PRESET, PYTHON_STACK_PRESET,
        RUST_STACK_PRESET,
    };

    let src = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/scripts/sweep.sh"
    ))
    .expect("read assets/scripts/sweep.sh");

    let read_mostly = extract_py_array(&src, "READ_MOSTLY");
    assert_eq!(
        read_mostly,
        to_strings(READ_MOSTLY_VERBS),
        "sweep.sh READ_MOSTLY must equal READ_MOSTLY_VERBS"
    );

    // EXPLICIT_SAFE + READ_MOSTLY is the helper's built-in whitelist; it must
    // equal the Rust default_safe_commands(), in order.
    let explicit = extract_py_array(&src, "EXPLICIT_SAFE");
    let combined: Vec<String> = explicit.iter().chain(read_mostly.iter()).cloned().collect();
    assert_eq!(
        combined,
        to_strings(default_safe_commands()),
        "sweep.sh EXPLICIT_SAFE + READ_MOSTLY must equal default_safe_commands()"
    );

    assert_eq!(
        extract_py_array(&src, "DEV_UNIVERSAL"),
        to_strings(DEV_ALLOWLIST_PRESET),
        "sweep.sh DEV_UNIVERSAL must equal DEV_ALLOWLIST_PRESET"
    );
    assert_eq!(
        extract_py_array(&src, "STACK_RUST"),
        to_strings(RUST_STACK_PRESET),
        "sweep.sh STACK_RUST must equal RUST_STACK_PRESET"
    );
    assert_eq!(
        extract_py_array(&src, "STACK_NODE"),
        to_strings(NODE_STACK_PRESET),
        "sweep.sh STACK_NODE must equal NODE_STACK_PRESET"
    );
    assert_eq!(
        extract_py_array(&src, "STACK_PYTHON"),
        to_strings(PYTHON_STACK_PRESET),
        "sweep.sh STACK_PYTHON must equal PYTHON_STACK_PRESET"
    );
    assert_eq!(
        extract_py_array(&src, "STACK_GO"),
        to_strings(GO_STACK_PRESET),
        "sweep.sh STACK_GO must equal GO_STACK_PRESET"
    );
}
