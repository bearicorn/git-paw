//! Per-surface prompt-shape profiles (`cli-profile-threading`).
//!
//! Every prompt-shape detector takes the resolved profile of the CLI on the
//! surface it inspects. These tests drive the public detectors directly: the
//! byte-identical guard for CLIs with no override, the load-time validation of
//! override patterns, and an override pattern reaching its detector.

use std::collections::HashMap;
use std::fs;

use git_paw::agent_cli::CliProfile;
use git_paw::config::{CompiledPromptProfile, CustomCli, load_config_from};
use git_paw::coordination::inventory::{Mode, detect_mode};
use git_paw::supervisor::approval_gate::live_prompt_in_tail;
use git_paw::supervisor::drive::{pane_is_idle_and_empty, pane_is_responsive};
use git_paw::supervisor::permission_prompt::{
    extract_command_slice, extract_path_from_file_prompt, has_approval_marker, is_live_prompt,
    is_option_line,
};
use git_paw::tmux::{PaneReadiness, classify_pane_readiness};
use tempfile::TempDir;

/// The profile a pane running `cli` resolves with no `[clis]` table.
fn profile_of(cli: &str) -> (CliProfile, CompiledPromptProfile) {
    let record = CliProfile::resolve(cli, &HashMap::new());
    let compiled = record
        .compiled_prompt_profile()
        .expect("the embedded profile compiles");
    (record, compiled)
}

/// Every detector's decision on one capture — a pinned row, not a state.
#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
struct Decisions {
    live: bool,
    live_in_tail: bool,
    approval_marker: bool,
    command_slice: Option<String>,
    file_path: Option<String>,
    responsive: bool,
    idle_and_empty: bool,
    readiness: PaneReadiness,
    mode: Mode,
}

fn decide(capture: &str, record: &CliProfile, profile: &CompiledPromptProfile) -> Decisions {
    Decisions {
        live: is_live_prompt(capture, profile),
        live_in_tail: live_prompt_in_tail(capture, profile),
        approval_marker: has_approval_marker(capture, profile),
        command_slice: extract_command_slice(capture, profile),
        file_path: extract_path_from_file_prompt(capture, profile),
        responsive: pane_is_responsive(capture, profile),
        idle_and_empty: pane_is_idle_and_empty(capture, profile),
        readiness: classify_pane_readiness(capture, profile, record.dialog_markers()),
        mode: detect_mode("", capture, profile),
    }
}

/// Spec scenarios "No overrides reproduces current decisions"
/// (`cli-prompt-profiles`) and "Unconfigured CLI detects as before"
/// (`approval-command-safety`): with no
/// `prompt_profile` override, every detector run against the capture
/// fixtures the existing per-detector tests use returns the decision it made
/// before profiles were threaded — for a Claude pane and for a pane whose CLI
/// resolves `generic`.
#[test]
fn no_override_reproduces_every_detector_decision() {
    #[rustfmt::skip]
    let table: [(&str, &str, Decisions); 7] = [
        (
            "live shell prompt",
            "Bash command\n  ls -la\nDo you want to proceed?\n\u{276f} 1. Yes\n  2. No\n(esc to cancel)",
            Decisions { live: true, live_in_tail: true, approval_marker: false,
                command_slice: Some("ls -la".into()), file_path: None, responsive: false,
                idle_and_empty: false, readiness: PaneReadiness::Indeterminate,
                mode: Mode::Interactive },
        ),
        (
            "live file-write prompt",
            "Do you want to allow this write to Containerfile?\n\u{276f} 1. Yes\n  2. No\n(esc to cancel)",
            Decisions { live: true, live_in_tail: true, approval_marker: false,
                command_slice: None, file_path: Some("Containerfile".into()), responsive: false,
                idle_and_empty: false, readiness: PaneReadiness::Indeterminate,
                mode: Mode::Interactive },
        ),
        (
            "idle borderless box holding text",
            "────────────\n\u{276f} please continue\n────────────\n  \u{23f5}\u{23f5} accept edits on (shift+tab to cycle)",
            Decisions { live: false, live_in_tail: false, approval_marker: false,
                command_slice: None, file_path: None, responsive: true,
                idle_and_empty: false, readiness: PaneReadiness::Indeterminate,
                mode: Mode::AcceptEdits },
        ),
        (
            "idle and ready",
            "? for shortcuts",
            Decisions { live: false, live_in_tail: false, approval_marker: false,
                command_slice: None, file_path: None, responsive: true,
                idle_and_empty: true, readiness: PaneReadiness::Ready,
                mode: Mode::Interactive },
        ),
        (
            "mid-response",
            "\u{25cf} Thinking\u{2026}\n  esc to interrupt",
            Decisions { live: false, live_in_tail: false, approval_marker: false,
                command_slice: None, file_path: None, responsive: false,
                idle_and_empty: false, readiness: PaneReadiness::Ready,
                mode: Mode::Unknown },
        ),
        (
            "bare shell",
            "user@host ~/repo % ",
            Decisions { live: false, live_in_tail: false, approval_marker: false,
                command_slice: None, file_path: None, responsive: true,
                idle_and_empty: true, readiness: PaneReadiness::BareShell,
                mode: Mode::Unknown },
        ),
        (
            "approval-worded prompt",
            "requires approval [y/N]\nBash command:\ngit commit -m hi",
            Decisions { live: false, live_in_tail: true, approval_marker: true,
                command_slice: Some("git commit -m hi".into()), file_path: None, responsive: false,
                idle_and_empty: false, readiness: PaneReadiness::Indeterminate,
                mode: Mode::Interactive },
        ),
    ];
    for cli in ["claude", "some-unlisted-cli"] {
        let (record, profile) = profile_of(cli);
        for (name, capture, expected) in &table {
            assert_eq!(
                &decide(capture, &record, &profile),
                expected,
                "{cli}: {name}"
            );
        }
    }
}

/// Writes a `.git-paw/config.toml` whose `[clis.mycli.prompt_profile]` table
/// holds `profile_line` under `repo`.
fn write_mycli_profile(repo: &std::path::Path, profile_line: &str) {
    let paw = repo.join(".git-paw");
    fs::create_dir_all(&paw).expect("mk .git-paw");
    fs::write(
        paw.join("config.toml"),
        format!(
            "[clis.mycli]\ncommand = \"mycli\"\n\n[clis.mycli.prompt_profile]\n{profile_line}\n"
        ),
    )
    .expect("write config");
}

/// Loads a config whose `mycli` profile holds `profile_line`.
fn load_with_mycli_profile(profile_line: &str) -> Result<HashMap<String, CustomCli>, String> {
    let repo = TempDir::new().expect("repo");
    write_mycli_profile(repo.path(), profile_line);
    load_config_from(&repo.path().join("no-global.toml"), repo.path())
        .map(|c| c.clis)
        .map_err(|e| e.to_string())
}

fn load_with_mycli_pattern(
    field: &str,
    pattern: &str,
) -> Result<HashMap<String, CustomCli>, String> {
    load_with_mycli_profile(&format!("{field} = '{pattern}'"))
}

/// An empty or whitespace-only marker would match every line under the
/// helper's `grep -E`, so it fails the load in every marker field.
#[test]
fn an_empty_marker_fails_the_load_naming_the_field() {
    for field in [
        "readiness_markers",
        "approval_markers",
        "live_prompt_markers",
        "mid_response_markers",
        "prompt_boilerplate_markers",
        "command_header_markers",
        "input_sigils",
        "mode_accept_edits_markers",
        "mode_interactive_markers",
        "stream_error_markers",
        "cli_unavailable_markers",
    ] {
        for empty in ["\"\"", "\"  \""] {
            let err =
                load_with_mycli_profile(&format!("{field} = [\"ok\", {empty}]")).expect_err(field);
            assert!(
                err.contains(&format!("[clis.mycli].prompt_profile.{field}")),
                "{field}: {err}"
            );
        }
    }
    let err = load_with_mycli_profile("broad_grant_marker = \" \"").expect_err("blank marker");
    assert!(
        err.contains("[clis.mycli].prompt_profile.broad_grant_marker"),
        "{err}"
    );
}

/// A pattern the helper also matches with `grep -E` must stay within the
/// subset both engines run; one that only Rust understands fails the load.
#[test]
fn a_shell_matched_pattern_outside_grep_e_fails_the_load() {
    for (field, pattern) in [
        ("option_line_pattern", r"^\d[.)]"),
        ("context_bloat_pattern", r"(?i)clear to save (\d+)k"),
        ("paste_buffer_pattern", r"Pasted text #[0-9]+?"),
    ] {
        let err = load_with_mycli_pattern(field, pattern).expect_err(field);
        assert!(err.contains("grep -E"), "{field}: {err}");
    }
    // file_prompt_pattern is matched only in Rust, so the full syntax stays.
    load_with_mycli_pattern("file_prompt_pattern", r"(?i)write to\s+(.+)$").expect("loads");
}

/// `__prompt-profile` exits non-zero on a config that fails to load, rather
/// than printing the defaults as if they were the configured profile.
#[test]
fn prompt_profile_subcommand_fails_on_an_invalid_config() {
    let repo = TempDir::new().expect("repo");
    let ok = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(repo.path())
        .status()
        .expect("git init")
        .success();
    assert!(ok);
    write_mycli_profile(repo.path(), "option_line_pattern = '^[0-9'");
    assert_cmd::Command::cargo_bin("git-paw")
        .expect("binary")
        .args([
            "__prompt-profile",
            "--cli",
            "mycli",
            "--field",
            "option_line_pattern",
        ])
        .current_dir(repo.path())
        .assert()
        .failure()
        .stdout("");
}

/// Spec scenario "An invalid pattern fails the load".
#[test]
fn an_invalid_override_pattern_fails_the_load_naming_the_field() {
    let err = load_with_mycli_pattern("option_line_pattern", "^[0-9")
        .expect_err("an unclosed class must not load");
    assert!(
        err.contains("[clis.mycli].prompt_profile.option_line_pattern"),
        "the error names the field: {err}"
    );
}

/// Every pattern field is validated, not just the one the detectors use most.
#[test]
fn every_pattern_field_is_validated_at_load() {
    for field in [
        "file_prompt_pattern",
        "option_line_pattern",
        "context_bloat_pattern",
        "paste_buffer_pattern",
    ] {
        let err = load_with_mycli_pattern(field, "(unclosed").expect_err(field);
        assert!(
            err.contains(&format!("[clis.mycli].prompt_profile.{field}")),
            "{field}: {err}"
        );
    }
}

/// Spec scenario "A valid pattern is used by the detector".
#[test]
fn a_valid_override_pattern_is_used_by_the_detector() {
    let clis = load_with_mycli_pattern("option_line_pattern", r"^\([0-9]\)").expect("loads");
    let mycli = CliProfile::resolve("mycli", &clis)
        .compiled_prompt_profile()
        .expect("validated at load, so it compiles");
    let (_, claude) = profile_of("claude");
    for line in ["(1) Yes", "(2) No"] {
        assert!(is_option_line(line, &mycli), "mycli recognises {line:?}");
        assert!(!is_option_line(line, &claude), "claude's shape does not");
    }
}
