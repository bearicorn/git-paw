//! Ratatui TUI status table for pane 0.
//!
//! A thin client of the broker: it polls `GET /status` and
//! `GET /log?since=<seq>` through [`BrokerClient`] on its idle cadence and
//! renders a read-only agent status table. It holds no broker state, so
//! closing it never affects the broker, and an unreachable broker is a
//! degraded display, never an exit. The dashboard is display-only — the only
//! interaction is quitting with `q` and the broker-log panel's keys.
//!
//! The dashboard is structured as **Model-View-Update**:
//!
//! - the **Model** ([`Model`]) holds the redraw state — the derived agent rows,
//!   the footer status line, the Broker log buffer, and the quit flag;
//! - the **View** ([`mod@view`]) is a pure function of the Model producing the
//!   rendered frame;
//! - the **Update** ([`update`]) applies a [`Msg`] — a key press, a status
//!   snapshot, or a broker-log ingest — to the Model, and performs no I/O.
//!
//! This module keeps what MVU cannot: the terminal lifecycle, the SIGHUP/
//! `poll_tty` FFI, the orphan-exit gate, and the event loop that drains
//! crossterm events into `Msg` values and renders the resulting Model.

pub mod broker_log;
pub mod view;

use std::io::{self, Stdout};
use std::time::{Duration, Instant, SystemTime};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::broker::client::{BrokerClient, BrokerStatus};
use crate::broker::{AgentStatusEntry, WaveStatus};
use crate::dashboard::broker_log::{BrokerLog, LogEntry, LogKeyAction};
use crate::error::PawError;

pub use view::{
    AgentRow, AgentTableRow, arrange_with_supervisor_pinned, format_age, format_agent_rows,
    format_status_line, format_status_line_with_wave, render_dashboard, status_symbol,
};

/// Idle refresh interval for the dashboard draw loop.
///
/// The loop waits in `poll_tty(TICK_INTERVAL)`, so a keystroke wakes it
/// immediately — typing latency is near-zero and independent of this value —
/// while an *idle* dashboard only re-renders the broker-state snapshot once
/// per interval instead of busy-redrawing. 800ms (~1.25 Hz) keeps the status
/// panel current without burning CPU on a near-static view; the previous 50ms
/// (~20 Hz) unconditional redraw was the ~10%-per-dashboard idle cost. Input
/// stays instant regardless, since a keystroke wakes the blocking poll.
const TICK_INTERVAL: Duration = Duration::from_millis(800);

/// Returns `true` when this dashboard process has been orphaned — its parent
/// died and it was reparented to init (PID 1).
///
/// `git paw start` launches the dashboard as a child of its tmux pane. When
/// the session or pane is torn down, tmux normally delivers SIGHUP (see the
/// handler in `cmd_dashboard`) and the draw loop exits. But teardown paths
/// that skip SIGHUP — an abrupt `tmux kill-server`, a crash, the machine
/// sleeping, or an e2e test dropping the session — leave the dashboard alive,
/// reparented to PID 1, where it would otherwise busy-render to a dead
/// terminal forever (the leaked-process CPU-drain bug). Polling `getppid` each
/// tick lets the loop notice the reparent and exit on its own, so no dashboard
/// can outlive its session however that session ended.
#[cfg(unix)]
#[must_use]
pub fn orphaned() -> bool {
    // SAFETY: `getppid` is async-signal-safe, takes no arguments, and cannot
    // fail — it just returns this process's current parent PID.
    unsafe extern "C" {
        fn getppid() -> i32;
    }
    (unsafe { getppid() }) == 1
}

/// Non-unix stub: reparent-to-init is a POSIX concept and the dashboard only
/// runs on unix (tmux). Always reports "not orphaned".
#[cfg(not(unix))]
#[must_use]
pub fn orphaned() -> bool {
    false
}

/// Outcome of one draw-loop input wait ([`poll_tty`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TtyPoll {
    /// The interval elapsed with no input pending — re-run the gate and render.
    Timeout,
    /// Input is readable now — drain and handle events.
    Readable,
    /// The controlling terminal hung up (pane/pty destroyed) — the loop must exit.
    HangUp,
}

/// Waits up to `timeout` for input on the controlling terminal (stdin, fd 0),
/// reporting timeout / readable / hang-up.
///
/// This replaces a bare `crossterm::event::poll(TICK_INTERVAL)` for the *outer*
/// wait because that call does not return on a dead terminal: when the tmux pane
/// is torn down (`git paw stop`, `kill-session`, a crash), the pty hangs up, and
/// a hung-up fd is *perpetually* ready to `poll(2)`. crossterm therefore keeps
/// seeing readiness, reads EOF, and busy-loops internally — never returning to
/// the draw loop, never honoring its timeout, so the loop's lifecycle gate
/// ([`should_exit`]) is never re-reached and the orphaned process spins at ~100%
/// CPU forever. Polling the fd
/// ourselves lets us (a) honor the timeout so the gate re-runs every tick
/// (catching the reparent case) and (b) detect `POLLHUP`/`POLLERR`/`POLLNVAL`
/// and exit instead of trapping. Only when the fd is readable *without* hang-up
/// do we delegate to crossterm's non-blocking read, which then returns promptly.
#[cfg(unix)]
fn poll_tty(timeout: Duration) -> TtyPoll {
    // poll(2) event bits — identical values on Linux and macOS.
    const POLLIN: i16 = 0x0001;
    const POLLERR: i16 = 0x0008;
    const POLLHUP: i16 = 0x0010;
    const POLLNVAL: i16 = 0x0020;

    #[repr(C)]
    struct PollFd {
        fd: i32,
        events: i16,
        revents: i16,
    }

    // SAFETY: `poll` is async-signal-safe; we pass one initialized `PollFd`,
    // `nfds` = 1 matching that single element, and a millisecond timeout. `poll`
    // only writes `revents`, which we read back afterwards.
    unsafe extern "C" {
        fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    }

    let mut pfd = PollFd {
        fd: 0, // stdin — the tmux pane's pty
        events: POLLIN,
        revents: 0,
    };
    let ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: see the extern block above — `&mut pfd` is a single valid PollFd.
    let rc = unsafe { poll(&raw mut pfd, 1, ms) };

    if rc < 0 {
        // EINTR or an unexpected error. Sleep out the interval so a persistent
        // error (e.g. a bad fd) degrades to a quiet tick rather than a busy
        // loop, then report a timeout so the gate — which checks `orphaned()` —
        // still runs and can exit.
        std::thread::sleep(timeout);
        return TtyPoll::Timeout;
    }
    if rc == 0 {
        return TtyPoll::Timeout;
    }
    if pfd.revents & (POLLHUP | POLLERR | POLLNVAL) != 0 {
        return TtyPoll::HangUp;
    }
    if pfd.revents & POLLIN != 0 {
        return TtyPoll::Readable;
    }
    TtyPoll::Timeout
}

/// Non-unix stub: the dashboard only runs on unix (tmux). Sleeps out the
/// interval and reports a timeout so the draw loop keeps ticking.
#[cfg(not(unix))]
fn poll_tty(timeout: Duration) -> TtyPoll {
    std::thread::sleep(timeout);
    TtyPoll::Timeout
}

/// Waits up to `timeout` for the controlling terminal (stdin) to hang up,
/// returning `true` when it has.
///
/// For a process that renders nothing and never reads its terminal — the
/// broker host in its service surface. Unlike [`poll_tty`] it asks for no
/// input events, so keystrokes typed into the surface can never make the wait
/// return early and spin; only a hang-up (`POLLHUP`/`POLLERR`/`POLLNVAL`)
/// ends it. A process whose stdin is not a terminal was given none, so it
/// just sleeps out the interval.
#[cfg(unix)]
#[must_use]
pub fn terminal_hung_up(timeout: Duration) -> bool {
    use std::io::IsTerminal;

    const POLLERR: i16 = 0x0008;
    const POLLHUP: i16 = 0x0010;
    const POLLNVAL: i16 = 0x0020;

    #[repr(C)]
    struct PollFd {
        fd: i32,
        events: i16,
        revents: i16,
    }

    // SAFETY: as for `poll_tty` — one initialized `PollFd`, `nfds` = 1.
    unsafe extern "C" {
        fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    }

    if !io::stdin().is_terminal() {
        std::thread::sleep(timeout);
        return false;
    }
    let mut pfd = PollFd {
        fd: 0,
        events: 0,
        revents: 0,
    };
    let ms = i32::try_from(timeout.as_millis()).unwrap_or(i32::MAX);
    // SAFETY: see the extern block above.
    let rc = unsafe { poll(&raw mut pfd, 1, ms) };
    if rc < 0 {
        std::thread::sleep(timeout);
        return false;
    }
    rc > 0 && pfd.revents & (POLLHUP | POLLERR | POLLNVAL) != 0
}

/// Non-unix stub: sleeps out the interval and reports no hang-up.
#[cfg(not(unix))]
#[must_use]
pub fn terminal_hung_up(timeout: Duration) -> bool {
    std::thread::sleep(timeout);
    false
}

/// The dashboard draw loop's lifecycle gate: returns `true` when the loop must
/// exit. It folds the three terminal conditions checked on every iteration:
///
/// - `shutdown` — a clean SIGHUP set the shutdown flag (tmux kill-session).
/// - `orphaned` — the process was reparented to init (see [`orphaned`]), so its
///   session was torn down without SIGHUP (`tmux kill-server`, a crash, sleep).
/// - `tty_gone` — the controlling terminal is gone: an `event::poll` error or a
///   failed write to the terminal was observed. This catches the
///   reparent-to-a-lingering-shell case, where `orphaned` stays `false` (the
///   parent is a live but unrelated process) yet the pane is already dead.
///
/// An unreachable broker is deliberately NOT an input: the dashboard renders
/// its degraded state and keeps polling instead of exiting.
///
/// Extracting the gate as a pure predicate lets it be evaluated identically on
/// *every* loop path — the normal poll arm and any error/degraded arm alike, so
/// no branch can bypass it and busy-loop — and makes the exit decision
/// unit-testable without a live terminal.
fn should_exit(shutdown: bool, orphaned: bool, tty_gone: bool) -> bool {
    shutdown || orphaned || tty_gone
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The dashboard's redraw state — everything the [`view`] needs to draw a
/// frame, and nothing else.
///
/// Before the MVU refactor these fields lived as local `mut` bindings inside
/// the draw loop, recomputed inline on every iteration. Collecting them into
/// one value is what makes the state transitions testable: a [`Msg`] applied
/// by [`update`] is a pure function of this struct, with no terminal attached.
#[derive(Debug)]
pub struct Model {
    /// The formatted agent rows rendered in the status table.
    pub rows: Vec<AgentRow>,
    /// The footer summary line (agent counts by status).
    pub status_line: String,
    /// The Broker log ring buffer and its panel state.
    ///
    /// Owned by the dashboard for its whole lifetime and never cleared, so a
    /// transient broker-watcher restart leaves history intact (design.md D8).
    pub broker_log: BrokerLog,
    /// Set once a quit key has been handled; the loop exits on the next check.
    pub quit: bool,
    /// Row count of the visible Broker log panel (from
    /// `[dashboard.broker_log] height_lines`).
    pub panel_height: u16,
    /// Whether the most recent poll failed to reach the broker. Rendered as
    /// the status line's "broker unreachable" indicator; the rows and log keep
    /// the last successful snapshot meanwhile.
    pub broker_unreachable: bool,
    /// The broker's `uptime_seconds` at the last successful snapshot. A later
    /// snapshot reporting less means the broker host restarted.
    pub last_uptime: Option<u64>,
    /// The `/log` epoch (the serving host's start time) last seen. A
    /// different epoch also means the host restarted — the signal that
    /// catches a restart the whole-second uptime cannot.
    pub last_epoch: Option<u64>,
    /// Whether the session was saved without a broker, so there is none to
    /// poll: rendered as "broker disabled" instead of "broker unreachable".
    pub broker_disabled: bool,
}

impl Model {
    /// Builds the dashboard's initial state, mirroring the draw loop's state
    /// before its first iteration: no rows, an empty status line, a fresh
    /// Broker log sized by `max_messages`/`default_visible`, and not quitting.
    #[must_use]
    pub fn new(max_messages: usize, default_visible: bool, panel_height: u16) -> Self {
        Self {
            rows: Vec::new(),
            status_line: String::new(),
            broker_log: BrokerLog::new(max_messages, default_visible),
            quit: false,
            panel_height,
            broker_unreachable: false,
            last_uptime: None,
            last_epoch: None,
            broker_disabled: false,
        }
    }

    /// The status line as rendered: the summary, plus the broker-unreachable
    /// indicator while the last poll failed. Independent of the stalled-wave
    /// indicator the summary itself may carry.
    #[must_use]
    pub fn displayed_status_line(&self) -> String {
        if self.broker_disabled {
            format!("{}  \u{b7}  broker disabled", self.status_line)
        } else if self.broker_unreachable {
            format!("{}  \u{b7}  \u{26a0} broker unreachable", self.status_line)
        } else {
            self.status_line.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

/// An event the dashboard reacts to, decoded from the draw loop's I/O.
///
/// The loop owns the I/O — polling the tty, reading crossterm events, taking a
/// broker snapshot — and turns each outcome into one of these values; [`update`]
/// owns the resulting state transition.
#[derive(Debug)]
pub enum Msg {
    /// A key was pressed. Offered to the Broker log panel first; a key the
    /// panel ignores falls through to the quit check.
    Key(KeyCode),
    /// The tick/redraw event: a fresh agent-status snapshot taken at `now`,
    /// from which the rows and the footer status line are derived.
    Snapshot {
        /// The agent status entries read from the broker.
        agents: Vec<AgentStatusEntry>,
        /// The instant the snapshot was taken, used to age each row.
        now: Instant,
        /// The broker's wave-progress object at the same instant, from which
        /// the footer's stalled-wave indicator is derived. `None` leaves the
        /// footer unchanged — the per-agent summary alone, as before.
        ///
        /// Read from the same `GET /status` answer as the roster, so the
        /// dashboard and an external reader can never disagree about whether
        /// the wave is stalled.
        wave: Option<WaveStatus>,
        /// The broker's `uptime_seconds` in the same answer. Lower than the
        /// last one seen means the broker host restarted, and its log begins
        /// again at sequence 1. `None` skips restart detection.
        uptime_seconds: Option<u64>,
    },
    /// The poll could not reach the broker: keep the last snapshot and show
    /// the broker-unreachable indicator.
    BrokerUnreachable,
    /// The epoch of the broker that served the last `/log` answer. A change
    /// from the one last seen is a restarted host: the cursor rewinds.
    LogEpoch(u64),
    /// The session has no broker at all, so nothing will ever answer.
    BrokerDisabled,
    /// Broker-log entries newer than the log's cursor, to append to the panel.
    BrokerIngest(Vec<LogEntry>),
}

/// Applies one [`Msg`] to the [`Model`].
///
/// Pure: it touches no terminal, spawns no process, and reads no clock — every
/// input it needs is carried by the message. That is what lets each transition
/// be asserted directly against a constructed `Model`.
pub fn update(model: &mut Model, msg: Msg) {
    match msg {
        Msg::Key(code) => {
            // Offer the key to the panel first. It returns `Ignored`
            // for keys it does not own (notably `q`), which then
            // fall through to the quit check.
            if broker_log::handle_key(&mut model.broker_log, code) == LogKeyAction::Ignored
                && should_quit(code)
            {
                model.quit = true;
            }
        }
        Msg::Snapshot {
            agents,
            now,
            wave,
            uptime_seconds,
        } => {
            model.broker_unreachable = false;
            if let Some(uptime) = uptime_seconds {
                // A restarted host numbers its log from 1 again, and an empty
                // `/log` answer holds the cursor where it was, so the restart
                // is read here: rewind the cursor so the new log is re-read
                // from 0. The buffer is kept, and its ingest skips what it
                // already holds.
                if model.last_uptime.is_some_and(|last| uptime < last) {
                    model.broker_log.rewind_cursor();
                }
                model.last_uptime = Some(uptime);
            }
            model.rows = format_agent_rows(&agents, now);
            let working = agents.iter().filter(|a| a.status == "working").count();
            let done = agents
                .iter()
                .filter(|a| a.status == "done" || a.status == "verified")
                .count();
            let blocked = agents.iter().filter(|a| a.status == "blocked").count();
            let committed = agents.iter().filter(|a| a.status == "committed").count();
            // Recomputed on every snapshot, so the indicator clears itself on
            // the first refresh that reports the wave progressing again.
            model.status_line =
                format_status_line_with_wave(agents.len(), working, done, blocked, committed, wave);
        }
        // Push the messages newer than the cursor onto the ring buffer
        // (newest ends up at the top).
        Msg::BrokerIngest(entries) => model.broker_log.ingest(entries),
        Msg::BrokerUnreachable => model.broker_unreachable = true,
        Msg::LogEpoch(epoch) => {
            // `0` is a broker predating the field: constant, never a restart.
            if epoch != 0 && model.last_epoch.is_some_and(|last| last != epoch) {
                model.broker_log.rewind_cursor();
            }
            model.last_epoch = Some(epoch);
        }
        Msg::BrokerDisabled => model.broker_disabled = true,
    }
}

// ---------------------------------------------------------------------------
// Terminal lifecycle
// ---------------------------------------------------------------------------

/// Guard that restores the terminal on drop, ensuring cleanup even on panic
/// or early return.
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

/// Enters raw mode and the alternate screen, returning a configured terminal.
fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, PawError> {
    terminal::enable_raw_mode()
        .map_err(|e| PawError::DashboardError(format!("failed to enable raw mode: {e}")))?;
    crossterm::execute!(io::stdout(), EnterAlternateScreen)
        .map_err(|e| PawError::DashboardError(format!("failed to enter alternate screen: {e}")))?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
        .map_err(|e| PawError::DashboardError(format!("failed to create terminal: {e}")))
}

/// Disables raw mode, leaves the alternate screen, and shows the cursor.
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), PawError> {
    terminal::disable_raw_mode()
        .map_err(|e| PawError::DashboardError(format!("failed to disable raw mode: {e}")))?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .map_err(|e| PawError::DashboardError(format!("failed to leave alternate screen: {e}")))?;
    terminal
        .show_cursor()
        .map_err(|e| PawError::DashboardError(format!("failed to show cursor: {e}")))
}

/// Returns true when the given key code should terminate the dashboard
/// event loop. Only `q` (lowercase, no modifiers) quits; every other key
/// — including `Tab`, printable characters, and arrow keys — is ignored.
///
/// The supervisor-as-pane removal (v0.5.0) deleted the prompt inbox, so
/// the dashboard has no input buffer to accumulate characters into and
/// no focusable element for `Tab` to advance through.
pub(crate) fn should_quit(code: KeyCode) -> bool {
    matches!(code, KeyCode::Char('q'))
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

/// Runs the dashboard TUI against the broker `client` talks to.
///
/// Polls `GET /status` and `GET /log?since=<seq>` on the idle cadence. Press
/// `q` to quit, or set `shutdown` to `true` to trigger a graceful exit (used
/// by the SIGHUP handler when tmux kills the session). The dashboard holds no
/// broker state: exiting it has no effect on the broker.
///
/// The dashboard is observation-only: it does not collect human input
/// beyond the `q`-to-quit keybind and the broker-log panel's keys.
///
/// # Errors
/// Returns [`PawError::DashboardError`] when the terminal cannot be set up
/// or restored.
pub fn run_dashboard(
    client: &BrokerClient,
    shutdown: &std::sync::atomic::AtomicBool,
) -> Result<(), PawError> {
    let mut resolve = || BrokerSource::Ready(client.clone());
    run_dashboard_with_panes(
        &mut resolve,
        shutdown,
        500,
        false,
        crate::config::BrokerLogConfig::default().height_lines,
    )
}

/// One poll of the broker: the status snapshot, then the log entries after
/// `since`. `None` when the broker could not be reached.
fn poll_broker(client: &BrokerClient, since: u64) -> Option<(BrokerStatus, u64, Vec<LogEntry>)> {
    let status = client.status().ok()?;
    let page = client.log_page(since, None).ok()?;
    let entries = page
        .entries
        .into_iter()
        .map(|e| {
            (
                e.seq,
                SystemTime::UNIX_EPOCH + Duration::from_secs(e.timestamp_unix_secs),
                e.message,
            )
        })
        .collect();
    Some((status, page.epoch, entries))
}

/// Where the dashboard's broker client comes from on a given wake-up.
#[derive(Debug, Clone)]
pub enum BrokerSource {
    /// The broker URL is known.
    Ready(BrokerClient),
    /// Not known yet — a fresh launch saves its receipt after the panes
    /// exist. Asked again on the next wake-up.
    Pending,
    /// The session was saved without a broker: nothing will ever answer.
    Disabled,
}

/// Runs the dashboard, resolving its broker client through `resolve` until
/// one is known.
///
/// `resolve` is asked on every wake-up while it has not produced a client
/// yet — a fresh launch starts the dashboard before it saves the session
/// receipt the URL is read from — and the dashboard renders degraded
/// meanwhile.
///
/// `max_messages` caps the Broker log panel's ring buffer, `default_visible`
/// sets its initial visibility, and `height_lines` sizes the visible panel's
/// vertical segment (all from `[dashboard.broker_log]`).
///
/// # Errors
/// Returns [`PawError::DashboardError`] when the terminal cannot be set up
/// or restored.
pub fn run_dashboard_with_panes(
    resolve: &mut dyn FnMut() -> BrokerSource,
    shutdown: &std::sync::atomic::AtomicBool,
    max_messages: usize,
    default_visible: bool,
    height_lines: u16,
) -> Result<(), PawError> {
    // Install a panic hook that restores the terminal before printing the panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let terminal = setup_terminal()?;
    let mut guard = TerminalGuard { terminal };

    // The redraw state. Its Broker log ring buffer is owned by the dashboard
    // process for its whole lifetime: it is fed each tick from `GET /log` via
    // a seq cursor and is never cleared, so a broker outage, pause or restart
    // leaves history intact. Every mutation below goes through `update`.
    let mut model = Model::new(max_messages, default_visible, height_lines);
    let mut client: Option<BrokerClient> = None;

    // Latches once the controlling terminal is observed to be gone — a poll
    // error or a failed terminal write. It is consulted by `should_exit` at the
    // top of every iteration, so once set the loop exits on its next pass no
    // matter which branch set it. This is the reparent-to-a-lingering-shell
    // leak path, where `orphaned()` stays false but the pane is already dead.
    let mut tty_gone = false;

    'draw: loop {
        // Unified lifecycle gate, evaluated on EVERY loop path before any work:
        // exit promptly on a clean SIGHUP (tmux kill-session), when orphaned
        // (reparented to init), or when the controlling terminal is gone.
        // Hoisting the single check here means no branch below — normal or
        // error/degraded — can bypass it and busy-render to a dead terminal.
        if should_exit(
            shutdown.load(std::sync::atomic::Ordering::Relaxed),
            orphaned(),
            tty_gone,
        ) {
            break;
        }

        // Wait up to TICK_INTERVAL for input. This yields the CPU while idle
        // instead of redrawing every tick, yet wakes the instant a key arrives
        // — decoupling typing latency from the redraw cadence. We poll the fd
        // ourselves (see `poll_tty`) rather than block in `event::poll`: on a
        // dead terminal the latter never returns, so it would trap the loop
        // before the gate above could exit. A hang-up latches tty_gone and
        // loops back to the gate; a timeout falls through to re-render.
        match poll_tty(TICK_INTERVAL) {
            TtyPoll::Timeout => {}
            TtyPoll::HangUp => {
                tty_gone = true;
                continue;
            }
            TtyPoll::Readable => {
                // Drain up to 32 pending input events before re-rendering. `q`
                // quits; the Broker log panel claims its own keys (l / a / 1-9
                // / Up / Down / Enter / Esc); everything else is ignored. A
                // poll/read error here is the same tty-gone signal — latch it
                // and return to the gate instead of propagating an error.
                for _ in 0..32 {
                    match event::poll(Duration::ZERO) {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(_) => {
                            tty_gone = true;
                            continue 'draw;
                        }
                    }
                    let Ok(ev) = event::read() else {
                        tty_gone = true;
                        continue 'draw;
                    };
                    if let Event::Key(key) = ev
                        && key.kind == KeyEventKind::Press
                    {
                        update(&mut model, Msg::Key(key.code));
                        if model.quit {
                            return restore_terminal(&mut guard.terminal);
                        }
                    }
                }
            }
        }

        // The tick/redraw: poll the broker and read the clock here — the loop
        // owns the I/O — and let `update` derive the rows and the status line
        // from them. A failed poll keeps the last snapshot and flips the
        // degraded indicator; it is never an exit condition, and the gate at
        // the top of the loop still runs on the next pass.
        if client.is_none() && !model.broker_disabled {
            match resolve() {
                BrokerSource::Ready(c) => client = Some(c),
                BrokerSource::Pending => {}
                BrokerSource::Disabled => update(&mut model, Msg::BrokerDisabled),
            }
        }
        let cursor = model.broker_log.last_seq();
        match client.as_ref().and_then(|c| poll_broker(c, cursor)) {
            Some((status, epoch, entries)) => {
                let now = Instant::now();
                let agents = status
                    .agents
                    .into_iter()
                    .map(|row| row.into_entry(now))
                    .collect();
                update(
                    &mut model,
                    Msg::Snapshot {
                        agents,
                        now,
                        wave: status.wave,
                        uptime_seconds: Some(status.uptime_seconds),
                    },
                );
                update(&mut model, Msg::LogEpoch(epoch));
                // After a restart (either signal rewound the cursor) the
                // entries just read were requested past the old cursor; the
                // rewound cursor re-reads the new log from 0 on the next
                // wake-up instead.
                if cursor == 0 || model.broker_log.last_seq() == cursor {
                    update(&mut model, Msg::BrokerIngest(entries));
                }
            }
            None if model.broker_disabled => {}
            None => update(&mut model, Msg::BrokerUnreachable),
        }

        // A failed draw means the write to the terminal failed — the same
        // tty-gone signal as a poll error. Latch it; the gate at the top of the
        // next iteration exits rather than propagating an error or spinning
        // against a dead terminal.
        if guard.terminal.draw(|f| view::render(f, &model)).is_err() {
            tty_gone = true;
        }
    }

    // Explicit restore for clean exit; guard also restores on drop as a safety net.
    restore_terminal(&mut guard.terminal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The orphan guard must run without panicking and report "not orphaned"
    /// for a normal process — the test runner is its live parent, so `getppid`
    /// is not 1. It only trips once the parent dies and the process reparents
    /// to init, which is exactly when the draw loop should exit on its own.
    #[cfg(unix)]
    #[test]
    fn orphaned_is_false_when_parent_alive() {
        assert!(
            !orphaned(),
            "a process with a live parent must not be reported orphaned"
        );
    }

    /// The lifecycle gate exits on the tty-gone signal even when the process is
    /// neither shut down nor orphaned — the reparent-to-a-lingering-shell leak
    /// path, where `orphaned()` is false but the controlling terminal is gone.
    #[test]
    fn should_exit_on_tty_gone_signal() {
        assert!(
            should_exit(false, false, true),
            "a set tty-gone signal must exit the loop even when not shut down or orphaned"
        );
    }

    /// The gate also exits on the shutdown flag and on the orphan signal, the
    /// two conditions the pre-hardening loop already honoured.
    #[test]
    fn should_exit_on_shutdown_or_orphaned() {
        assert!(
            should_exit(true, false, false),
            "shutdown must exit the loop"
        );
        assert!(
            should_exit(false, true, false),
            "orphaned (reparented to init) must exit the loop"
        );
    }

    /// The gate keeps the loop running only while all three terminal conditions
    /// are clear: not shut down, parent alive, and the controlling terminal
    /// present.
    #[test]
    fn should_not_exit_while_all_clear() {
        assert!(
            !should_exit(false, false, false),
            "the loop must continue while not shut down, not orphaned, and the tty is present"
        );
    }

    // supervisor-as-pane[-followups] dashboard input contract.
    //
    // After the prompt-inbox removal in v0.5.0 the dashboard has no
    // focused-question or input-buffer state. The tests below assert the
    // ignored-input contract for the keys most likely to confuse a user
    // who remembers the pre-removal shape (Tab to focus, printable chars
    // to type into a buffer).

    #[test]
    fn tab_key_ignored_no_buffer() {
        // Tab is not a quit key — the handler must ignore it. There is no
        // observable side effect to assert beyond `should_quit` returning
        // false, because the dashboard has no buffer or focus state for
        // Tab to mutate.
        assert!(
            !should_quit(KeyCode::Tab),
            "Tab must not quit the dashboard and must not have any other side effect (no input buffer exists)",
        );
    }

    #[test]
    fn printable_char_ignored_no_buffer() {
        // Printable characters other than `q` must be ignored — the
        // dashboard has no buffer to accumulate them into.
        assert!(
            !should_quit(KeyCode::Char('a')),
            "printable char 'a' must not quit and must not accumulate into any buffer",
        );
        assert!(
            !should_quit(KeyCode::Char(' ')),
            "space must not quit and must not accumulate into any buffer",
        );
        // Sanity-check the positive case so the test really exercises the
        // handler contract and not just a constant false.
        assert!(
            should_quit(KeyCode::Char('q')),
            "lowercase 'q' must quit the dashboard",
        );
    }

    // -----------------------------------------------------------------------
    // Model / update (MVU)
    //
    // Every assertion below runs with no terminal attached, which is the point:
    // `update` applies exactly the mutation the pre-refactor loop performed
    // inline, and it does so purely.
    // -----------------------------------------------------------------------

    /// The production default panel height (`[dashboard.broker_log]
    /// height_lines`), so the constructed Models match the launcher's.
    fn default_panel_height() -> u16 {
        crate::config::BrokerLogConfig::default().height_lines
    }

    /// Builds a status entry seen `0s` ago, for the snapshot transitions.
    fn entry(agent_id: &str, status: &str) -> AgentStatusEntry {
        AgentStatusEntry {
            agent_id: agent_id.to_string(),
            cli: "claude".to_string(),
            status: status.to_string(),
            last_seen: Instant::now(),
            last_seen_seconds: 0,
            phase: None,
            modified_files: Vec::new(),
        }
    }

    /// Builds a broker-log entry for the ingest transitions.
    fn log_entry(seq: u64, agent_id: &str) -> LogEntry {
        (
            seq,
            std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
            crate::broker::messages::BrokerMessage::Status {
                agent_id: agent_id.to_string(),
                payload: crate::broker::messages::StatusPayload {
                    status: "working".to_string(),
                    modified_files: vec![],
                    message: Some(format!("msg-{seq}")),
                    ..Default::default()
                },
            },
        )
    }

    #[test]
    fn model_new_mirrors_the_loops_initial_state() {
        // Before its first iteration the pre-refactor loop had no rows, no
        // status line, a freshly constructed Broker log, and no quit signal.
        let model = Model::new(500, true, default_panel_height());
        assert!(model.rows.is_empty(), "no rows before the first snapshot");
        assert!(
            model.status_line.is_empty(),
            "no status line before the first snapshot"
        );
        assert!(!model.quit, "the loop does not start in the quit state");
        assert_eq!(model.panel_height, default_panel_height());
        assert!(
            model.broker_log.visible,
            "the log's initial visibility comes from `default_visible`"
        );
        assert_eq!(model.broker_log.capacity(), 500);
        assert_eq!(model.broker_log.len(), 0);
        assert_eq!(model.broker_log.last_seq(), 0);
    }

    #[test]
    fn update_quit_key_sets_the_quit_flag() {
        // Pre-refactor, `q` fell through the panel as `Ignored` and hit
        // `should_quit`, which returned from the loop. The Model's quit flag is
        // that same decision, made without a terminal to restore.
        let mut model = Model::new(500, false, default_panel_height());
        update(&mut model, Msg::Key(KeyCode::Char('q')));
        assert!(model.quit, "`q` must set the quit flag");
    }

    #[test]
    fn update_key_the_panel_claims_does_not_quit() {
        // `l` toggles the panel and reports `Handled`, so the quit check is
        // never consulted — exactly as the pre-refactor short-circuit did.
        let mut model = Model::new(500, false, default_panel_height());
        update(&mut model, Msg::Key(KeyCode::Char('l')));
        assert!(
            model.broker_log.visible,
            "`l` must toggle the panel through `update`"
        );
        assert!(!model.quit, "a panel-claimed key must not quit");
    }

    #[test]
    fn update_key_neither_the_panel_nor_quit_claims_is_inert() {
        // Keys the panel ignores and `should_quit` rejects leave the Model
        // untouched — the dashboard has no buffer to accumulate them into.
        let mut model = Model::new(500, false, default_panel_height());
        for code in [KeyCode::Tab, KeyCode::Char('z'), KeyCode::Char(' ')] {
            update(&mut model, Msg::Key(code));
            assert!(!model.quit, "{code:?} must not quit the dashboard");
            assert!(
                !model.broker_log.visible,
                "{code:?} must not touch the panel"
            );
            assert!(model.rows.is_empty(), "{code:?} must not touch the rows");
        }
    }

    #[test]
    fn update_snapshot_derives_the_same_rows_and_status_line_as_the_inline_recompute() {
        // The pre-refactor loop called `format_agent_rows`, tallied the four
        // status buckets, and built the status line inline. The `Snapshot`
        // message must produce byte-identical results.
        let agents = vec![
            entry("supervisor", "working"),
            entry("feat-a", "done"),
            entry("feat-b", "blocked"),
            entry("feat-c", "committed"),
            entry("feat-d", "verified"),
        ];
        let now = Instant::now();
        let expected_rows = format_agent_rows(&agents, now);
        // total=5, working=1, done+verified=2, blocked=1, committed=1.
        let expected_status_line = format_status_line(5, 1, 2, 1, 1);

        let mut model = Model::new(500, false, default_panel_height());
        update(
            &mut model,
            Msg::Snapshot {
                agents,
                now,
                wave: None,
                uptime_seconds: None,
            },
        );

        assert_eq!(
            model.rows, expected_rows,
            "the snapshot must yield the same rows the loop computed inline"
        );
        assert_eq!(
            model.status_line, expected_status_line,
            "the snapshot must yield the same counters the loop computed inline"
        );
    }

    #[test]
    fn update_snapshot_replaces_rather_than_accumulates_rows() {
        // Each tick re-derived the rows from scratch; a second snapshot must
        // not append to the previous one.
        let mut model = Model::new(500, false, default_panel_height());
        update(
            &mut model,
            Msg::Snapshot {
                agents: vec![entry("feat-a", "working"), entry("feat-b", "working")],
                now: Instant::now(),
                wave: None,
                uptime_seconds: None,
            },
        );
        assert_eq!(model.rows.len(), 2);

        update(
            &mut model,
            Msg::Snapshot {
                agents: vec![entry("feat-a", "done")],
                now: Instant::now(),
                wave: None,
                uptime_seconds: None,
            },
        );
        assert_eq!(
            model.rows.len(),
            1,
            "a snapshot replaces the derived rows, it does not accumulate"
        );
        assert_eq!(model.status_line, format_status_line(1, 0, 1, 0, 0));
    }

    /// `broker-dashboard`'s two scenarios at the REFRESH level, not the
    /// formatter level: a snapshot carrying a stalled wave puts the
    /// indicator in the model's status line, and the next snapshot
    /// reporting progress takes it away.
    ///
    /// The render tests build the status line by hand, so they cannot catch
    /// the draw loop passing `wave: None` — exactly the regression that
    /// would silently un-ship this feature.
    #[test]
    fn a_snapshot_carrying_a_stalled_wave_puts_the_indicator_in_the_status_line() {
        let wave = |idle_seconds, stalled| {
            Some(WaveStatus {
                last_progress_unix_secs: 1_000,
                idle_seconds,
                stall_threshold_seconds: 1_800,
                stalled,
            })
        };
        let mut model = Model::new(500, false, default_panel_height());

        update(
            &mut model,
            Msg::Snapshot {
                agents: vec![entry("feat-a", "working")],
                now: Instant::now(),
                wave: wave(3_600, true),
                uptime_seconds: None,
            },
        );
        assert!(
            model.status_line.contains("wave stalled") && model.status_line.contains("1h"),
            "got {:?}",
            model.status_line
        );

        update(
            &mut model,
            Msg::Snapshot {
                agents: vec![entry("feat-a", "working")],
                now: Instant::now(),
                wave: wave(3, false),
                uptime_seconds: None,
            },
        );
        assert!(
            !model.status_line.contains("wave stalled"),
            "the next progressing refresh must clear it; got {:?}",
            model.status_line
        );
    }

    #[test]
    fn update_broker_ingest_appends_the_same_entries_as_a_direct_ingest() {
        // The pre-refactor loop called `broker_log.ingest(...)` directly. The
        // `BrokerIngest` message must leave the log in the same state, cursor
        // included.
        let entries = vec![log_entry(1, "feat-a"), log_entry(2, "feat-b")];

        let mut model = Model::new(500, true, default_panel_height());
        update(&mut model, Msg::BrokerIngest(entries.clone()));

        let mut expected = BrokerLog::new(500, true);
        expected.ingest(entries);

        assert_eq!(
            model.broker_log.last_seq(),
            expected.last_seq(),
            "the ingest must advance the seq cursor identically"
        );
        assert_eq!(
            model.broker_log.iter_visible().collect::<Vec<_>>(),
            expected.iter_visible().collect::<Vec<_>>(),
            "the ingest must append the same rows the loop appended"
        );
    }

    /// A status snapshot at `uptime_seconds` with one working agent.
    fn snapshot_at(uptime_seconds: u64) -> Msg {
        Msg::Snapshot {
            agents: vec![entry("feat-a", "working")],
            now: Instant::now(),
            wave: None,
            uptime_seconds: Some(uptime_seconds),
        }
    }

    /// `broker-dashboard` "Paused broker shows the indicator": a failed poll
    /// shows the indicator and keeps the last snapshot's rows.
    #[test]
    fn an_unreachable_broker_shows_the_indicator_and_keeps_the_last_snapshot() {
        let mut model = Model::new(500, false, default_panel_height());
        update(&mut model, snapshot_at(10));
        let rows = model.rows.clone();

        update(&mut model, Msg::BrokerUnreachable);

        assert!(
            model.displayed_status_line().contains("broker unreachable"),
            "got {:?}",
            model.displayed_status_line()
        );
        assert_eq!(model.rows, rows, "the agent table keeps the last snapshot");
        assert!(
            !model.quit,
            "an unreachable broker is never an exit condition"
        );
    }

    /// `broker-dashboard` "Recovered broker clears the indicator".
    #[test]
    fn the_first_successful_poll_clears_the_indicator() {
        let mut model = Model::new(500, false, default_panel_height());
        update(&mut model, Msg::BrokerUnreachable);
        update(&mut model, snapshot_at(1));
        assert!(
            !model.displayed_status_line().contains("broker unreachable"),
            "got {:?}",
            model.displayed_status_line()
        );
    }

    /// The degraded indicator is independent of the stalled-wave one: both
    /// can show at once.
    #[test]
    fn the_unreachable_indicator_is_independent_of_the_stalled_wave() {
        let mut model = Model::new(500, false, default_panel_height());
        update(
            &mut model,
            Msg::Snapshot {
                agents: vec![entry("feat-a", "working")],
                now: Instant::now(),
                wave: Some(WaveStatus {
                    last_progress_unix_secs: 0,
                    idle_seconds: 3_600,
                    stall_threshold_seconds: 1_800,
                    stalled: true,
                }),
                uptime_seconds: Some(5),
            },
        );
        update(&mut model, Msg::BrokerUnreachable);
        let line = model.displayed_status_line();
        assert!(
            line.contains("wave stalled") && line.contains("broker unreachable"),
            "got {line:?}"
        );
    }

    /// `broker-dashboard` "Buffer survives a broker outage": history stays,
    /// and new messages keep arriving after the broker answers again.
    #[test]
    fn the_buffer_survives_a_broker_outage() {
        let mut model = Model::new(500, true, default_panel_height());
        update(&mut model, snapshot_at(10));
        update(
            &mut model,
            Msg::BrokerIngest(vec![log_entry(1, "feat-a"), log_entry(2, "feat-a")]),
        );
        update(&mut model, Msg::BrokerUnreachable);
        update(&mut model, snapshot_at(20));
        update(&mut model, Msg::BrokerIngest(vec![log_entry(3, "feat-a")]));
        assert_eq!(model.broker_log.len(), 3);
        assert_eq!(model.broker_log.last_seq(), 3);
    }

    /// `broker-dashboard` "Fresh broker log is ingested without duplicates":
    /// an uptime lower than the last one seen is a restarted host, so the
    /// cursor rewinds; the old rows stay and the new log is appended once.
    #[test]
    fn a_restarted_broker_log_is_ingested_without_duplicates() {
        let mut model = Model::new(500, true, default_panel_height());
        update(&mut model, snapshot_at(500));
        update(
            &mut model,
            Msg::BrokerIngest((1..=40).map(|s| log_entry(s, "feat-a")).collect()),
        );
        assert_eq!(model.broker_log.last_seq(), 40);

        // The restarted host answers with a lower uptime.
        update(&mut model, snapshot_at(3));
        assert_eq!(
            model.broker_log.last_seq(),
            0,
            "a restarted host rewinds the cursor"
        );

        let restarted: Vec<LogEntry> = (1..=2)
            .map(|s| {
                let (seq, _, msg) = log_entry(s, "feat-b");
                (
                    seq,
                    std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(10_000 + s),
                    msg,
                )
            })
            .collect();
        update(&mut model, Msg::BrokerIngest(restarted.clone()));
        // Re-reading the same new entries (the cursor holds, but a second
        // rewind could replay them) adds nothing.
        model.broker_log.rewind_cursor();
        update(&mut model, Msg::BrokerIngest(restarted));
        assert_eq!(
            model.broker_log.len(),
            42,
            "40 historical rows plus the new log's 2 entries, once each"
        );
    }

    /// A changed `/log` epoch is a restarted host even when the uptime did
    /// not drop (a restart inside the first second); an unchanged one is not.
    #[test]
    fn a_changed_log_epoch_rewinds_the_cursor() {
        let mut model = Model::new(500, true, default_panel_height());
        update(&mut model, Msg::LogEpoch(1_000));
        update(&mut model, Msg::BrokerIngest(vec![log_entry(7, "feat-a")]));
        update(&mut model, Msg::LogEpoch(1_000));
        assert_eq!(
            model.broker_log.last_seq(),
            7,
            "same epoch keeps the cursor"
        );
        update(&mut model, Msg::LogEpoch(1_001));
        assert_eq!(model.broker_log.last_seq(), 0, "a new epoch rewinds it");
        assert_eq!(model.broker_log.len(), 1, "history is kept");
    }

    /// A session saved without a broker says so instead of reporting an
    /// outage forever.
    #[test]
    fn a_session_without_a_broker_shows_broker_disabled_not_unreachable() {
        let mut model = Model::new(500, false, default_panel_height());
        update(&mut model, Msg::BrokerDisabled);
        let line = model.displayed_status_line();
        assert!(line.contains("broker disabled"), "got {line:?}");
        assert!(!line.contains("unreachable"), "got {line:?}");
    }

    /// The same uptime moving forward is not a restart.
    #[test]
    fn a_rising_uptime_keeps_the_cursor() {
        let mut model = Model::new(500, true, default_panel_height());
        update(&mut model, snapshot_at(5));
        update(&mut model, Msg::BrokerIngest(vec![log_entry(1, "feat-a")]));
        update(&mut model, snapshot_at(6));
        assert_eq!(model.broker_log.last_seq(), 1);
    }

    #[test]
    fn update_is_pure_and_needs_no_terminal() {
        // Purity, asserted behaviourally: two independently constructed Models
        // driven through the identical message sequence end in the same
        // observable state — and every one of these calls runs with no
        // terminal, no tty, and no process spawned.
        let now = Instant::now();
        let agents = vec![entry("feat-a", "working"), entry("supervisor", "blocked")];
        let entries = vec![log_entry(1, "feat-a")];

        let mut first = Model::new(500, true, default_panel_height());
        let mut second = Model::new(500, true, default_panel_height());
        for model in [&mut first, &mut second] {
            update(
                model,
                Msg::Snapshot {
                    agents: agents.clone(),
                    now,
                    wave: None,
                    uptime_seconds: None,
                },
            );
            update(model, Msg::BrokerIngest(entries.clone()));
            update(model, Msg::Key(KeyCode::Char('q')));
        }

        assert_eq!(first.rows, second.rows);
        assert_eq!(first.status_line, second.status_line);
        assert_eq!(first.quit, second.quit);
        assert_eq!(
            first.broker_log.iter_visible().collect::<Vec<_>>(),
            second.broker_log.iter_visible().collect::<Vec<_>>(),
        );
    }
}
