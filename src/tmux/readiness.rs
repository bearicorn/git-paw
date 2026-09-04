//! Launch-readiness gate for agent CLI panes (design D1, G1).
//!
//! Classifies a captured pane as ready / bare-shell / indeterminate and polls
//! (with a bounded relaunch budget) before boot-block injection.

use std::time::Duration;

use crate::command_runner::{CommandRunner, RealCommandRunner};

// ---------------------------------------------------------------------------
// Launch-readiness gate (design D1, G1)
// ---------------------------------------------------------------------------

/// Substrings that positively identify a launched agent CLI's interactive
/// ready state, as opposed to a bare shell prompt. Conservative phrase matches
/// drawn from the agent CLIs git-paw supports — a bare shell that merely echoed
/// a failed command never contains any of these, so a match means the CLI's UI
/// is up and the boot block is safe to inject.
///
/// Extend this when a new agent CLI surfaces a different ready banner. An
/// unrecognised CLI whose UI matches nothing here falls back to fixed-budget
/// injection (never worse than the prior fixed-sleep launch).
pub const CLI_READY_MARKERS: &[&str] = &[
    "? for shortcuts",
    "? for help",
    "Welcome to Claude Code",
    "esc to interrupt",
    "Bypassing Permissions",
    "│ >",
];

/// Identifies a first-run acceptance/trust dialog (GP-03b) by a heading/body
/// substring, paired with the case-insensitive substring of its affirmative
/// option's OWN text.
///
/// The affirmative option is selected by matching its text, never a
/// hardcoded digit or a blind `Enter` — see [`DIALOG_MARKERS`] for why.
struct DialogMarker {
    /// Substring identifying this dialog in a pane capture.
    heading: &'static str,
    /// Case-insensitive substring of the option line to select.
    affirmative: &'static str,
}

/// Recognised first-run acceptance/trust dialogs (GP-03b). A dialog whose
/// heading matches none of these classifies [`PaneReadiness::Indeterminate`]
/// or [`PaneReadiness::BareShell`] exactly as before, so an unrecognised
/// CLI's dialog falls back to the existing readiness / relaunch / fixed-
/// budget behaviour — never worse than prior launch behaviour.
///
/// - Claude Code's bypass-permissions acceptance dialog: shown the first time
///   `--dangerously-skip-permissions` runs against a config directory that
///   has never accepted it. Its DEFAULT-highlighted option is `No, exit` —
///   exactly the boot-exit failure mode GP-03 fixes — so the gate selects
///   the affirmative option by TEXT, never a hardcoded digit or position.
/// - Claude Code's "trust this folder?" dialog: shown the first time a CLI
///   launches against a directory it has not seen before.
const DIALOG_MARKERS: &[DialogMarker] = &[
    DialogMarker {
        heading: "Bypass Permissions mode",
        affirmative: "yes, i accept",
    },
    DialogMarker {
        heading: "trust the files in this folder",
        affirmative: "yes, proceed",
    },
];

/// Scans `captured` for a numbered option line (`1. …`, `2) …`, optionally
/// prefixed with common TUI decoration) whose text contains `affirmative`
/// (case-insensitive), returning its leading digit.
///
/// Used to answer a recognised dialog by selecting the option whose WORDING
/// is affirmative, rather than a hardcoded digit or position — the
/// bypass-permissions dialog defaults its highlighted option to `No, exit`,
/// so answering by position risks accepting the wrong choice.
fn find_option_digit(captured: &str, affirmative: &str) -> Option<u8> {
    for raw in captured.lines() {
        let line = raw.trim_start_matches(|c: char| {
            c.is_whitespace() || matches!(c, '│' | '❯' | '●' | '•' | '*' | '·' | '>')
        });
        let mut chars = line.chars();
        let Some(digit_char) = chars.next().filter(char::is_ascii_digit) else {
            continue;
        };
        match chars.next() {
            Some('.' | ')') => {}
            _ => continue,
        }
        if chars.as_str().to_ascii_lowercase().contains(affirmative) {
            return digit_char.to_digit(10).and_then(|d| u8::try_from(d).ok());
        }
    }
    None
}

/// Default per-attempt readiness timeout (ms). Matches the prior fixed
/// pre-injection sleep so the conservative fall-back path (an unrecognised CLI
/// that never matches a marker) is never slower than the old behaviour; a
/// recognised CLI returns as soon as its marker appears, typically sooner.
/// Overridable via `GIT_PAW_READINESS_TIMEOUT_MS` so tests exercise the
/// fall-back path quickly.
const READINESS_TIMEOUT_MS: u64 = 2000;
/// Interval between readiness polls (ms).
const READINESS_POLL_INTERVAL_MS: u64 = 150;
/// Number of CLI relaunch attempts after a bare-shell timeout before falling
/// back to injection.
const READINESS_RELAUNCH_ATTEMPTS: usize = 1;

/// Classification of a captured pane's content for the launch-readiness gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneReadiness {
    /// A CLI-readiness marker was observed; the boot block is safe to inject.
    Ready,
    /// The pane is still a bare shell prompt (the CLI never started) — the
    /// G1 condition; relaunch is warranted.
    BareShell,
    /// A recognised first-run acceptance/trust dialog is showing (GP-03b).
    /// Carries the 1-based option digit that selects its affirmative choice,
    /// or `None` when the digit could not be determined from the capture —
    /// the gate fails the launch loudly rather than guessing.
    Dialog(Option<u8>),
    /// Neither ready nor an obvious bare shell (e.g. a blank/clearing screen
    /// or an unrecognised CLI). Wait, then conservatively fall back.
    Indeterminate,
}

/// Outcome of gating a pane before boot-block injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessOutcome {
    /// A CLI-readiness marker was observed; inject the boot block.
    Ready,
    /// The readiness budget elapsed without a positive classification (an
    /// unrecognised CLI, or a relaunched-but-never-ready pane). The caller
    /// injects anyway — behaviour matches the prior fixed-sleep launch.
    FellBack,
    /// A first-run acceptance/trust dialog was recognised but the gate could
    /// not resolve it — either its affirmative option's digit could not be
    /// determined, or it was answered but the pane never reached
    /// [`PaneReadiness::Ready`] within the launch budget (GP-03b). The
    /// caller SHALL NOT relaunch into the dialog and SHALL NOT inject the
    /// boot block on top of it — the launch fails loudly instead.
    DialogStuck,
}

/// Per-attempt timeout, poll interval, and relaunch budget for the gate.
#[derive(Debug, Clone, Copy)]
pub struct ReadinessBudget {
    /// Interval between `capture-pane` polls.
    pub poll_interval: Duration,
    /// How long to poll for readiness within a single attempt before declaring
    /// the attempt timed out.
    pub timeout: Duration,
    /// Number of CLI relaunch attempts after a bare-shell timeout.
    pub relaunch_attempts: usize,
}

impl Default for ReadinessBudget {
    fn default() -> Self {
        let timeout_ms = std::env::var("GIT_PAW_READINESS_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(READINESS_TIMEOUT_MS);
        Self {
            poll_interval: Duration::from_millis(READINESS_POLL_INTERVAL_MS),
            timeout: Duration::from_millis(timeout_ms),
            relaunch_attempts: READINESS_RELAUNCH_ATTEMPTS,
        }
    }
}

/// Returns whether `captured` (the last non-empty line) looks like a returned
/// shell prompt — ending in a common prompt sigil. Used to distinguish the
/// G1 bare-shell condition from a CLI whose UI simply has not rendered yet.
fn looks_like_bare_shell(captured: &str) -> bool {
    match captured.lines().rev().find(|l| !l.trim().is_empty()) {
        Some(line) => {
            let trimmed = line.trim_end();
            trimmed.ends_with('$')
                || trimmed.ends_with('%')
                || trimmed.ends_with('#')
                || trimmed.ends_with('❯')
                || trimmed.ends_with('➜')
        }
        None => false,
    }
}

/// Classify a captured pane's content for the readiness gate.
#[must_use]
pub fn classify_pane_readiness(captured: &str) -> PaneReadiness {
    if CLI_READY_MARKERS.iter().any(|m| captured.contains(m)) {
        return PaneReadiness::Ready;
    }
    if let Some(marker) = DIALOG_MARKERS.iter().find(|d| captured.contains(d.heading)) {
        return PaneReadiness::Dialog(find_option_digit(captured, marker.affirmative));
    }
    if looks_like_bare_shell(captured) {
        PaneReadiness::BareShell
    } else {
        PaneReadiness::Indeterminate
    }
}

/// Core readiness loop, generic over the capture, relaunch, sleep, and
/// dialog-answer primitives so it is unit-testable without a live tmux server
/// or wall-clock waits (design D1; dialog handling GP-03b).
///
/// Polls `capture` on `budget.poll_interval` until a [`PaneReadiness::Ready`]
/// classification is seen or `budget.timeout` elapses. On a bare-shell timeout
/// it invokes `relaunch` and re-polls, up to `budget.relaunch_attempts`. An
/// indeterminate or persistently-bare pane returns [`ReadinessOutcome::FellBack`].
///
/// A recognised first-run acceptance/trust dialog is answered via `answer`
/// (called with the affirmative option's digit) AT MOST ONCE per attempt,
/// then re-polled for [`PaneReadiness::Ready`] within the same timeout — it
/// is NEVER treated as a bare shell and relaunched (which would just re-open
/// the dialog). A dialog whose digit could not be determined, or one that was
/// answered but never reached `Ready` before the budget elapsed, returns
/// [`ReadinessOutcome::DialogStuck`] immediately rather than falling back to
/// injection.
pub(crate) fn gate_pane_generic<C, R, S, A>(
    budget: ReadinessBudget,
    mut capture: C,
    mut relaunch: R,
    mut sleep: S,
    mut answer: A,
) -> ReadinessOutcome
where
    C: FnMut() -> Option<String>,
    R: FnMut(),
    S: FnMut(Duration),
    A: FnMut(u8),
{
    for attempt in 0..=budget.relaunch_attempts {
        let mut waited = Duration::ZERO;
        let mut answered = false;
        loop {
            let captured = capture().unwrap_or_default();
            match classify_pane_readiness(&captured) {
                PaneReadiness::Ready => return ReadinessOutcome::Ready,
                PaneReadiness::Dialog(None) => return ReadinessOutcome::DialogStuck,
                PaneReadiness::Dialog(Some(digit)) if !answered => {
                    answer(digit);
                    answered = true;
                }
                _ => {}
            }
            if waited >= budget.timeout {
                break;
            }
            sleep(budget.poll_interval);
            waited = waited.saturating_add(budget.poll_interval);
        }
        if answered {
            // Answered but never reached Ready: never relaunch into a dialog
            // already progressed past — fail loudly instead.
            return ReadinessOutcome::DialogStuck;
        }
        // Attempt timed out. Relaunch only when the pane is positively a bare
        // shell AND a relaunch attempt remains; otherwise fall back.
        let final_state = classify_pane_readiness(&capture().unwrap_or_default());
        if final_state == PaneReadiness::BareShell && attempt < budget.relaunch_attempts {
            relaunch();
        } else {
            break;
        }
    }
    ReadinessOutcome::FellBack
}

/// Gate an agent pane before boot-block injection (design D1, G1; dialog
/// handling GP-03b).
///
/// Polls `tmux capture-pane` for a CLI-readiness marker. If the pane is still a
/// bare shell when the per-attempt timeout elapses, relaunches `cli_command`
/// into the pane (clearing the input line with `C-u` first, as the launch path
/// does) and re-polls, up to the relaunch budget. An unrecognised CLI whose UI
/// matches no marker falls back to [`ReadinessOutcome::FellBack`] so the caller
/// injects anyway — never worse than the prior fixed-sleep launch.
///
/// A recognised first-run acceptance/trust dialog is answered by selecting
/// its affirmative option (see [`DIALOG_MARKERS`]); see
/// [`ReadinessOutcome::DialogStuck`] for what the caller must do when it
/// cannot be resolved.
#[must_use]
pub fn gate_pane_for_injection(
    session_name: &str,
    pane_index: usize,
    cli_command: &str,
) -> ReadinessOutcome {
    gate_pane_generic(
        ReadinessBudget::default(),
        || crate::supervisor::permission_prompt::capture_pane(session_name, pane_index),
        || relaunch_cli_into_pane(&RealCommandRunner, session_name, pane_index, cli_command),
        std::thread::sleep,
        |digit| answer_dialog_in_pane(&RealCommandRunner, session_name, pane_index, digit),
    )
}

/// Answers a recognised first-run acceptance/trust dialog by selecting option
/// `digit` (GP-03b): sends the digit key followed by `Enter`. Best-effort —
/// tmux errors are swallowed, matching [`relaunch_cli_into_pane`]'s posture; a
/// pane that fails to respond still surfaces as
/// [`ReadinessOutcome::DialogStuck`] once the launch budget elapses.
pub(crate) fn answer_dialog_in_pane(
    runner: &dyn CommandRunner,
    session_name: &str,
    pane_index: usize,
    digit: u8,
) {
    let target = format!("{session_name}:0.{pane_index}");
    let key = digit.to_string();
    let _ = runner.run_inheriting_stdio("tmux", &["send-keys", "-t", &target, &key, "Enter"]);
}

/// Relaunch `cli_command` into a pane that never reached readiness: clear the
/// input line with `C-u` (matching the launch path) then send the command and
/// `Enter`. Best-effort — tmux errors are swallowed so the fall-back injection
/// still proceeds.
///
/// Runs through the [`CommandRunner`] seam so the two-step send-keys argv is
/// assertable without a live pane. Both invocations inherit git-paw's stdio, as
/// the previous inline `Command::new("tmux")…status()` calls did, so a tmux
/// diagnostic still reaches the user's stderr even though the status is ignored.
pub(crate) fn relaunch_cli_into_pane(
    runner: &dyn CommandRunner,
    session_name: &str,
    pane_index: usize,
    cli_command: &str,
) {
    let target = format!("{session_name}:0.{pane_index}");
    let _ = runner.run_inheriting_stdio("tmux", &["send-keys", "-t", &target, "C-u"]);
    let _ =
        runner.run_inheriting_stdio("tmux", &["send-keys", "-t", &target, cli_command, "Enter"]);
}
