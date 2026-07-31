//! Launch-readiness gate for agent CLI panes (design D1, G1).
//!
//! Classifies a captured pane as ready / bare-shell / indeterminate and polls
//! (with a bounded relaunch budget) before boot-block injection.

use std::process::Command;
use std::time::Duration;

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
    /// Neither ready nor an obvious bare shell (e.g. a blank/clearing screen
    /// or an unrecognised CLI). Wait, then conservatively fall back.
    Indeterminate,
}

/// Outcome of gating a pane before boot-block injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateOutcome {
    /// A CLI-readiness marker was observed; inject the boot block.
    Ready,
    /// The readiness budget elapsed without a positive classification (an
    /// unrecognised CLI, or a relaunched-but-never-ready pane). The caller
    /// injects anyway — behaviour matches the prior fixed-sleep launch.
    FellBack,
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
        PaneReadiness::Ready
    } else if looks_like_bare_shell(captured) {
        PaneReadiness::BareShell
    } else {
        PaneReadiness::Indeterminate
    }
}

/// Core readiness loop, generic over the capture, relaunch, and sleep
/// primitives so it is unit-testable without a live tmux server or wall-clock
/// waits (design D1).
///
/// Polls `capture` on `budget.poll_interval` until a [`PaneReadiness::Ready`]
/// classification is seen or `budget.timeout` elapses. On a bare-shell timeout
/// it invokes `relaunch` and re-polls, up to `budget.relaunch_attempts`. An
/// indeterminate or persistently-bare pane returns [`GateOutcome::FellBack`].
pub(crate) fn gate_pane_generic<C, R, S>(
    budget: ReadinessBudget,
    mut capture: C,
    mut relaunch: R,
    mut sleep: S,
) -> GateOutcome
where
    C: FnMut() -> Option<String>,
    R: FnMut(),
    S: FnMut(Duration),
{
    for attempt in 0..=budget.relaunch_attempts {
        let mut waited = Duration::ZERO;
        loop {
            let captured = capture().unwrap_or_default();
            if classify_pane_readiness(&captured) == PaneReadiness::Ready {
                return GateOutcome::Ready;
            }
            if waited >= budget.timeout {
                break;
            }
            sleep(budget.poll_interval);
            waited = waited.saturating_add(budget.poll_interval);
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
    GateOutcome::FellBack
}

/// Gate an agent pane before boot-block injection (design D1, G1).
///
/// Polls `tmux capture-pane` for a CLI-readiness marker. If the pane is still a
/// bare shell when the per-attempt timeout elapses, relaunches `cli_command`
/// into the pane (clearing the input line with `C-u` first, as the launch path
/// does) and re-polls, up to the relaunch budget. An unrecognised CLI whose UI
/// matches no marker falls back to [`GateOutcome::FellBack`] so the caller
/// injects anyway — never worse than the prior fixed-sleep launch.
#[must_use]
pub fn gate_pane_for_injection(
    session_name: &str,
    pane_index: usize,
    cli_command: &str,
) -> GateOutcome {
    gate_pane_generic(
        ReadinessBudget::default(),
        || crate::supervisor::permission_prompt::capture_pane(session_name, pane_index),
        || relaunch_cli_into_pane(session_name, pane_index, cli_command),
        std::thread::sleep,
    )
}

/// Relaunch `cli_command` into a pane that never reached readiness: clear the
/// input line with `C-u` (matching the launch path) then send the command and
/// `Enter`. Best-effort — tmux errors are swallowed so the fall-back injection
/// still proceeds.
fn relaunch_cli_into_pane(session_name: &str, pane_index: usize, cli_command: &str) {
    let target = format!("{session_name}:0.{pane_index}");
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &target, "C-u"])
        .status();
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", &target, cli_command, "Enter"])
        .status();
}
