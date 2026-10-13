//! Lockstep tests for the live-prompt gate: the bundled `sweep.sh` marker
//! detection must agree with the Rust gate
//! (`git_paw::supervisor::permission_prompt::is_live_prompt`) on shared fixtures
//! (capability `automatic-approval`, scenario "Detection agrees across
//! auto-approver and sweep helper", change `approve-send-gate-hardening`).
//!
//! The shell half is driven through `sweep.sh approve`, which is now the only
//! consumer of the shell gate: a non-live capture makes it report
//! `cleared before send`, and any other outcome means the gate saw a live
//! prompt. The suite needs no real tmux and no broker — a recording fake
//! `tmux` on `$PATH` serves the fixture as the pane capture.
//!
//! `cli-prompt-profiles` capability (D3): since `LIVE_PROMPT_MARKERS_REGEX` /
//! `LIVE_PROMPT_OPTION_REGEX` are now sourced from `git paw __prompt-profile`
//! rather than a hand-copied literal, `sweep_sees_live` puts the freshly
//! built binary on `PATH` — the fixture agreement below is no longer "two
//! independently maintained copies happen to match", it is "the shell
//! subprocess reads the same embedded profile the in-process Rust gate
//! does". `one_source_is_read_by_both_consumers` pins that directly.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;

use git_paw::config::claude_prompt_profile;
use git_paw::supervisor::permission_prompt::is_live_prompt;
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

/// Copies the bundled sweep.sh asset — together with the `_paw_common.sh`
/// preamble it sources — into `<repo>/.git-paw/scripts/`, mirroring the
/// co-deployment `git paw init` guarantees.
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

/// Directory holding the freshly built binary, prepended to the child's
/// `PATH` so `git paw __prompt-profile` resolves to THIS build rather than
/// falling back to sweep.sh's hardcoded defaults.
fn built_bin_dir() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("git-paw")
        .parent()
        .expect("built binary has a parent dir")
        .to_path_buf()
}

/// Writes the per-repo discovery JSON so sweep.sh resolves `session_name`
/// without `$TMUX`.
fn write_session_json(repo: &Path) {
    let dir = repo.join(".git-paw/sessions");
    fs::create_dir_all(&dir).expect("mk sessions dir");
    let body = "{\n  \"session_name\": \"paw-parity\",\n  \"agents\": []\n}";
    fs::write(dir.join("paw-parity.json"), body).expect("write session json");
}

/// Installs a fake `tmux` that serves `$FAKE_TMUX_CAPTURE` for `capture-pane`
/// and swallows everything else, so the gate can be exercised on a fixture
/// with no real tmux server.
fn install_fake_tmux(repo: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = repo.join("fakebin");
    fs::create_dir_all(&bin).expect("mk fakebin");
    let tmux = bin.join("tmux");
    fs::write(
        &tmux,
        "#!/bin/sh\n\
         case \"$1\" in\n\
           capture-pane) cat \"${FAKE_TMUX_CAPTURE}\" ;;\n\
         esac\n\
         exit 0\n",
    )
    .expect("write fake tmux");
    let mut perms = fs::metadata(&tmux).expect("stat fake tmux").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&tmux, perms).expect("chmod fake tmux");
    bin
}

/// Runs the shell half of the live-prompt gate on `capture`: lays it behind a
/// fake tmux, runs `sweep.sh approve`, and returns whether the helper saw a
/// LIVE prompt.
///
/// `approve` is the gate's only remaining consumer, so this drives the
/// production path rather than a test-only entry point. Its guards past the
/// gate (option 1 missing, or option 1 reading as a durable grant) still mean
/// the gate fired — only `cleared before send` means it did not.
fn sweep_sees_live(repo: &Path, sweep: &Path, capture: &str) -> bool {
    let fakebin = install_fake_tmux(repo);
    write_session_json(repo);
    let capture_file = repo.join("parity-capture.txt");
    fs::write(&capture_file, capture).expect("write capture fixture");
    let path = format!(
        "{}:{}:{}",
        fakebin.display(),
        built_bin_dir().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = StdCommand::new("bash")
        .arg(sweep)
        .args(["approve", "2"])
        .current_dir(repo)
        .env("PATH", path)
        .env("FAKE_TMUX_CAPTURE", &capture_file)
        .env_remove("TMUX")
        .env_remove("TMUX_PANE")
        .output()
        .expect("run sweep.sh approve");
    // Each fixture gets a fresh pane claim; the previous run released it on
    // exit, but drop any stale file so a fixture is never skipped as claimed.
    let _ = fs::remove_file(repo.join(".git-paw/tmp/approve-pane-2.claim"));
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !combined.contains("claimed by another approver"),
        "the parity probe must not be blocked by a stale claim: {combined}"
    );
    !combined.contains("cleared before send")
}

/// A bare confirmation question followed by ordinary command output and a
/// shell prompt — no option list, no footer.
fn answered_then_output_fixture() -> String {
    let mut capture = String::from("Do you want to proceed?\n");
    for line in ["ran it", "line b", "line c", "line d"] {
        capture.push_str(line);
        capture.push('\n');
    }
    capture.push_str("$ ");
    capture
}

/// A live multi-option prompt (footer + option list) followed by `n`
/// ordinary trailing lines — pins the exact effective-tolerance boundary
/// (`LIVE_PROMPT_TRAILING_BOUND` + `LIVE_PROMPT_TAIL` - 1 = 13; see the doc
/// comment on `LIVE_PROMPT_TRAILING_BOUND`). 13 is still live, 14 is not.
fn multi_option_prompt_with_trailing_lines(n: usize) -> String {
    let mut capture = String::from("Do you want to proceed?\n❯ 1. Yes\n  2. No\n  Esc to cancel\n");
    for i in 1..=n {
        let _ = writeln!(capture, "output line {i}");
    }
    capture
}

/// Gate-review safety finding: a FULL modal copied verbatim into tool output
/// (e.g. the supervisor's own pane running `sweep.sh capture 2`, echoing
/// another pane's footer + option lines quoted behind the tool-output corner
/// glyph `⎿`) followed by the supervisor's own idle input box.
fn captured_modal_with_idle_input_box_fixture() -> String {
    "$ sweep.sh capture 2\n\
     ⎿ Bash command\n\
     ⎿   rm -rf /tmp/foo\n\
     ⎿ Do you want to proceed?\n\
     ⎿ ❯ 1. Yes\n\
     ⎿   2. No\n\
     ⎿ Esc to cancel\n\
     Done.\n\
     ╭──────────────────────────────────────────────╮\n\
     │ >                                              │\n\
     ╰──────────────────────────────────────────────╯"
        .to_string()
}

/// Same scenario, but with current Claude Code's REAL idle input-box shape —
/// ground-truthed against a live `tmux capture-pane -p`: NO side borders at
/// all, just a rule line, the bare sigil line, another rule line, then the
/// mode strip.
fn captured_modal_with_borderless_idle_input_box_fixture() -> String {
    "$ sweep.sh capture 2\n\
     ⎿ Bash command\n\
     ⎿   rm -rf /tmp/foo\n\
     ⎿ Do you want to proceed?\n\
     ⎿ ❯ 1. Yes\n\
     ⎿   2. No\n\
     ⎿ Esc to cancel\n\
     Done.\n\
     ──────────────────────────────────────────────\n\
     ❯\n\
     ──────────────────────────────────────────────\n\
       ⏵⏵ accept edits on (shift+tab to cycle)"
        .to_string()
}

/// The shared fixture set: `(name, capture, expected_live)`. Every entry is
/// evaluated by BOTH the Rust gate and the sweep.sh mirror, and both must
/// agree with the expectation.
fn fixtures() -> Vec<(&'static str, String, bool)> {
    vec![
        (
            "multi-option block with a don't-ask-again option, inline (esc) only",
            "╭──────────────────────────────────────────────────────────────╮\n\
             │ Bash command                                                 │\n\
             │   cargo test --workspace                                     │\n\
             │   Run the full test suite                                    │\n\
             │ Do you want to proceed?                                      │\n\
             │ ❯ 1. Yes                                                     │\n\
             │   2. Yes, and don't ask again for cargo test in this project │\n\
             │   3. No, and tell Claude what to do differently (esc)        │\n\
             ╰──────────────────────────────────────────────────────────────╯"
                .to_string(),
            true,
        ),
        (
            "multi-option block with an explicit Esc to cancel footer",
            "Do you want to proceed?\n❯ 1. Yes\n  2. Yes, and don't ask again for: git status\n  3. No\n  Esc to cancel".to_string(),
            true,
        ),
        (
            "single prompt with the question at the tail",
            "Bash command\n  git status\nDo you want to proceed?".to_string(),
            true,
        ),
        (
            "prose narration mentioning a safe command",
            "I might run cargo test soon.\nHere is some narration about the plan.\n$ ls -la\ndone.".to_string(),
            false,
        ),
        (
            "numbered list in prose without any prompt marker",
            "Here is my plan:\n1. run the tests\n2. commit the work\n3. stand by for verification".to_string(),
            false,
        ),
        (
            // Original pinned regression fixture — restored to its original
            // size after a gate-review finding: stretching it hid a bug
            // rather than exercised it. No option list anywhere, so the
            // shape-gated extension can never succeed regardless of the
            // footer's distance from the end.
            "footer with no option list anywhere is not a live modal",
            "Do you want to proceed?\n  Esc to cancel\noutput line 1\noutput line 2\noutput line 3\noutput line 4\noutput line 5".to_string(),
            false,
        ),
        (
            "live multi-option prompt with a six-line trailing task-list region",
            "Bash command\n  rm -rf /tmp/paw-build-9\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n  Esc to cancel\n\
             Tasks\n☐ Implement feature\n☐ Write tests\n☐ Update docs\nmain* ⇡2\nSonnet 5.1 · 42% context used"
                .to_string(),
            true,
        ),
        (
            "answered prompt followed by command output is not live",
            answered_then_output_fixture(),
            false,
        ),
        (
            "exactly 13 trailing lines (effective tolerance) is still live",
            multi_option_prompt_with_trailing_lines(13),
            true,
        ),
        (
            "14 trailing lines, one past the effective tolerance, is not live",
            multi_option_prompt_with_trailing_lines(14),
            false,
        ),
        (
            // Gate-review safety finding: narration mentioning the footer
            // text, more than LIVE_PROMPT_TAIL lines from the end so only
            // the offset>0 shape-gated path can reach it, with no option
            // list anywhere, must NOT read as live.
            "narration mentioning the footer text is not a live modal",
            "I checked the pane and its footer currently reads \"Esc to cancel\" below it.\n\
             Nothing else is happening right now.\n\
             Just idle, waiting for the next task.\n\
             Still idle.\n\
             Still nothing.\n\
             $ "
                .to_string(),
            false,
        ),
        (
            // Same reasoning, but the mention comes from grep-style output
            // echoing a matching line (prefixed by file/line context, so it
            // does not START the line either).
            "grep-style output echoing the footer text is not a live modal",
            "grep -rn \"esc to cancel\" src/\n\
             src/supervisor/permission_prompt.rs:1378:const LIVE_PROMPT_FOOTER_MARKERS: &[&str] = &[\"esc to cancel\"];\n\
             done searching\n\
             no further prompts found\n\
             agent waiting for next instruction\n\
             $ "
                .to_string(),
            false,
        ),
        (
            "a captured modal in tool output with an idle input box below it is not live",
            captured_modal_with_idle_input_box_fixture(),
            false,
        ),
        (
            "a captured modal above the real borderless idle input box is not live",
            captured_modal_with_borderless_idle_input_box_fixture(),
            false,
        ),
    ]
}

/// Task 4.3: `bash -n` on the edited helper — the bundled script must always
/// parse (guards the embedded-Python heredoc quote-tracking landmine).
#[test]
fn sweep_sh_parses_under_bash_n() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/scripts/sweep.sh");
    let out = StdCommand::new("bash")
        .arg("-n")
        .arg(&src)
        .output()
        .expect("run bash -n");
    assert!(
        out.status.success(),
        "bash -n must accept sweep.sh: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `cli-prompt-profiles` capability, requirement "Rust and the supervisor
/// helper script read one source": `git paw __prompt-profile` (invoked by
/// `sweep.sh` via `discover_prompt_profile_field`) prints the SAME
/// `live_prompt_markers` the in-process Rust gate resolves from
/// `claude_prompt_profile()` — not a second, independently maintained copy.
/// Neither default marker needs regex escaping, so a plain `|`-split
/// round-trips exactly.
#[test]
fn one_source_is_read_by_both_consumers() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());

    let out = StdCommand::new(assert_cmd::cargo::cargo_bin("git-paw"))
        .args(["__prompt-profile", "--field", "live_prompt_markers"])
        .current_dir(repo.path())
        .output()
        .expect("run __prompt-profile");
    assert!(out.status.success(), "{:?}", out.stderr);
    let printed = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let shell_markers: Vec<String> = printed.split('|').map(str::to_string).collect();

    assert_eq!(
        shell_markers,
        claude_prompt_profile().live_prompt_markers,
        "sweep.sh's subprocess call must surface the exact in-process profile data"
    );
}

/// Spec scenario "Detection agrees across auto-approver and sweep helper":
/// the Rust gate and the sweep.sh mirror return the same liveness verdict on
/// every shared fixture, and that verdict matches the expectation.
#[test]
fn rust_gate_and_sweep_helper_agree_on_shared_fixtures() {
    let repo = TempDir::new().expect("repo");
    init_git_repo(repo.path());
    let sweep = install_sweep(repo.path());

    for (name, capture, expected_live) in fixtures() {
        let rust_live = is_live_prompt(&capture);
        let sweep_live = sweep_sees_live(repo.path(), &sweep, &capture);
        assert_eq!(
            rust_live, expected_live,
            "Rust gate disagrees with expectation on fixture: {name}"
        );
        assert_eq!(
            sweep_live, expected_live,
            "sweep.sh disagrees with expectation on fixture: {name}"
        );
    }
}
