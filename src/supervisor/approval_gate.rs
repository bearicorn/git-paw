//! The shared live-prompt re-confirm and the approval dedup key.
//!
//! What survives the classifier's retirement from the
//! `broker-mediated-approvals` work. No git-paw process sends approval
//! keystrokes any more, so the in-binary send gate is gone; the discipline it
//! enforced lives on in `assets/scripts/sweep.sh`'s `approve`, the only
//! approval path left, which re-confirms the prompt on a fresh capture and
//! refuses pane 0 in shell.
//!
//! Two pieces are still consumed in Rust:
//!
//! - [`live_prompt_in_tail`] — the live-prompt structural check widened with
//!   the profile's approval markers, so detection and the shell helper agree
//!   on what "a live prompt" means.
//! - [`approval_dedup_key`] — keys a repeated prompt on `(agent_id, command)`
//!   so the drive loop escalates it once per window rather than every sweep.
//!
//! This module adds **no** `BrokerMessage` variant and **no** per-CLI
//! permission hook: the escalation signal reuses the existing `agent.question`
//! and `agent.status` variants.

use crate::config::claude_prompt_profile;
use crate::supervisor::permission_prompt::{
    extract_command_slice, is_live_prompt, live_prompt_markers_at_tail,
};

/// Returns whether the prompt's live structural markers anchor the modal
/// block of `capture`.
///
/// Delegates to the Live-prompt gate's structural check
/// ([`is_live_prompt`] — option glyphs / `Do you want to …` / `Esc to
/// cancel`, anchored over a window wide enough to span a full multi-option
/// prompt block and searched upward over a bounded trailing region so a CLI
/// that renders chrome under its input area doesn't hide a live modal),
/// extended with the embedded Claude Code profile's approval markers so the
/// re-confirm also recognises other agent CLIs' prompt wordings (`requires
/// approval`, `(y/n)`, …) that `permission-detection` accepts. A marker found
/// only in scrollback above the tail — or found only as a bare substring
/// mention with no accompanying option list once the anchor has shifted past
/// the capture's true end — is treated as *not live*: the first is a prompt
/// already answered and scrolled away, the second is narration or tool
/// output that merely mentions the footer text rather than the modal itself.
#[must_use]
pub fn live_prompt_in_tail(capture: &str) -> bool {
    is_live_prompt(capture)
        || live_prompt_markers_at_tail(
            capture,
            &claude_prompt_profile()
                .approval_markers
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        )
}

/// Computes the dedup key for a pending/repeated approval, keyed on the
/// **command identity** (the command text the modal is asking about) combined
/// with the **agent identity** — never on the prompt's boilerplate/footer text
/// (Decision 3, W15-19).
///
/// The command slice is extracted with [`extract_command_slice`], which reads
/// only the text between the `Bash command` / `Bash(…)` header and the
/// confirmation question — so the shared footer ("Do you want to proceed?")
/// can never contribute to the key. Two prompts on the same agent whose
/// commands differ (`cargo test` vs `git push`) therefore produce distinct
/// keys and are treated as distinct approval events; repeated captures of the
/// same unanswered command collapse to one key.
///
/// This is the identity-keyed complement to wait-for-clear (the natural
/// consequence of [`approval_send_gate`]'s re-confirm): once a prompt is
/// answered its footer disappears and the next distinct prompt re-confirms
/// fresh.
#[must_use]
pub fn approval_dedup_key(agent_id: &str, capture: &str) -> String {
    let command = extract_command_slice(capture).unwrap_or_default();
    // The unit separator keeps the two identity components unambiguous even
    // when a command contains arbitrary punctuation.
    format!("{agent_id}\u{1f}{command}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Task 5.1: tail check -------------------------------------------

    /// A permission marker within the last 4 non-blank lines is live.
    #[test]
    fn marker_in_tail_is_live() {
        let capture = "Bash command\n  cargo test\nDo you want to proceed?\n❯ 1. Yes\n  2. No";
        assert!(live_prompt_in_tail(capture));
    }

    /// A marker only in scrollback above the tail is NOT live (spec scenario
    /// "An answered prompt followed by command output is not live"): the
    /// question line has no option list and no `Esc to cancel` footer after
    /// it, so the bounded trailing region must NOT admit it — only the
    /// modal's structural footer can anchor a match beyond the narrow tail.
    #[test]
    fn marker_only_in_scrollback_is_not_live() {
        // The marker line is followed by more than 4 non-blank lines, so it
        // falls outside the tail window, and nothing resembling a footer
        // ever appears — a bare question answered and scrolled past.
        let capture = "Do you want to proceed?\nran it\nline b\nline c\nline d\n$ ";
        assert!(!live_prompt_in_tail(capture));
    }

    /// A multi-option prompt whose confirmation question sits above the
    /// 4-line tail anchor is still live: the numbered option list anchors the
    /// tail and the question is within the block window (spec scenario "Live
    /// multi-option prompt is detected", `approve-send-gate-hardening`).
    #[test]
    fn multi_option_block_above_narrow_tail_is_live() {
        let capture = "\
Bash command
  cargo test --workspace
Do you want to proceed?
❯ 1. Yes
  2. Yes, and don't ask again for cargo test in this project
  3. No, and tell Claude what to do differently (esc)";
        assert!(live_prompt_in_tail(capture));
    }

    /// Blank lines between the marker and the bottom do not push the marker
    /// out of the tail — only non-blank lines count toward the window.
    #[test]
    fn blank_lines_do_not_evict_marker_from_tail() {
        let capture = "requires approval\n\n\n\n";
        assert!(live_prompt_in_tail(capture));
    }

    /// Matching is case-insensitive so a real "Do you want to proceed?" (capital
    /// D) matches the lowercase marker.
    #[test]
    fn tail_check_is_case_insensitive() {
        assert!(live_prompt_in_tail("Do You Want To Proceed?"));
    }

    #[test]
    fn empty_capture_is_not_live() {
        assert!(!live_prompt_in_tail(""));
    }

    // --- Task 5.4: dedup on command identity, not footer ----------------

    /// `cargo test` and `git push` prompts sharing the identical footer are
    /// distinct approval events (keyed on their differing command identity).
    #[test]
    fn dedup_distinguishes_commands_with_identical_footer() {
        let footer = "Do you want to proceed?\n❯ 1. Yes\n  2. No\n(esc to cancel)";
        let cargo = format!("Bash command\n  cargo test --workspace\n{footer}");
        let push = format!("Bash command\n  git push origin main\n{footer}");
        let k_cargo = approval_dedup_key("feat-a", &cargo);
        let k_push = approval_dedup_key("feat-a", &push);
        assert_ne!(
            k_cargo, k_push,
            "distinct commands must not collapse under a shared footer"
        );
    }

    /// The same agent on the same unanswered prompt across consecutive sweeps
    /// yields the same dedup key (one approval event, not many).
    #[test]
    fn dedup_collapses_repeated_capture_of_same_prompt() {
        let cap = "Bash command\n  cargo test\nDo you want to proceed?\n❯ 1. Yes";
        let first = approval_dedup_key("feat-a", cap);
        let second = approval_dedup_key("feat-a", cap);
        assert_eq!(first, second, "repeated unchanged prompt is one event");
    }

    /// The same command on different agents is not collapsed (agent identity
    /// participates in the key).
    #[test]
    fn dedup_keys_on_agent_identity_too() {
        let cap = "Bash command\n  cargo test\nDo you want to proceed?";
        assert_ne!(
            approval_dedup_key("feat-a", cap),
            approval_dedup_key("feat-b", cap)
        );
    }
}
