//! Ratatui TUI status table for pane 0.
//!
//! Reads from [`BrokerState`] on a 1-second tick
//! and renders a read-only agent status table. The v0.3.0 dashboard is
//! display-only — the only interaction is quitting with `q`.

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
use crate::error::PawError;

/// Tick interval for the dashboard draw loop.
const TICK_INTERVAL: Duration = Duration::from_secs(1);

/// A formatted row for display in the agent status table.
#[derive(Debug, Clone)]
pub struct AgentRow {
    /// The agent identifier (slugified branch name).
    pub agent_id: String,
    /// The CLI name (e.g. `"claude"`).
    pub cli: String,
    /// Status symbol and label (e.g. `"🔵 working"`).
    pub status: String,
    /// Relative time since last message (e.g. `"3m ago"`).
    pub age: String,
    /// One-line summary from the last message.
    pub summary: String,
}

/// Maps an agent status label to a Unicode symbol.
///
/// | Input | Output |
/// |---|---|
/// | `"working"` | `"🔵"` |
/// | `"done"` | `"🟢"` |
/// | `"verified"` | `"🟢"` |
/// | `"blocked"` | `"🟡"` |
/// | anything else | `"⚪"` |
pub fn status_symbol(status: &str) -> &'static str {
    match status {
        "working" => "🔵",
        "done" | "verified" => "🟢",
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
/// Pure function: performs no I/O, holds no locks, and is deterministic
/// given the same inputs.
pub fn format_agent_rows(agents: &[AgentStatusEntry], now: Instant) -> Vec<AgentRow> {
    agents
        .iter()
        .map(|agent| {
            let elapsed = now.saturating_duration_since(agent.last_seen);
            let symbol = status_symbol(&agent.status);
            AgentRow {
                agent_id: agent.agent_id.clone(),
                cli: agent.cli.clone(),
                status: format!("{symbol} {}", agent.status),
                age: format_age(elapsed),
                summary: agent.summary.clone(),
            }
        })
        .collect()
}

/// Produces a summary status line for the dashboard footer.
///
/// Returns a string like `"4 agents: 2 working, 1 done, 1 blocked"`.
pub fn format_status_line(total: usize, working: usize, done: usize, blocked: usize) -> String {
    format!("{total} agents: {working} working, {done} done, {blocked} blocked")
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

/// Renders one frame of the dashboard TUI.
fn draw_frame(frame: &mut Frame, rows: &[AgentRow], status_line: &str) {
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(frame.area());

    let title =
        Paragraph::new("git-paw dashboard").style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    if rows.is_empty() {
        let empty = Paragraph::new("No agents connected yet").alignment(Alignment::Center);
        frame.render_widget(empty, chunks[1]);
    } else {
        let header = Row::new(["Agent", "CLI", "Status", "Last Update", "Summary"])
            .style(Style::default().add_modifier(Modifier::BOLD));
        let table_rows: Vec<Row> = rows
            .iter()
            .map(|r| {
                Row::new([
                    r.agent_id.as_str(),
                    r.cli.as_str(),
                    r.status.as_str(),
                    r.age.as_str(),
                    r.summary.as_str(),
                ])
            })
            .collect();
        let widths = [
            Constraint::Min(15),
            Constraint::Length(10),
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Min(20),
        ];
        let table = Table::new(table_rows, widths).header(header);
        frame.render_widget(table, chunks[1]);
    }

    let status = Paragraph::new(status_line.to_string());
    frame.render_widget(status, chunks[2]);
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
pub fn run_dashboard(
    state: &Arc<BrokerState>,
    _broker_handle: BrokerHandle,
    shutdown: &std::sync::atomic::AtomicBool,
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

    loop {
        // Check for SIGHUP-triggered shutdown (e.g. tmux kill-session)
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        if event::poll(Duration::ZERO)
            .map_err(|e| PawError::DashboardError(format!("event poll failed: {e}")))?
            && let Event::Key(key) = event::read()
                .map_err(|e| PawError::DashboardError(format!("event read failed: {e}")))?
            && key.kind == KeyEventKind::Press
            && key.code == KeyCode::Char('q')
        {
            break;
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
        let status_line = format_status_line(agents.len(), working, done, blocked);

        guard
            .terminal
            .draw(|f| draw_frame(f, &rows, &status_line))
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
        assert_eq!(format_age(Duration::from_secs(180)), "3m ago");
    }

    #[test]
    fn format_age_one_hour_exact() {
        assert_eq!(format_age(Duration::from_secs(3600)), "1h 0m ago");
    }

    #[test]
    fn format_age_one_hour_fifteen_minutes() {
        assert_eq!(format_age(Duration::from_secs(4500)), "1h 15m ago");
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
                summary: "msg a".to_string(),
            },
            AgentStatusEntry {
                agent_id: "feat-b".to_string(),
                cli: "cursor".to_string(),
                status: "done".to_string(),
                last_seen: now.checked_sub(Duration::from_secs(60)).unwrap(),
                last_seen_seconds: 60,
                summary: "msg b".to_string(),
            },
            AgentStatusEntry {
                agent_id: "feat-c".to_string(),
                cli: "claude".to_string(),
                status: "blocked".to_string(),
                last_seen: now.checked_sub(Duration::from_secs(300)).unwrap(),
                last_seen_seconds: 300,
                summary: String::new(),
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
            last_seen: now.checked_sub(Duration::from_secs(180)).unwrap(),
            last_seen_seconds: 180,
            summary: "finished".to_string(),
        }];
        let rows = format_agent_rows(&agents, now);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].agent_id, "feat-errors");
        assert_eq!(rows[0].age, "3m ago");
        assert!(rows[0].status.contains("done"));
    }

    #[test]
    fn format_agent_rows_empty_input() {
        let rows = format_agent_rows(&[], Instant::now());
        assert!(rows.is_empty());
    }

    // -----------------------------------------------------------------------
    // format_status_line
    // -----------------------------------------------------------------------

    #[test]
    fn format_status_line_mixed() {
        assert_eq!(
            format_status_line(4, 2, 1, 1),
            "4 agents: 2 working, 1 done, 1 blocked"
        );
    }

    #[test]
    fn format_status_line_all_done() {
        assert_eq!(
            format_status_line(3, 0, 3, 0),
            "3 agents: 0 working, 3 done, 0 blocked"
        );
    }

    #[test]
    fn format_status_line_zero_agents() {
        assert_eq!(
            format_status_line(0, 0, 0, 0),
            "0 agents: 0 working, 0 done, 0 blocked"
        );
    }
}
