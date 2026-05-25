//! Ratatui TUI status table for pane 0.
//!
//! Reads from [`BrokerState`] on a 1-second tick
//! and renders a read-only agent status table. The v0.3.0 dashboard is
//! display-only — the only interaction is quitting with `q`.

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
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};

use crate::broker::delivery;
use crate::broker::messages::BrokerMessage;
use crate::broker::{AgentStatusEntry, BrokerHandle, BrokerState};
use crate::error::PawError;

/// Tick interval for the dashboard draw loop.
///
/// Also bounds the worst-case typing latency: any keystroke that arrives
/// mid-sleep is picked up on the next tick. 50ms is comfortably below the
/// ~100ms perceptual threshold for interactive UIs while keeping the
/// broker-state snapshot rate modest (~20 Hz against an in-process lock).
const TICK_INTERVAL: Duration = Duration::from_millis(50);

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
    /// One-line summary from the last message.
    pub summary: String,
}

/// Maximum number of messages displayed in the broker messages panel.
const MAX_VISIBLE_MESSAGES: usize = 20;

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

/// A formatted broker message for display in the messages panel.
#[derive(Debug, Clone)]
pub struct MessageEntry {
    /// Formatted timestamp (e.g., "14:30:45").
    pub timestamp: String,
    /// The agent identifier (slugified branch name).
    pub agent_id: String,
    /// Message type symbol and label (e.g., "📤 status").
    pub message_type: String,
    /// The formatted message content.
    pub content: String,
}

/// Maps a broker message type to a Unicode symbol.
pub fn message_type_symbol(msg_type: &str) -> &'static str {
    match msg_type {
        "agent.status" => "📤",
        "agent.artifact" => "📦",
        "agent.blocked" => "🚧",
        "agent.verified" => "✅",
        "agent.feedback" => "💬",
        "agent.question" => "❓",
        _ => "📄",
    }
}

/// Formats a broker message for display in the messages panel.
pub fn format_message_entry(
    _seq: u64,
    timestamp: std::time::SystemTime,
    msg: &BrokerMessage,
) -> MessageEntry {
    // Format timestamp as HH:MM:SS
    let time = timestamp.duration_since(std::time::UNIX_EPOCH).map_or_else(
        |_| "00:00:00".to_string(),
        |d| {
            let secs = d.as_secs() % 86400; // seconds in day
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let secs = secs % 60;
            format!("{hours:02}:{mins:02}:{secs:02}")
        },
    );

    let msg_type = match msg {
        BrokerMessage::Status { .. } => "status",
        BrokerMessage::Artifact { .. } => "artifact",
        BrokerMessage::Blocked { .. } => "blocked",
        BrokerMessage::Verified { .. } => "verified",
        BrokerMessage::Feedback { .. } => "feedback",
        BrokerMessage::Question { .. } => "question",
        BrokerMessage::Intent { .. } => "intent",
    };
    let symbol = message_type_symbol(&format!("agent.{msg_type}"));
    let _status_label = msg.status_label().to_string();

    MessageEntry {
        timestamp: time,
        agent_id: msg.agent_id().to_string(),
        message_type: format!("{symbol} {msg_type}"),
        content: msg.to_string(),
    }
}

/// Formats a list of broker messages for display.
pub fn format_message_entries(
    messages: &[(u64, std::time::SystemTime, BrokerMessage)],
) -> Vec<MessageEntry> {
    messages
        .iter()
        .map(|(seq, ts, msg)| format_message_entry(*seq, *ts, msg))
        .collect()
}

/// Converts raw agent status entries into formatted display rows.
///
/// When an entry's most-recent status message carries a `phase`, the row's
/// status field renders that phase (with the matching status symbol)
/// instead of the message-type-derived label. Used by the supervisor row,
/// where labels like `"feedback"` (the wire message type) are misleading
/// and the real lifecycle phase is `"watching"`, `"merging"`, etc.
///
/// Pure function: performs no I/O, holds no locks, and is deterministic
/// given the same inputs.
pub fn format_agent_rows(agents: &[AgentStatusEntry], now: Instant) -> Vec<AgentRow> {
    agents
        .iter()
        .map(|agent| {
            let elapsed = now.saturating_duration_since(agent.last_seen);
            let label = agent.phase.as_deref().unwrap_or(&agent.status);
            let symbol = status_symbol(label);
            AgentRow {
                agent_id: agent.agent_id.clone(),
                cli: agent.cli.clone(),
                status: format!("{symbol} {label}"),
                age: format_age(elapsed),
                summary: agent.summary.clone(),
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
    message_entries: &[MessageEntry],
    show_message_log: bool,
) {
    draw_frame(frame, rows, status_line, message_entries, show_message_log);
}

/// Returns the vertical layout constraints for the dashboard frame.
///
/// `show_message_log = false` (the v0.5.0 default after the prompt-inbox
/// removal) produces a three-segment layout: title, agent table, status
/// line. `show_message_log = true` appends a fourth segment for the
/// broker messages panel.
pub(crate) fn build_layout_constraints(show_message_log: bool) -> Vec<Constraint> {
    if show_message_log {
        vec![
            Constraint::Length(1),  // title
            Constraint::Min(0),     // agent table
            Constraint::Length(1),  // status line
            Constraint::Length(12), // messages panel
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
fn draw_frame(
    frame: &mut Frame,
    rows: &[AgentRow],
    status_line: &str,
    message_entries: &[MessageEntry],
    show_message_log: bool,
) {
    // The prompt-inbox panel was removed in v0.5.0 (supervisor-as-pane-
    // followups D3). The supervisor pane is the human's input surface for
    // replying to `agent.question` events; the dashboard is observation-
    // only.
    let layout_constraints = build_layout_constraints(show_message_log);

    let chunks = Layout::vertical(layout_constraints).split(frame.area());

    let title =
        Paragraph::new("git-paw dashboard").style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    if rows.is_empty() {
        let empty = Paragraph::new("No agents connected yet").alignment(Alignment::Center);
        frame.render_widget(empty, chunks[1]);
    } else {
        let header = Row::new(["Agent", "CLI", "Status", "Last Update", "Summary"])
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
                    r.summary.clone(),
                ]),
                AgentTableRow::Divider => Row::new(vec![
                    divider_segment.clone(),
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
            Constraint::Length(10),
            Constraint::Min(20),
        ];
        let table = Table::new(table_rows, widths).header(header);
        frame.render_widget(table, chunks[1]);
    }

    let status = Paragraph::new(status_line.to_string());
    frame.render_widget(status, chunks[2]);

    // Messages panel (only shown when enabled)
    if show_message_log {
        let messages_title = format!("Messages ({} recent)", message_entries.len());
        let messages_block = Block::default().borders(Borders::ALL).title(messages_title);
        let messages_text = if message_entries.is_empty() {
            "(no recent messages)".to_string()
        } else {
            message_entries
                .iter()
                .take(MAX_VISIBLE_MESSAGES)
                .map(|entry| {
                    format!(
                        "{} [{}] {}: {}",
                        entry.timestamp, entry.agent_id, entry.message_type, entry.content
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let messages = Paragraph::new(messages_text).block(messages_block);
        frame.render_widget(messages, chunks[3]);
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
    run_dashboard_with_panes(state, broker_handle, shutdown, &HashMap::new(), None, false)
}

/// Runs the dashboard with an explicit agent ID → tmux pane index map and
/// session name. Retained for source compatibility with v0.4 launchers, but
/// `pane_map` and `session_name` are now unused — the prompt-inbox panel
/// that consumed them was removed in v0.5.0.
///
/// `show_message_log` controls whether the broker messages panel is displayed.
pub fn run_dashboard_with_panes<S: std::hash::BuildHasher>(
    state: &Arc<BrokerState>,
    broker_handle: BrokerHandle,
    shutdown: &std::sync::atomic::AtomicBool,
    _pane_map: &HashMap<String, usize, S>,
    _session_name: Option<&str>,
    show_message_log: bool,
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

    loop {
        // Check for SIGHUP-triggered shutdown (e.g. tmux kill-session)
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        // Drain up to 32 pending input events before re-rendering. Only
        // `q` (quit) is handled; every other key is silently ignored.
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
                && should_quit(key.code)
            {
                return restore_terminal(&mut guard.terminal);
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

        // Retrieve recent messages for the messages panel
        let recent_msgs = delivery::recent_messages(state, MAX_VISIBLE_MESSAGES);
        let message_entries = format_message_entries(&recent_msgs);

        guard
            .terminal
            .draw(|f| {
                draw_frame(f, &rows, &status_line, &message_entries, show_message_log);
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
    use crate::broker::messages::{
        ArtifactPayload, BlockedPayload, FeedbackPayload, QuestionPayload, StatusPayload,
        VerifiedPayload,
    };

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
    // message_type_symbol
    // -----------------------------------------------------------------------

    #[test]
    fn message_type_symbol_status() {
        assert_eq!(message_type_symbol("agent.status"), "📤");
    }

    #[test]
    fn message_type_symbol_artifact() {
        assert_eq!(message_type_symbol("agent.artifact"), "📦");
    }

    #[test]
    fn message_type_symbol_blocked() {
        assert_eq!(message_type_symbol("agent.blocked"), "🚧");
    }

    #[test]
    fn message_type_symbol_verified() {
        assert_eq!(message_type_symbol("agent.verified"), "✅");
    }

    #[test]
    fn message_type_symbol_feedback() {
        assert_eq!(message_type_symbol("agent.feedback"), "💬");
    }

    #[test]
    fn message_type_symbol_question() {
        assert_eq!(message_type_symbol("agent.question"), "❓");
    }

    #[test]
    fn message_type_symbol_unknown() {
        assert_eq!(message_type_symbol("agent.unknown"), "📄");
    }

    // -----------------------------------------------------------------------
    // format_message_entry
    // -----------------------------------------------------------------------

    #[test]
    fn format_message_entry_status() {
        let msg = BrokerMessage::Status {
            agent_id: "feat-errors".to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: vec!["src/main.rs".to_string()],
                message: Some("refactoring".to_string()),
                ..Default::default()
            },
        };
        let entry = format_message_entry(1, std::time::SystemTime::now(), &msg);
        assert_eq!(entry.agent_id, "feat-errors");
        assert!(entry.message_type.contains("📤 status"));
        assert!(entry.content.contains("[feat-errors] status: working"));
    }

    #[test]
    fn format_message_entry_artifact() {
        let msg = BrokerMessage::Artifact {
            agent_id: "feat-errors".to_string(),
            payload: ArtifactPayload {
                status: "done".to_string(),
                exports: vec!["PawError".to_string()],
                modified_files: vec!["src/error.rs".to_string()],
            },
        };
        let entry = format_message_entry(2, std::time::SystemTime::now(), &msg);
        assert_eq!(entry.agent_id, "feat-errors");
        assert!(entry.message_type.contains("📦 artifact"));
        assert!(entry.content.contains("[feat-errors] artifact: done"));
    }

    #[test]
    fn format_message_entries_empty() {
        let entries = format_message_entries(&[]);
        assert!(entries.is_empty());
    }

    #[test]
    fn format_message_entries_multiple() {
        let msg1 = BrokerMessage::Status {
            agent_id: "feat-a".to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: vec![],
                message: None,
                ..Default::default()
            },
        };
        let msg2 = BrokerMessage::Artifact {
            agent_id: "feat-b".to_string(),
            payload: ArtifactPayload {
                status: "done".to_string(),
                exports: vec![],
                modified_files: vec![],
            },
        };
        let messages = vec![
            (1, std::time::SystemTime::now(), msg1),
            (2, std::time::SystemTime::now(), msg2),
        ];
        let entries = format_message_entries(&messages);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].agent_id, "feat-a");
        assert_eq!(entries[1].agent_id, "feat-b");
    }

    #[test]
    fn format_message_entries_all_types() {
        let messages = vec![
            (
                1,
                std::time::SystemTime::now(),
                BrokerMessage::Status {
                    agent_id: "feat-a".to_string(),
                    payload: StatusPayload {
                        status: "working".to_string(),
                        modified_files: vec![],
                        message: None,
                        ..Default::default()
                    },
                },
            ),
            (
                2,
                std::time::SystemTime::now(),
                BrokerMessage::Artifact {
                    agent_id: "feat-b".to_string(),
                    payload: ArtifactPayload {
                        status: "done".to_string(),
                        exports: vec![],
                        modified_files: vec![],
                    },
                },
            ),
            (
                3,
                std::time::SystemTime::now(),
                BrokerMessage::Blocked {
                    agent_id: "feat-c".to_string(),
                    payload: BlockedPayload {
                        needs: "types".to_string(),
                        from: "feat-b".to_string(),
                    },
                },
            ),
            (
                4,
                std::time::SystemTime::now(),
                BrokerMessage::Verified {
                    agent_id: "feat-d".to_string(),
                    payload: VerifiedPayload {
                        verified_by: "supervisor".to_string(),
                        message: None,
                    },
                },
            ),
            (
                5,
                std::time::SystemTime::now(),
                BrokerMessage::Feedback {
                    agent_id: "feat-e".to_string(),
                    payload: FeedbackPayload {
                        from: "supervisor".to_string(),
                        errors: vec!["error".to_string()],
                    },
                },
            ),
            (
                6,
                std::time::SystemTime::now(),
                BrokerMessage::Question {
                    agent_id: "feat-f".to_string(),
                    payload: QuestionPayload {
                        question: "question?".to_string(),
                    },
                },
            ),
        ];

        let entries = format_message_entries(&messages);
        assert_eq!(entries.len(), 6);

        // Verify all message types are represented
        let type_symbols: Vec<&str> = entries
            .iter()
            .map(|entry| entry.message_type.split(' ').next().unwrap())
            .collect();
        assert!(type_symbols.contains(&"📤")); // status
        assert!(type_symbols.contains(&"📦")); // artifact
        assert!(type_symbols.contains(&"🚧")); // blocked
        assert!(type_symbols.contains(&"✅")); // verified
        assert!(type_symbols.contains(&"💬")); // feedback
        assert!(type_symbols.contains(&"❓")); // question
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
                summary: "msg a".to_string(),
                phase: None,
            },
            AgentStatusEntry {
                agent_id: "feat-b".to_string(),
                cli: "cursor".to_string(),
                status: "done".to_string(),
                last_seen: now.checked_sub(Duration::from_mins(1)).unwrap(),
                last_seen_seconds: 60,
                summary: "msg b".to_string(),
                phase: None,
            },
            AgentStatusEntry {
                agent_id: "feat-c".to_string(),
                cli: "claude".to_string(),
                status: "blocked".to_string(),
                last_seen: now.checked_sub(Duration::from_mins(5)).unwrap(),
                last_seen_seconds: 300,
                summary: String::new(),
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
            summary: "finished".to_string(),
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
                summary: "changes committed".to_string(),
                phase: None,
            },
            AgentStatusEntry {
                agent_id: "feat-working".to_string(),
                cli: "cursor".to_string(),
                status: "working".to_string(),
                last_seen: now.checked_sub(Duration::from_secs(30)).unwrap(),
                last_seen_seconds: 30,
                summary: "in progress".to_string(),
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
            summary: String::new(),
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
            summary: String::new(),
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
    // arrange_with_supervisor_pinned (tasks 4.4 - 4.6)
    // -----------------------------------------------------------------------

    fn agent_row(id: &str) -> AgentRow {
        AgentRow {
            agent_id: id.to_string(),
            cli: "claude".to_string(),
            status: "🔵 working".to_string(),
            age: "0s ago".to_string(),
            summary: String::new(),
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
            .draw(|f| draw_frame(f, &rows, "3 agents", &[], false))
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
            .draw(|f| draw_frame(f, &[], "0 agents", &[], false))
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
}
