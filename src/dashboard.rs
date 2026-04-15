//! Ratatui TUI status table for pane 0.
//!
//! Reads from [`BrokerState`] on a 1-second tick
//! and renders a read-only agent status table. The v0.3.0 dashboard is
//! display-only — the only interaction is quitting with `q`.

use std::collections::HashMap;
use std::io::{self, Stdout};
use std::process::Command;
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

/// Maximum number of pending questions rendered in the prompts section.
const MAX_VISIBLE_QUESTIONS: usize = 5;

/// Tick interval for the dashboard draw loop.
///
/// Also bounds the worst-case typing latency: any keystroke that arrives
/// mid-sleep is picked up on the next tick. 50ms is comfortably below the
/// ~100ms perceptual threshold for interactive UIs while keeping the
/// broker-state snapshot rate modest (~20 Hz against an in-process lock).
const TICK_INTERVAL: Duration = Duration::from_millis(50);

/// A pending question from an agent awaiting a human reply.
///
/// `pane_index` is the tmux pane the agent is running in; it is the routing
/// target when the supervisor presses `Enter` to send a reply via
/// `tmux send-keys`.
#[derive(Debug, Clone)]
pub struct QuestionEntry {
    /// Slugified branch name of the asking agent.
    pub agent_id: String,
    /// Tmux pane index the agent process is running in.
    pub pane_index: usize,
    /// The question text.
    pub question: String,
    /// Broker sequence number used for ordering and dedup.
    pub seq: u64,
}

impl QuestionEntry {
    /// Creates a `QuestionEntry` from a `BrokerMessage::Question`.
    ///
    /// `pane_index` is the tmux pane the agent is running in.
    ///
    /// # Panics
    ///
    /// Panics if `msg` is not a `BrokerMessage::Question` variant.
    pub fn from_broker_message(msg: &BrokerMessage, pane_index: usize) -> Self {
        if let BrokerMessage::Question { agent_id, payload } = msg {
            Self {
                agent_id: agent_id.clone(),
                pane_index,
                question: payload.question.clone(),
                seq: 0, // seq is not used in tests, set to 0
            }
        } else {
            panic!("Expected BrokerMessage::Question, got {msg:?}");
        }
    }
}

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
#[allow(clippy::too_many_arguments)]
pub fn render_dashboard(
    frame: &mut Frame,
    rows: &[AgentRow],
    status_line: &str,
    questions: &[QuestionEntry],
    focused_question: Option<usize>,
    input_buffer: &str,
    message_entries: &[MessageEntry],
    show_message_log: bool,
) {
    draw_frame(
        frame,
        rows,
        status_line,
        questions,
        focused_question,
        input_buffer,
        message_entries,
        show_message_log,
    );
}

/// Drives one tick of the dashboard's question-polling path.
///
/// Polls the supervisor inbox in `state` for messages newer than `last_seq`,
/// converts any `agent.question` messages into `QuestionEntry` values (using
/// `pane_map` to resolve the routing pane), and appends them to `questions`.
/// `last_seq` is updated to the highest sequence number observed.
///
/// Pulled out as a free function so tests can exercise the production
/// poll → enqueue path without spinning up a terminal.
pub fn drive_question_tick<S: std::hash::BuildHasher>(
    state: &Arc<BrokerState>,
    pane_map: &HashMap<String, usize, S>,
    questions: &mut Vec<QuestionEntry>,
    last_seq: &mut u64,
) {
    let (new_msgs, observed_seq) = delivery::poll_messages(state, "supervisor", *last_seq);
    if observed_seq > *last_seq {
        *last_seq = observed_seq;
    }
    for msg in new_msgs {
        if let BrokerMessage::Question { agent_id, payload } = msg {
            let pane_index = pane_map.get(&agent_id).copied().unwrap_or(0);
            questions.push(QuestionEntry {
                agent_id,
                pane_index,
                question: payload.question,
                seq: observed_seq,
            });
        }
    }
}

/// Renders one frame of the dashboard TUI.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn draw_frame(
    frame: &mut Frame,
    rows: &[AgentRow],
    status_line: &str,
    questions: &[QuestionEntry],
    focused_question: Option<usize>,
    input_buffer: &str,
    message_entries: &[MessageEntry],
    show_message_log: bool,
) {
    // Calculate layout constraints based on whether message log is shown
    let layout_constraints = if show_message_log {
        vec![
            Constraint::Length(1),  // title
            Constraint::Min(0),     // agent table
            Constraint::Length(1),  // status line
            Constraint::Length(12), // messages panel
            Constraint::Length(7),  // prompts section
            Constraint::Length(3),  // input field
        ]
    } else {
        vec![
            Constraint::Length(1), // title
            Constraint::Min(0),    // agent table
            Constraint::Length(1), // status line
            Constraint::Length(7), // prompts section
            Constraint::Length(3), // input field
        ]
    };

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

        // Adjust indices for remaining sections when messages panel is shown
        let prompts_chunk_idx = 4;
        let input_chunk_idx = 5;

        // Prompts section
        let prompts_title = format!("Questions ({} pending)", questions.len());
        let prompts_block = Block::default().borders(Borders::ALL).title(prompts_title);
        let prompts_text = if questions.is_empty() {
            "(no pending questions)".to_string()
        } else {
            questions
                .iter()
                .take(MAX_VISIBLE_QUESTIONS)
                .enumerate()
                .map(|(i, q)| {
                    let marker = if Some(i) == focused_question {
                        ">"
                    } else {
                        " "
                    };
                    format!("{marker} [{}] {}", q.agent_id, q.question)
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let prompts = Paragraph::new(prompts_text).block(prompts_block);
        frame.render_widget(prompts, chunks[prompts_chunk_idx]);

        // Input field
        let focused_agent = focused_question
            .and_then(|i| questions.get(i))
            .map_or("(none)", |q| q.agent_id.as_str());
        let input_block = Block::default().borders(Borders::ALL);
        let input_text = format!("Reply to {focused_agent}> {input_buffer}_");
        let input = Paragraph::new(input_text).block(input_block);
        frame.render_widget(input, chunks[input_chunk_idx]);
    } else {
        // Original layout without messages panel
        let prompts_chunk_idx = 3;
        let input_chunk_idx = 4;

        // Prompts section
        let prompts_title = format!("Questions ({} pending)", questions.len());
        let prompts_block = Block::default().borders(Borders::ALL).title(prompts_title);
        let prompts_text = if questions.is_empty() {
            "(no pending questions)".to_string()
        } else {
            questions
                .iter()
                .take(MAX_VISIBLE_QUESTIONS)
                .enumerate()
                .map(|(i, q)| {
                    let marker = if Some(i) == focused_question {
                        ">"
                    } else {
                        " "
                    };
                    format!("{marker} [{}] {}", q.agent_id, q.question)
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        let prompts = Paragraph::new(prompts_text).block(prompts_block);
        frame.render_widget(prompts, chunks[prompts_chunk_idx]);

        // Input field
        let focused_agent = focused_question
            .and_then(|i| questions.get(i))
            .map_or("(none)", |q| q.agent_id.as_str());
        let input_block = Block::default().borders(Borders::ALL);
        let input_text = format!("Reply to {focused_agent}> {input_buffer}_");
        let input = Paragraph::new(input_text).block(input_block);
        frame.render_widget(input, chunks[input_chunk_idx]);
    }
}

/// Sends `text` to the given tmux pane via `tmux send-keys`, followed by
/// `Enter`. Returns `Ok(())` on success or whenever no pane is configured.
///
/// Pulled out as a free function so tests can verify the call shape (the
/// argument vector) without spawning a real tmux process.
///
/// The target string uses the `<session>:<window>.<pane>` form. Window 0 is
/// the only window git-paw creates, so we pin it; the bare `<session>:<pane>`
/// form would make tmux interpret the suffix as a window index, not a pane
/// index, and reply text would land in the wrong place when there are
/// multiple windows.
fn build_send_keys_args(session_name: &str, pane_index: usize, text: &str) -> Vec<String> {
    vec![
        "send-keys".to_string(),
        "-t".to_string(),
        format!("{session_name}:0.{pane_index}"),
        text.to_string(),
        "Enter".to_string(),
    ]
}

/// Sends `text` to `<session_name>:<pane_index>` via `tmux send-keys`, followed
/// by `Enter`. This is the production seam used by the prompt-inbox Enter
/// handler — exposed so integration tests can drive a real tmux session
/// through the same code path the dashboard event loop uses.
///
/// Returns `Ok(())` when `tmux` exits successfully. The function does not
/// distinguish between "no such pane" and other tmux errors; callers that need
/// to differentiate should use [`build_send_keys_args`] and run tmux directly.
pub fn send_reply_to_pane(session_name: &str, pane_index: usize, text: &str) -> io::Result<()> {
    let args = build_send_keys_args(session_name, pane_index, text);
    Command::new("tmux").args(&args).status().map(|_| ())
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
/// This entry point uses an empty pane map and disables reply routing.
/// Use [`run_dashboard_with_panes`] to enable interactive replies.
/// Outcome of one key press in the dashboard event loop.
#[derive(Debug, PartialEq, Eq)]
enum KeyAction {
    /// Continue the loop.
    Continue,
    /// Exit the dashboard.
    Quit,
}

/// Applies a key press to dashboard state, using `send` to deliver the reply
/// when Enter is pressed against a focused question.
///
/// The injected sender takes `(session_name, pane_index, text)` and returns
/// the argument vector that would be passed to `tmux send-keys`. Production
/// uses [`send_reply_to_pane`], which actually invokes tmux; tests can pass
/// a recording closure to verify the constructed argument vector without
/// spawning a real tmux process.
fn handle_key_with_sender(
    code: KeyCode,
    questions: &mut Vec<QuestionEntry>,
    focused_question: &mut Option<usize>,
    input_buffer: &mut String,
    session_name: Option<&str>,
    send: &mut dyn FnMut(&str, usize, &str),
) -> KeyAction {
    match code {
        KeyCode::Char('q') => return KeyAction::Quit,
        KeyCode::Tab => {
            if !questions.is_empty() {
                *focused_question = Some(match *focused_question {
                    Some(i) => (i + 1) % questions.len(),
                    None => 0,
                });
            }
        }
        KeyCode::Backspace => {
            input_buffer.pop();
        }
        KeyCode::Enter => {
            if !input_buffer.is_empty()
                && let Some(idx) = *focused_question
                && idx < questions.len()
            {
                let entry = questions[idx].clone();
                if let Some(session) = session_name {
                    send(session, entry.pane_index, input_buffer);
                }
                questions.remove(idx);
                input_buffer.clear();
                *focused_question = if questions.is_empty() {
                    None
                } else if idx >= questions.len() {
                    Some(0)
                } else {
                    Some(idx)
                };
            }
        }
        KeyCode::Char(c) if !c.is_control() => {
            input_buffer.push(c);
        }
        _ => {}
    }
    KeyAction::Continue
}

/// Production wrapper around [`handle_key_with_sender`] that uses
/// [`send_reply_to_pane`] (i.e. invokes `tmux send-keys`) as the sender.
fn handle_key(
    code: KeyCode,
    questions: &mut Vec<QuestionEntry>,
    focused_question: &mut Option<usize>,
    input_buffer: &mut String,
    session_name: Option<&str>,
) -> KeyAction {
    handle_key_with_sender(
        code,
        questions,
        focused_question,
        input_buffer,
        session_name,
        &mut |session, pane, text| {
            let _ = send_reply_to_pane(session, pane, text);
        },
    )
}

pub fn run_dashboard(
    state: &Arc<BrokerState>,
    broker_handle: BrokerHandle,
    shutdown: &std::sync::atomic::AtomicBool,
) -> Result<(), PawError> {
    run_dashboard_with_panes(state, broker_handle, shutdown, &HashMap::new(), None, false)
}

/// Runs the dashboard with an explicit agent ID → tmux pane index map and
/// session name for reply routing. `pane_map` may be empty (no replies will
/// be routed); `session_name` is required only if pane routing is desired.
/// `show_message_log` controls whether the broker messages panel is displayed.
pub fn run_dashboard_with_panes<S: std::hash::BuildHasher>(
    state: &Arc<BrokerState>,
    broker_handle: BrokerHandle,
    shutdown: &std::sync::atomic::AtomicBool,
    pane_map: &HashMap<String, usize, S>,
    session_name: Option<&str>,
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

    let mut questions: Vec<QuestionEntry> = Vec::new();
    let mut focused_question: Option<usize> = None;
    let mut input_buffer = String::new();
    let mut last_question_seq: u64 = 0;

    loop {
        // Check for SIGHUP-triggered shutdown (e.g. tmux kill-session)
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }

        // Drain up to 32 pending input events before re-rendering so typing
        // is processed immediately instead of waiting for the next tick.
        // The cap prevents a hot loop if the pty floods us with non-Key
        // events (e.g. repeated Resize during tmux teardown) — we'll
        // return to the top of the outer loop, where the shutdown flag
        // is checked, within 32 reads.
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
                && handle_key(
                    key.code,
                    &mut questions,
                    &mut focused_question,
                    &mut input_buffer,
                    session_name,
                ) == KeyAction::Quit
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

        // Poll supervisor inbox for new questions.
        let (new_msgs, last_seq) = delivery::poll_messages(state, "supervisor", last_question_seq);
        if last_seq > last_question_seq {
            last_question_seq = last_seq;
        }
        for msg in new_msgs {
            if let BrokerMessage::Question { agent_id, payload } = msg {
                let pane_index = pane_map.get(&agent_id).copied().unwrap_or(0);
                questions.push(QuestionEntry {
                    agent_id,
                    pane_index,
                    question: payload.question,
                    seq: last_seq,
                });
                if focused_question.is_none() {
                    focused_question = Some(0);
                }
            }
        }

        guard
            .terminal
            .draw(|f| {
                draw_frame(
                    f,
                    &rows,
                    &status_line,
                    &questions,
                    focused_question,
                    &input_buffer,
                    &message_entries,
                    show_message_log,
                );
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
    fn format_agent_rows_with_committed_status() {
        let now = Instant::now();
        let agents = vec![
            AgentStatusEntry {
                agent_id: "feat-committed".to_string(),
                cli: "claude".to_string(),
                status: "committed".to_string(),
                last_seen: now.checked_sub(Duration::from_secs(60)).unwrap(),
                last_seen_seconds: 60,
                summary: "changes committed".to_string(),
            },
            AgentStatusEntry {
                agent_id: "feat-working".to_string(),
                cli: "cursor".to_string(),
                status: "working".to_string(),
                last_seen: now.checked_sub(Duration::from_secs(30)).unwrap(),
                last_seen_seconds: 30,
                summary: "in progress".to_string(),
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
    // Prompt inbox: pure helpers
    // -----------------------------------------------------------------------

    fn make_q(agent_id: &str, question: &str, pane_index: usize, seq: u64) -> QuestionEntry {
        QuestionEntry {
            agent_id: agent_id.to_string(),
            pane_index,
            question: question.to_string(),
            seq,
        }
    }

    fn advance_focus(focused: Option<usize>, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        Some(match focused {
            Some(i) => (i + 1) % len,
            None => 0,
        })
    }

    #[test]
    fn tab_advances_focus_to_next() {
        let qs = [make_q("a", "q1", 1, 1), make_q("b", "q2", 2, 2)];
        let next = advance_focus(Some(0), qs.len());
        assert_eq!(next, Some(1));
    }

    #[test]
    fn tab_wraps_from_last_to_first() {
        let qs = [make_q("a", "q1", 1, 1), make_q("b", "q2", 2, 2)];
        let next = advance_focus(Some(1), qs.len());
        assert_eq!(next, Some(0));
    }

    #[test]
    fn tab_with_empty_questions_is_noop() {
        let next = advance_focus(None, 0);
        assert_eq!(next, None);
    }

    #[test]
    fn build_send_keys_args_shape() {
        let args = build_send_keys_args("paw-myproj", 2, "Yes, do it");
        assert_eq!(
            args,
            vec![
                "send-keys".to_string(),
                "-t".to_string(),
                "paw-myproj:0.2".to_string(),
                "Yes, do it".to_string(),
                "Enter".to_string(),
            ]
        );
    }

    /// Simulates the Enter handler: removes the focused question and clears
    /// the buffer when input is non-empty. Mirrors the in-loop logic.
    fn handle_enter(
        questions: &mut Vec<QuestionEntry>,
        focused: &mut Option<usize>,
        buffer: &mut String,
    ) -> bool {
        if buffer.is_empty() {
            return false;
        }
        let Some(idx) = *focused else { return false };
        if idx >= questions.len() {
            return false;
        }
        questions.remove(idx);
        buffer.clear();
        *focused = if questions.is_empty() {
            None
        } else if idx >= questions.len() {
            Some(0)
        } else {
            Some(idx)
        };
        true
    }

    #[test]
    fn enter_with_empty_input_is_noop() {
        let mut qs = vec![make_q("a", "q1", 1, 1)];
        let mut focused = Some(0);
        let mut buffer = String::new();
        let acted = handle_enter(&mut qs, &mut focused, &mut buffer);
        assert!(!acted);
        assert_eq!(qs.len(), 1);
        assert_eq!(focused, Some(0));
    }

    #[test]
    fn enter_with_input_removes_question_and_clears_buffer() {
        let mut qs = vec![make_q("a", "q1", 1, 1), make_q("b", "q2", 2, 2)];
        let mut focused = Some(0);
        let mut buffer = "Yes".to_string();
        let acted = handle_enter(&mut qs, &mut focused, &mut buffer);
        assert!(acted);
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].agent_id, "b");
        assert!(buffer.is_empty());
        assert_eq!(focused, Some(0));
    }

    #[test]
    fn enter_clears_focus_when_last_question_answered() {
        let mut qs = vec![make_q("a", "q1", 1, 1)];
        let mut focused = Some(0);
        let mut buffer = "Yes".to_string();
        handle_enter(&mut qs, &mut focused, &mut buffer);
        assert!(qs.is_empty());
        assert_eq!(focused, None);
    }

    #[test]
    fn prompts_section_caps_at_five_questions() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        // 7 questions, with distinctive markers in the question text so we
        // can count visible rows in the rendered buffer.
        let many_questions: Vec<_> = (0..7)
            .map(|i| {
                make_q(
                    &format!("agent-{i:02}"),
                    &format!("question-marker-{i:02}"),
                    i,
                    i as u64,
                )
            })
            .collect();

        let backend = TestBackend::new(140, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_frame(f, &[], "0 agents", &many_questions, Some(0), "", &[], false);
            })
            .unwrap();

        // Flatten the buffer into a string for substring assertions.
        let buffer = terminal.backend().buffer().clone();
        let mut rendered = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                rendered.push_str(buffer[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        // Exactly the first MAX_VISIBLE_QUESTIONS markers must appear; the
        // overflow ones must be cropped out.
        let mut visible_count = 0;
        for i in 0..MAX_VISIBLE_QUESTIONS {
            let marker = format!("question-marker-{i:02}");
            assert!(
                rendered.contains(&marker),
                "expected first {MAX_VISIBLE_QUESTIONS} questions to render; missing {marker} in:\n{rendered}"
            );
            visible_count += 1;
        }
        for i in MAX_VISIBLE_QUESTIONS..many_questions.len() {
            let marker = format!("question-marker-{i:02}");
            assert!(
                !rendered.contains(&marker),
                "questions beyond cap should not render; found {marker} in:\n{rendered}"
            );
        }
        assert_eq!(visible_count, MAX_VISIBLE_QUESTIONS);
        // Header still reports the true total.
        assert!(
            rendered.contains("7 pending"),
            "header should still show full pending count; got:\n{rendered}"
        );
    }

    // -----------------------------------------------------------------------
    // handle_key_event - printable characters
    // -----------------------------------------------------------------------

    #[test]
    fn printable_char_appends_to_input_buffer() {
        let mut buffer = String::new();
        let mut focused = None;
        let mut questions = vec![];
        let action = handle_key(
            KeyCode::Char('x'),
            &mut questions,
            &mut focused,
            &mut buffer,
            None,
        );
        assert_eq!(action, KeyAction::Continue);
        assert_eq!(buffer, "x");
    }

    #[test]
    fn backspace_removes_last_char_from_input_buffer() {
        let mut buffer = "hello".to_string();
        let mut focused = None;
        let mut questions = vec![];
        let action = handle_key(
            KeyCode::Backspace,
            &mut questions,
            &mut focused,
            &mut buffer,
            None,
        );
        assert_eq!(action, KeyAction::Continue);
        assert_eq!(buffer, "hell");
    }

    #[test]
    fn backspace_on_empty_buffer_is_noop() {
        let mut buffer = String::new();
        let mut focused = None;
        let mut questions = vec![];
        let action = handle_key(
            KeyCode::Backspace,
            &mut questions,
            &mut focused,
            &mut buffer,
            None,
        );
        assert_eq!(action, KeyAction::Continue);
        assert_eq!(buffer, "");
    }

    // -----------------------------------------------------------------------
    // QuestionEntry::from_broker_message
    // -----------------------------------------------------------------------

    #[test]
    fn question_entry_from_broker_message() {
        let msg = BrokerMessage::Question {
            agent_id: "feat-errors".to_string(),
            payload: crate::broker::messages::QuestionPayload {
                question: "Should I use anyhow or thiserror?".to_string(),
            },
        };
        let entry = QuestionEntry::from_broker_message(&msg, 2);
        assert_eq!(entry.agent_id, "feat-errors");
        assert_eq!(entry.pane_index, 2);
        assert_eq!(entry.question, "Should I use anyhow or thiserror?");
    }

    // -----------------------------------------------------------------------
    // advance_focus wrapping
    // -----------------------------------------------------------------------

    #[test]
    fn advance_focus_wraps_around_when_at_end() {
        let focused = Some(2); // last of 3 items
        let questions = [
            make_q("a", "q1", 1, 1),
            make_q("b", "q2", 2, 2),
            make_q("c", "q3", 3, 3),
        ];
        let new_focused = advance_focus(focused, questions.len());
        assert_eq!(new_focused, Some(0));
    }

    #[test]
    fn advance_focus_on_empty_list_is_noop() {
        let focused = None;
        let questions: Vec<QuestionEntry> = vec![];
        let new_focused = advance_focus(focused, questions.len());
        assert_eq!(new_focused, None);
    }

    // -----------------------------------------------------------------------
    // Enter key send-reply wiring
    // -----------------------------------------------------------------------

    /// Drives the production `handle_key_with_sender` path with `Enter`
    /// against a fixture pane map, captures the constructed
    /// `tmux send-keys` argument vector via the injected sender, and
    /// asserts on its shape.
    #[test]
    fn enter_invokes_send_reply_with_focused_pane() {
        let mut questions = vec![
            make_q("feat-auth", "q1", 1, 1),
            make_q("feat-db", "q2", 7, 2),
            make_q("feat-api", "q3", 3, 3),
        ];
        let mut focused = Some(1); // focus the middle question (pane 7)
        let mut buffer = "Yes please".to_string();

        // Capture the arguments the production sender would pass to tmux.
        let mut captured: Vec<Vec<String>> = Vec::new();
        {
            let mut record = |session: &str, pane: usize, text: &str| {
                captured.push(build_send_keys_args(session, pane, text));
            };

            let action = handle_key_with_sender(
                KeyCode::Enter,
                &mut questions,
                &mut focused,
                &mut buffer,
                Some("paw-myproj"),
                &mut record,
            );
            assert_eq!(action, KeyAction::Continue);
        }

        // The sender must have been invoked exactly once with the focused
        // pane's index, the active session name, and the full input buffer.
        assert_eq!(
            captured.len(),
            1,
            "send should fire exactly once for one Enter press"
        );
        assert_eq!(
            captured[0],
            vec![
                "send-keys".to_string(),
                "-t".to_string(),
                "paw-myproj:0.7".to_string(),
                "Yes please".to_string(),
                "Enter".to_string(),
            ],
            "tmux send-keys argv must target the focused pane"
        );

        // Side effects on dashboard state:
        // - the answered question is removed,
        // - the input buffer is cleared,
        // - focus stays on the same index (now pointing at what was the next
        //   question), since the answered one was at idx 1 and there is a
        //   question after it (originally feat-api).
        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0].agent_id, "feat-auth");
        assert_eq!(questions[1].agent_id, "feat-api");
        assert!(buffer.is_empty(), "input buffer should be cleared");
        assert_eq!(
            focused,
            Some(1),
            "focus should remain on the same index when one remains after it"
        );
    }

    /// Enter with no `session_name` configured must not invoke the sender,
    /// even if there is a focused question and non-empty input.
    #[test]
    fn enter_without_session_name_does_not_invoke_sender() {
        let mut questions = vec![make_q("feat-auth", "q1", 1, 1)];
        let mut focused = Some(0);
        let mut buffer = "noop".to_string();

        let mut sender_calls = 0;
        let mut record = |_: &str, _: usize, _: &str| {
            sender_calls += 1;
        };
        let action = handle_key_with_sender(
            KeyCode::Enter,
            &mut questions,
            &mut focused,
            &mut buffer,
            None,
            &mut record,
        );
        assert_eq!(action, KeyAction::Continue);
        assert_eq!(sender_calls, 0, "sender must not fire without a session");
        // Question is still removed (the dashboard considers the question
        // answered locally even if no tmux session is configured).
        assert!(questions.is_empty());
        assert!(buffer.is_empty());
    }
}
