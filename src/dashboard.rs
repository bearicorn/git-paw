//! Ratatui TUI status table for pane 0.
//!
//! Reads from [`BrokerState`] on a 1-second tick
//! and renders a read-only agent status table. The v0.3.0 dashboard is
//! display-only — the only interaction is quitting with `q`.

pub mod broker_log;

use std::collections::HashMap;
use std::io::{self, Stdout};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Paragraph, Row, Table};

use crate::broker::delivery;
use crate::broker::{AgentStatusEntry, BrokerHandle, BrokerState};
use crate::dashboard::broker_log::{BrokerLog, LogKeyAction};
use crate::error::PawError;

/// Tick interval for the dashboard draw loop.
///
/// Also bounds the worst-case typing latency: any keystroke that arrives
/// mid-sleep is picked up on the next tick. 50ms is comfortably below the
/// ~100ms perceptual threshold for interactive UIs while keeping the
/// broker-state snapshot rate modest (~20 Hz against an in-process lock).
const TICK_INTERVAL: Duration = Duration::from_millis(50);

/// Placeholder shown in the agent table's CLI column when an agent's CLI
/// cannot be resolved (neither its `agent.status` payload nor the seeded
/// `agent_clis` map names one). A visible `"?"` reads as "unknown" rather
/// than a blank cell that looks like a rendering bug (W15-15).
const UNKNOWN_CLI: &str = "?";

/// The `agent_id` of the supervisor's pinned row. The supervisor is the only
/// publisher whose `phase` introspection field is surfaced unconditionally in
/// the agent table (see [`format_agent_rows`]).
const SUPERVISOR_AGENT_ID: &str = "supervisor";

/// The one `phase` value the dashboard honours on a *non-supervisor* row.
///
/// `detect-stuck` (the bundled sweep helper) publishes a synthetic
/// `agent.status` with `phase = "stuck-on-prompt"` *targeting the stalled
/// coding agent's row* so the stall is visible there without scraping panes.
/// This is a supervisor-authored alert about the subject agent, not the coding
/// agent's own introspection, so it is the documented exception to the
/// "phase is supervisor-only" rule in the `supervisor-introspection`
/// capability.
const STUCK_ON_PROMPT_PHASE: &str = "stuck-on-prompt";

/// A formatted row for display in the agent status table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRow {
    /// The agent identifier (slugified branch name).
    pub agent_id: String,
    /// The CLI name (e.g. `"claude"`).
    pub cli: String,
    /// Status symbol and label (e.g. `"🔵 working"`).
    pub status: String,
    /// Relative time since last message (e.g. `"3m ago"`).
    pub age: String,
}

/// Maps an agent status label to a Unicode symbol.
///
/// | Input | Output |
/// |---|---|
/// | `"working"` | `"🔵"` |
/// | `"done"` | `"🟢"` |
/// | `"verified"` | `"🟢"` |
/// | `"committed"` | `"🟣"` |
/// | `"blocked"` | `"🟡"` |
/// | anything else | `"⚪"` |
pub fn status_symbol(status: &str) -> &'static str {
    match status {
        "working" => "🔵",
        "done" | "verified" => "🟢",
        "committed" => "🟣",
        "blocked" => "🟡",
        _ => "⚪",
    }
}

/// Formats an elapsed duration as a human-readable relative time string.
///
/// - Less than 60 seconds: `"Xs ago"` (e.g. `"30s ago"`)
/// - 1 to 59 minutes: `"Xm ago"` (e.g. `"3m ago"`)
/// - 60 minutes or more: `"Xh Ym ago"` (e.g. `"1h 15m ago"`)
pub fn format_age(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        let mins = secs / 60;
        format!("{mins}m ago")
    } else {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        format!("{hours}h {mins}m ago")
    }
}

/// Converts raw agent status entries into formatted display rows.
///
/// The `phase` introspection field is the supervisor's lifecycle surface
/// (`supervisor-introspection` capability): when present on the supervisor
/// row, the status field renders that phase (with the matching status symbol)
/// instead of the message-type-derived label — labels like `"feedback"` (the
/// wire message type) are misleading, and the real lifecycle phase is `"sweep"`,
/// `"audit"`, `"merge"`, etc.
///
/// `phase` is honoured **only** for the supervisor row. A non-supervisor row
/// ignores its `phase` and renders the message-type-derived status label —
/// coding agents do not emit introspection phases in v0.6.0. The single
/// exception is the supervisor-published [`STUCK_ON_PROMPT_PHASE`] alert, which
/// `detect-stuck` targets at the stalled coding agent's row by design.
///
/// Pure function: performs no I/O, holds no locks, and is deterministic
/// given the same inputs.
pub fn format_agent_rows(agents: &[AgentStatusEntry], now: Instant) -> Vec<AgentRow> {
    agents
        .iter()
        .map(|agent| {
            let elapsed = now.saturating_duration_since(agent.last_seen);
            // Surface `phase` for the supervisor row, plus the one
            // supervisor-authored `stuck-on-prompt` alert that targets a
            // coding agent's row. Every other non-supervisor phase is ignored.
            let honour_phase = agent.agent_id == SUPERVISOR_AGENT_ID
                || agent.phase.as_deref() == Some(STUCK_ON_PROMPT_PHASE);
            let label = match agent.phase.as_deref() {
                Some(phase) if honour_phase => phase,
                _ => &agent.status,
            };
            let symbol = status_symbol(label);
            let cli = if agent.cli.trim().is_empty() {
                UNKNOWN_CLI.to_string()
            } else {
                agent.cli.clone()
            };
            AgentRow {
                agent_id: agent.agent_id.clone(),
                cli,
                status: format!("{symbol} {label}"),
                age: format_age(elapsed),
            }
        })
        .collect()
}

/// One entry in the dashboard's agent table, either an agent row or a
/// visual divider rendered between the pinned supervisor row and the
/// coding-agent rows beneath it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentTableRow {
    /// A normal agent row.
    Agent(AgentRow),
    /// A divider separating the pinned supervisor row from coding-agent rows.
    Divider,
}

/// Reorders a slice of `AgentRow` so the supervisor row (if present) is
/// pinned to position 0, followed by a [`AgentTableRow::Divider`], with
/// the remaining coding-agent rows in their incoming (alphabetical) order.
///
/// When no row has `agent_id == "supervisor"`, the output preserves the
/// incoming order and contains no divider.
///
/// Pure function: no I/O, no locks, deterministic.
pub fn arrange_with_supervisor_pinned(rows: Vec<AgentRow>) -> Vec<AgentTableRow> {
    let mut supervisor: Option<AgentRow> = None;
    let mut coding: Vec<AgentRow> = Vec::with_capacity(rows.len());
    for row in rows {
        if row.agent_id == "supervisor" {
            supervisor = Some(row);
        } else {
            coding.push(row);
        }
    }

    let mut out: Vec<AgentTableRow> = Vec::with_capacity(coding.len() + 2);
    if let Some(sup) = supervisor {
        out.push(AgentTableRow::Agent(sup));
        out.push(AgentTableRow::Divider);
    }
    out.extend(coding.into_iter().map(AgentTableRow::Agent));
    out
}

/// Produces a summary status line for the dashboard footer.
///
/// Returns a string like `"5 agents: 2 working, 1 done, 1 blocked, 1 committed"`.
pub fn format_status_line(
    total: usize,
    working: usize,
    done: usize,
    blocked: usize,
    committed: usize,
) -> String {
    format!(
        "{total} agents: {working} working, {done} done, {blocked} blocked, {committed} committed"
    )
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

// ---------------------------------------------------------------------------
// Draw
// ---------------------------------------------------------------------------

/// Renders one frame of the dashboard TUI to the given `Frame`.
///
/// Public wrapper around the internal `draw_frame` so integration tests can
/// drive a real frame with `ratatui::backend::TestBackend` and assert against
/// the resulting buffer.
pub fn render_dashboard(
    frame: &mut Frame,
    rows: &[AgentRow],
    status_line: &str,
    broker_log: &BrokerLog,
) {
    draw_frame(frame, rows, status_line, broker_log);
}

/// Returns the vertical layout constraints for the dashboard frame.
///
/// `show_panel = false` (the v0.5.0 layout after the prompt-inbox removal)
/// produces a three-segment layout: title, agent table, status line. This is
/// the byte-equivalent baseline the Broker log panel must reproduce when
/// hidden. `show_panel = true` appends a fourth segment for the Broker log
/// panel.
pub(crate) fn build_layout_constraints(show_panel: bool) -> Vec<Constraint> {
    if show_panel {
        vec![
            Constraint::Length(1),  // title
            Constraint::Min(0),     // agent table
            Constraint::Length(1),  // status line
            Constraint::Length(12), // broker log panel
        ]
    } else {
        vec![
            Constraint::Length(1), // title
            Constraint::Min(0),    // agent table
            Constraint::Length(1), // status line
        ]
    }
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

/// Renders one frame of the dashboard TUI.
fn draw_frame(frame: &mut Frame, rows: &[AgentRow], status_line: &str, broker_log: &BrokerLog) {
    // The prompt-inbox panel was removed in v0.5.0 (supervisor-as-pane-
    // followups D3). The supervisor pane is the human's input surface for
    // replying to `agent.question` events; the dashboard is observation-
    // only. v0.6.0 fills the freed region with the Broker log panel when
    // `broker_log.visible`; when hidden the layout is byte-equivalent to
    // the v0.5.0 three-segment shape.
    let layout_constraints = build_layout_constraints(broker_log.visible);

    let chunks = Layout::vertical(layout_constraints).split(frame.area());

    let title =
        Paragraph::new("git-paw dashboard").style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    if rows.is_empty() {
        let empty = Paragraph::new("No agents connected yet").alignment(Alignment::Center);
        frame.render_widget(empty, chunks[1]);
    } else {
        let header = Row::new(["Agent", "CLI", "Status", "Last Update"])
            .style(Style::default().add_modifier(Modifier::BOLD));
        // Pin the supervisor row to row 0 and insert a divider beneath it
        // before rendering. The arrangement is computed from the same
        // `rows` slice rather than reaching back into the snapshot —
        // tests can verify the ordering against `arrange_with_supervisor_pinned`
        // independently of ratatui internals.
        let arranged = arrange_with_supervisor_pinned(rows.to_vec());
        let divider_segment = "─".repeat(20);
        let table_rows: Vec<Row> = arranged
            .iter()
            .map(|entry| match entry {
                AgentTableRow::Agent(r) => Row::new(vec![
                    r.agent_id.clone(),
                    r.cli.clone(),
                    r.status.clone(),
                    r.age.clone(),
                ]),
                AgentTableRow::Divider => Row::new(vec![
                    divider_segment.clone(),
                    divider_segment.clone(),
                    divider_segment.clone(),
                    divider_segment.clone(),
                ])
                .style(Style::default().add_modifier(Modifier::DIM)),
            })
            .collect();
        let widths = [
            Constraint::Min(15),
            Constraint::Length(10),
            Constraint::Length(15),
            // Wide enough to render the full "Last Update" header label (11
            // chars) and relative-time values like "1h 15m ago" without
            // truncation — space reclaimed from the dropped Summary column.
            Constraint::Length(12),
        ];
        let table = Table::new(table_rows, widths).header(header);
        frame.render_widget(table, chunks[1]);
    }

    // When the Broker log panel is hidden, its title bar (which documents the
    // `l` toggle) is gone, so append a one-line restore hint to the always-
    // present status line. The agent-table/segment layout stays byte-identical
    // to v0.5.0 — only the status text gains the suffix.
    let status_text = if broker_log.visible {
        status_line.to_string()
    } else {
        format!("{status_line}  ·  broker log hidden — press l to show")
    };
    let status = Paragraph::new(status_text);
    frame.render_widget(status, chunks[2]);

    // Broker log panel: occupies the v0.5.0-freed inbox region when visible.
    // When hidden there is no fourth chunk, so the layout above is identical
    // to v0.5.0's three-segment shape (spec: "Hidden layout matches v0.5.0").
    if broker_log.visible {
        broker_log::render(frame, chunks[3], broker_log);
    }
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

/// Runs the dashboard TUI, polling broker state on a 1-second tick.
///
/// Takes ownership of [`BrokerHandle`] so the broker shuts down automatically
/// when the dashboard exits. Press `q` to quit, or set `shutdown` to `true`
/// to trigger a graceful exit (used by the SIGHUP handler when tmux kills the
/// session).
///
/// The dashboard is observation-only: it does not collect human input
/// beyond the `q`-to-quit keybind. `agent.question` messages flow through
/// the broker to the supervisor's inbox; the supervisor pane is the
/// human's input surface for replies (supervisor-as-pane-followups D3).
pub fn run_dashboard(
    state: &Arc<BrokerState>,
    broker_handle: BrokerHandle,
    shutdown: &std::sync::atomic::AtomicBool,
) -> Result<(), PawError> {
    run_dashboard_with_panes(
        state,
        broker_handle,
        shutdown,
        &HashMap::new(),
        None,
        500,
        false,
    )
}

/// Runs the dashboard with an explicit agent ID → tmux pane index map and
/// session name. Retained for source compatibility with v0.4 launchers, but
/// `pane_map` and `session_name` are now unused — the prompt-inbox panel
/// that consumed them was removed in v0.5.0.
///
/// `max_messages` caps the Broker log panel's ring buffer and
/// `default_visible` sets its initial visibility (both from
/// `[dashboard.broker_log]`).
pub fn run_dashboard_with_panes<S: std::hash::BuildHasher>(
    state: &Arc<BrokerState>,
    broker_handle: BrokerHandle,
    shutdown: &std::sync::atomic::AtomicBool,
    _pane_map: &HashMap<String, usize, S>,
    _session_name: Option<&str>,
    max_messages: usize,
    default_visible: bool,
) -> Result<(), PawError> {
    let _broker_handle = broker_handle;
    // Install a panic hook that restores the terminal before printing the panic.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let terminal = setup_terminal()?;
    let mut guard = TerminalGuard { terminal };

    // The Broker log ring buffer is owned by the dashboard process for its
    // whole lifetime. It is fed each tick from the broker's in-process
    // message log via a monotonic seq cursor and is never cleared, so a
    // transient broker-watcher restart leaves history intact (design.md D8).
    let mut broker_log = BrokerLog::new(max_messages, default_visible);

    loop {
        // Check for SIGHUP-triggered shutdown (e.g. tmux kill-session)
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        // Drain up to 32 pending input events before re-rendering. `q` quits;
        // the Broker log panel claims its own keys (l / a / 1-9 / Up / Down /
        // Enter / Esc); everything else is ignored.
        for _ in 0..32 {
            if !event::poll(Duration::ZERO)
                .map_err(|e| PawError::DashboardError(format!("event poll failed: {e}")))?
            {
                break;
            }
            let ev = event::read()
                .map_err(|e| PawError::DashboardError(format!("event read failed: {e}")))?;
            if let Event::Key(key) = ev
                && key.kind == KeyEventKind::Press
            {
                // Offer the key to the panel first. It returns `Ignored` for
                // keys it does not own (notably `q`), which then fall through
                // to the quit check.
                if broker_log::handle_key(&mut broker_log, key.code) == LogKeyAction::Ignored
                    && should_quit(key.code)
                {
                    return restore_terminal(&mut guard.terminal);
                }
            }
        }

        let agents = delivery::agent_status_snapshot(state);
        let now = Instant::now();
        let rows = format_agent_rows(&agents, now);
        let working = agents.iter().filter(|a| a.status == "working").count();
        let done = agents
            .iter()
            .filter(|a| a.status == "done" || a.status == "verified")
            .count();
        let blocked = agents.iter().filter(|a| a.status == "blocked").count();
        let committed = agents.iter().filter(|a| a.status == "committed").count();
        let status_line = format_status_line(agents.len(), working, done, blocked, committed);

        // Feed the Broker log: pull only messages newer than the cursor and
        // push them onto the ring buffer (newest ends up at the top). This is
        // the same in-process state the agent table reads — no extra traffic.
        broker_log.ingest(delivery::full_log(state, broker_log.last_seq()));

        guard
            .terminal
            .draw(|f| {
                draw_frame(f, &rows, &status_line, &broker_log);
            })
            .map_err(|e| PawError::DashboardError(format!("draw failed: {e}")))?;

        thread::sleep(TICK_INTERVAL);
    }

    // Explicit restore for clean exit; guard also restores on drop as a safety net.
    restore_terminal(&mut guard.terminal)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hidden Broker log panel for `draw_frame` calls that exercise the
    /// agent-table/observation layout. Hidden so the rendered frame is the
    /// v0.5.0 three-segment shape these assertions expect.
    fn hidden_log() -> BrokerLog {
        BrokerLog::new(500, false)
    }

    // -----------------------------------------------------------------------
    // status_symbol
    // -----------------------------------------------------------------------

    #[test]
    fn status_symbol_working() {
        assert_eq!(status_symbol("working"), "🔵");
    }

    #[test]
    fn status_symbol_done() {
        assert_eq!(status_symbol("done"), "🟢");
    }

    #[test]
    fn status_symbol_verified() {
        assert_eq!(status_symbol("verified"), "🟢");
    }

    #[test]
    fn status_symbol_blocked() {
        assert_eq!(status_symbol("blocked"), "🟡");
    }

    #[test]
    fn status_symbol_committed() {
        assert_eq!(status_symbol("committed"), "🟣");
    }

    #[test]
    fn status_symbol_idle() {
        assert_eq!(status_symbol("idle"), "⚪");
    }

    #[test]
    fn status_symbol_unknown() {
        assert_eq!(status_symbol("something-unexpected"), "⚪");
    }

    // -----------------------------------------------------------------------
    // format_age
    // -----------------------------------------------------------------------

    #[test]
    fn format_age_zero_seconds() {
        assert_eq!(format_age(Duration::from_secs(0)), "0s ago");
    }

    #[test]
    fn format_age_thirty_seconds() {
        assert_eq!(format_age(Duration::from_secs(30)), "30s ago");
    }

    #[test]
    fn format_age_three_minutes() {
        assert_eq!(format_age(Duration::from_mins(3)), "3m ago");
    }

    #[test]
    fn format_age_one_hour_exact() {
        assert_eq!(format_age(Duration::from_hours(1)), "1h 0m ago");
    }

    #[test]
    fn format_age_one_hour_fifteen_minutes() {
        assert_eq!(format_age(Duration::from_mins(75)), "1h 15m ago");
    }

    // -----------------------------------------------------------------------
    // format_agent_rows
    // -----------------------------------------------------------------------

    #[test]
    fn format_agent_rows_three_agents() {
        let now = Instant::now();
        let agents = vec![
            AgentStatusEntry {
                agent_id: "feat-a".to_string(),
                cli: "claude".to_string(),
                status: "working".to_string(),
                last_seen: now.checked_sub(Duration::from_secs(10)).unwrap(),
                last_seen_seconds: 10,
                phase: None,
            },
            AgentStatusEntry {
                agent_id: "feat-b".to_string(),
                cli: "cursor".to_string(),
                status: "done".to_string(),
                last_seen: now.checked_sub(Duration::from_mins(1)).unwrap(),
                last_seen_seconds: 60,
                phase: None,
            },
            AgentStatusEntry {
                agent_id: "feat-c".to_string(),
                cli: "claude".to_string(),
                status: "blocked".to_string(),
                last_seen: now.checked_sub(Duration::from_mins(5)).unwrap(),
                last_seen_seconds: 300,
                phase: None,
            },
        ];
        let rows = format_agent_rows(&agents, now);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].agent_id, "feat-a");
        assert_eq!(rows[1].agent_id, "feat-b");
        assert_eq!(rows[2].agent_id, "feat-c");
    }

    #[test]
    fn format_agent_rows_single_done_three_minutes() {
        let now = Instant::now();
        let agents = vec![AgentStatusEntry {
            agent_id: "feat-errors".to_string(),
            cli: "claude".to_string(),
            status: "done".to_string(),
            last_seen: now.checked_sub(Duration::from_mins(3)).unwrap(),
            last_seen_seconds: 180,
            phase: None,
        }];
        let rows = format_agent_rows(&agents, now);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].agent_id, "feat-errors");
        assert_eq!(rows[0].age, "3m ago");
        assert!(rows[0].status.contains("done"));
    }

    #[test]
    fn format_agent_rows_with_committed_status() {
        let now = Instant::now();
        let agents = vec![
            AgentStatusEntry {
                agent_id: "feat-committed".to_string(),
                cli: "claude".to_string(),
                status: "committed".to_string(),
                last_seen: now.checked_sub(Duration::from_mins(1)).unwrap(),
                last_seen_seconds: 60,
                phase: None,
            },
            AgentStatusEntry {
                agent_id: "feat-working".to_string(),
                cli: "cursor".to_string(),
                status: "working".to_string(),
                last_seen: now.checked_sub(Duration::from_secs(30)).unwrap(),
                last_seen_seconds: 30,
                phase: None,
            },
        ];
        let rows = format_agent_rows(&agents, now);
        assert_eq!(rows.len(), 2);

        // Find the committed agent and verify it has the correct symbol
        let committed_row = rows
            .iter()
            .find(|r| r.agent_id == "feat-committed")
            .unwrap();
        assert!(committed_row.status.contains("🟣"));
        assert!(committed_row.status.contains("committed"));

        // Find the working agent and verify it has the correct symbol
        let working_row = rows.iter().find(|r| r.agent_id == "feat-working").unwrap();
        assert!(working_row.status.contains("🔵"));
        assert!(working_row.status.contains("working"));
    }

    #[test]
    fn format_agent_rows_empty_input() {
        let rows = format_agent_rows(&[], Instant::now());
        assert!(rows.is_empty());
    }

    #[test]
    fn agent_row_exposes_only_four_fields_no_summary() {
        // Scenario: AgentRow exposes no summary field. The agent-status table
        // no longer renders a Summary column, so the row struct carries exactly
        // `agent_id`, `cli`, `status`, `age` and nothing else. This construction
        // names every field exhaustively — if a `summary` (or any other) field
        // were reintroduced, this would fail to compile.
        let now = Instant::now();
        let agents = vec![AgentStatusEntry {
            agent_id: "feat-errors".to_string(),
            cli: "claude".to_string(),
            status: "done".to_string(),
            last_seen: now.checked_sub(Duration::from_mins(3)).unwrap(),
            last_seen_seconds: 180,
            phase: None,
        }];
        let rows = format_agent_rows(&agents, now);
        assert_eq!(rows.len(), 1);
        let AgentRow {
            agent_id,
            cli,
            status,
            age,
        } = &rows[0];
        assert_eq!(agent_id, "feat-errors");
        assert_eq!(cli, "claude");
        assert!(status.contains("done"));
        assert_eq!(age, "3m ago");
    }

    // -----------------------------------------------------------------------
    // CLI column population (W15-15)
    // -----------------------------------------------------------------------

    #[test]
    fn format_agent_rows_populates_cli_for_every_agent() {
        // W15-15: the CLI column was blank for coding agents (only the
        // supervisor row carried a CLI). Every row must render its CLI.
        let now = Instant::now();
        let agents = vec![
            AgentStatusEntry {
                agent_id: "supervisor".to_string(),
                cli: "claude-oss".to_string(),
                status: "working".to_string(),
                last_seen: now,
                last_seen_seconds: 0,
                phase: Some("watching".to_string()),
            },
            AgentStatusEntry {
                agent_id: "feat-a".to_string(),
                cli: "claude-oss".to_string(),
                status: "working".to_string(),
                last_seen: now,
                last_seen_seconds: 0,
                phase: None,
            },
            AgentStatusEntry {
                agent_id: "feat-b".to_string(),
                cli: "claude-oss".to_string(),
                status: "working".to_string(),
                last_seen: now,
                last_seen_seconds: 0,
                phase: None,
            },
        ];
        let rows = format_agent_rows(&agents, now);
        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert_eq!(
                row.cli, "claude-oss",
                "every agent row must render its CLI, not just the supervisor: {row:?}",
            );
        }
    }

    #[test]
    fn format_agent_rows_shows_placeholder_for_unresolved_cli() {
        // W15-15: an unresolved CLI shows the documented "?" placeholder
        // rather than a blank cell that reads as a rendering bug.
        let now = Instant::now();
        let agents = vec![AgentStatusEntry {
            agent_id: "feat-mystery".to_string(),
            cli: String::new(),
            status: "working".to_string(),
            last_seen: now,
            last_seen_seconds: 0,
            phase: None,
        }];
        let rows = format_agent_rows(&agents, now);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].cli, UNKNOWN_CLI,
            "blank CLI must render the documented placeholder, not an empty string",
        );
        assert!(!rows[0].cli.is_empty());
    }

    // -----------------------------------------------------------------------
    // Bug 8: dashboard accepts committed -> working re-entry
    // -----------------------------------------------------------------------

    #[test]
    fn dashboard_row_transitions_committed_to_working_within_ttl() {
        use crate::broker::BrokerState;
        use crate::broker::delivery::{agent_status_snapshot, publish_message};
        use crate::broker::messages::{ArtifactPayload, BrokerMessage, StatusPayload};
        use std::sync::Arc;

        let state = Arc::new(BrokerState::new(None)); // default TTL 60s
        publish_message(
            &state,
            &BrokerMessage::Artifact {
                agent_id: "feat-x".to_string(),
                payload: ArtifactPayload {
                    status: "committed".to_string(),
                    exports: vec![],
                    modified_files: vec![],
                },
            },
        );
        // Render shows committed.
        let snap = agent_status_snapshot(&state);
        let rows = format_agent_rows(&snap, Instant::now());
        let row = rows.iter().find(|r| r.agent_id == "feat-x").unwrap();
        assert!(row.status.contains("committed"), "should start committed");

        // Agent keeps working within the TTL window.
        publish_message(
            &state,
            &BrokerMessage::Status {
                agent_id: "feat-x".to_string(),
                payload: StatusPayload {
                    status: "working".to_string(),
                    modified_files: vec!["src/lib.rs".to_string()],
                    message: None,
                    ..Default::default()
                },
            },
        );
        let snap = agent_status_snapshot(&state);
        let rows = format_agent_rows(&snap, Instant::now());
        let row = rows.iter().find(|r| r.agent_id == "feat-x").unwrap();
        assert!(
            row.status.contains("working") && row.status.contains("🔵"),
            "dashboard row must transition committed -> working, got {:?}",
            row.status
        );
    }

    #[test]
    fn dashboard_row_stays_committed_when_ttl_zero() {
        // v0.5.0 byte-equivalence: with TTL=0 the row stays committed.
        use crate::broker::BrokerState;
        use crate::broker::delivery::{agent_status_snapshot, publish_message};
        use crate::broker::messages::{ArtifactPayload, BrokerMessage, StatusPayload};
        use std::sync::Arc;

        let state = Arc::new(BrokerState::new(None));
        state.set_republish_working_ttl(Duration::ZERO);
        publish_message(
            &state,
            &BrokerMessage::Artifact {
                agent_id: "feat-y".to_string(),
                payload: ArtifactPayload {
                    status: "committed".to_string(),
                    exports: vec![],
                    modified_files: vec![],
                },
            },
        );
        publish_message(
            &state,
            &BrokerMessage::Status {
                agent_id: "feat-y".to_string(),
                payload: StatusPayload {
                    status: "working".to_string(),
                    modified_files: vec!["src/lib.rs".to_string()],
                    message: None,
                    ..Default::default()
                },
            },
        );
        let snap = agent_status_snapshot(&state);
        let rows = format_agent_rows(&snap, Instant::now());
        let row = rows.iter().find(|r| r.agent_id == "feat-y").unwrap();
        assert!(
            row.status.contains("committed"),
            "with TTL=0 the dashboard row must stay committed, got {:?}",
            row.status
        );
    }

    // -----------------------------------------------------------------------
    // Phase-aware status rendering (tasks 5.4, 5.5)
    // -----------------------------------------------------------------------

    #[test]
    fn format_agent_rows_prefers_phase_over_status_for_supervisor() {
        let now = Instant::now();
        let agents = vec![AgentStatusEntry {
            agent_id: "supervisor".to_string(),
            cli: "claude".to_string(),
            status: "feedback".to_string(),
            last_seen: now,
            last_seen_seconds: 0,
            phase: Some("merging".to_string()),
        }];
        let rows = format_agent_rows(&agents, now);
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0].status.contains("merging"),
            "expected phase 'merging' in status field; got {:?}",
            rows[0].status,
        );
        assert!(
            !rows[0].status.contains("feedback"),
            "phase must replace status label, not append; got {:?}",
            rows[0].status,
        );
    }

    #[test]
    fn format_agent_rows_falls_back_to_status_when_phase_is_none() {
        let now = Instant::now();
        let agents = vec![AgentStatusEntry {
            agent_id: "feat-broker".to_string(),
            cli: "claude".to_string(),
            status: "working".to_string(),
            last_seen: now,
            last_seen_seconds: 0,
            phase: None,
        }];
        let rows = format_agent_rows(&agents, now);
        assert!(
            rows[0].status.contains("working"),
            "expected 'working' in status field; got {:?}",
            rows[0].status,
        );
    }

    // -----------------------------------------------------------------------
    // supervisor-introspection: phase honoured for supervisor row only
    // (tasks 3.1 - 3.4)
    // -----------------------------------------------------------------------

    /// Builds an entry with an explicit phase for the introspection tests.
    fn entry_with_phase(agent_id: &str, status: &str, phase: Option<&str>) -> AgentStatusEntry {
        AgentStatusEntry {
            agent_id: agent_id.to_string(),
            cli: "claude".to_string(),
            status: status.to_string(),
            last_seen: Instant::now(),
            last_seen_seconds: 0,
            phase: phase.map(str::to_string),
        }
    }

    #[test]
    fn format_agent_rows_supervisor_shows_introspection_phase() {
        // Scenario: supervisor row shows phase when present.
        let now = Instant::now();
        let agents = vec![entry_with_phase("supervisor", "working", Some("audit"))];
        let rows = format_agent_rows(&agents, now);
        assert!(
            rows[0].status.contains("audit"),
            "supervisor row must surface the introspection phase; got {:?}",
            rows[0].status,
        );
    }

    #[test]
    fn format_agent_rows_supervisor_falls_back_when_phase_absent() {
        // Scenario: supervisor row falls back to the status label when phase
        // absent (v0.5.0 layout preserved).
        let now = Instant::now();
        let agents = vec![entry_with_phase("supervisor", "working", None)];
        let rows = format_agent_rows(&agents, now);
        assert!(
            rows[0].status.contains("working"),
            "without a phase the supervisor row renders the status label; got {:?}",
            rows[0].status,
        );
    }

    #[test]
    fn format_agent_rows_non_supervisor_ignores_phase() {
        // Scenario: non-supervisor agent rows unchanged — a coding agent that
        // set a phase still renders as v0.5.0 (phase ignored).
        let now = Instant::now();
        let agents = vec![entry_with_phase("feat-auth", "working", Some("audit"))];
        let rows = format_agent_rows(&agents, now);
        assert!(
            rows[0].status.contains("working"),
            "a coding agent's phase must be ignored; got {:?}",
            rows[0].status,
        );
        assert!(
            !rows[0].status.contains("audit"),
            "the introspection phase must not leak onto a coding-agent row; got {:?}",
            rows[0].status,
        );
    }

    #[test]
    fn format_agent_rows_non_supervisor_still_shows_stuck_on_prompt() {
        // The one documented exception: the supervisor-published
        // `stuck-on-prompt` alert targets the coding agent's row by design and
        // must remain visible there.
        let now = Instant::now();
        let agents = vec![entry_with_phase(
            "feat-auth",
            "working",
            Some(STUCK_ON_PROMPT_PHASE),
        )];
        let rows = format_agent_rows(&agents, now);
        assert!(
            rows[0].status.contains(STUCK_ON_PROMPT_PHASE),
            "the supervisor-authored stuck-on-prompt alert must surface on the \
             coding-agent row; got {:?}",
            rows[0].status,
        );
    }

    #[test]
    fn format_agent_rows_supervisor_phase_snapshot_layout() {
        // Snapshot: supervisor row with `phase` present renders the exact
        // `{symbol} {phase}` status field; without `phase` it matches the
        // v0.5.0 `{symbol} {status}` layout.
        let now = Instant::now();
        let with_phase = format_agent_rows(
            &[entry_with_phase("supervisor", "feedback", Some("merge"))],
            now,
        );
        assert_eq!(with_phase[0].status, "⚪ merge");

        let without_phase =
            format_agent_rows(&[entry_with_phase("supervisor", "working", None)], now);
        assert_eq!(without_phase[0].status, "🔵 working");
    }

    // -----------------------------------------------------------------------
    // arrange_with_supervisor_pinned (tasks 4.4 - 4.6)
    // -----------------------------------------------------------------------

    fn agent_row(id: &str) -> AgentRow {
        AgentRow {
            agent_id: id.to_string(),
            cli: "claude".to_string(),
            status: "🔵 working".to_string(),
            age: "0s ago".to_string(),
        }
    }

    #[test]
    fn arrange_with_supervisor_pinned_yields_supervisor_then_divider_then_coding() {
        let rows = vec![
            agent_row("feat-broker"),
            agent_row("feat-dashboard"),
            agent_row("supervisor"),
        ];
        let arranged = arrange_with_supervisor_pinned(rows);
        assert_eq!(arranged.len(), 4, "supervisor + divider + 2 coding agents");
        assert!(
            matches!(&arranged[0], AgentTableRow::Agent(r) if r.agent_id == "supervisor"),
            "supervisor must be at row 0; got {:?}",
            arranged[0]
        );
        assert_eq!(
            arranged[1],
            AgentTableRow::Divider,
            "divider must immediately follow supervisor"
        );
        assert!(matches!(&arranged[2], AgentTableRow::Agent(r) if r.agent_id == "feat-broker"),);
        assert!(matches!(&arranged[3], AgentTableRow::Agent(r) if r.agent_id == "feat-dashboard"),);
    }

    #[test]
    fn arrange_with_supervisor_pinned_emits_no_divider_when_supervisor_absent() {
        let rows = vec![agent_row("feat-broker"), agent_row("feat-dashboard")];
        let arranged = arrange_with_supervisor_pinned(rows);
        assert_eq!(arranged.len(), 2);
        for row in &arranged {
            assert!(
                !matches!(row, AgentTableRow::Divider),
                "no divider when supervisor is absent; got {row:?}"
            );
        }
        assert!(matches!(&arranged[0], AgentTableRow::Agent(r) if r.agent_id == "feat-broker"));
        assert!(matches!(&arranged[1], AgentTableRow::Agent(r) if r.agent_id == "feat-dashboard"));
    }

    #[test]
    fn arrange_with_supervisor_pinned_empty_input_yields_empty_output() {
        let arranged = arrange_with_supervisor_pinned(Vec::new());
        assert!(arranged.is_empty());
    }

    #[test]
    fn supervisor_row_appears_above_coding_rows_in_rendered_frame() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        // Construct three formatted rows with snapshot already in
        // alphabetical order (this matches what agent_status_snapshot
        // emits before pinning). The pinning happens inside draw_frame.
        let rows = vec![
            agent_row("feat-broker"),
            agent_row("feat-dashboard"),
            agent_row("supervisor"),
        ];

        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_frame(f, &rows, "3 agents", &hidden_log()))
            .unwrap();

        // Flatten the buffer to a single string so we can check row order
        // by substring positions across the rendered output.
        let buffer = terminal.backend().buffer().clone();
        let mut rendered = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                rendered.push_str(buffer[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        let pos_supervisor = rendered
            .find("supervisor")
            .expect("supervisor row should be in rendered frame");
        let pos_broker = rendered
            .find("feat-broker")
            .expect("feat-broker row should be in rendered frame");
        let pos_dashboard = rendered
            .find("feat-dashboard")
            .expect("feat-dashboard row should be in rendered frame");
        assert!(
            pos_supervisor < pos_broker && pos_supervisor < pos_dashboard,
            "supervisor row must render above coding-agent rows; supervisor@{pos_supervisor}, broker@{pos_broker}, dashboard@{pos_dashboard}",
        );

        // A divider row containing horizontal-line characters appears
        // between the supervisor row and the first coding-agent row.
        let pos_divider = rendered[pos_supervisor..]
            .find('─')
            .map(|p| pos_supervisor + p)
            .expect("divider row should contain horizontal-line characters");
        assert!(
            pos_divider > pos_supervisor && pos_divider < pos_broker,
            "divider must render between supervisor and first coding row; divider@{pos_divider}, supervisor@{pos_supervisor}, broker@{pos_broker}",
        );
    }

    #[test]
    fn header_row_has_four_columns_and_no_summary() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        // Scenario: Table has a header row. With at least one agent rendered,
        // the header must label exactly Agent, CLI, Status, Last Update and
        // must NOT contain a Summary column (the dead column was removed).
        let rows = vec![agent_row("feat-broker")];

        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_frame(f, &rows, "1 agent", &hidden_log()))
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let mut rendered = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                rendered.push_str(buffer[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        for label in ["Agent", "CLI", "Status", "Last Update"] {
            assert!(
                rendered.contains(label),
                "header must contain the {label:?} column label; got:\n{rendered}",
            );
        }
        assert!(
            !rendered.contains("Summary"),
            "header must NOT contain a 'Summary' column label; got:\n{rendered}",
        );
    }

    // -----------------------------------------------------------------------
    // format_status_line
    // -----------------------------------------------------------------------

    #[test]
    fn format_status_line_mixed() {
        assert_eq!(
            format_status_line(4, 2, 1, 1, 0),
            "4 agents: 2 working, 1 done, 1 blocked, 0 committed"
        );
    }

    #[test]
    fn format_status_line_all_done() {
        assert_eq!(
            format_status_line(3, 0, 3, 0, 0),
            "3 agents: 0 working, 3 done, 0 blocked, 0 committed"
        );
    }

    #[test]
    fn format_status_line_zero_agents() {
        assert_eq!(
            format_status_line(0, 0, 0, 0, 0),
            "0 agents: 0 working, 0 done, 0 blocked, 0 committed"
        );
    }

    #[test]
    fn format_status_line_with_committed() {
        assert_eq!(
            format_status_line(5, 2, 1, 1, 1),
            "5 agents: 2 working, 1 done, 1 blocked, 1 committed"
        );
    }

    // -----------------------------------------------------------------------
    // Prompt-inbox removal (tasks 6.8, 6.9)
    // -----------------------------------------------------------------------

    #[test]
    fn rendered_frame_contains_no_questions_or_reply_input() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_frame(f, &[], "0 agents", &hidden_log()))
            .unwrap();

        let buffer = terminal.backend().buffer().clone();
        let mut rendered = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                rendered.push_str(buffer[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        assert!(
            !rendered.contains("Questions ("),
            "dashboard MUST NOT render a 'Questions (' prompt-inbox header; got:\n{rendered}",
        );
        assert!(
            !rendered.contains("Reply to"),
            "dashboard MUST NOT render a 'Reply to' input prompt; got:\n{rendered}",
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

    #[test]
    fn layout_collapses_without_message_log() {
        // With show_message_log = false the layout is three segments
        // (title, agent table, status line). The pre-inbox-removal shape
        // had 5 or 6 segments — a regression to that would imply the
        // prompt-inbox panel is back.
        let constraints = build_layout_constraints(false);
        assert_eq!(
            constraints.len(),
            3,
            "layout without message log must be exactly 3 segments (title, table, status), got {} constraints",
            constraints.len(),
        );

        // With show_message_log = true the layout adds the messages
        // panel as a 4th segment. Asserting both shapes catches the case
        // where the helper accidentally drops the messages panel or
        // grows a spurious 5th segment.
        let with_log = build_layout_constraints(true);
        assert_eq!(
            with_log.len(),
            4,
            "layout with message log must be exactly 4 segments, got {} constraints",
            with_log.len(),
        );
    }

    // -----------------------------------------------------------------------
    // Broker log layout integration (tasks 5.1-5.3)
    // -----------------------------------------------------------------------

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn draw_to_buffer(rows: &[AgentRow], status: &str, log: &broker_log::BrokerLog) -> Buffer {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_frame(f, rows, status, log)).unwrap();
        terminal.backend().buffer().clone()
    }

    fn sample_log_entry(seq: u64) -> broker_log::LogEntry {
        (
            seq,
            std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
            crate::broker::messages::BrokerMessage::Status {
                agent_id: "feat-auth".to_string(),
                payload: crate::broker::messages::StatusPayload {
                    status: "working".to_string(),
                    modified_files: vec![],
                    message: Some("rebasing onto main".to_string()),
                    ..Default::default()
                },
            },
        )
    }

    fn log_entry_with_message(seq: u64, msg: &str) -> broker_log::LogEntry {
        (
            seq,
            std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(seq),
            crate::broker::messages::BrokerMessage::Status {
                agent_id: "feat-auth".to_string(),
                payload: crate::broker::messages::StatusPayload {
                    status: "working".to_string(),
                    modified_files: vec![],
                    message: Some(msg.to_string()),
                    ..Default::default()
                },
            },
        )
    }

    fn buffer_text(buffer: &Buffer) -> String {
        let mut rendered = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                rendered.push_str(buffer[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        rendered
    }

    #[test]
    fn scrolling_reaches_messages_beyond_the_first_screen() {
        // Bug: a plain List with no offset only ever showed the first
        // screenful. With stateful-list scrolling, moving the selection to the
        // bottom must scroll the viewport so the oldest message becomes visible.
        let rows = vec![agent_row("feat-auth")];
        let mut log = BrokerLog::new(500, true);
        // 40 distinct messages; push_front means msg-00 ends up at the bottom.
        for i in 0..40 {
            log.push(log_entry_with_message(i, &format!("scroll-msg-{i:02}")));
        }
        // At offset 0 the oldest (scroll-msg-00) is off-screen.
        let at_top = buffer_text(&draw_to_buffer(&rows, "1 agents", &log));
        assert!(
            !at_top.contains("scroll-msg-00"),
            "precondition: the oldest message should be off-screen before scrolling; got:\n{at_top}"
        );
        // Move the selection to the bottom row.
        for _ in 0..39 {
            log.select_down();
        }
        let scrolled = buffer_text(&draw_to_buffer(&rows, "1 agents", &log));
        assert!(
            scrolled.contains("scroll-msg-00"),
            "scrolling to the bottom must reveal the oldest message; got:\n{scrolled}"
        );
    }

    #[test]
    fn hidden_panel_status_line_shows_restore_hint() {
        let rows = vec![agent_row("feat-auth")];
        let log = BrokerLog::new(500, false); // hidden
        let rendered = buffer_text(&draw_to_buffer(&rows, "1 agents", &log));
        assert!(
            rendered.contains("press l to show"),
            "hidden panel must hint the `l` toggle in the status line; got:\n{rendered}"
        );
        assert!(
            !rendered.contains("Broker log ("),
            "hidden panel must not render the panel title region; got:\n{rendered}"
        );
    }

    #[test]
    fn hidden_panel_layout_is_byte_equivalent_regardless_of_buffer_contents() {
        // Task 5.3: with the panel hidden, the rendered frame must match the
        // v0.5.0 post-inbox-removal layout — i.e. the Broker log must have
        // zero effect on the rendered bytes. We prove this by rendering a
        // hidden panel with an empty buffer and a hidden panel holding many
        // messages: the buffers must be byte-identical.
        let rows = vec![agent_row("feat-auth"), agent_row("feat-db")];

        let empty = BrokerLog::new(500, false);
        let mut full = BrokerLog::new(500, false);
        for i in 1..=50 {
            full.push(sample_log_entry(i));
        }

        let buf_empty = draw_to_buffer(&rows, "2 agents", &empty);
        let buf_full = draw_to_buffer(&rows, "2 agents", &full);
        assert_eq!(
            buf_empty, buf_full,
            "a hidden Broker log must not alter the rendered frame regardless of buffered messages",
        );
    }

    #[test]
    fn visible_panel_renders_broker_log_region() {
        // Tasks 5.1/5.2: when visible the panel occupies the fourth segment
        // and renders its titled region with the buffered row.
        let rows = vec![agent_row("feat-auth")];
        let mut log = BrokerLog::new(500, true);
        log.push(sample_log_entry(1));

        let buffer = draw_to_buffer(&rows, "1 agents", &log);
        let mut rendered = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                rendered.push_str(buffer[(x, y)].symbol());
            }
            rendered.push('\n');
        }
        assert!(
            rendered.contains("Broker log"),
            "visible panel must render its titled region; got:\n{rendered}",
        );
        assert!(
            rendered.contains("rebasing onto main"),
            "visible panel must render the buffered message summary; got:\n{rendered}",
        );
    }

    #[test]
    fn toggling_visibility_returns_to_hidden_layout() {
        // Toggling the panel off via the `l` key must restore the exact
        // hidden-layout bytes (round-trip safety for the toggle hotkey).
        let rows = vec![agent_row("feat-auth")];
        let mut log = BrokerLog::new(500, false);
        log.push(sample_log_entry(1));
        let hidden_before = draw_to_buffer(&rows, "1 agents", &log);

        broker_log::handle_key(&mut log, KeyCode::Char('l')); // show
        assert!(log.visible);
        broker_log::handle_key(&mut log, KeyCode::Char('l')); // hide again
        assert!(!log.visible);
        let hidden_after = draw_to_buffer(&rows, "1 agents", &log);

        assert_eq!(
            hidden_before, hidden_after,
            "hiding the panel again must reproduce the hidden layout exactly",
        );
    }
}
