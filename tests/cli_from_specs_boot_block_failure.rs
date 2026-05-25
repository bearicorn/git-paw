//! Asserts that boot-block injection failure is non-fatal during
//! `git paw start --from-specs`.
//!
//! Maps to scenario `Boot-block injection failure is non-fatal` from
//! `from-specs-launch-fixes`. (test-coverage-v0-5-0 task 1.1)
//!
//! The shipped implementation runs `tmux send-keys` via
//! `std::process::Command::new("tmux").args(...).status()` and discards
//! the result with `let _ = ...`. Any non-zero exit from the tmux subprocess
//! is therefore non-fatal: the launch flow continues and the user is left
//! with the standard manual-attach hint.
//!
//! Implementing a real-process test would require shimming the `tmux`
//! binary on `PATH` to fail on `send-keys` but succeed on every other
//! sub-command (`new-session`, `split-window`, ...). That harness does not
//! ship in the dev-deps; this test pins the property at the source-code
//! level instead. The `build_boot_inject_args` helper is exercised by the
//! existing `tests/from_specs_launch_fixes_integration.rs` argv tests, so
//! the call shape is covered separately.

const MAIN_RS: &str = include_str!("../src/main.rs");

#[test]
fn boot_block_failure_is_non_fatal() {
    // Locate the boot-block injection call site. `cmd_start_with_specs`
    // delegates to `launch_spec_session` which contains the injection
    // loop; in `cmd_start` the loop is inlined. Both call sites are
    // covered by searching for `build_boot_inject_args` across the file
    // and then scanning the surrounding window for the non-fatal
    // discard pattern.
    let body = MAIN_RS;

    // The boot-block injection path must use `build_boot_inject_args` and
    // discard the tmux status with `let _ = ...`. Find every call site
    // (skipping the doc-comment lines) and assert each one matches the
    // non-fatal shape.
    let mut call_sites: Vec<usize> = Vec::new();
    let mut cursor = 0usize;
    while let Some(found) = body[cursor..].find("git_paw::tmux::build_boot_inject_args(") {
        call_sites.push(cursor + found);
        cursor += found + 1;
    }
    assert!(
        !call_sites.is_empty(),
        "expected at least one build_boot_inject_args call site in main.rs"
    );

    for site in &call_sites {
        let window_start = site.saturating_sub(200);
        let window_end = (*site + 300).min(body.len());
        let window = &body[window_start..window_end];
        assert!(
            window.contains("let _ ="),
            "call site at byte {site} must discard the tmux status; window:\n{window}"
        );
        assert!(
            window.contains("Command::new(\"tmux\")"),
            "call site at byte {site} must spawn tmux via Command::new; window:\n{window}"
        );
        assert!(
            !window.contains(".status()?"),
            "call site at byte {site} must not propagate tmux status; window:\n{window}"
        );
    }
}
