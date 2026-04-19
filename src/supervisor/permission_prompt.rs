//! Permission-prompt detection for stalled agent panes.
//!
//! Implements the `permission-detection` capability of the
//! `auto-approve-patterns` change: capture pane content via
//! `tmux capture-pane -p -t <session>:<pane>`, scan for known approval
//! markers, and classify the pending command into a [`PermissionType`].
//!
//! Detection is intentionally only a thin wrapper around `tmux capture-pane`
//! and string scanning — no parsing of agent CLI internals. Markers live in
//! [`APPROVAL_MARKERS`] and command classes in [`classify_capture`] so they
//! can be tweaked without touching call sites.

use std::process::Command;

/// Coarse classification of a detected permission prompt.
///
/// The auto-approver consults this to decide whether to send the
/// `BTab Down Enter` keystroke sequence. Anything that classifies as
/// [`PermissionType::Unknown`] is left for human review via the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionType {
    /// A `curl ...` command (typically broker traffic).
    Curl,
    /// A `cargo ...` command (`fmt`, `clippy`, `test`, `build`).
    Cargo,
    /// A `git ...` command (`commit`, `push`).
    Git,
    /// An approval prompt was detected but the command class is not
    /// recognised. Auto-approval MUST NOT fire for this variant.
    Unknown,
}

/// Pane-content substrings that indicate the agent CLI is waiting for an
/// approval decision.
///
/// Conservative by design — exact phrase matches keep false positives low.
/// Add a new marker when a new agent CLI surfaces a different prompt
/// wording.
pub const APPROVAL_MARKERS: &[&str] = &[
    "requires approval",
    "do you want to proceed",
    "do you want to allow",
    "(y/n)",
    "[y/N]",
    "Allow this command",
];

/// Captures the content of pane `pane_index` in `session` via
/// `tmux capture-pane -p -t <session>:<pane>`.
///
/// Returns `None` when tmux is unavailable, the target does not exist, or
/// the pane has no content. The function never panics on tmux exit codes —
/// any error is treated as "no content available".
#[must_use]
pub fn capture_pane(session: &str, pane_index: usize) -> Option<String> {
    let target = format!("{session}:0.{pane_index}");
    let output = Command::new("tmux")
        .args(["capture-pane", "-p", "-t", &target])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let s = String::from_utf8(output.stdout).ok()?;
    Some(s)
}

/// Scans `captured` for a known approval marker and returns the prompt's
/// classification, or `None` when no marker is present.
///
/// Pure function — exposed separately from [`detect_permission_prompt`] so
/// unit tests can drive it without touching tmux.
#[must_use]
pub fn classify_capture(captured: &str) -> Option<PermissionType> {
    if !APPROVAL_MARKERS.iter().any(|m| captured.contains(m)) {
        return None;
    }
    Some(classify_command_class(captured))
}

fn classify_command_class(captured: &str) -> PermissionType {
    if captured.contains("curl") {
        PermissionType::Curl
    } else if captured.contains("cargo fmt")
        || captured.contains("cargo clippy")
        || captured.contains("cargo test")
        || captured.contains("cargo build")
    {
        PermissionType::Cargo
    } else if captured.contains("git commit") || captured.contains("git push") {
        PermissionType::Git
    } else {
        PermissionType::Unknown
    }
}

/// Captures the pane and classifies it.
///
/// Combines [`capture_pane`] and [`classify_capture`]. Returns `None` when
/// either the capture fails or no approval marker is present.
#[must_use]
pub fn detect_permission_prompt(session: &str, pane_index: usize) -> Option<PermissionType> {
    let content = capture_pane(session, pane_index)?;
    classify_capture(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_with_no_marker_classify_to_none() {
        let captured = "lorem ipsum dolor sit amet\n$ ls\n";
        assert_eq!(classify_capture(captured), None);
    }

    #[test]
    fn curl_prompt_classifies_curl() {
        let captured = "curl http://127.0.0.1:9119/publish\nrequires approval\n";
        assert_eq!(classify_capture(captured), Some(PermissionType::Curl));
    }

    #[test]
    fn cargo_test_prompt_classifies_cargo() {
        let captured = "do you want to proceed\nRunning cargo test --workspace";
        assert_eq!(classify_capture(captured), Some(PermissionType::Cargo));
    }

    #[test]
    fn cargo_fmt_prompt_classifies_cargo() {
        let captured = "[y/N] cargo fmt --all";
        assert_eq!(classify_capture(captured), Some(PermissionType::Cargo));
    }

    #[test]
    fn cargo_clippy_prompt_classifies_cargo() {
        let captured = "Allow this command: cargo clippy";
        assert_eq!(classify_capture(captured), Some(PermissionType::Cargo));
    }

    #[test]
    fn cargo_build_prompt_classifies_cargo() {
        let captured = "(y/n) cargo build --release";
        assert_eq!(classify_capture(captured), Some(PermissionType::Cargo));
    }

    #[test]
    fn git_commit_prompt_classifies_git() {
        let captured = "git commit -m hi\nrequires approval";
        assert_eq!(classify_capture(captured), Some(PermissionType::Git));
    }

    #[test]
    fn git_push_prompt_classifies_git() {
        let captured = "git push origin main\nrequires approval";
        assert_eq!(classify_capture(captured), Some(PermissionType::Git));
    }

    #[test]
    fn unrecognized_command_classifies_unknown() {
        let captured = "rm -rf /tmp/foo\nrequires approval";
        assert_eq!(classify_capture(captured), Some(PermissionType::Unknown));
    }

    #[test]
    fn marker_alone_without_command_is_unknown() {
        let captured = "requires approval";
        assert_eq!(classify_capture(captured), Some(PermissionType::Unknown));
    }

    #[test]
    fn capture_pane_returns_none_for_nonexistent_session() {
        // No tmux session named this should ever exist.
        let out = capture_pane("paw-nonexistent-session-aabbccdd-zz", 0);
        assert!(
            out.is_none(),
            "nonexistent session should not capture, got {out:?}"
        );
    }

    #[test]
    fn detect_permission_prompt_returns_none_for_nonexistent_session() {
        let out = detect_permission_prompt("paw-nonexistent-session-aabbccdd-zz", 0);
        assert_eq!(out, None);
    }

    /// Spec scenario `auto-approve-patterns/permission-detection`: a Claude
    /// prompt typically combines the marker `requires approval` with a
    /// `[y/N]` choice indicator on the same buffer. The classifier MUST
    /// route each command class to its corresponding `PermissionType` even
    /// when both markers are present, and MUST fall through to `Unknown`
    /// when no recognised command appears.
    #[test]
    fn claude_y_n_requires_approval_marker_classifies_each_class() {
        // Curl: an outbound HTTP call with the combined Claude marker.
        let curl_prompt =
            "Bash command:\ncurl http://127.0.0.1:9119/publish\nrequires approval [y/N]";
        assert_eq!(
            classify_capture(curl_prompt),
            Some(PermissionType::Curl),
            "Claude curl prompt with `requires approval [y/N]` must classify Curl"
        );

        // Cargo: a `cargo test` invocation behind the same combined marker.
        let cargo_prompt = "Bash command:\ncargo test --workspace\nrequires approval [y/N]";
        assert_eq!(
            classify_capture(cargo_prompt),
            Some(PermissionType::Cargo),
            "Claude cargo prompt with combined markers must classify Cargo"
        );

        // Git: `git commit` behind the same combined marker.
        let git_prompt = "Bash command:\ngit commit -m \"wip\"\nrequires approval [y/N]";
        assert_eq!(
            classify_capture(git_prompt),
            Some(PermissionType::Git),
            "Claude git prompt with combined markers must classify Git"
        );

        // Unknown: a non-whitelisted command behind the same combined marker
        // must NOT be classified into a known class — the auto-approver
        // depends on Unknown to refuse to fire here.
        let unknown_prompt = "Bash command:\nrm -rf /tmp/foo\nrequires approval [y/N]";
        assert_eq!(
            classify_capture(unknown_prompt),
            Some(PermissionType::Unknown),
            "an unrecognised command must classify Unknown even with markers"
        );

        // Sanity: the same `rm -rf` text without any approval marker must
        // not be classified at all (no prompt detected).
        let no_marker = "rm -rf /tmp/foo\n$ ls\n";
        assert_eq!(
            classify_capture(no_marker),
            None,
            "absent any approval marker, classify_capture must return None"
        );
    }
}
