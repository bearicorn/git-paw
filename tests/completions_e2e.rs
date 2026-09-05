//! Integration tests for `git paw completions`.
//!
//! Maps to the `cli-parsing` spec scenarios "Completions prints a script for
//! a supported shell" and "Completions rejects an unsupported shell".

use assert_cmd::Command;
use predicates::prelude::*;

fn cmd() -> Command {
    Command::cargo_bin("git-paw").expect("binary exists")
}

#[test]
fn completions_prints_a_nonempty_script_for_each_supported_shell() {
    for shell in ["bash", "zsh", "fish"] {
        let out = cmd()
            .args(["completions", shell])
            .output()
            .unwrap_or_else(|e| panic!("run completions {shell}: {e}"));
        assert!(
            out.status.success(),
            "completions {shell} should exit 0; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            !stdout.trim().is_empty(),
            "completions {shell} should print a non-empty script"
        );
        // Every generated script names the binary somewhere in its body.
        assert!(
            stdout.contains("git-paw"),
            "completions {shell} script should reference git-paw; got: {stdout}"
        );
    }
}

#[test]
fn completions_rejects_an_unsupported_shell_naming_the_supported_ones() {
    cmd()
        .args(["completions", "not-a-real-shell"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("bash")
                .and(predicate::str::contains("zsh"))
                .and(predicate::str::contains("fish")),
        );
}

#[test]
fn completions_requires_a_shell_argument() {
    cmd().args(["completions"]).assert().failure();
}
