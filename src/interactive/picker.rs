//! `ratatui` fuzzy multi-select picker and its terminal lifecycle.
//!
//! Holds [`fuzzy_multi_select`] — the render/input loop shared by the branch
//! and spec multi-selects — plus the raw-mode/alternate-screen setup, restore,
//! and drop guard. The filter and selection bookkeeping it drives lives in the
//! terminal-free [`PickerState`] in [`super::resolver`], so everything here is
//! a thin shell over pure state.

use std::io::{self, Stdout};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{List, ListItem, Paragraph};

use crate::error::PawError;

use super::resolver::{PickerState, clamp_cursor};

/// Guard that restores the terminal on drop, ensuring cleanup even on panic or
/// early return. Mirrors the `TerminalGuard` discipline in `src/dashboard.rs`.
struct PickerTerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Drop for PickerTerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

/// Enters raw mode and the alternate screen, returning a configured terminal.
fn picker_setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, PawError> {
    terminal::enable_raw_mode()
        .map_err(|e| PawError::SessionError(format!("failed to enable raw mode: {e}")))?;
    crossterm::execute!(io::stdout(), EnterAlternateScreen)
        .map_err(|e| PawError::SessionError(format!("failed to enter alternate screen: {e}")))?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
        .map_err(|e| PawError::SessionError(format!("failed to create terminal: {e}")))
}

/// Disables raw mode, leaves the alternate screen, and shows the cursor.
fn picker_restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), PawError> {
    terminal::disable_raw_mode()
        .map_err(|e| PawError::SessionError(format!("failed to disable raw mode: {e}")))?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .map_err(|e| PawError::SessionError(format!("failed to leave alternate screen: {e}")))?;
    terminal
        .show_cursor()
        .map_err(|e| PawError::SessionError(format!("failed to show cursor: {e}")))
}

/// Renders one frame of the fuzzy multi-select picker.
///
/// Layout: a bold prompt line, the live filter query, then the visible rows
/// (each prefixed with a cursor marker and a `[x]`/`[ ]` checkbox). TUI draw
/// code is exempt from the coverage gate; the testable logic lives in
/// [`PickerState`].
fn draw_picker(frame: &mut Frame, prompt: &str, state: &PickerState, cursor: usize) {
    let chunks = Layout::vertical([
        Constraint::Length(1), // prompt
        Constraint::Length(1), // filter query
        Constraint::Min(1),    // candidate rows
    ])
    .split(frame.area());

    let title = Paragraph::new(prompt).style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(title, chunks[0]);

    let query_line = Paragraph::new(format!("filter: {}", state.query));
    frame.render_widget(query_line, chunks[1]);

    let items: Vec<ListItem> = state
        .visible_indices()
        .iter()
        .enumerate()
        .map(|(row, &original_index)| {
            let checkbox = if state.is_selected(original_index) {
                "[x]"
            } else {
                "[ ]"
            };
            let pointer = if row == cursor { '>' } else { ' ' };
            ListItem::new(format!(
                "{pointer} {checkbox} {}",
                state.labels[original_index]
            ))
        })
        .collect();

    frame.render_widget(List::new(items), chunks[2]);
}

/// Presents a `ratatui` fuzzy-filter multi-select over `labels` and returns the
/// selected **original** indices, or `None` when the user cancels (Ctrl+C or
/// Esc).
///
/// Key handling: printable characters edit the filter query, Backspace deletes
/// the last query character, Ctrl+U clears the whole query, Up/Down move the
/// cursor over the visible (filtered) rows, Space toggles the cursor row, Enter
/// confirms, and Ctrl+C / Esc cancel. Selection persists across query changes
/// because it is keyed by original index (see [`PickerState`]).
///
/// The terminal is always restored — raw mode disabled, alternate screen left —
/// on every exit path (clean exit, early `?` error, or panic) via
/// [`PickerTerminalGuard`] and an installed panic hook, mirroring
/// `src/dashboard.rs`.
///
/// # Errors
///
/// Returns [`PawError::SessionError`] if the terminal cannot be set up, drawn,
/// or read from.
pub(super) fn fuzzy_multi_select(
    prompt: &str,
    labels: &[String],
) -> Result<Option<Vec<usize>>, PawError> {
    // Restore the terminal before the default hook prints a panic message, so
    // a panic inside the loop never leaves the terminal in raw mode.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let terminal = picker_setup_terminal()?;
    let mut guard = PickerTerminalGuard { terminal };

    let mut state = PickerState::new(labels.to_vec());
    let mut cursor: usize = 0;

    let selection = loop {
        guard
            .terminal
            .draw(|f| draw_picker(f, prompt, &state, cursor))
            .map_err(|e| PawError::SessionError(format!("picker draw failed: {e}")))?;

        let event = event::read()
            .map_err(|e| PawError::SessionError(format!("picker input read failed: {e}")))?;
        let Event::Key(key) = event else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break None,
            KeyCode::Esc => break None,
            // Ctrl+U clears the whole filter (readline convention), restoring
            // the full list with selections intact.
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                state.set_query(String::new());
                cursor = 0;
            }
            KeyCode::Enter => break Some(state.confirm()),
            KeyCode::Up => cursor = cursor.saturating_sub(1),
            KeyCode::Down => {
                let visible = state.visible_indices().len();
                if visible > 0 {
                    cursor = (cursor + 1).min(visible - 1);
                }
            }
            KeyCode::Char(' ') => state.toggle(cursor),
            KeyCode::Backspace => {
                state.pop_char();
                clamp_cursor(&mut cursor, &state);
            }
            // Printable characters edit the query. Control/Alt combos (other
            // than the Ctrl+C handled above) are ignored rather than typed.
            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                state.push_char(c);
                clamp_cursor(&mut cursor, &state);
            }
            _ => {}
        }
    };

    // Explicit restore for the clean path; the guard also restores on drop as a
    // safety net for the early-return and panic paths.
    picker_restore_terminal(&mut guard.terminal)?;
    Ok(selection)
}
