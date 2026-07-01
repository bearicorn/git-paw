//! The `broker-mediated-approvals` approval-send gate.
//!
//! Every in-binary approver that clears an agent CLI permission prompt by
//! sending keystrokes via `tmux send-keys` MUST pass through
//! [`approval_send_gate`]. The gate closes the F5 blind-send-keys race
//! (findings W15-13 / W15-19): between the moment an approval decision is made
//! and the moment keys are dispatched, the prompt can clear (the agent moved
//! on, a human answered first, or another approver won the race). Sending keys
//! blind then lands them as literal chat text, polluting the agent's context
//! and leaving stray unsubmitted commands.
//!
//! The gate does three things, in order:
//!
//! 1. Refuse pane index 0 — the supervisor's own pane is never sent blind
//!    keystrokes (Decision 2).
//! 2. Re-capture the target pane *immediately before* the send and confirm a
//!    live permission-prompt marker is present within the last
//!    [`APPROVAL_TAIL_LINES`] non-blank lines (Decision 1). This is a fresh
//!    capture — the detection/decision-stage capture SHALL NOT substitute for
//!    it — and there is deliberately no classification work or broker
//!    round-trip between the re-confirm and the send.
//! 3. Only on a re-confirmed live prompt does it dispatch the keys.
//!
//! The tail check reuses the permission-prompt marker set shared with
//! `permission-detection`
//! ([`crate::supervisor::permission_prompt::APPROVAL_MARKERS`]) and the shell
//! `stuck-prompt-detection` helper (`sweep.sh`'s `STUCK_MARKERS_REGEX`), so the
//! Rust and shell approvers agree on what "a live prompt" means.
//!
//! This module adds **no** new `BrokerMessage` variant and **no** per-CLI
//! permission hook (Decision 4) — the approval-trigger and escalation signals
//! reuse the existing `agent.status` (`phase: "stuck-on-prompt"`) and
//! `agent.question` variants.

use crate::error::PawError;
use crate::supervisor::approve::KeyDispatcher;
use crate::supervisor::auto_approve::extract_command_slice;
use crate::supervisor::permission_prompt::APPROVAL_MARKERS;

/// Number of trailing non-blank lines the re-confirm check scans for a live
/// permission-prompt marker. The approval footer ("Do you want to proceed?" /
/// "❯ 1. Yes" / "No") is always the bottommost interactive region, so a match
/// anywhere above the tail is a prompt the agent already answered (scrolled up
/// into history) and SHALL NOT count as live.
pub const APPROVAL_TAIL_LINES: usize = 4;

/// Pane index reserved for the supervisor's own CLI. The blind send-keys gate
/// never types into it under any classification; clearing pane 0's own prompt
/// is a non-blind concern owned by the unattended drive loop.
pub const SUPERVISOR_PANE_INDEX: usize = 0;

/// Returns whether a live permission-prompt marker is present within the last
/// [`APPROVAL_TAIL_LINES`] non-blank lines of `capture`.
///
/// Reuses the [`APPROVAL_MARKERS`] set (matched case-insensitively) so the
/// re-confirm agrees with `permission-detection` on what a permission prompt
/// looks like. A marker found only in scrollback above the tail is treated as
/// *not live* — that is a prompt already answered and scrolled away.
#[must_use]
pub fn live_prompt_in_tail(capture: &str) -> bool {
    capture
        .lines()
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(APPROVAL_TAIL_LINES)
        .any(|line| {
            let lower = line.to_ascii_lowercase();
            APPROVAL_MARKERS
                .iter()
                .any(|marker| lower.contains(&marker.to_ascii_lowercase()))
        })
}

/// Abstraction over "capture the current text of a pane", so the gate's
/// re-confirm step is testable without spawning tmux.
///
/// The production capture is a thin shim over
/// [`crate::supervisor::permission_prompt::capture_pane`]; the poll loop's
/// [`crate::supervisor::poll::PaneInspector`] is a `PaneCapturer` (it is a
/// supertrait), so the loop passes its existing inspector straight through.
pub trait PaneCapturer {
    /// Returns the current captured text of pane `pane_index` in `session`,
    /// or the empty string when the capture fails.
    fn capture(&self, session: &str, pane_index: usize) -> String;
}

/// Outcome of a single [`approval_send_gate`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOutcome {
    /// A live prompt was re-confirmed and the keystrokes were dispatched.
    Sent,
    /// The re-confirm capture showed no live prompt in the tail (the prompt
    /// cleared between decision and send) — no keystrokes were dispatched.
    PromptCleared,
    /// The target pane index was 0 (the supervisor's own pane), excluded from
    /// the blind send-keys path — no keystrokes were dispatched.
    Pane0Excluded,
}

/// The single approval-send gate every in-binary keystroke approver passes
/// through.
///
/// Refuses pane 0, re-captures `pane_index` in `session` via `capturer`,
/// confirms a live prompt in the tail, and only then dispatches each key in
/// `keys` through `dispatcher`. Returns the [`GateOutcome`] describing which
/// branch was taken. The only error path is a `send-keys` failure once a live
/// prompt has been confirmed.
///
/// Callers MUST NOT pre-filter on a stale capture and skip this gate — the
/// re-capture here is the only point where "is the modal still up?" is true.
pub fn approval_send_gate<C, D>(
    capturer: &C,
    dispatcher: &mut D,
    session: &str,
    pane_index: usize,
    keys: &[String],
) -> Result<GateOutcome, PawError>
where
    C: PaneCapturer,
    D: KeyDispatcher,
{
    // Decision 2: pane 0 (supervisor) is excluded from blind send-keys under
    // any classification, even a re-confirmed live prompt.
    if pane_index == SUPERVISOR_PANE_INDEX {
        return Ok(GateOutcome::Pane0Excluded);
    }

    // Decision 1: re-confirm a live prompt with a fresh capture immediately
    // before the send. No classification work or broker round-trip sits
    // between this capture and the dispatch below.
    let capture = capturer.capture(session, pane_index);
    if !live_prompt_in_tail(&capture) {
        return Ok(GateOutcome::PromptCleared);
    }

    for key in keys {
        dispatcher
            .send_key(session, pane_index, key)
            .map_err(|e| PawError::TmuxError(format!("send-keys {key} failed: {e}")))?;
    }
    Ok(GateOutcome::Sent)
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

    /// Recording capturer that returns a fixed capture and counts calls, so a
    /// test can assert the gate re-captured exactly once.
    struct StubCapturer {
        capture: String,
        calls: std::cell::Cell<usize>,
    }
    impl StubCapturer {
        fn new(capture: &str) -> Self {
            Self {
                capture: capture.to_string(),
                calls: std::cell::Cell::new(0),
            }
        }
    }
    impl PaneCapturer for StubCapturer {
        fn capture(&self, _session: &str, _pane_index: usize) -> String {
            self.calls.set(self.calls.get() + 1);
            self.capture.clone()
        }
    }

    /// Recording dispatcher that captures every (session, pane, key) tuple.
    struct Recorder {
        events: Vec<(String, usize, String)>,
    }
    impl Recorder {
        fn new() -> Self {
            Self { events: Vec::new() }
        }
        fn keys(&self) -> Vec<&str> {
            self.events.iter().map(|(_, _, k)| k.as_str()).collect()
        }
    }
    impl KeyDispatcher for Recorder {
        fn send_key(&mut self, session: &str, pane_index: usize, key: &str) -> std::io::Result<()> {
            self.events
                .push((session.to_string(), pane_index, key.to_string()));
            Ok(())
        }
    }

    fn keys() -> Vec<String> {
        vec!["Down".to_string(), "Enter".to_string()]
    }

    // --- Task 5.1: tail check -------------------------------------------

    /// A permission marker within the last 4 non-blank lines is live.
    #[test]
    fn marker_in_tail_is_live() {
        let capture = "Bash command\n  cargo test\nDo you want to proceed?\n❯ 1. Yes\n  2. No";
        assert!(live_prompt_in_tail(capture));
    }

    /// A marker only in scrollback above the tail is NOT live (spec scenario
    /// "A stale marker only in scrollback is not treated as live").
    #[test]
    fn marker_only_in_scrollback_is_not_live() {
        // The marker line is followed by more than 4 non-blank lines, so it
        // falls outside the tail window.
        let capture = "Do you want to proceed?\nran it\nline b\nline c\nline d\n$ ";
        assert!(!live_prompt_in_tail(capture));
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

    // --- Task 5.2: gate dispatches on live, nothing on cleared ----------

    /// The gate dispatches the keys when the re-confirm capture shows a live
    /// prompt in the tail, and it re-captures exactly once.
    #[test]
    fn gate_dispatches_on_reconfirmed_live_prompt() {
        let capturer = StubCapturer::new("Bash command\n  cargo test\nDo you want to proceed?");
        let mut rec = Recorder::new();
        let outcome = approval_send_gate(&capturer, &mut rec, "paw-x", 2, &keys()).unwrap();
        assert_eq!(outcome, GateOutcome::Sent);
        assert_eq!(rec.keys(), vec!["Down", "Enter"]);
        assert_eq!(capturer.calls.get(), 1, "gate must re-capture exactly once");
    }

    /// A cleared prompt (no marker in the tail) suppresses the send entirely.
    #[test]
    fn gate_sends_nothing_when_prompt_cleared() {
        let capturer = StubCapturer::new("$ \nagent moved on\nall done\n");
        let mut rec = Recorder::new();
        let outcome = approval_send_gate(&capturer, &mut rec, "paw-x", 2, &keys()).unwrap();
        assert_eq!(outcome, GateOutcome::PromptCleared);
        assert!(rec.events.is_empty(), "cleared prompt => no keystrokes");
    }

    // --- Task 5.3: pane 0 refused, coding-agent pane approved -----------

    /// Pane 0 is refused with no keystrokes and no capture, even when a live
    /// prompt would be present (spec scenario "Pane 0 is never sent blind
    /// keystrokes").
    #[test]
    fn gate_refuses_pane_zero() {
        let capturer = StubCapturer::new("Do you want to proceed?");
        let mut rec = Recorder::new();
        let outcome = approval_send_gate(&capturer, &mut rec, "paw-x", 0, &keys()).unwrap();
        assert_eq!(outcome, GateOutcome::Pane0Excluded);
        assert!(rec.events.is_empty(), "pane 0 => no keystrokes");
        assert_eq!(
            capturer.calls.get(),
            0,
            "pane 0 is refused before any capture"
        );
    }

    /// A coding-agent pane (index 2) with a live prompt is approved (spec
    /// scenario "Coding agent panes are still approvable").
    #[test]
    fn gate_approves_coding_agent_pane_two() {
        let capturer = StubCapturer::new("Bash command\n  cargo build\nDo you want to proceed?");
        let mut rec = Recorder::new();
        let outcome = approval_send_gate(&capturer, &mut rec, "paw-x", 2, &keys()).unwrap();
        assert_eq!(outcome, GateOutcome::Sent);
        assert!(rec.events.iter().all(|(_, p, _)| *p == 2));
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
