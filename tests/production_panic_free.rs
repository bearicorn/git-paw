//! Pins the panic-free production-code convention (`core-project-conventions`)
//! and the binary-crate test gate (`core-ci-hygiene`).
//!
//! Covers:
//! - the crate-root lint attributes and the `clippy.toml` in-test exemption
//!   that make `just lint` enforce "no `unwrap()`/`expect()` in non-test code";
//! - the two clap-guaranteed call sites (`git paw add`, `git paw replay`)
//!   erroring rather than panicking when their branch argument is absent;
//! - the `[[bin]]` target staying testable by default (GP-52).

use std::time::Duration;

use assert_cmd::Command;
use tempfile::TempDir;

mod helpers;
use helpers::*;

const LIB_RS: &str = include_str!("../src/lib.rs");
const MAIN_RS: &str = include_str!("../src/main.rs");
const CARGO_TOML: &str = include_str!("../Cargo.toml");

/// Reads `clippy.toml` at run time — it is read rather than `include_str!`-ed
/// so a missing file is a test failure, not a compile error.
fn clippy_toml() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("clippy.toml");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

/// Returns the crate-level attributes (`#![...]`) declared in `source`,
/// each flattened onto one line so multi-line attributes match too.
fn crate_attributes(source: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut current: Option<String> = None;
    let mut depth = 0usize;

    for line in source.lines() {
        let trimmed = line.trim();
        if current.is_none() {
            if !trimmed.starts_with("#![") {
                continue;
            }
            current = Some(String::new());
        }
        let buf = current.get_or_insert_with(String::new);
        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(trimmed);
        depth += trimmed.matches('[').count();
        depth = depth.saturating_sub(trimmed.matches(']').count());
        if depth == 0 {
            attrs.push(buf.clone());
            current = None;
        }
    }
    attrs
}

fn enables_panic_lints(source: &str) -> bool {
    crate_attributes(source).iter().any(|attr| {
        (attr.starts_with("#![warn(") || attr.starts_with("#![deny("))
            && attr.contains("clippy::unwrap_used")
            && attr.contains("clippy::expect_used")
    })
}

/// `core-project-conventions`: "The lint configuration enforces the rule on
/// both crate roots" — the crate-level attribute half.
#[test]
fn crate_roots_enable_unwrap_and_expect_lints() {
    assert!(
        enables_panic_lints(LIB_RS),
        "src/lib.rs must carry a crate-level #![warn(..)]/#![deny(..)] naming both \
         clippy::unwrap_used and clippy::expect_used; attributes found: {:?}",
        crate_attributes(LIB_RS)
    );
    assert!(
        enables_panic_lints(MAIN_RS),
        "src/main.rs must carry a crate-level #![warn(..)]/#![deny(..)] naming both \
         clippy::unwrap_used and clippy::expect_used; attributes found: {:?}",
        crate_attributes(MAIN_RS)
    );
}

/// `core-project-conventions`: "The lint configuration enforces the rule on
/// both crate roots" — the test-exemption half.
#[test]
fn clippy_toml_exempts_test_code() {
    let contents = clippy_toml();
    let normalized: String = contents.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(
        normalized.contains("allow-unwrap-in-tests=true"),
        "clippy.toml must set allow-unwrap-in-tests = true; got:\n{contents}"
    );
    assert!(
        normalized.contains("allow-expect-in-tests=true"),
        "clippy.toml must set allow-expect-in-tests = true; got:\n{contents}"
    );
}

/// `core-project-conventions`: "The helper is the only expect in non-test
/// code".
///
/// `just lint` cannot police this on its own — an `#[allow]` is exactly what
/// silences the lint — so the suppression set is pinned here instead. A
/// crate-level `#![allow(..)]` would equally defeat the gate, so both forms
/// are checked.
#[test]
fn static_regex_helper_is_the_only_panic_lint_suppression() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders: Vec<String> = Vec::new();

    for path in rust_sources(&src) {
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (n, line) in contents.lines().enumerate() {
            if line.contains("allow(clippy::expect_used)")
                || line.contains("allow(clippy::unwrap_used)")
            {
                let relative = path.strip_prefix(&src).unwrap_or(&path);
                offenders.push(format!("{}:{}", relative.display(), n + 1));
            }
        }
    }

    assert_eq!(
        offenders.len(),
        1,
        "exactly one panic-lint suppression may exist in src/ (the static_regex helper); found: {offenders:?}"
    );
    assert!(
        offenders[0].starts_with("static_regex.rs:"),
        "the only panic-lint suppression must live in src/static_regex.rs; found: {offenders:?}"
    );
}

/// Every `.rs` file under `dir`, recursively.
fn rust_sources(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read dir {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("read dir entry").path();
        if path.is_dir() {
            found.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// `core-ci-hygiene`: "The binary target is not excluded from testing".
#[test]
fn bin_target_is_not_excluded_from_testing() {
    let bin_section = CARGO_TOML
        .split("[[bin]]")
        .nth(1)
        .expect("Cargo.toml declares a [[bin]] target");
    // The target's own keys run until the next table header.
    let bin_keys = bin_section
        .split("\n[")
        .next()
        .expect("split always yields a first element");
    let has_test_false = bin_keys.lines().any(|line| {
        // Drop a trailing comment, then every kind of whitespace, so
        // `test\t=\tfalse` and `test = false  # keep` are caught too.
        let code = line.split('#').next().unwrap_or("");
        let squeezed: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        squeezed == "test=false"
    });
    assert!(
        !has_test_false,
        "the [[bin]] target must not set `test = false` — binary-crate unit tests \
         have to run under a plain `cargo test`; got:\n{bin_keys}"
    );
}

/// `core-project-conventions`: "`git paw add` without a branch or spec errors
/// instead of panicking".
#[test]
fn add_without_branch_or_spec_errors_without_panic() {
    let tr = setup_test_repo();
    let home = TempDir::new().expect("home tempdir");

    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .arg("add")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw add with no arguments");

    assert_no_panic_usage_error(&output, "add");
}

/// `core-project-conventions`: "`git paw replay` without a branch or `--list`
/// errors instead of panicking".
#[test]
fn replay_without_branch_or_list_errors_without_panic() {
    let tr = setup_test_repo();
    let home = TempDir::new().expect("home tempdir");

    let output = cmd()
        .current_dir(tr.path())
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .arg("replay")
        .timeout(Duration::from_secs(30))
        .output()
        .expect("run git paw replay with no arguments");

    assert_no_panic_usage_error(&output, "replay");
}

/// Asserts the observable contract: a clap usage rejection (exit 2 with the
/// missing-argument message and the usage line), and no panic.
///
/// The assertion is deliberately specific. A bare "exited non-zero and stderr
/// mentions an error" check would also pass on an unrelated failure — and on
/// the pre-change code, where the same invocation was rejected by clap before
/// reaching the `expect`.
fn assert_no_panic_usage_error(output: &std::process::Output, subcommand: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    assert_eq!(
        output.status.code(),
        Some(2),
        "`git paw {subcommand}` without its required argument must exit 2 (clap usage error); \
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "`git paw {subcommand}` must not panic; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("required arguments were not provided"),
        "`git paw {subcommand}` must name the missing argument; stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("Usage:"),
        "`git paw {subcommand}` must print the usage line; stderr:\n{stderr}"
    );
}
