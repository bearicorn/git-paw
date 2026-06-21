//! Interactive selection prompts.
//!
//! User-facing selection flows for `git paw start`. The two multi-select
//! prompts — the branch picker ([`TerminalPrompter::select_branches`]) and the
//! spec picker ([`TerminalPrompter::select_specs`]) — are built on a shared
//! `ratatui` + `crossterm` fuzzy multi-select helper ([`fuzzy_multi_select`])
//! that lets the user type a query to filter a long candidate list. The
//! single-select prompts (mode picker and CLI pickers) stay on
//! `dialoguer::Select`. Logic is separated from UI via the [`Prompter`] trait,
//! and the filter/selection bookkeeping lives in the pure, terminal-free
//! [`PickerState`] for testability.

use std::collections::HashSet;
use std::fmt;
use std::io::{self, Stdout};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use dialoguer::Select;
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{List, ListItem, Paragraph};

use crate::config::PawConfig;
use crate::error::PawError;
use crate::specs::SpecEntry;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Information about an available AI CLI.
///
/// Contains the data needed to display a CLI option in interactive prompts.
pub struct CliInfo {
    /// Human-readable name shown in prompts (e.g., "My Agent").
    pub display_name: String,
    /// Binary name used for invocation (e.g., "my-agent").
    pub binary_name: String,
}

impl fmt::Display for CliInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.display_name == self.binary_name {
            write!(f, "{}", self.binary_name)
        } else {
            write!(f, "{} ({})", self.display_name, self.binary_name)
        }
    }
}

/// How the user wants to assign CLIs to branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliMode {
    /// Same CLI for all selected branches.
    Uniform,
    /// Different CLI for each branch.
    PerBranch,
}

impl fmt::Display for CliMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uniform => write!(f, "Same CLI for all branches"),
            Self::PerBranch => write!(f, "Different CLI per branch"),
        }
    }
}

/// Result of the full interactive selection flow.
#[derive(Debug)]
pub struct SelectionResult {
    /// Branch-to-CLI mappings as `(branch_name, cli_binary_name)` pairs.
    pub mappings: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------
// Prompter trait (separates logic from UI)
// ---------------------------------------------------------------------------

/// Abstraction over interactive prompts, allowing test doubles.
pub trait Prompter {
    /// Ask the user to choose between uniform and per-branch CLI assignment.
    fn select_mode(&self) -> Result<CliMode, PawError>;

    /// Ask the user to pick one or more branches. Returns selected branch names.
    fn select_branches(&self, branches: &[String]) -> Result<Vec<String>, PawError>;

    /// Ask the user to pick a single CLI for all branches. Returns binary name.
    ///
    /// When `default` is `Some` and matches a CLI's `binary_name`, that entry
    /// is pre-selected in the picker. Otherwise the first item is selected.
    fn select_cli(&self, clis: &[CliInfo], default: Option<&str>) -> Result<String, PawError>;

    /// Ask the user to pick a CLI for a specific branch. Returns binary name.
    fn select_cli_for_branch(&self, branch: &str, clis: &[CliInfo]) -> Result<String, PawError>;

    /// Ask the user to pick one or more specs. Returns the selected
    /// `SpecEntry` values expanded from grouped logical units.
    ///
    /// Each row in the picker represents one logical unit (a Spec Kit
    /// feature, an `OpenSpec` change, or a Markdown spec). Selecting a row
    /// returns every underlying `SpecEntry` belonging to that unit.
    fn select_specs(&self, specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError>;
}

// ---------------------------------------------------------------------------
// Real prompter (dialoguer)
// ---------------------------------------------------------------------------

/// Interactive prompter using `dialoguer` for terminal UI.
pub struct TerminalPrompter;

impl Prompter for TerminalPrompter {
    fn select_mode(&self) -> Result<CliMode, PawError> {
        let modes = [CliMode::Uniform, CliMode::PerBranch];
        let labels: Vec<String> = modes.iter().map(ToString::to_string).collect();

        let selection = Select::new()
            .with_prompt("CLI assignment mode")
            .items(&labels)
            .default(0)
            .interact_opt()
            .map_err(|e| map_dialoguer_error(&e))?;

        match selection {
            Some(idx) => Ok(modes[idx]),
            None => Err(PawError::UserCancelled),
        }
    }

    fn select_branches(&self, branches: &[String]) -> Result<Vec<String>, PawError> {
        let selection = fuzzy_multi_select(
            "Select branches (type to filter, ctrl-u to clear, space to toggle, enter to confirm)",
            branches,
        )?;
        finalize_branch_selection(branches, selection)
    }

    fn select_cli(&self, clis: &[CliInfo], default: Option<&str>) -> Result<String, PawError> {
        let labels: Vec<String> = clis.iter().map(ToString::to_string).collect();

        let default_idx = default
            .and_then(|name| clis.iter().position(|c| c.binary_name == name))
            .unwrap_or(0);

        let selection = Select::new()
            .with_prompt("Select AI CLI for all branches")
            .items(&labels)
            .default(default_idx)
            .interact_opt()
            .map_err(|e| map_dialoguer_error(&e))?;

        match selection {
            Some(idx) => Ok(clis[idx].binary_name.clone()),
            None => Err(PawError::UserCancelled),
        }
    }

    fn select_cli_for_branch(&self, branch: &str, clis: &[CliInfo]) -> Result<String, PawError> {
        let labels: Vec<String> = clis.iter().map(ToString::to_string).collect();

        let selection = Select::new()
            .with_prompt(format!("Select CLI for {branch}"))
            .items(&labels)
            .default(0)
            .interact_opt()
            .map_err(|e| map_dialoguer_error(&e))?;

        match selection {
            Some(idx) => Ok(clis[idx].binary_name.clone()),
            None => Err(PawError::UserCancelled),
        }
    }

    fn select_specs(&self, specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError> {
        let groups = group_specs_by_unit(specs);
        let labels: Vec<String> = groups.iter().map(|(label, _)| label.clone()).collect();

        let selection = fuzzy_multi_select(
            "Select specs (type to filter, ctrl-u to clear, space to toggle, enter to confirm)",
            &labels,
        )?;

        finalize_spec_selection(specs, &groups, selection)
    }
}

/// Pure post-processing for `select_branches`: maps the picker's
/// `Option<Vec<usize>>` selection (indices into the original `branches` slice)
/// back to branch names, treating both `None` (Ctrl+C / Esc) and `Some(empty)`
/// (zero rows toggled) as `PawError::UserCancelled` — matching `select_specs`
/// via [`finalize_spec_selection`].
fn finalize_branch_selection(
    branches: &[String],
    selection: Option<Vec<usize>>,
) -> Result<Vec<String>, PawError> {
    match selection {
        Some(indices) if indices.is_empty() => Err(PawError::UserCancelled),
        Some(indices) => Ok(indices.into_iter().map(|i| branches[i].clone()).collect()),
        None => Err(PawError::UserCancelled),
    }
}

/// Pure post-processing for `select_specs`: maps the dialoguer
/// `Option<Vec<usize>>` selection (over grouped rows) back to the underlying
/// `SpecEntry` values, and treats both `None` (Ctrl+C) and `Some(empty)`
/// (zero rows selected) as `PawError::UserCancelled` — matching
/// `select_branches`.
fn finalize_spec_selection(
    specs: &[SpecEntry],
    groups: &[(String, Vec<usize>)],
    selection: Option<Vec<usize>>,
) -> Result<Vec<SpecEntry>, PawError> {
    match selection {
        Some(indices) if indices.is_empty() => Err(PawError::UserCancelled),
        Some(indices) => {
            let mut out = Vec::new();
            for row in indices {
                for &entry_idx in &groups[row].1 {
                    out.push(specs[entry_idx].clone());
                }
            }
            Ok(out)
        }
        None => Err(PawError::UserCancelled),
    }
}

/// Groups `SpecEntry` values by logical unit (Spec Kit feature, `OpenSpec`
/// change, or Markdown spec) and produces a display label per unit.
///
/// Returns a vector of `(label, indices_into_specs)` pairs. Each label is
/// either the bare unit id (for one-entry units) or a Spec Kit summary like
/// `"003-user-list — 3 worktrees: 2 [P] + 1 phase/"`.
///
/// Order follows the discovery order of the first entry in each group, so
/// the picker preserves the backend's natural listing.
fn group_specs_by_unit(specs: &[SpecEntry]) -> Vec<(String, Vec<usize>)> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();

    for (idx, entry) in specs.iter().enumerate() {
        let unit = unit_id_of(&entry.id);
        if !groups.contains_key(&unit) {
            order.push(unit.clone());
        }
        groups.entry(unit).or_default().push(idx);
    }

    order
        .into_iter()
        .map(|unit| {
            let idxs = groups.remove(&unit).unwrap_or_default();
            let label = build_unit_label(&unit, &idxs, specs);
            (label, idxs)
        })
        .collect()
}

/// Extracts the logical unit id (feature for Spec Kit, change/file stem for
/// `OpenSpec` and Markdown).
fn unit_id_of(id: &str) -> String {
    if let Some((before, after)) = id.rsplit_once("-phase-")
        && !after.is_empty()
        && after.chars().all(|c| c.is_ascii_digit())
    {
        return before.to_string();
    }
    if let Some((before, after)) = id.rsplit_once("-T")
        && !after.is_empty()
        && after.chars().all(|c| c.is_ascii_digit())
    {
        return before.to_string();
    }
    id.to_string()
}

fn build_unit_label(unit: &str, indices: &[usize], specs: &[SpecEntry]) -> String {
    if indices.len() <= 1 {
        return unit.to_string();
    }
    let total = indices.len();
    let mut parallel = 0;
    let mut phase = 0;
    for &i in indices {
        let id = &specs[i].id;
        if id_is_parallel_task(id) {
            parallel += 1;
        } else if id_is_phase(id) {
            phase += 1;
        }
    }
    let mut parts = Vec::new();
    if parallel > 0 {
        parts.push(format!("{parallel} [P]"));
    }
    if phase > 0 {
        parts.push(format!("{phase} phase/"));
    }
    if parts.is_empty() {
        format!("{unit} \u{2014} {total} worktrees")
    } else {
        format!("{unit} \u{2014} {total} worktrees: {}", parts.join(" + "))
    }
}

fn id_is_parallel_task(id: &str) -> bool {
    let Some((_, after)) = id.rsplit_once("-T") else {
        return false;
    };
    !after.is_empty() && after.chars().all(|c| c.is_ascii_digit())
}

fn id_is_phase(id: &str) -> bool {
    let Some((_, after)) = id.rsplit_once("-phase-") else {
        return false;
    };
    !after.is_empty() && after.chars().all(|c| c.is_ascii_digit())
}

/// Maps dialoguer errors to `PawError`, treating I/O interrupted (Ctrl+C) as
/// user cancellation.
fn map_dialoguer_error(err: &dialoguer::Error) -> PawError {
    match err {
        dialoguer::Error::IO(io_err) if io_err.kind() == std::io::ErrorKind::Interrupted => {
            PawError::UserCancelled
        }
        dialoguer::Error::IO(_) => {
            PawError::SessionError(format!("Interactive prompt failed: {err}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Fuzzy multi-select picker
// ---------------------------------------------------------------------------

/// Pure filtering and selection state for the fuzzy multi-select picker.
///
/// Holds the immutable `labels`, the current filter `query`, and the set of
/// `selected` rows keyed by **original** label index. Keying selection by the
/// original index (rather than the visible row) is what lets a selection
/// survive a query change: toggling a row under one query and then filtering it
/// out of view never drops it from `selected`. The struct has no terminal
/// dependency, so the filter/selection contract is unit-tested without a TTY;
/// the `ratatui` render loop in [`fuzzy_multi_select`] is a thin shell over it.
struct PickerState {
    /// All candidate labels, in their original (unfiltered) order.
    labels: Vec<String>,
    /// Current filter query. Empty means "show everything".
    query: String,
    /// Selected rows, keyed by original index into `labels`.
    selected: HashSet<usize>,
}

impl PickerState {
    /// Creates a picker over `labels` with an empty query and no selection.
    fn new(labels: Vec<String>) -> Self {
        Self {
            labels,
            query: String::new(),
            selected: HashSet::new(),
        }
    }

    /// Returns the original indices whose label matches the current query.
    ///
    /// An empty query yields every index in original order. Otherwise a label
    /// matches when the query is a case-insensitive substring of it; matching
    /// indices are returned in original order.
    fn visible_indices(&self) -> Vec<usize> {
        if self.query.is_empty() {
            return (0..self.labels.len()).collect();
        }
        let needle = self.query.to_lowercase();
        self.labels
            .iter()
            .enumerate()
            .filter(|(_, label)| label.to_lowercase().contains(&needle))
            .map(|(idx, _)| idx)
            .collect()
    }

    /// Replaces the filter query wholesale.
    fn set_query(&mut self, query: String) {
        self.query = query;
    }

    /// Appends one character to the filter query.
    fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    /// Removes the last character from the filter query (no-op when empty).
    fn pop_char(&mut self) {
        self.query.pop();
    }

    /// Toggles the selection of the original index that `visible_row` maps to.
    ///
    /// `visible_row` is an index into the current [`Self::visible_indices`]; a
    /// row outside that range is ignored.
    fn toggle(&mut self, visible_row: usize) {
        if let Some(&original_index) = self.visible_indices().get(visible_row) {
            // `insert` returns false when the value was already present, so a
            // failed insert means "was selected" → remove it.
            if !self.selected.insert(original_index) {
                self.selected.remove(&original_index);
            }
        }
    }

    /// Returns true when the given original index is currently selected.
    fn is_selected(&self, original_index: usize) -> bool {
        self.selected.contains(&original_index)
    }

    /// Returns the selected original indices, sorted ascending.
    fn confirm(&self) -> Vec<usize> {
        let mut out: Vec<usize> = self.selected.iter().copied().collect();
        out.sort_unstable();
        out
    }
}

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

/// Clamps `cursor` so it stays a valid index into the visible rows after the
/// query changed. An empty visible list parks the cursor at 0.
fn clamp_cursor(cursor: &mut usize, state: &PickerState) {
    let visible = state.visible_indices().len();
    if visible == 0 {
        *cursor = 0;
    } else if *cursor >= visible {
        *cursor = visible - 1;
    }
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
fn fuzzy_multi_select(prompt: &str, labels: &[String]) -> Result<Option<Vec<usize>>, PawError> {
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

// ---------------------------------------------------------------------------
// Core selection logic (independent of UI)
// ---------------------------------------------------------------------------

/// Runs the full interactive selection flow, skipping prompts when CLI flags
/// provide the necessary data.
///
/// # Errors
///
/// Returns `PawError::NoCLIsFound` if `clis` is empty.
/// Returns `PawError::BranchError` if `branches` is empty.
/// Returns `PawError::UserCancelled` if the user cancels any prompt.
pub fn run_selection(
    prompter: &dyn Prompter,
    branches: &[String],
    clis: &[CliInfo],
    cli_flag: Option<&str>,
    branches_flag: Option<&[String]>,
) -> Result<SelectionResult, PawError> {
    if clis.is_empty() {
        return Err(PawError::NoCLIsFound);
    }
    if branches.is_empty() {
        return Err(PawError::BranchError("No branches available.".to_string()));
    }

    // Determine which branches to use.
    let selected_branches = if let Some(flagged) = branches_flag {
        flagged.to_vec()
    } else {
        prompter.select_branches(branches)?
    };

    // Determine CLI mapping.
    let mappings = if let Some(cli) = cli_flag {
        selected_branches
            .into_iter()
            .map(|branch| (branch, cli.to_string()))
            .collect()
    } else {
        let mode = prompter.select_mode()?;
        match mode {
            CliMode::Uniform => {
                let cli = prompter.select_cli(clis, None)?;
                selected_branches
                    .into_iter()
                    .map(|branch| (branch, cli.clone()))
                    .collect()
            }
            CliMode::PerBranch => {
                let mut pairs = Vec::with_capacity(selected_branches.len());
                for branch in selected_branches {
                    let cli = prompter.select_cli_for_branch(&branch, clis)?;
                    pairs.push((branch, cli));
                }
                pairs
            }
        }
    };

    Ok(SelectionResult { mappings })
}

// ---------------------------------------------------------------------------
// Spec-driven CLI resolution
// ---------------------------------------------------------------------------

/// Resolves which CLI to use for each spec-driven branch using a 5-level
/// priority chain:
///
/// 1. `cli_flag` (from `--cli`) → all branches, no prompt
/// 2. `spec.cli` (`paw_cli` in spec) → that branch only
/// 3. `config.default_spec_cli` → remaining branches, no prompt
/// 4. `config.default_cli` → pre-selects in picker for remaining
/// 5. Nothing → full picker for remaining
///
/// Prompts at most once. Validates all resolved CLI names against
/// `available_clis`.
pub fn resolve_cli_for_specs(
    specs: &[SpecEntry],
    cli_flag: Option<&str>,
    config: &PawConfig,
    available_clis: &[CliInfo],
    prompter: &dyn Prompter,
) -> Result<Vec<(String, String)>, PawError> {
    let cli_exists = |name: &str| available_clis.iter().any(|c| c.binary_name == name);

    // Priority 1: --cli flag overrides everything
    if let Some(flag) = cli_flag {
        if !cli_exists(flag) {
            return Err(PawError::CliNotFound(flag.to_string()));
        }
        return Ok(specs
            .iter()
            .map(|s| (s.branch.clone(), flag.to_string()))
            .collect());
    }

    let mut mappings: Vec<(String, String)> = Vec::with_capacity(specs.len());
    let mut remaining: Vec<&SpecEntry> = Vec::new();

    // Priority 2: per-spec paw_cli
    for spec in specs {
        if let Some(ref cli_name) = spec.cli {
            if !cli_exists(cli_name) {
                return Err(PawError::CliNotFound(cli_name.clone()));
            }
            mappings.push((spec.branch.clone(), cli_name.clone()));
        } else {
            remaining.push(spec);
        }
    }

    if remaining.is_empty() {
        return Ok(mappings);
    }

    // Priority 3: default_spec_cli (no prompt)
    if let Some(ref spec_cli) = config.default_spec_cli {
        if !cli_exists(spec_cli) {
            return Err(PawError::CliNotFound(spec_cli.clone()));
        }
        for spec in &remaining {
            mappings.push((spec.branch.clone(), spec_cli.clone()));
        }
        return Ok(mappings);
    }

    // Priority 4+5: prompt once (pre-selected if default_cli set)
    let chosen = prompter.select_cli(available_clis, config.default_cli.as_deref())?;
    for spec in &remaining {
        mappings.push((spec.branch.clone(), chosen.clone()));
    }

    Ok(mappings)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Fake prompter for testing
    // -----------------------------------------------------------------------

    use std::cell::Cell;

    /// A configurable fake prompter that returns predetermined responses.
    /// Uses `Cell` for interior mutability to track per-branch call order
    /// and to capture the `default` parameter passed to `select_cli()`.
    struct TrackingPrompter {
        mode: CliMode,
        branch_indices: Vec<usize>,
        uniform_cli: String,
        per_branch_clis: Vec<String>,
        per_branch_call_count: Cell<usize>,
        cancel_on_branch_select: bool,
        cancel_on_cli_select: bool,
        /// Captures the `default` parameter passed to the last `select_cli()` call.
        last_select_cli_default: Cell<Option<String>>,
    }

    impl TrackingPrompter {
        fn uniform(branch_indices: Vec<usize>, cli: &str) -> Self {
            Self {
                mode: CliMode::Uniform,
                branch_indices,
                uniform_cli: cli.to_string(),
                per_branch_clis: vec![],
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: false,
                cancel_on_cli_select: false,
                last_select_cli_default: Cell::new(None),
            }
        }

        fn per_branch(branch_indices: Vec<usize>, clis: Vec<&str>) -> Self {
            Self {
                mode: CliMode::PerBranch,
                branch_indices,
                uniform_cli: String::new(),
                per_branch_clis: clis.into_iter().map(String::from).collect(),
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: false,
                cancel_on_cli_select: false,
                last_select_cli_default: Cell::new(None),
            }
        }

        fn cancel_on_branches() -> Self {
            Self {
                mode: CliMode::Uniform,
                branch_indices: vec![],
                uniform_cli: String::new(),
                per_branch_clis: vec![],
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: true,
                cancel_on_cli_select: false,
                last_select_cli_default: Cell::new(None),
            }
        }

        fn cancel_on_cli(branch_indices: Vec<usize>) -> Self {
            Self {
                mode: CliMode::Uniform,
                branch_indices,
                uniform_cli: String::new(),
                per_branch_clis: vec![],
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: false,
                cancel_on_cli_select: true,
                last_select_cli_default: Cell::new(None),
            }
        }

        /// Creates a prompter that returns a fixed CLI, used for spec resolution tests.
        fn for_specs(cli: &str) -> Self {
            Self {
                mode: CliMode::Uniform,
                branch_indices: vec![],
                uniform_cli: cli.to_string(),
                per_branch_clis: vec![],
                per_branch_call_count: Cell::new(0),
                cancel_on_branch_select: false,
                cancel_on_cli_select: false,
                last_select_cli_default: Cell::new(None),
            }
        }
    }

    impl Prompter for TrackingPrompter {
        fn select_mode(&self) -> Result<CliMode, PawError> {
            Ok(self.mode)
        }

        fn select_branches(&self, branches: &[String]) -> Result<Vec<String>, PawError> {
            if self.cancel_on_branch_select || self.branch_indices.is_empty() {
                return Err(PawError::UserCancelled);
            }
            Ok(self
                .branch_indices
                .iter()
                .map(|&i| branches[i].clone())
                .collect())
        }

        fn select_cli(&self, _clis: &[CliInfo], default: Option<&str>) -> Result<String, PawError> {
            self.last_select_cli_default.set(default.map(String::from));
            if self.cancel_on_cli_select {
                return Err(PawError::UserCancelled);
            }
            Ok(self.uniform_cli.clone())
        }

        fn select_cli_for_branch(
            &self,
            _branch: &str,
            _clis: &[CliInfo],
        ) -> Result<String, PawError> {
            let idx = self.per_branch_call_count.get();
            self.per_branch_call_count.set(idx + 1);
            self.per_branch_clis
                .get(idx)
                .cloned()
                .ok_or(PawError::UserCancelled)
        }

        fn select_specs(&self, _specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError> {
            Err(PawError::UserCancelled)
        }
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn test_clis() -> Vec<CliInfo> {
        vec![
            CliInfo {
                display_name: "Alpha CLI".to_string(),
                binary_name: "alpha".to_string(),
            },
            CliInfo {
                display_name: "Beta CLI".to_string(),
                binary_name: "beta".to_string(),
            },
        ]
    }

    fn test_branches() -> Vec<String> {
        vec!["feature/auth".to_string(), "fix/api".to_string()]
    }

    // -----------------------------------------------------------------------
    // Behavior tests: flag-based prompt skipping
    // -----------------------------------------------------------------------

    #[test]
    fn both_flags_skips_all_prompts_and_maps_cli_to_all_branches() {
        let prompter = TrackingPrompter::cancel_on_branches(); // should never be called
        let branches = test_branches();
        let clis = test_clis();
        let flag_branches = vec!["feature/auth".to_string(), "fix/api".to_string()];

        let result = run_selection(
            &prompter,
            &branches,
            &clis,
            Some("alpha"),
            Some(&flag_branches),
        )
        .unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "alpha".to_string()),
                ("fix/api".to_string(), "alpha".to_string()),
            ]
        );
    }

    #[test]
    fn cli_flag_skips_cli_prompt_but_prompts_for_branches() {
        let prompter = TrackingPrompter::uniform(vec![0], "should-not-be-used");
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, Some("alpha"), None).unwrap();

        // Should use the flag CLI, and the branch from the prompter (index 0)
        assert_eq!(
            result.mappings,
            vec![("feature/auth".to_string(), "alpha".to_string())]
        );
    }

    #[test]
    fn branches_flag_skips_branch_prompt_but_prompts_for_cli_uniform() {
        let prompter = TrackingPrompter::uniform(vec![], "beta");
        let branches = test_branches();
        let clis = test_clis();
        let flag_branches = vec!["feature/auth".to_string(), "fix/api".to_string()];

        let result =
            run_selection(&prompter, &branches, &clis, None, Some(&flag_branches)).unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "beta".to_string()),
                ("fix/api".to_string(), "beta".to_string()),
            ]
        );
    }

    // -----------------------------------------------------------------------
    // Behavior tests: interactive mode selection
    // -----------------------------------------------------------------------

    #[test]
    fn uniform_mode_maps_same_cli_to_all_selected_branches() {
        let prompter = TrackingPrompter::uniform(vec![0, 1], "alpha");
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None).unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "alpha".to_string()),
                ("fix/api".to_string(), "alpha".to_string()),
            ]
        );
    }

    #[test]
    fn per_branch_mode_maps_different_cli_to_each_branch() {
        let prompter = TrackingPrompter::per_branch(vec![0, 1], vec!["alpha", "beta"]);
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None).unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "alpha".to_string()),
                ("fix/api".to_string(), "beta".to_string()),
            ]
        );
    }

    #[test]
    fn per_branch_mode_with_branches_flag() {
        let prompter = TrackingPrompter::per_branch(vec![], vec!["beta", "alpha"]);
        let branches = test_branches();
        let clis = test_clis();
        let flag_branches = vec!["feature/auth".to_string(), "fix/api".to_string()];

        let result =
            run_selection(&prompter, &branches, &clis, None, Some(&flag_branches)).unwrap();

        assert_eq!(
            result.mappings,
            vec![
                ("feature/auth".to_string(), "beta".to_string()),
                ("fix/api".to_string(), "alpha".to_string()),
            ]
        );
    }

    // -----------------------------------------------------------------------
    // Behavior tests: cancellation / error cases
    // -----------------------------------------------------------------------

    #[test]
    fn no_clis_available_returns_error() {
        let prompter = TrackingPrompter::cancel_on_branches();
        let branches = test_branches();
        let clis: Vec<CliInfo> = vec![];

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::NoCLIsFound)));
    }

    #[test]
    fn no_branches_available_returns_error() {
        let prompter = TrackingPrompter::cancel_on_branches();
        let branches: Vec<String> = vec![];
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::BranchError(_))));
    }

    #[test]
    fn user_cancels_branch_selection_returns_cancelled() {
        let prompter = TrackingPrompter::cancel_on_branches();
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    #[test]
    fn user_selects_no_branches_returns_cancelled() {
        // Empty branch_indices with cancel_on_branch_select=false still returns cancelled
        let prompter = TrackingPrompter::uniform(vec![], "alpha");
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    #[test]
    fn user_cancels_cli_selection_returns_cancelled() {
        let prompter = TrackingPrompter::cancel_on_cli(vec![0]);
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None);

        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    // -----------------------------------------------------------------------
    // Behavior tests: selection with subset of branches
    // -----------------------------------------------------------------------

    #[test]
    fn selecting_subset_of_branches_works() {
        let prompter = TrackingPrompter::uniform(vec![1], "alpha"); // only fix/api
        let branches = test_branches();
        let clis = test_clis();

        let result = run_selection(&prompter, &branches, &clis, None, None).unwrap();

        assert_eq!(
            result.mappings,
            vec![("fix/api".to_string(), "alpha".to_string())]
        );
    }

    // -----------------------------------------------------------------------
    // Display impls
    // -----------------------------------------------------------------------

    #[test]
    fn cli_mode_display() {
        assert_eq!(CliMode::Uniform.to_string(), "Same CLI for all branches");
        assert_eq!(CliMode::PerBranch.to_string(), "Different CLI per branch");
    }

    #[test]
    fn cli_info_display_same_names() {
        let info = CliInfo {
            display_name: "claude".to_string(),
            binary_name: "claude".to_string(),
        };
        assert_eq!(info.to_string(), "claude");
    }

    #[test]
    fn cli_info_display_different_names() {
        let info = CliInfo {
            display_name: "My Agent".to_string(),
            binary_name: "my-agent".to_string(),
        };
        assert_eq!(info.to_string(), "My Agent (my-agent)");
    }

    // -----------------------------------------------------------------------
    // resolve_cli_for_specs tests
    // -----------------------------------------------------------------------

    fn default_config() -> PawConfig {
        PawConfig::default()
    }

    fn spec(branch: &str, cli: Option<&str>) -> SpecEntry {
        SpecEntry {
            id: branch.to_string(),
            backend: crate::specs::SpecBackendKind::Markdown,
            branch: branch.to_string(),
            cli: cli.map(String::from),
            prompt: String::new(),
            owned_files: None,
        }
    }

    fn test_specs() -> Vec<SpecEntry> {
        vec![
            spec("spec/auth", None),
            spec("spec/api", None),
            spec("spec/db", None),
        ]
    }

    #[test]
    fn cli_flag_overrides_all_specs() {
        let prompter = TrackingPrompter::for_specs("should-not-be-used");
        let clis = test_clis();
        let specs = test_specs();

        let result =
            resolve_cli_for_specs(&specs, Some("alpha"), &default_config(), &clis, &prompter)
                .unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|(_, cli)| cli == "alpha"));
    }

    #[test]
    fn paw_cli_per_spec_overrides_config() {
        let specs = vec![spec("spec/auth", Some("beta")), spec("spec/api", None)];
        let mut config = default_config();
        config.default_spec_cli = Some("alpha".to_string());
        let prompter = TrackingPrompter::for_specs("should-not-be-used");
        let clis = test_clis();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert!(result.iter().any(|(b, c)| b == "spec/auth" && c == "beta"));
        assert!(result.iter().any(|(b, c)| b == "spec/api" && c == "alpha"));
    }

    #[test]
    fn default_spec_cli_fills_remaining_without_prompt() {
        let mut config = default_config();
        config.default_spec_cli = Some("alpha".to_string());
        let prompter = TrackingPrompter::cancel_on_cli(vec![]); // would fail if called
        let clis = test_clis();
        let specs = test_specs();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|(_, cli)| cli == "alpha"));
    }

    #[test]
    fn default_cli_pre_selects_in_picker() {
        let mut config = default_config();
        config.default_cli = Some("beta".to_string());
        let prompter = TrackingPrompter::for_specs("beta");
        let clis = test_clis();
        let specs = vec![spec("spec/auth", None)];

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert_eq!(result, vec![("spec/auth".to_string(), "beta".to_string())]);
        // Verify default was passed to select_cli
        assert_eq!(
            prompter.last_select_cli_default.take(),
            Some("beta".to_string())
        );
    }

    #[test]
    fn no_defaults_picker_fires_with_none_default() {
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();
        let specs = vec![spec("spec/auth", None)];

        let result =
            resolve_cli_for_specs(&specs, None, &default_config(), &clis, &prompter).unwrap();

        assert_eq!(result, vec![("spec/auth".to_string(), "alpha".to_string())]);
        assert_eq!(prompter.last_select_cli_default.take(), None);
    }

    #[test]
    fn mixed_paw_cli_and_default_spec_cli() {
        let specs = vec![
            spec("spec/auth", Some("beta")),
            spec("spec/api", None),
            spec("spec/db", None),
        ];
        let mut config = default_config();
        config.default_spec_cli = Some("alpha".to_string());
        let prompter = TrackingPrompter::for_specs("should-not-be-used");
        let clis = test_clis();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.iter().any(|(b, c)| b == "spec/auth" && c == "beta"));
        assert!(result.iter().any(|(b, c)| b == "spec/api" && c == "alpha"));
        assert!(result.iter().any(|(b, c)| b == "spec/db" && c == "alpha"));
    }

    #[test]
    fn mixed_paw_cli_and_interactive() {
        let specs = vec![
            spec("spec/auth", Some("beta")),
            spec("spec/api", None),
            spec("spec/db", None),
        ];
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();

        let result =
            resolve_cli_for_specs(&specs, None, &default_config(), &clis, &prompter).unwrap();

        assert_eq!(result.len(), 3);
        assert!(result.iter().any(|(b, c)| b == "spec/auth" && c == "beta"));
        assert!(result.iter().any(|(b, c)| b == "spec/api" && c == "alpha"));
        assert!(result.iter().any(|(b, c)| b == "spec/db" && c == "alpha"));
    }

    #[test]
    fn picker_fires_at_most_once_for_multiple_remaining() {
        let specs = vec![
            spec("spec/a", Some("beta")),
            spec("spec/b", None),
            spec("spec/c", None),
            spec("spec/d", None),
        ];
        // If select_cli is called more than once this will still return "alpha",
        // but we verify the behavior: all remaining get the same CLI.
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();

        let result =
            resolve_cli_for_specs(&specs, None, &default_config(), &clis, &prompter).unwrap();

        let remaining: Vec<_> = result.iter().filter(|(_, c)| c == "alpha").collect();
        assert_eq!(remaining.len(), 3);
    }

    #[test]
    fn all_resolved_via_flag_no_prompt() {
        let prompter = TrackingPrompter::cancel_on_cli(vec![]); // would fail if called
        let clis = test_clis();
        let specs = test_specs();

        let result =
            resolve_cli_for_specs(&specs, Some("alpha"), &default_config(), &clis, &prompter)
                .unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn all_resolved_via_paw_cli_and_default_spec_cli_no_prompt() {
        let specs = vec![spec("spec/auth", Some("alpha")), spec("spec/api", None)];
        let mut config = default_config();
        config.default_spec_cli = Some("beta".to_string());
        let prompter = TrackingPrompter::cancel_on_cli(vec![]); // would fail if called
        let clis = test_clis();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn paw_cli_references_unknown_cli_returns_error() {
        let specs = vec![spec("spec/auth", Some("nonexistent"))];
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();

        let result = resolve_cli_for_specs(&specs, None, &default_config(), &clis, &prompter);
        assert!(matches!(result, Err(PawError::CliNotFound(ref name)) if name == "nonexistent"));
    }

    #[test]
    fn default_spec_cli_references_unknown_cli_returns_error() {
        let mut config = default_config();
        config.default_spec_cli = Some("nonexistent".to_string());
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();
        let specs = test_specs();

        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter);
        assert!(matches!(result, Err(PawError::CliNotFound(ref name)) if name == "nonexistent"));
    }

    #[test]
    fn cli_flag_references_unknown_cli_returns_error() {
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();
        let specs = test_specs();

        let result = resolve_cli_for_specs(
            &specs,
            Some("nonexistent"),
            &default_config(),
            &clis,
            &prompter,
        );
        assert!(matches!(result, Err(PawError::CliNotFound(ref name)) if name == "nonexistent"));
    }

    #[test]
    fn select_cli_with_default_present_and_in_list() {
        let prompter = TrackingPrompter::for_specs("beta");
        let clis = test_clis();
        let specs = vec![spec("spec/x", None)];
        let mut config = default_config();
        config.default_cli = Some("beta".to_string());

        resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();

        assert_eq!(
            prompter.last_select_cli_default.take(),
            Some("beta".to_string())
        );
    }

    #[test]
    fn select_cli_with_default_not_in_list_graceful() {
        let prompter = TrackingPrompter::for_specs("alpha");
        let clis = test_clis();
        let specs = vec![spec("spec/x", None)];
        let mut config = default_config();
        config.default_cli = Some("nonexistent".to_string());

        // Should not error — the default just doesn't pre-select
        let result = resolve_cli_for_specs(&specs, None, &config, &clis, &prompter).unwrap();
        assert_eq!(result, vec![("spec/x".to_string(), "alpha".to_string())]);
        assert_eq!(
            prompter.last_select_cli_default.take(),
            Some("nonexistent".to_string())
        );
    }

    // -----------------------------------------------------------------------
    // Spec multi-select picker grouping (cross-format-spec-selection)
    // -----------------------------------------------------------------------

    fn bare_spec(id: &str) -> SpecEntry {
        SpecEntry {
            id: id.to_string(),
            backend: crate::specs::SpecBackendKind::Markdown,
            branch: format!("spec/{id}"),
            cli: None,
            prompt: String::new(),
            owned_files: None,
        }
    }

    #[test]
    fn group_flat_specs_yields_one_row_each() {
        let specs = vec![
            bare_spec("add-auth"),
            bare_spec("fix-session"),
            bare_spec("add-logging"),
        ];
        let groups = group_specs_by_unit(&specs);
        let labels: Vec<&str> = groups.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, vec!["add-auth", "fix-session", "add-logging"]);
        for (_, idxs) in &groups {
            assert_eq!(idxs.len(), 1);
        }
    }

    #[test]
    fn finalize_spec_selection_returns_chosen_subset_for_flat_entries() {
        let specs = vec![
            bare_spec("add-auth"),
            bare_spec("fix-session"),
            bare_spec("add-logging"),
        ];
        let groups = group_specs_by_unit(&specs);
        // User toggles "add-auth" (row 0) and "add-logging" (row 2).
        let result = finalize_spec_selection(&specs, &groups, Some(vec![0, 2])).unwrap();
        let ids: Vec<&str> = result.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, vec!["add-auth", "add-logging"]);
    }

    #[test]
    fn finalize_spec_selection_expands_spec_kit_feature_row_to_all_entries() {
        let specs = vec![
            bare_spec("003-user-list-T009"),
            bare_spec("003-user-list-T010"),
            bare_spec("003-user-list-phase-2"),
        ];
        let groups = group_specs_by_unit(&specs);
        // Single row "003-user-list" → all 3 underlying entries.
        let result = finalize_spec_selection(&specs, &groups, Some(vec![0])).unwrap();
        let ids: Vec<&str> = result.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "003-user-list-T009",
                "003-user-list-T010",
                "003-user-list-phase-2",
            ]
        );
    }

    #[test]
    fn finalize_spec_selection_none_returns_user_cancelled() {
        // dialoguer returns None when the user presses Ctrl+C.
        let specs = vec![bare_spec("add-auth")];
        let groups = group_specs_by_unit(&specs);
        let result = finalize_spec_selection(&specs, &groups, None);
        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    #[test]
    fn finalize_spec_selection_empty_indices_returns_user_cancelled() {
        // User confirms (Enter) without toggling any row → empty Vec.
        let specs = vec![bare_spec("add-auth"), bare_spec("fix-session")];
        let groups = group_specs_by_unit(&specs);
        let result = finalize_spec_selection(&specs, &groups, Some(vec![]));
        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    #[test]
    fn group_spec_kit_feature_collapses_to_one_row_with_count_hint() {
        let specs = vec![
            bare_spec("003-user-list-T009"),
            bare_spec("003-user-list-T010"),
            bare_spec("003-user-list-phase-2"),
            bare_spec("004-error-handling-phase-1"),
        ];
        let groups = group_specs_by_unit(&specs);
        assert_eq!(groups.len(), 2);
        let user_list = &groups[0];
        assert!(
            user_list.0.starts_with("003-user-list"),
            "first group label should start with feature id; got: {}",
            user_list.0
        );
        assert!(user_list.0.contains("3 worktrees"), "got: {}", user_list.0);
        assert!(user_list.0.contains("2 [P]"), "got: {}", user_list.0);
        assert!(user_list.0.contains("1 phase/"), "got: {}", user_list.0);
        assert_eq!(user_list.1.len(), 3);

        let error_handling = &groups[1];
        assert_eq!(error_handling.0, "004-error-handling");
        assert_eq!(error_handling.1.len(), 1);
    }

    // --- test-coverage-v0-5-0: spec picker cancellation paths -----------------
    //
    // The two scenarios `User cancels spec picker via Ctrl+C` and `User confirms
    // with zero rows selected` both expect the caller to propagate
    // `PawError::UserCancelled`. The TerminalPrompter implementation routes
    // both through `finalize_spec_selection`. For the unit tests we exercise
    // the mapping function directly (which is the production code path) and
    // assert the resulting Err shape.

    /// A `Prompter` whose `select_specs` always returns
    /// `Err(PawError::UserCancelled)` — the Ctrl+C path.
    struct CancelOnSpecsPrompter;

    impl Prompter for CancelOnSpecsPrompter {
        fn select_mode(&self) -> Result<CliMode, PawError> {
            Err(PawError::UserCancelled)
        }
        fn select_branches(&self, _branches: &[String]) -> Result<Vec<String>, PawError> {
            Err(PawError::UserCancelled)
        }
        fn select_cli(
            &self,
            _clis: &[CliInfo],
            _default: Option<&str>,
        ) -> Result<String, PawError> {
            Err(PawError::UserCancelled)
        }
        fn select_cli_for_branch(
            &self,
            _branch: &str,
            _clis: &[CliInfo],
        ) -> Result<String, PawError> {
            Err(PawError::UserCancelled)
        }
        fn select_specs(&self, _specs: &[SpecEntry]) -> Result<Vec<SpecEntry>, PawError> {
            Err(PawError::UserCancelled)
        }
    }

    // Maps to scenario `User cancels spec picker via Ctrl+C` from
    // cross-format-spec-selection. (test-coverage-v0-5-0 task 7.1)
    #[test]
    fn select_specs_cancel_returns_user_cancelled() {
        let prompter = CancelOnSpecsPrompter;
        let specs = vec![bare_spec("003-user-list")];
        let result = prompter.select_specs(&specs);
        assert!(
            matches!(result, Err(PawError::UserCancelled)),
            "select_specs cancel path must propagate UserCancelled; got: {result:?}"
        );
    }

    // Maps to scenario `User confirms with zero rows selected` from
    // cross-format-spec-selection. The TerminalPrompter wires the
    // `fuzzy_multi_select` picker's `Some(empty Vec)` result through
    // `finalize_spec_selection`, which maps it to UserCancelled. This test
    // exercises that mapping function directly with `Some(empty)` because
    // that is where the production decision lives.
    // (test-coverage-v0-5-0 task 7.2)
    #[test]
    fn select_specs_zero_selection_returns_user_cancelled() {
        let specs = vec![bare_spec("003-user-list")];
        let groups = group_specs_by_unit(&specs);
        // `Some(vec![])` represents the user confirming with zero rows
        // selected — the picker returns an empty index list.
        let result = finalize_spec_selection(&specs, &groups, Some(vec![]));
        assert!(
            matches!(result, Err(PawError::UserCancelled)),
            "zero-row confirmation must map to UserCancelled; got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // searchable-pickers: PickerState filtering + selection (interactive-
    // selection capability). The TUI render loop in `fuzzy_multi_select` is
    // coverage-exempt; the filter/selection contract is tested here through
    // the pure `PickerState`, and the cancellation/return-shape mapping
    // through `finalize_branch_selection`.
    // -----------------------------------------------------------------------

    fn picker_branches() -> Vec<String> {
        vec![
            "feature/auth".to_string(),
            "fix/api".to_string(),
            "feature/login".to_string(),
        ]
    }

    // Scenario `Typing a query filters the branch candidates` + scenario
    // `Empty filter shows the full branch list` (task 4.1).
    #[test]
    fn picker_query_filters_branches_and_empty_shows_full_list() {
        let mut state = PickerState::new(picker_branches());

        // Empty query → all candidates, original order.
        assert_eq!(state.visible_indices(), vec![0, 1, 2]);

        // Typing `feature` keeps only the two feature/* branches; `fix/api`
        // (original index 1) is hidden.
        state.set_query("feature".to_string());
        assert_eq!(state.visible_indices(), vec![0, 2]);
    }

    // Scenario `Typing a query filters the branch candidates` — case-
    // insensitive substring match.
    #[test]
    fn picker_query_match_is_case_insensitive() {
        let mut state = PickerState::new(vec!["Feature/Auth".to_string(), "fix/api".to_string()]);
        state.set_query("FEAT".to_string());
        assert_eq!(state.visible_indices(), vec![0]);
    }

    // Scenario `Selection under an active filter is preserved when the filter
    // changes` (task 4.2).
    #[test]
    fn picker_selection_persists_across_query_changes() {
        let mut state = PickerState::new(picker_branches());

        // Type `feature`, toggle the visible `feature/auth` row (visible row 0
        // → original index 0).
        state.set_query("feature".to_string());
        state.toggle(0);

        // Change the query to `fix`, toggle the visible `fix/api` row (visible
        // row 0 → original index 1).
        state.set_query("fix".to_string());
        state.toggle(0);

        // Confirm returns both original indices, sorted — even though each was
        // toggled under a query that hid the other.
        assert_eq!(state.confirm(), vec![0, 1]);
    }

    // Scenario `Clearing the filter restores the full list with selections
    // intact` (task 4.3).
    #[test]
    fn picker_clearing_query_restores_full_list_with_selection() {
        let mut state = PickerState::new(picker_branches());

        state.set_query("feature".to_string());
        state.toggle(0); // feature/auth (original index 0)

        // Clear the query back to empty.
        state.set_query(String::new());

        // Full list visible again, in original order...
        assert_eq!(state.visible_indices(), vec![0, 1, 2]);
        // ...and the earlier toggle is still marked.
        assert!(state.is_selected(0));
        assert_eq!(state.confirm(), vec![0]);
    }

    // Scenario `User cancels the filtered branch picker via Ctrl+C` (task
    // 4.5). The render loop maps Ctrl+C (and Esc) to `None` regardless of the
    // active filter or any pending selection; `finalize_branch_selection`
    // turns that into `UserCancelled`. We assert the mapping with a selection
    // present to capture the *filtered* Ctrl+C path at the helper boundary.
    #[test]
    fn filtered_branch_picker_ctrl_c_maps_to_user_cancelled() {
        // A picker that had an active filter and a toggled row...
        let mut state = PickerState::new(picker_branches());
        state.set_query("feature".to_string());
        state.toggle(0);
        assert!(
            !state.confirm().is_empty(),
            "precondition: a row is selected"
        );

        // ...still cancels when the loop returns `None` (Ctrl+C / Esc).
        let result = finalize_branch_selection(&picker_branches(), None);
        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    // Scenario `Confirming with zero branches selected cancels` (task 4.5).
    #[test]
    fn branch_picker_zero_selection_maps_to_user_cancelled() {
        let result = finalize_branch_selection(&picker_branches(), Some(vec![]));
        assert!(matches!(result, Err(PawError::UserCancelled)));
    }

    // Scenario `Selecting one of two branches` at the helper boundary: the
    // returned original indices map back to the right branch names.
    #[test]
    fn branch_picker_indices_map_to_names() {
        let branches = picker_branches();
        let result = finalize_branch_selection(&branches, Some(vec![1])).unwrap();
        assert_eq!(result, vec!["fix/api".to_string()]);
    }

    fn spec_row_labels(specs: &[SpecEntry]) -> Vec<String> {
        group_specs_by_unit(specs)
            .into_iter()
            .map(|(label, _)| label)
            .collect()
    }

    // Scenario `Typing a query filters the spec rows` + scenario `Empty filter
    // shows every grouped spec row`.
    #[test]
    fn picker_query_filters_spec_rows_and_empty_shows_all() {
        let specs = vec![
            bare_spec("add-auth"),
            bare_spec("fix-session"),
            bare_spec("add-logging"),
        ];
        let mut state = PickerState::new(spec_row_labels(&specs));

        // Empty query → all three grouped rows, original order.
        assert_eq!(state.visible_indices(), vec![0, 1, 2]);

        // `add` keeps add-auth (row 0) and add-logging (row 2); fix-session
        // (row 1) is hidden.
        state.set_query("add".to_string());
        assert_eq!(state.visible_indices(), vec![0, 2]);
    }

    // Scenario `Clearing the spec filter restores all rows with selections
    // intact`.
    #[test]
    fn picker_clearing_spec_filter_restores_rows_with_selection() {
        let specs = vec![
            bare_spec("add-auth"),
            bare_spec("fix-session"),
            bare_spec("add-logging"),
        ];
        let mut state = PickerState::new(spec_row_labels(&specs));

        state.set_query("add".to_string());
        state.toggle(0); // add-auth row (original index 0)

        state.set_query(String::new());

        assert_eq!(state.visible_indices(), vec![0, 1, 2]);
        assert!(state.is_selected(0));
    }

    // Task 4.4 + scenario `Selecting a filtered Spec Kit row still expands to
    // all its entries`: filtering by `003` keeps the `003-user-list` row, and
    // selecting that visible row expands (via `finalize_spec_selection`) to all
    // 3 underlying `SpecEntry` values — crossing the PickerState → finalize
    // boundary.
    #[test]
    fn filtered_spec_kit_row_expands_to_all_underlying_entries() {
        let specs = vec![
            bare_spec("003-user-list-T009"),
            bare_spec("003-user-list-T010"),
            bare_spec("003-user-list-phase-2"),
            bare_spec("004-error-handling-phase-1"),
        ];
        let groups = group_specs_by_unit(&specs);
        let labels: Vec<String> = groups.iter().map(|(label, _)| label.clone()).collect();

        let mut state = PickerState::new(labels);
        state.set_query("003".to_string());

        // Only the 003-user-list grouped row is visible.
        let visible = state.visible_indices();
        assert_eq!(visible.len(), 1);
        assert!(groups[visible[0]].0.starts_with("003-user-list"));

        // Selecting visible row 0 and confirming yields the original group
        // index, which finalize expands to all 3 underlying entries.
        state.toggle(0);
        let selected_rows = state.confirm();
        let result = finalize_spec_selection(&specs, &groups, Some(selected_rows)).unwrap();
        let ids: Vec<&str> = result.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "003-user-list-T009",
                "003-user-list-T010",
                "003-user-list-phase-2",
            ]
        );
    }
}
