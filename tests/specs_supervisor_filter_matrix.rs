//! Regression matrix for the `cli-specs-supervisor-filter` capability.
//!
//! Pins the v0.6.0 dogfood fix: `git paw start --supervisor --specs <list>`
//! launches ONLY the named subset, matching the non-supervisor `--specs`
//! path. The matrix exercises every combination of selection flag and
//! supervisor state (design D2) so any future regression in one cell fails
//! that cell's assertion with the offending combination named.
//!
//! Assertions run against `--dry-run`, whose plan lists exactly the worktrees
//! that would be created (one `spec/<id>` line per launched spec/branch). The
//! dry-run plan is the worktree set: a `--dry-run` plan listing `spec/alpha`
//! and `spec/beta` is a guarantee that a real launch creates exactly those
//! two worktrees. Picker cells (bare `--specs`) and bare-selection cells
//! require an interactive TTY; `assert_cmd` runs the binary non-TTY, so those
//! cells assert the TTY-gated behaviour (no automatic discovery of every
//! spec) rather than a worktree set.

use std::collections::BTreeSet;
use std::fs;

use assert_cmd::Command;

mod helpers;
use helpers::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// The four committed specs every matrix repo carries.
const SPEC_IDS: [&str; 4] = ["alpha", "beta", "gamma", "delta"];

/// Writes a `.git-paw/config.toml` with the `OpenSpec` backend (scanning
/// `specs/`), broker enabled, and an `echo` CLI. Deliberately omits any
/// `[supervisor]` section so supervisor mode is governed purely by the
/// `--supervisor` / `--no-supervisor` flag in each matrix cell (the
/// dry-run resolution short-circuits the config-absent prompt to "off").
fn write_matrix_config(repo: &std::path::Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    let config = r#"
default_cli = "echo"

[specs]
type = "openspec"
dir = "specs"

[broker]
enabled = true
port = 19219

[clis.echo]
command = "echo"
display_name = "Echo"
"#;
    fs::write(paw_dir.join("config.toml"), config).expect("write config");
}

/// Writes and commits a pending `OpenSpec` change at `specs/<id>/tasks.md`.
fn write_committed_spec(repo: &std::path::Path, id: &str) {
    let change_dir = repo.join("specs").join(id);
    fs::create_dir_all(&change_dir).expect("create change dir");
    fs::write(change_dir.join("tasks.md"), format!("- [ ] task for {id}")).expect("write tasks.md");
    std::process::Command::new("git")
        .current_dir(repo)
        .args(["add", "."])
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .current_dir(repo)
        .args(["commit", "-m", "add spec"])
        .output()
        .expect("git commit");
}

/// A matrix repo with all four specs committed and the matrix config written.
fn matrix_repo() -> TestRepo {
    let tr = setup_test_repo();
    write_matrix_config(tr.path());
    for id in SPEC_IDS {
        write_committed_spec(tr.path(), id);
    }
    tr
}

/// Runs a `--dry-run` start with the given args and returns (success, stdout).
fn run_dry(repo: &std::path::Path, args: &[&str]) -> (bool, String) {
    let mut full = vec!["start", "--dry-run", "--cli", "echo"];
    full.extend_from_slice(args);
    let output = cmd()
        .current_dir(repo)
        .args(&full)
        .output()
        .expect("run start --dry-run");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

/// Extracts the set of spec ids whose `spec/<id>` branch appears in a dry-run
/// plan (restricted to the four matrix specs).
fn branch_set(stdout: &str) -> BTreeSet<String> {
    SPEC_IDS
        .iter()
        .filter(|id| stdout.contains(&format!("spec/{id}")))
        .map(|id| (*id).to_string())
        .collect()
}

fn set_of(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| (*s).to_string()).collect()
}

// ---------------------------------------------------------------------------
// Row: --from-all-specs (launch every discovered spec) × supervisor states
// ---------------------------------------------------------------------------

#[test]
fn from_all_specs_launches_all_no_supervisor() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--from-all-specs"]);
    assert!(ok, "from-all-specs/no-supervisor should succeed: {out}");
    assert_eq!(
        branch_set(&out),
        set_of(&SPEC_IDS),
        "from-all-specs/no-supervisor cell: expected all four specs"
    );
}

#[test]
fn from_all_specs_launches_all_with_supervisor() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--from-all-specs", "--supervisor"]);
    assert!(ok, "from-all-specs/supervisor should succeed: {out}");
    assert_eq!(
        branch_set(&out),
        set_of(&SPEC_IDS),
        "from-all-specs/supervisor cell: expected all four specs (current behaviour preserved)"
    );
}

#[test]
fn from_all_specs_launches_all_no_supervisor_flag() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--from-all-specs", "--no-supervisor"]);
    assert!(ok, "from-all-specs/--no-supervisor should succeed: {out}");
    assert_eq!(
        branch_set(&out),
        set_of(&SPEC_IDS),
        "from-all-specs/--no-supervisor cell: expected all four specs"
    );
}

// ---------------------------------------------------------------------------
// Row: --specs <names> (named subset) × supervisor states — the regression
// ---------------------------------------------------------------------------

#[test]
fn specs_named_subset_no_supervisor() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--specs", "alpha,beta"]);
    assert!(ok, "specs-named/no-supervisor should succeed: {out}");
    assert_eq!(
        branch_set(&out),
        set_of(&["alpha", "beta"]),
        "specs-named/no-supervisor cell: expected exactly the named subset"
    );
}

#[test]
fn specs_named_subset_with_supervisor() {
    // The v0.6.0 dogfood bug: this cell used to launch ALL discovered specs.
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--specs", "alpha,beta", "--supervisor"]);
    assert!(ok, "specs-named/supervisor should succeed: {out}");
    assert_eq!(
        branch_set(&out),
        set_of(&["alpha", "beta"]),
        "specs-named/supervisor cell (THE FIX): expected exactly {{alpha,beta}}, \
         got {:?} — a regression here means --supervisor --specs <list> is \
         ignoring the named subset again",
        branch_set(&out)
    );
}

#[test]
fn specs_named_subset_no_supervisor_flag() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--specs", "alpha,beta", "--no-supervisor"]);
    assert!(ok, "specs-named/--no-supervisor should succeed: {out}");
    assert_eq!(
        branch_set(&out),
        set_of(&["alpha", "beta"]),
        "specs-named/--no-supervisor cell: expected exactly the named subset"
    );
}

#[test]
fn specs_single_named_with_supervisor() {
    // Spec scenario: `--supervisor --specs cold-start-ci-parity` → one worktree.
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--specs", "gamma", "--supervisor"]);
    assert!(ok, "specs-single/supervisor should succeed: {out}");
    assert_eq!(
        branch_set(&out),
        set_of(&["gamma"]),
        "specs-single/supervisor cell: expected exactly {{gamma}}"
    );
}

// ---------------------------------------------------------------------------
// Row: --branches <list> × supervisor states
// ---------------------------------------------------------------------------

#[test]
fn branches_explicit_no_supervisor() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--branches", "spec/alpha"]);
    assert!(ok, "branches/no-supervisor should succeed: {out}");
    assert_eq!(branch_set(&out), set_of(&["alpha"]));
}

#[test]
fn branches_explicit_with_supervisor() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--branches", "spec/alpha", "--supervisor"]);
    assert!(ok, "branches/supervisor should succeed: {out}");
    assert_eq!(branch_set(&out), set_of(&["alpha"]));
}

#[test]
fn branches_explicit_no_supervisor_flag() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--branches", "spec/alpha", "--no-supervisor"]);
    assert!(ok, "branches/--no-supervisor should succeed: {out}");
    assert_eq!(branch_set(&out), set_of(&["alpha"]));
}

// ---------------------------------------------------------------------------
// Row: bare --specs (picker) × supervisor states — TTY-gated
// ---------------------------------------------------------------------------

#[test]
fn picker_requires_tty_no_supervisor() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--specs"]);
    assert!(
        !ok,
        "picker/no-supervisor in a non-TTY should fail, not auto-launch: {out}"
    );
}

#[test]
fn picker_requires_tty_with_supervisor() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--specs", "--supervisor"]);
    assert!(
        !ok,
        "picker/supervisor in a non-TTY should fail, not auto-launch every spec: {out}"
    );
}

#[test]
fn picker_requires_tty_no_supervisor_flag() {
    let tr = matrix_repo();
    let (ok, out) = run_dry(tr.path(), &["--specs", "--no-supervisor"]);
    assert!(
        !ok,
        "picker/--no-supervisor in a non-TTY should fail, not auto-launch: {out}"
    );
}

// ---------------------------------------------------------------------------
// Row: no selection flag × supervisor states — branch picker, no spec
// auto-discovery (the explicit spec requirement for bare --supervisor)
// ---------------------------------------------------------------------------

#[test]
fn bare_no_flag_does_not_autodiscover_specs_no_supervisor() {
    let tr = matrix_repo();
    let (_ok, out) = run_dry(tr.path(), &[]);
    assert_ne!(
        branch_set(&out),
        set_of(&SPEC_IDS),
        "bare/no-supervisor must not auto-discover every spec"
    );
}

#[test]
fn bare_supervisor_uses_branch_picker_not_spec_discovery() {
    // Spec scenario: `--supervisor` with neither --specs nor --from-all-specs
    // behaves like `git paw start` (branch picker) — NOT auto-discovery of
    // every spec. In a non-TTY this surfaces as the picker failing rather
    // than the all-specs plan being printed.
    let tr = matrix_repo();
    let (_ok, out) = run_dry(tr.path(), &["--supervisor"]);
    assert_ne!(
        branch_set(&out),
        set_of(&SPEC_IDS),
        "bare --supervisor must NOT auto-discover every spec (it uses the branch picker)"
    );
}

#[test]
fn bare_no_flag_does_not_autodiscover_specs_no_supervisor_flag() {
    let tr = matrix_repo();
    let (_ok, out) = run_dry(tr.path(), &["--no-supervisor"]);
    assert_ne!(
        branch_set(&out),
        set_of(&SPEC_IDS),
        "bare/--no-supervisor must not auto-discover every spec"
    );
}

// ---------------------------------------------------------------------------
// Backend-agnostic: the same `--supervisor --specs <name>` filter narrows to
// exactly the named subset on the Markdown and Spec Kit backends, matching the
// OpenSpec rows above (capability requirement "Stack-agnostic and
// backend-agnostic"). The fix lives in the shared dispatcher/apply_spec_mode
// path, so no backend-specific code is exercised.
// ---------------------------------------------------------------------------

/// `.git-paw/config.toml` for the Markdown backend scanning `specs/*.md`.
fn write_markdown_config(repo: &std::path::Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"echo\"\n\n[specs]\ntype = \"markdown\"\ndir = \"specs\"\n\n\
         [broker]\nenabled = true\nport = 19220\n\n\
         [clis.echo]\ncommand = \"echo\"\ndisplay_name = \"Echo\"\n",
    )
    .expect("write config");
}

/// Commits a pending Markdown spec at `specs/<id>.md` (frontmatter drives the
/// id; default `branch_prefix` makes the branch `spec/<id>`).
fn write_committed_markdown_spec(repo: &std::path::Path, id: &str) {
    let specs = repo.join("specs");
    fs::create_dir_all(&specs).expect("create specs dir");
    fs::write(
        specs.join(format!("{id}.md")),
        format!("---\npaw_status: pending\npaw_branch: {id}\n---\nMarkdown spec body for {id}.\n"),
    )
    .expect("write md spec");
    for args in [&["add", "."][..], &["commit", "-m", "add md spec"][..]] {
        std::process::Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .expect("git");
    }
}

#[test]
fn markdown_backend_supervisor_specs_filters_to_subset() {
    let tr = setup_test_repo();
    write_markdown_config(tr.path());
    for id in SPEC_IDS {
        write_committed_markdown_spec(tr.path(), id);
    }

    let (ok, out) = run_dry(tr.path(), &["--supervisor", "--specs", "alpha"]);
    assert!(ok, "markdown dry-run should succeed; got:\n{out}");
    assert_eq!(
        branch_set(&out),
        set_of(&["alpha"]),
        "Markdown backend: --supervisor --specs alpha must launch exactly one worktree"
    );
}

/// `.git-paw/config.toml` for the Spec Kit backend scanning `.specify/specs/`.
fn write_speckit_config(repo: &std::path::Path) {
    let paw_dir = repo.join(".git-paw");
    fs::create_dir_all(&paw_dir).expect("create .git-paw");
    fs::write(
        paw_dir.join("config.toml"),
        "default_cli = \"echo\"\n\n[specs]\ntype = \"speckit\"\ndir = \".specify/specs\"\n\n\
         [broker]\nenabled = true\nport = 19221\n\n\
         [clis.echo]\ncommand = \"echo\"\ndisplay_name = \"Echo\"\n",
    )
    .expect("write config");
}

/// Commits a Spec Kit feature with a single sequential phase, so it decomposes
/// to exactly one consolidated entry (branch `phase/<feature>-...`).
fn write_committed_speckit_feature(repo: &std::path::Path, feature: &str) {
    let dir = repo.join(".specify/specs").join(feature);
    fs::create_dir_all(&dir).expect("create feature dir");
    fs::write(dir.join("spec.md"), format!("{feature} spec.")).expect("spec.md");
    fs::write(
        dir.join("tasks.md"),
        "## Phase 1: Build\n- [ ] T001 Do the work\n- [ ] T002 Finish the work\n",
    )
    .expect("tasks.md");
    for args in [&["add", "."][..], &["commit", "-m", "add feature"][..]] {
        std::process::Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .expect("git");
    }
}

#[test]
fn speckit_backend_supervisor_specs_filters_to_subset() {
    let tr = setup_test_repo();
    write_speckit_config(tr.path());
    // Numbered feature dirs, mirroring the Spec Kit convention; each yields one
    // consolidated entry (branch `phase/<feature>-build`).
    let features = ["001-alpha", "002-beta", "003-gamma"];
    for f in features {
        write_committed_speckit_feature(tr.path(), f);
    }

    let (ok, out) = run_dry(tr.path(), &["--supervisor", "--specs", "003-gamma"]);
    assert!(ok, "speckit dry-run should succeed; got:\n{out}");
    // Exactly the requested feature is launched; the others are excluded.
    assert!(
        out.contains("003-gamma") || out.contains("gamma"),
        "Spec Kit backend: --specs 003-gamma must launch that feature;\n{out}"
    );
    for excluded in ["001-alpha", "002-beta", "alpha", "beta"] {
        assert!(
            !out.contains(excluded),
            "Spec Kit backend: feature '{excluded}' must NOT be launched (filter leaked);\n{out}"
        );
    }
}
