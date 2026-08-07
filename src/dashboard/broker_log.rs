//! The dashboard's "Broker log" panel: a bounded ring buffer of observed
//! broker messages plus the ratatui widget that renders them with per-type
//! filtering and a details overlay.
//!
//! v0.6.0 fills the screen region freed by v0.5.0's prompt-inbox removal
//! with this panel. It lists every broker message type, newest at the top,
//! filterable per-type via single-press hotkeys and toggleable on/off.
//!
//! # Architecture note
//!
//! design.md D3 described feeding the panel from a
//! `tokio::sync::broadcast::Receiver`. The actual dashboard is a synchronous
//! thread-polling loop (`std::thread::sleep`), not a tokio task, so there is
//! no broadcast channel to subscribe to. Instead the panel is fed by
//! [`BrokerLog::ingest`] from the dashboard's existing per-tick poll of
//! [`crate::broker::BrokerState`] via a monotonic sequence cursor. This adds
//! no broker traffic (the same in-process state the agent table already
//! reads) and yields the watcher-restart resilience design.md D8 requires
//! for free: the ring buffer lives in the dashboard process and is never
//! cleared, and the seq cursor only ever advances.

use std::collections::VecDeque;
use std::time::SystemTime;

use crossterm::event::KeyCode;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::broker::messages::BrokerMessage;

/// One retained log entry: the broker sequence number, the wall-clock
/// timestamp the broker recorded, and the message itself. Mirrors the tuple
/// shape of [`crate::broker::BrokerStateInner::message_log`].
pub type LogEntry = (u64, SystemTime, BrokerMessage);

// ---------------------------------------------------------------------------
// Filter bitmask (design.md D4)
// ---------------------------------------------------------------------------

/// Filter bit for `agent.status` messages.
pub const BIT_STATUS: u16 = 1 << 0;
/// Filter bit for `agent.artifact` messages.
pub const BIT_ARTIFACT: u16 = 1 << 1;
/// Filter bit for `agent.blocked` messages.
pub const BIT_BLOCKED: u16 = 1 << 2;
/// Filter bit for `agent.verified` messages.
pub const BIT_VERIFIED: u16 = 1 << 3;
/// Filter bit for `agent.feedback` messages.
pub const BIT_FEEDBACK: u16 = 1 << 4;
/// Filter bit for `agent.question` messages.
pub const BIT_QUESTION: u16 = 1 << 5;
/// Filter bit for `agent.intent` messages.
pub const BIT_INTENT: u16 = 1 << 6;
/// Filter bit for `supervisor.verify-now` messages.
pub const BIT_VERIFY_NOW: u16 = 1 << 7;
/// Filter bit for `agent.advanced-main` messages.
pub const BIT_ADVANCED_MAIN: u16 = 1 << 8;
/// Filter bit for `agent.learning` messages.
pub const BIT_LEARNING: u16 = 1 << 9;
/// Filter bit for `agent.answer` messages. Carries its own bit so the mask
/// stays one-bit-per-variant, but has no chip row entry yet — the digit
/// hotkeys (`1`-`9` then `0`) are exhausted at ten chips, so answers are
/// visible in the default `All` mode and a dedicated chip awaits a keybinding
/// decision in a dashboard-scoped change.
pub const BIT_ANSWER: u16 = 1 << 10;

/// The "show everything" sentinel. Distinct from the bitwise-OR of every
/// known bit so that selecting every chip individually is still treated as
/// an explicit (non-`All`) selection.
pub const FILTER_ALL: u16 = 0xFFFF;

/// Ordered chip table: `(bit, short label)`, indexed by the `1`-`9` then `0`
/// hotkeys (the tenth chip is reached with `0`).
///
/// One chip per [`BrokerMessage`] variant the panel knows about, per spec
/// requirement "Per-type filter chips" — one chip per known message type.
/// The broker-emitted `verify-now`, the supervisor `advanced-main`, and the
/// aggregator `learning` variants all exist and each get a chip (the
/// proposal's illustrative list named the `learning` chip; it became real
/// once the `agent-learning-variant` change merged).
pub const CHIPS: [(u16, &str); 10] = [
    (BIT_STATUS, "status"),
    (BIT_ARTIFACT, "artifact"),
    (BIT_BLOCKED, "blocked"),
    (BIT_VERIFIED, "verified"),
    (BIT_FEEDBACK, "feedback"),
    (BIT_QUESTION, "question"),
    (BIT_INTENT, "intent"),
    (BIT_VERIFY_NOW, "verify-now"),
    (BIT_ADVANCED_MAIN, "advanced-main"),
    (BIT_LEARNING, "learning"),
];

/// Maps a message to its filter bit.
#[must_use]
pub fn message_bit(msg: &BrokerMessage) -> u16 {
    match msg {
        BrokerMessage::Status { .. } => BIT_STATUS,
        BrokerMessage::Artifact { .. } => BIT_ARTIFACT,
        BrokerMessage::Blocked { .. } => BIT_BLOCKED,
        BrokerMessage::Verified { .. } => BIT_VERIFIED,
        BrokerMessage::Feedback { .. } => BIT_FEEDBACK,
        BrokerMessage::Question { .. } => BIT_QUESTION,
        BrokerMessage::Intent { .. } => BIT_INTENT,
        BrokerMessage::VerifyNow { .. } => BIT_VERIFY_NOW,
        BrokerMessage::AdvancedMain { .. } => BIT_ADVANCED_MAIN,
        BrokerMessage::Learning { .. } => BIT_LEARNING,
        BrokerMessage::Answer { .. } => BIT_ANSWER,
    }
}

/// The panel's per-type filter state.
///
/// The value [`FILTER_ALL`] means "All" mode — every message is visible and
/// no individual chip is highlighted. Any other value is an explicit
/// selection set: a message is visible iff its [`message_bit`] is set.
///
/// Toggle semantics (matching the spec scenarios):
/// - Toggling a chip while in `All` mode leaves `All` and narrows to *only*
///   that type.
/// - Toggling a chip in selection mode flips its bit; emptying the set
///   returns to `All`.
/// - [`FilterMask::reset`] returns to `All` unconditionally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterMask(u16);

impl Default for FilterMask {
    fn default() -> Self {
        Self(FILTER_ALL)
    }
}

impl FilterMask {
    /// The default `All` mask.
    #[must_use]
    pub fn all() -> Self {
        Self(FILTER_ALL)
    }

    /// Returns `true` when in `All` mode (every message visible).
    #[must_use]
    pub fn is_all(self) -> bool {
        self.0 == FILTER_ALL
    }

    /// Resets to `All` mode.
    pub fn reset(&mut self) {
        self.0 = FILTER_ALL;
    }

    /// Toggles a single chip bit, applying the `All`-mode transition rules.
    pub fn toggle(&mut self, bit: u16) {
        if self.0 == FILTER_ALL {
            // Leaving All: narrow to just this type.
            self.0 = bit;
        } else {
            self.0 ^= bit;
            if self.0 == 0 {
                // Emptied the selection — fall back to All.
                self.0 = FILTER_ALL;
            }
        }
    }

    /// Returns `true` when the given message passes the active filter.
    #[must_use]
    pub fn matches(self, msg: &BrokerMessage) -> bool {
        self.is_all() || (self.0 & message_bit(msg)) != 0
    }

    /// Returns `true` when a chip is part of the active explicit selection.
    /// Always `false` in `All` mode (no individual chip is "selected").
    #[must_use]
    pub fn is_chip_active(self, bit: u16) -> bool {
        !self.is_all() && (self.0 & bit) != 0
    }
}

// ---------------------------------------------------------------------------
// Ring buffer
// ---------------------------------------------------------------------------

/// The Broker log panel's state: a bounded ring buffer of observed messages
/// plus the UI state (active filter, visibility, selected row, overlay).
#[derive(Debug)]
pub struct BrokerLog {
    /// Retained messages, newest at the front.
    buffer: VecDeque<LogEntry>,
    /// Hard cap on retained messages (`[dashboard.broker_log] max_messages`).
    max: usize,
    /// Active per-type filter.
    filter: FilterMask,
    /// Whether the panel is currently rendered.
    pub visible: bool,
    /// Highest broker sequence number ingested so far; the poll cursor.
    last_seq: u64,
    /// Index of the highlighted row *within the currently visible subset*.
    selected: usize,
    /// Whether the details overlay is open.
    overlay_open: bool,
}

impl BrokerLog {
    /// Creates an empty log with the given capacity and initial visibility.
    ///
    /// A `max` of 0 is clamped to 1 so the buffer can always hold at least
    /// the newest message (a zero-capacity ring buffer would be useless and
    /// `truncate(0)` would drop everything immediately).
    #[must_use]
    pub fn new(max_messages: usize, visible: bool) -> Self {
        Self {
            buffer: VecDeque::new(),
            max: max_messages.max(1),
            filter: FilterMask::all(),
            visible,
            last_seq: 0,
            selected: 0,
            overlay_open: false,
        }
    }

    /// Pushes one entry to the front and truncates to the cap, dropping the
    /// oldest entries beyond `max` (task 2.2).
    pub fn push(&mut self, entry: LogEntry) {
        self.buffer.push_front(entry);
        self.buffer.truncate(self.max);
    }

    /// Ingests a batch of new messages fetched from the broker state.
    ///
    /// `new_msgs` is expected in chronological (seq-ascending) order — the
    /// shape [`crate::broker::delivery::full_log`] returns. Only entries with
    /// a sequence number beyond [`BrokerLog::last_seq`] are taken, so calling
    /// this every tick with the full filtered slice is idempotent and the
    /// cursor only advances. Pushing in chronological order leaves the newest
    /// message at the front.
    pub fn ingest(&mut self, new_msgs: impl IntoIterator<Item = LogEntry>) {
        for entry in new_msgs {
            if entry.0 <= self.last_seq {
                continue;
            }
            self.last_seq = entry.0;
            self.push(entry);
        }
    }

    /// The poll cursor: the highest sequence number ingested so far. The
    /// dashboard passes this to [`crate::broker::delivery::full_log`] to fetch
    /// only messages newer than what the buffer already holds.
    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// The configured capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.max
    }

    /// Total retained messages, ignoring the active filter.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the buffer holds no messages.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// The active filter mask.
    #[must_use]
    pub fn filter(&self) -> FilterMask {
        self.filter
    }

    /// Whether the details overlay is open.
    #[must_use]
    pub fn overlay_open(&self) -> bool {
        self.overlay_open
    }

    /// Yields the messages matching the active filter, newest first
    /// (task 2.4). The underlying buffer retains every message regardless of
    /// the filter.
    pub fn iter_visible(&self) -> impl Iterator<Item = &LogEntry> {
        self.buffer
            .iter()
            .filter(|entry| self.filter.matches(&entry.2))
    }

    /// The panel's rendered rows: the filtered messages, newest first, with
    /// consecutive identical status heartbeats collapsed into counted rows
    /// (see [`collapse_status_runs`]). The ring buffer keeps every message
    /// regardless — the collapse is a view, not a data change.
    #[must_use]
    pub fn visible_rows(&self) -> Vec<CollapsedRow<'_>> {
        collapse_status_runs(self.iter_visible())
    }

    /// Count of rendered rows: messages matching the active filter, after
    /// consecutive identical heartbeats collapse. Row selection is indexed
    /// against this count, so it is what the panel highlights and scrolls.
    #[must_use]
    pub fn visible_count(&self) -> usize {
        self.visible_rows().len()
    }

    /// The currently highlighted row's entry, if any visible row exists. For a
    /// collapsed run this is the newest heartbeat of the run.
    #[must_use]
    pub fn selected_entry(&self) -> Option<&LogEntry> {
        self.visible_rows().get(self.selected).map(|row| row.entry)
    }

    /// The highlighted row index, clamped to the visible range.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Clamps `selected` so it never points past the last visible row. Called
    /// after any operation that can shrink the visible set (new filter, etc.).
    fn clamp_selection(&mut self) {
        let visible = self.visible_count();
        if visible == 0 {
            self.selected = 0;
        } else if self.selected >= visible {
            self.selected = visible - 1;
        }
    }

    /// Moves the highlight one row toward the top (newer message).
    pub fn select_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Moves the highlight one row toward the bottom (older message).
    pub fn select_down(&mut self) {
        let visible = self.visible_count();
        if visible > 0 && self.selected + 1 < visible {
            self.selected += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Summary extraction + row formatting (section 4)
// ---------------------------------------------------------------------------

/// The short type label rendered in a row's type column.
#[must_use]
pub fn type_short(msg: &BrokerMessage) -> &'static str {
    match msg {
        BrokerMessage::Status { .. } => "status",
        BrokerMessage::Artifact { .. } => "artifact",
        BrokerMessage::Blocked { .. } => "blocked",
        BrokerMessage::Verified { .. } => "verified",
        BrokerMessage::Feedback { .. } => "feedback",
        BrokerMessage::Question { .. } => "question",
        BrokerMessage::Intent { .. } => "intent",
        BrokerMessage::VerifyNow { .. } => "verify-now",
        BrokerMessage::AdvancedMain { .. } => "advanced-main",
        BrokerMessage::Learning { .. } => "learning",
        BrokerMessage::Answer { .. } => "answer",
    }
}

/// Derives a one-line summary from a message body (task 4.2). One arm per
/// known variant; the row formatter truncates the result to fit the panel.
#[must_use]
pub fn derive_summary(msg: &BrokerMessage) -> String {
    match msg {
        BrokerMessage::Status { payload, .. } => {
            // The bare-status / `status: message` shapes are exactly what this
            // arm produced before `activity` existed; the phrase is appended
            // in parentheses only when the payload carries a non-blank one, so
            // a status without it renders byte-identically to before.
            let base = match &payload.message {
                Some(m) if !m.trim().is_empty() => format!("{}: {m}", payload.status),
                _ => payload.status.clone(),
            };
            match &payload.activity {
                Some(a) if !a.trim().is_empty() => format!("{base} ({a})"),
                _ => base,
            }
        }
        BrokerMessage::Artifact { payload, .. } => {
            if let Some(first) = payload.modified_files.first() {
                format!("{}: {first}", payload.status)
            } else if !payload.exports.is_empty() {
                format!("{}: exports {}", payload.status, payload.exports.join(", "))
            } else {
                payload.status.clone()
            }
        }
        BrokerMessage::Blocked { payload, .. } => {
            format!("needs {} from {}", payload.needs, payload.from)
        }
        BrokerMessage::Verified { payload, .. } => match &payload.message {
            Some(m) if !m.trim().is_empty() => format!("by {}: {m}", payload.verified_by),
            _ => format!("by {}", payload.verified_by),
        },
        BrokerMessage::Feedback { payload, .. } => {
            let n = payload.errors.len();
            let suffix = if n == 1 { "error" } else { "errors" };
            format!("from {}: {n} {suffix}", payload.from)
        }
        BrokerMessage::Question { payload, .. } => payload.question.clone(),
        BrokerMessage::Intent { payload, .. } => {
            // Surface the first declared region (if any) alongside the
            // human summary so region-scoped intents are distinguishable at
            // a glance. The row formatter truncates the combined string.
            let first_region = payload
                .files
                .iter()
                .find_map(|f| f.regions().and_then(<[_]>::first));
            match first_region {
                Some(region) => format!("{}: {region}", payload.summary),
                None => payload.summary.clone(),
            }
        }
        BrokerMessage::VerifyNow { branch_id } => format!("verify {branch_id}"),
        BrokerMessage::AdvancedMain { payload, .. } => match &payload.summary {
            Some(s) if !s.trim().is_empty() => s.clone(),
            _ => format!(
                "{} merged into {} @ {}",
                payload.merged_branch, payload.base, payload.new_main_sha
            ),
        },
        BrokerMessage::Learning { payload, .. } => {
            format!("{}: {}", payload.category, payload.title)
        }
        BrokerMessage::Answer { payload, .. } => match &payload.re {
            Some(re) if !re.trim().is_empty() => {
                format!("from {} (re: {re}): {}", payload.from, payload.answer)
            }
            _ => format!("from {}: {}", payload.from, payload.answer),
        },
    }
}

// ---------------------------------------------------------------------------
// Heartbeat run collapse (render-time)
// ---------------------------------------------------------------------------

/// One rendered broker-log row: an entry plus how many consecutive identical
/// status heartbeats it stands for.
///
/// The ring buffer is untouched by the collapse — this is a render-time view
/// built by [`collapse_status_runs`]. `repeat` is `1` for every row that was
/// not collapsed, including every non-status message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollapsedRow<'a> {
    /// The entry the row renders. For a collapsed run this is the newest
    /// member, so the row carries the latest timestamp of the run.
    pub entry: &'a LogEntry,
    /// How many consecutive identical status heartbeats the row represents.
    pub repeat: usize,
}

/// The identity that decides whether two adjacent rows are the same heartbeat
/// repeated: agent, status label, activity phrase, and message.
///
/// The key spans every payload field [`derive_summary`] renders, so a run
/// collapses only when the line a reader sees is identical — keying on less
/// would fold rows that display differently and lose their text. Returns
/// `None` for any message that is not a status; those never merge.
fn heartbeat_key(msg: &BrokerMessage) -> Option<(&str, &str, Option<&str>, Option<&str>)> {
    match msg {
        BrokerMessage::Status { agent_id, payload } => Some((
            agent_id.as_str(),
            payload.status.as_str(),
            payload.activity.as_deref(),
            payload.message.as_deref(),
        )),
        _ => None,
    }
}

/// Collapses consecutive identical status heartbeats into single counted rows,
/// so frequent bare heartbeats do not bury real state transitions.
///
/// `entries` is consumed newest-first — the order [`BrokerLog::iter_visible`]
/// yields — so each run's representative is its newest member and the row
/// carries the latest timestamp of that run. A status differing in agent,
/// status label, activity, or message ends the run, as does any non-status
/// message in between. The underlying message log is unchanged.
#[must_use]
pub fn collapse_status_runs<'a>(
    entries: impl IntoIterator<Item = &'a LogEntry>,
) -> Vec<CollapsedRow<'a>> {
    let mut rows: Vec<CollapsedRow<'a>> = Vec::new();
    for entry in entries {
        let key = heartbeat_key(&entry.2);
        let extends_run = key.is_some()
            && rows
                .last()
                .is_some_and(|last| heartbeat_key(&last.entry.2) == key);
        if extends_run {
            if let Some(last) = rows.last_mut() {
                last.repeat += 1;
            }
            continue;
        }
        rows.push(CollapsedRow { entry, repeat: 1 });
    }
    rows
}

/// Formats a broker wall-clock timestamp as `HH:MM:SS` (UTC day clock).
#[must_use]
pub fn format_timestamp(ts: SystemTime) -> String {
    ts.duration_since(SystemTime::UNIX_EPOCH).map_or_else(
        |_| "00:00:00".to_string(),
        |d| {
            let secs = d.as_secs() % 86_400;
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let secs = secs % 60;
            format!("{hours:02}:{mins:02}:{secs:02}")
        },
    )
}

/// Truncates `s` to at most `max` characters, appending `…` when it would
/// otherwise overflow (task 4.3). The ellipsis counts toward `max`.
#[must_use]
pub fn truncate_ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

/// Composes a single compact row line: `HH:MM:SS · type · agent · summary`,
/// with the summary truncated so the whole line fits `width` columns without
/// wrapping (compact-row-format spec). When the fixed prefix alone exceeds
/// `width`, the whole line is truncated with an ellipsis.
#[must_use]
pub fn format_row_line(entry: &LogEntry, width: usize) -> String {
    compose_row_line(entry, 1, width)
}

/// Composes a collapsed row's line, appending a ` ×N` repeat marker when the
/// row stands for more than one heartbeat. A row with `repeat == 1` renders
/// exactly as [`format_row_line`] does.
///
/// The marker is reserved out of the width budget before the summary is
/// truncated, so a long summary never pushes the count off the line.
#[must_use]
pub fn format_collapsed_row_line(row: &CollapsedRow<'_>, width: usize) -> String {
    compose_row_line(row.entry, row.repeat, width)
}

/// Shared body of the two row formatters: prefix, then a width-fitted summary,
/// then the repeat marker when `repeat > 1`.
fn compose_row_line(entry: &LogEntry, repeat: usize, width: usize) -> String {
    let (_, ts, msg) = entry;
    let prefix = format!(
        "{} · {} · {} · ",
        format_timestamp(*ts),
        type_short(msg),
        msg.agent_id(),
    );
    let suffix = if repeat > 1 {
        format!(" ×{repeat}")
    } else {
        String::new()
    };
    let prefix_len = prefix.chars().count();
    let suffix_len = suffix.chars().count();
    if prefix_len + suffix_len >= width {
        return truncate_ellipsis(&format!("{prefix}{suffix}"), width);
    }
    let summary = derive_summary(msg);
    format!(
        "{prefix}{}{suffix}",
        truncate_ellipsis(&summary, width - prefix_len - suffix_len)
    )
}

/// Pretty-prints a message as indented JSON for the details overlay
/// (task 7.2). Falls back to the `Display` form if serialization somehow
/// fails (it should not for a well-formed message).
#[must_use]
pub fn pretty_json(msg: &BrokerMessage) -> String {
    serde_json::to_string_pretty(msg).unwrap_or_else(|_| msg.to_string())
}

// ---------------------------------------------------------------------------
// Key handling (section 6)
// ---------------------------------------------------------------------------

/// The result of routing a key through the Broker log panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKeyAction {
    /// The key mutated panel state; the caller should redraw.
    Handled,
    /// The panel did not consume the key; the caller may handle it (e.g. `q`).
    Ignored,
}

/// Routes a key press through the Broker log panel, mutating its state.
///
/// Bindings (design.md D5):
/// - `l` — toggle panel visibility
/// - `a` — reset filter to `All`
/// - `1`-`9` then `0` — toggle the corresponding chip (ten chips total)
/// - `Up`/`k`, `Down`/`j` — move the row highlight
/// - `Enter` — open the details overlay on the highlighted row
/// - `Esc` — close the overlay
///
/// While the overlay is open, only `Esc` (close) is consumed so the rest of
/// the dashboard's keys (notably `q` to quit) keep working. Returns
/// [`LogKeyAction::Ignored`] for any key the panel does not own.
pub fn handle_key(log: &mut BrokerLog, code: KeyCode) -> LogKeyAction {
    // Overlay mode swallows only Esc; everything else passes through.
    if log.overlay_open {
        if code == KeyCode::Esc {
            log.overlay_open = false;
            return LogKeyAction::Handled;
        }
        return LogKeyAction::Ignored;
    }

    match code {
        KeyCode::Char('l') => {
            log.visible = !log.visible;
            LogKeyAction::Handled
        }
        KeyCode::Char('a') => {
            log.filter.reset();
            log.clamp_selection();
            LogKeyAction::Handled
        }
        KeyCode::Char(c @ ('0'..='9')) => {
            // Digits `1`-`9` select chips 0-8; `0` selects the tenth chip.
            let idx = if c == '0' {
                9
            } else {
                (c as u8 - b'1') as usize
            };
            if let Some((bit, _)) = CHIPS.get(idx) {
                log.filter.toggle(*bit);
                log.clamp_selection();
            }
            LogKeyAction::Handled
        }
        KeyCode::Up | KeyCode::Char('k') => {
            log.select_up();
            LogKeyAction::Handled
        }
        KeyCode::Down | KeyCode::Char('j') => {
            log.select_down();
            LogKeyAction::Handled
        }
        KeyCode::Enter => {
            if log.selected_entry().is_some() {
                log.overlay_open = true;
                LogKeyAction::Handled
            } else {
                LogKeyAction::Ignored
            }
        }
        _ => LogKeyAction::Ignored,
    }
}

// ---------------------------------------------------------------------------
// Rendering (section 4 + 7)
// ---------------------------------------------------------------------------

/// Builds the header chip line: `[All] status artifact …`, highlighting `All`
/// in `All` mode and any active chips otherwise.
fn chip_line(filter: FilterMask) -> Line<'static> {
    let active = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let inactive = Style::default().fg(Color::DarkGray);

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(CHIPS.len() * 2 + 2);
    spans.push(Span::styled(
        " All ",
        if filter.is_all() { active } else { inactive },
    ));
    for (i, (bit, label)) in CHIPS.iter().enumerate() {
        spans.push(Span::raw(" "));
        let style = if filter.is_chip_active(*bit) {
            active
        } else {
            inactive
        };
        // Prefix each chip with its hotkey digit for discoverability: chips
        // 0-8 use `1`-`9`, and the tenth chip uses `0`.
        let digit = if i == 9 { 0 } else { i + 1 };
        spans.push(Span::styled(format!("{digit}:{label}"), style));
    }
    Line::from(spans)
}

/// Renders the Broker log panel into `area` (section 4). When the details
/// overlay is open, it is drawn on top of the panel (section 7).
pub fn render(frame: &mut Frame, area: Rect, log: &BrokerLog) {
    // Build the collapsed row view once — the title, the empty check, and the
    // list items all read from it.
    let visible_rows = log.visible_rows();
    // The title doubles as the in-app key reference (task 6.6): the chip row
    // below documents the 1-9/0 digit filters; the title documents the rest.
    let title = format!(
        "Broker log ({} shown / {} held) — l hide · a all · 1-9·0 filter · ↵ details · Esc close",
        visible_rows.len(),
        log.len()
    );
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Header chip row, then the scrolling list beneath it.
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).split(inner);
    frame.render_widget(Paragraph::new(chip_line(log.filter)), rows[0]);

    let list_area = rows[1];
    let width = list_area.width as usize;
    if visible_rows.is_empty() {
        let empty = Paragraph::new("(no messages match the active filter)")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(empty, list_area);
    } else {
        let highlight = Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD);
        let items: Vec<ListItem> = visible_rows
            .iter()
            .map(|row| ListItem::new(format_collapsed_row_line(row, width.max(1))))
            .collect();
        // Render as a stateful list so ratatui scrolls the viewport to keep the
        // selected row visible — `Up`/`Down`/`k`/`j` then reach every retained
        // message, not just the first screenful (a plain `List`/`render_widget`
        // draws from the top with no offset and cannot scroll).
        let list = List::new(items).highlight_style(highlight);
        let mut state = ListState::default();
        state.select(Some(log.selected));
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    if log.overlay_open {
        render_overlay(frame, area, log);
    }
}

/// Renders the details overlay (section 7): a centered modal showing the
/// highlighted message's pretty-printed JSON, dismissed with `Esc`.
fn render_overlay(frame: &mut Frame, area: Rect, log: &BrokerLog) {
    let Some(entry) = log.selected_entry() else {
        return;
    };
    let overlay_area = centered_rect(area, 80, 80);
    frame.render_widget(Clear, overlay_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .title("Message details — Esc to close")
        .title_alignment(Alignment::Center)
        .style(Style::default().bg(Color::Black));
    let body = pretty_json(&entry.2);
    let paragraph = Paragraph::new(body).block(block).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, overlay_area);
}

/// Returns a `Rect` centered within `area` sized to the given width/height
/// percentages.
fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::messages::{
        AnswerPayload, ArtifactPayload, BlockedPayload, FeedbackPayload, FileIntent, IntentPayload,
        QuestionPayload, StatusPayload, VerifiedPayload,
    };

    fn ts(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)
    }

    fn status(agent: &str, status: &str, message: Option<&str>) -> BrokerMessage {
        BrokerMessage::Status {
            agent_id: agent.to_string(),
            payload: StatusPayload {
                status: status.to_string(),
                modified_files: vec![],
                message: message.map(str::to_string),
                ..Default::default()
            },
        }
    }

    fn status_with_activity(
        agent: &str,
        status_label: &str,
        message: Option<&str>,
        activity: Option<&str>,
    ) -> BrokerMessage {
        BrokerMessage::Status {
            agent_id: agent.to_string(),
            payload: StatusPayload {
                status: status_label.to_string(),
                modified_files: vec![],
                message: message.map(str::to_string),
                activity: activity.map(str::to_string),
                ..Default::default()
            },
        }
    }

    fn entry(seq: u64, msg: BrokerMessage) -> LogEntry {
        (seq, ts(seq), msg)
    }

    // -- Ring buffer (task 2.5) ------------------------------------------

    #[test]
    fn push_beyond_cap_drops_oldest() {
        let mut log = BrokerLog::new(3, true);
        for i in 1..=5 {
            log.push(entry(i, status("feat-a", "working", None)));
        }
        assert_eq!(log.len(), 3, "buffer must cap at max");
        // Newest (seq 5) at front; oldest retained is seq 3.
        let seqs: Vec<u64> = log.iter_visible().map(|e| e.0).collect();
        assert_eq!(seqs, vec![5, 4, 3]);
    }

    #[test]
    fn new_clamps_zero_capacity_to_one() {
        let mut log = BrokerLog::new(0, true);
        log.push(entry(1, status("a", "working", None)));
        log.push(entry(2, status("a", "working", None)));
        assert_eq!(log.len(), 1);
        assert_eq!(log.iter_visible().next().unwrap().0, 2);
    }

    #[test]
    fn push_front_keeps_newest_at_top() {
        let mut log = BrokerLog::new(10, true);
        log.push(entry(1, status("a", "working", None)));
        log.push(entry(2, status("b", "done", None)));
        let seqs: Vec<u64> = log.iter_visible().map(|e| e.0).collect();
        assert_eq!(seqs, vec![2, 1], "most recent message is first");
    }

    /// supervisor-introspection task 5.2 / dashboard-broker-log: a phased
    /// supervisor status (`phase = "audit"`, with a detail body) is still a
    /// plain `agent.status`, so it classifies under the `status` filter bit
    /// with no separate phase filter — it shows up in the broker log filtered
    /// by type = status.
    #[test]
    fn phased_supervisor_status_classifies_as_status() {
        let msg = BrokerMessage::Status {
            agent_id: "supervisor".to_string(),
            payload: StatusPayload {
                status: "working".to_string(),
                modified_files: vec![],
                message: Some("auditing feat/auth".to_string()),
                phase: Some("audit".to_string()),
                detail: Some(serde_json::json!({"branch": "feat/auth", "audit_step": "tests"})),
                ..Default::default()
            },
        };
        assert_eq!(message_bit(&msg), BIT_STATUS);
        assert!(
            FilterMask::all().matches(&msg),
            "a phased status passes the default (all) filter"
        );
        // Narrowing the filter to just `status` (toggling from All) still
        // surfaces the phased status — no separate phase filter needed.
        let mut only_status = FilterMask::all();
        only_status.toggle(BIT_STATUS);
        assert!(
            only_status.matches(&msg),
            "a phased status passes the status-only filter — no separate phase filter needed"
        );
    }

    #[test]
    fn ingest_only_advances_past_cursor() {
        let mut log = BrokerLog::new(10, true);
        log.ingest(vec![
            entry(1, status("a", "working", None)),
            entry(2, status("b", "done", None)),
        ]);
        assert_eq!(log.last_seq(), 2);
        assert_eq!(log.len(), 2);
        // Re-ingesting the same batch plus one new entry is idempotent for
        // the already-seen entries and only appends the new one.
        log.ingest(vec![
            entry(1, status("a", "working", None)),
            entry(2, status("b", "done", None)),
            entry(3, status("c", "blocked", None)),
        ]);
        assert_eq!(log.len(), 3, "duplicate seqs must not be re-added");
        assert_eq!(log.last_seq(), 3);
        let seqs: Vec<u64> = log.iter_visible().map(|e| e.0).collect();
        assert_eq!(seqs, vec![3, 2, 1]);
    }

    // -- Filter bitmask (task 2.5) ---------------------------------------

    #[test]
    fn filter_all_is_default_and_shows_everything() {
        let f = FilterMask::default();
        assert!(f.is_all());
        assert!(f.matches(&status("a", "working", None)));
    }

    #[test]
    fn toggling_one_chip_narrows_to_that_type() {
        let mut f = FilterMask::all();
        f.toggle(BIT_STATUS);
        assert!(!f.is_all());
        assert!(f.matches(&status("a", "working", None)));
        let intent = BrokerMessage::Intent {
            agent_id: "a".to_string(),
            payload: IntentPayload {
                files: vec![FileIntent::from("x")],
                summary: "s".to_string(),
                valid_for_seconds: 60,
            },
        };
        assert!(!f.matches(&intent), "non-status must be hidden");
    }

    #[test]
    fn two_chips_combine_inclusively() {
        let mut f = FilterMask::all();
        f.toggle(BIT_STATUS);
        f.toggle(BIT_INTENT);
        let intent = BrokerMessage::Intent {
            agent_id: "a".to_string(),
            payload: IntentPayload {
                files: vec![FileIntent::from("x")],
                summary: "s".to_string(),
                valid_for_seconds: 60,
            },
        };
        assert!(f.matches(&status("a", "working", None)));
        assert!(f.matches(&intent));
        let blocked = BrokerMessage::Blocked {
            agent_id: "a".to_string(),
            payload: BlockedPayload {
                needs: "x".to_string(),
                from: "b".to_string(),
            },
        };
        assert!(!f.matches(&blocked), "unselected type stays hidden");
    }

    #[test]
    fn reset_returns_to_all() {
        let mut f = FilterMask::all();
        f.toggle(BIT_STATUS);
        f.reset();
        assert!(f.is_all());
    }

    #[test]
    fn toggling_chip_off_empties_back_to_all() {
        let mut f = FilterMask::all();
        f.toggle(BIT_STATUS); // -> only status
        f.toggle(BIT_STATUS); // -> empty -> All
        assert!(f.is_all());
    }

    #[test]
    fn is_chip_active_false_in_all_mode() {
        let f = FilterMask::all();
        assert!(!f.is_chip_active(BIT_STATUS));
    }

    #[test]
    fn iter_visible_respects_filter_but_buffer_retains_all() {
        let mut log = BrokerLog::new(10, true);
        log.push(entry(1, status("a", "working", None)));
        log.push(entry(
            2,
            BrokerMessage::Blocked {
                agent_id: "b".to_string(),
                payload: BlockedPayload {
                    needs: "x".to_string(),
                    from: "c".to_string(),
                },
            },
        ));
        log.filter.toggle(BIT_STATUS);
        assert_eq!(log.visible_count(), 1, "only status shows");
        assert_eq!(log.len(), 2, "buffer retains all regardless of filter");
    }

    // -- Summary extractors, one per variant (task 4.5) ------------------

    #[test]
    fn summary_status_with_message() {
        let s = derive_summary(&status("a", "working", Some("rebasing onto main")));
        assert_eq!(s, "working: rebasing onto main");
    }

    #[test]
    fn summary_status_without_message() {
        assert_eq!(derive_summary(&status("a", "idle", None)), "idle");
    }

    #[test]
    fn summary_status_with_activity_shows_the_phrase() {
        // GIVEN an agent.status with status `working` and an activity phrase
        // WHEN the panel derives its summary
        // THEN the rendered line includes the phrase.
        let s = derive_summary(&status_with_activity(
            "a",
            "working",
            None,
            Some("running cargo test"),
        ));
        assert!(
            s.contains("running cargo test"),
            "activity phrase must reach the rendered line; got {s}"
        );

        // It also survives alongside a message rather than replacing it.
        let with_message = derive_summary(&status_with_activity(
            "a",
            "working",
            Some("rebasing onto main"),
            Some("running cargo test"),
        ));
        assert!(
            with_message.contains("rebasing onto main")
                && with_message.contains("running cargo test"),
            "activity must not displace the message; got {with_message}"
        );
    }

    #[test]
    fn summary_status_without_activity_matches_pre_existing_format() {
        // GIVEN statuses carrying no activity (the pre-activity shape)
        // WHEN the panel derives their summaries
        // THEN they are byte-identical to the bare-status format that existed
        // before the field — an explicitly-None activity behaves as an absent
        // one, and a blank phrase is treated as absent rather than rendering
        // empty parentheses.
        assert_eq!(derive_summary(&status("a", "idle", None)), "idle");
        assert_eq!(
            derive_summary(&status("a", "working", Some("rebasing onto main"))),
            "working: rebasing onto main"
        );
        assert_eq!(
            derive_summary(&status_with_activity("a", "idle", None, None)),
            "idle"
        );
        assert_eq!(
            derive_summary(&status_with_activity("a", "idle", None, Some("   "))),
            "idle",
            "a blank activity must not render empty parentheses"
        );
    }

    // -- Heartbeat run collapse ------------------------------------------

    /// Feeds entries to the collapse in the newest-first order the panel
    /// iterates, given a chronological (oldest-first) slice.
    fn rows_of(entries: &[LogEntry]) -> Vec<CollapsedRow<'_>> {
        collapse_status_runs(entries.iter().rev())
    }

    #[test]
    fn repeated_identical_heartbeats_collapse_to_one_counted_row() {
        // GIVEN three consecutive identical `working` heartbeats from one agent
        // WHEN the panel builds its rows
        // THEN they appear as a single row showing a repeat count of 3 and the
        // latest of the three timestamps.
        let entries: Vec<LogEntry> = (1..=3)
            .map(|i| entry(i, status("feat-a", "working", None)))
            .collect();
        let rows = rows_of(&entries);

        assert_eq!(rows.len(), 1, "identical heartbeats must collapse to a row");
        assert_eq!(rows[0].repeat, 3, "the row must count the whole run");
        assert_eq!(
            rows[0].entry.1,
            ts(3),
            "the row must carry the latest timestamp of the run"
        );
    }

    #[test]
    fn changed_status_breaks_the_heartbeat_run() {
        // GIVEN two `working` heartbeats followed by a `verified` status
        // WHEN the panel builds its rows
        // THEN the two `working` heartbeats collapse and `verified` is its own
        // row. Rows are newest-first, so `verified` leads.
        let entries = vec![
            entry(1, status("feat-a", "working", None)),
            entry(2, status("feat-a", "working", None)),
            entry(3, status("feat-a", "verified", None)),
        ];
        let rows = rows_of(&entries);

        assert_eq!(rows.len(), 2, "the transition must break the run");
        assert_eq!(rows[0].repeat, 1, "verified renders as its own row");
        assert_eq!(rows[0].entry.1, ts(3));
        assert_eq!(rows[1].repeat, 2, "the working heartbeats collapse");
        assert_eq!(rows[1].entry.1, ts(2));
    }

    #[test]
    fn heartbeats_differing_only_by_message_do_not_collapse() {
        // GIVEN two `working` heartbeats from one agent whose messages differ
        // WHEN the panel builds its rows
        // THEN they stay two rows — the key spans every rendered field, so
        // rows that display differently are never folded together.
        let entries = vec![
            entry(1, status("feat-a", "working", Some("reading spec"))),
            entry(
                2,
                status("feat-a", "working", Some("editing broker_log.rs")),
            ),
        ];
        let rows = rows_of(&entries);

        assert_eq!(rows.len(), 2, "differing messages must not collapse");
        assert!(
            rows.iter().all(|row| row.repeat == 1),
            "neither row stands for more than one heartbeat"
        );
    }

    #[test]
    fn heartbeats_differing_only_by_activity_do_not_collapse() {
        let entries = vec![
            entry(
                1,
                status_with_activity("feat-a", "working", None, Some("running cargo test")),
            ),
            entry(
                2,
                status_with_activity("feat-a", "working", None, Some("editing drive.rs")),
            ),
        ];
        assert_eq!(
            rows_of(&entries).len(),
            2,
            "a changed activity phrase must break the run"
        );
    }

    #[test]
    fn identical_heartbeats_from_different_agents_do_not_collapse() {
        let entries = vec![
            entry(1, status("feat-a", "working", None)),
            entry(2, status("feat-b", "working", None)),
        ];
        assert_eq!(
            rows_of(&entries).len(),
            2,
            "heartbeats from different agents must stay separate rows"
        );
    }

    #[test]
    fn non_status_message_between_heartbeats_breaks_the_run() {
        // A non-status message ends the run even when the heartbeats around it
        // are identical — two genuinely distinct working phases stay distinct.
        let entries = vec![
            entry(1, status("feat-a", "working", None)),
            entry(
                2,
                BrokerMessage::Question {
                    agent_id: "feat-a".to_string(),
                    payload: QuestionPayload {
                        question: "rs256 or hs256?".to_string(),
                    },
                },
            ),
            entry(3, status("feat-a", "working", None)),
        ];
        let rows = rows_of(&entries);

        assert_eq!(rows.len(), 3, "the question must split the two runs");
        assert!(rows.iter().all(|row| row.repeat == 1));
    }

    #[test]
    fn collapsed_row_line_shows_the_repeat_marker() {
        let entries: Vec<LogEntry> = (1..=3)
            .map(|i| entry(i, status("feat-a", "working", None)))
            .collect();
        let rows = rows_of(&entries);
        let line = format_collapsed_row_line(&rows[0], 120);

        assert_eq!(line, "00:00:03 · status · feat-a · working ×3");
    }

    #[test]
    fn uncollapsed_row_line_matches_format_row_line() {
        // A row standing for a single message renders byte-identically to the
        // pre-collapse formatter — no stray marker on ordinary rows.
        let entries = vec![entry(1, status("feat-a", "working", Some("rebasing")))];
        let rows = rows_of(&entries);

        assert_eq!(
            format_collapsed_row_line(&rows[0], 120),
            format_row_line(&entries[0], 120)
        );
    }

    #[test]
    fn repeat_marker_survives_a_narrow_width() {
        // The marker is reserved out of the budget, so a long summary is
        // truncated around it rather than pushing it off the line.
        let entries: Vec<LogEntry> = (1..=4)
            .map(|i| {
                entry(
                    i,
                    status(
                        "feat-a",
                        "working",
                        Some("a very long status message indeed"),
                    ),
                )
            })
            .collect();
        let rows = rows_of(&entries);
        let line = format_collapsed_row_line(&rows[0], 50);

        assert!(line.ends_with("×4"), "the count must survive truncation");
        assert!(line.chars().count() <= 50, "the line must fit the width");
    }

    #[test]
    fn panel_collapses_heartbeats_but_the_buffer_retains_them() {
        // The collapse is a render-time view: the panel shows one row while
        // the ring buffer still holds every heartbeat.
        let mut log = BrokerLog::new(50, true);
        for i in 1..=3 {
            log.push(entry(i, status("feat-a", "working", None)));
        }

        assert_eq!(log.len(), 3, "the buffer retains every message");
        assert_eq!(log.visible_count(), 1, "the panel shows one collapsed row");
        assert_eq!(log.visible_rows()[0].repeat, 3);
        assert_eq!(
            log.selected_entry().map(|e| e.0),
            Some(3),
            "the highlighted row resolves to the newest heartbeat of the run"
        );
    }

    #[test]
    fn row_line_for_status_without_activity_is_unchanged() {
        // Back-compat (6.3): the full composed row for a pre-activity status
        // is byte-identical to what the panel rendered before the field.
        let e = entry(1, status("feat-auth", "working", Some("rebasing")));
        assert_eq!(
            format_row_line(&e, 120),
            "00:00:01 · status · feat-auth · working: rebasing"
        );
    }

    #[test]
    fn summary_artifact_uses_first_modified_file() {
        let msg = BrokerMessage::Artifact {
            agent_id: "a".to_string(),
            payload: ArtifactPayload {
                status: "done".to_string(),
                exports: vec![],
                modified_files: vec!["src/error.rs".to_string(), "src/lib.rs".to_string()],
            },
        };
        assert_eq!(derive_summary(&msg), "done: src/error.rs");
    }

    #[test]
    fn summary_artifact_falls_back_to_exports_then_status() {
        let with_exports = BrokerMessage::Artifact {
            agent_id: "a".to_string(),
            payload: ArtifactPayload {
                status: "done".to_string(),
                exports: vec!["PawError".to_string()],
                modified_files: vec![],
            },
        };
        assert_eq!(derive_summary(&with_exports), "done: exports PawError");
        let bare = BrokerMessage::Artifact {
            agent_id: "a".to_string(),
            payload: ArtifactPayload {
                status: "committed".to_string(),
                exports: vec![],
                modified_files: vec![],
            },
        };
        assert_eq!(derive_summary(&bare), "committed");
    }

    #[test]
    fn summary_blocked() {
        let msg = BrokerMessage::Blocked {
            agent_id: "a".to_string(),
            payload: BlockedPayload {
                needs: "error types".to_string(),
                from: "feat-errors".to_string(),
            },
        };
        assert_eq!(derive_summary(&msg), "needs error types from feat-errors");
    }

    #[test]
    fn summary_verified_with_and_without_message() {
        let with = BrokerMessage::Verified {
            agent_id: "a".to_string(),
            payload: VerifiedPayload {
                verified_by: "supervisor".to_string(),
                message: Some("all tests pass".to_string()),
            },
        };
        assert_eq!(derive_summary(&with), "by supervisor: all tests pass");
        let without = BrokerMessage::Verified {
            agent_id: "a".to_string(),
            payload: VerifiedPayload {
                verified_by: "supervisor".to_string(),
                message: None,
            },
        };
        assert_eq!(derive_summary(&without), "by supervisor");
    }

    #[test]
    fn summary_feedback_pluralizes() {
        let one = BrokerMessage::Feedback {
            agent_id: "a".to_string(),
            payload: FeedbackPayload {
                from: "supervisor".to_string(),
                errors: vec!["e1".to_string()],
            },
        };
        assert_eq!(derive_summary(&one), "from supervisor: 1 error");
        let many = BrokerMessage::Feedback {
            agent_id: "a".to_string(),
            payload: FeedbackPayload {
                from: "supervisor".to_string(),
                errors: vec!["e1".to_string(), "e2".to_string()],
            },
        };
        assert_eq!(derive_summary(&many), "from supervisor: 2 errors");
    }

    #[test]
    fn summary_question() {
        let msg = BrokerMessage::Question {
            agent_id: "a".to_string(),
            payload: QuestionPayload {
                question: "rs256 or hs256?".to_string(),
            },
        };
        assert_eq!(derive_summary(&msg), "rs256 or hs256?");
    }

    #[test]
    fn summary_intent() {
        let msg = BrokerMessage::Intent {
            agent_id: "a".to_string(),
            payload: IntentPayload {
                files: vec![FileIntent::from("src/a.rs")],
                summary: "wire AuthClient".to_string(),
                valid_for_seconds: 900,
            },
        };
        assert_eq!(derive_summary(&msg), "wire AuthClient");
    }

    #[test]
    fn summary_intent_with_regions_includes_first_region() {
        use crate::broker::messages::Region;
        let msg = BrokerMessage::Intent {
            agent_id: "a".to_string(),
            payload: IntentPayload {
                files: vec![FileIntent::Detailed {
                    path: "src/auth.rs".to_string(),
                    regions: vec![
                        Region::Function {
                            name: "validate_token".to_string(),
                        },
                        Region::Function {
                            name: "refresh_session".to_string(),
                        },
                    ],
                }],
                summary: "harden auth".to_string(),
                valid_for_seconds: 900,
            },
        };
        assert_eq!(derive_summary(&msg), "harden auth: function validate_token");
    }

    #[test]
    fn summary_verify_now() {
        let msg = BrokerMessage::VerifyNow {
            branch_id: "feat-bar".to_string(),
        };
        assert_eq!(derive_summary(&msg), "verify feat-bar");
    }

    // -- Truncation (task 4.3) -------------------------------------------

    #[test]
    fn truncate_shorter_than_max_is_unchanged() {
        assert_eq!(truncate_ellipsis("hello", 10), "hello");
        assert_eq!(truncate_ellipsis("hello", 5), "hello");
    }

    #[test]
    fn truncate_adds_ellipsis_and_fits_width() {
        let out = truncate_ellipsis("hello world", 5);
        assert_eq!(out.chars().count(), 5);
        assert!(out.ends_with('…'));
        assert_eq!(out, "hell…");
    }

    #[test]
    fn truncate_zero_width_is_empty() {
        assert_eq!(truncate_ellipsis("hello", 0), "");
    }

    // -- Row formatting --------------------------------------------------

    #[test]
    fn row_contains_four_documented_fields() {
        let e = entry(
            1,
            status("feat-auth", "working", Some("rebasing onto main")),
        );
        let line = format_row_line(&e, 120);
        assert!(line.contains("00:00:01"), "timestamp HH:MM:SS: {line}");
        assert!(line.contains("status"), "type short form: {line}");
        assert!(line.contains("feat-auth"), "agent id: {line}");
        assert!(line.contains("rebasing onto main"), "summary: {line}");
    }

    #[test]
    fn row_truncates_long_summary_to_width() {
        let long = "x".repeat(300);
        let e = entry(1, status("feat-auth", "working", Some(&long)));
        let line = format_row_line(&e, 60);
        assert_eq!(line.chars().count(), 60, "row must fit the panel width");
        assert!(
            line.ends_with('…'),
            "overflowing summary ends with ellipsis"
        );
    }

    #[test]
    fn row_handles_prefix_wider_than_width() {
        let e = entry(1, status("feat-auth", "working", Some("anything")));
        let line = format_row_line(&e, 8);
        assert_eq!(line.chars().count(), 8);
        assert!(line.ends_with('…'));
    }

    // -- Timestamp -------------------------------------------------------

    #[test]
    fn timestamp_formats_hh_mm_ss() {
        // 14:35:09 UTC = 14*3600 + 35*60 + 9 = 52509 seconds into the day.
        assert_eq!(format_timestamp(ts(52_509)), "14:35:09");
    }

    // -- Selection navigation --------------------------------------------

    #[test]
    fn selection_navigates_within_visible_bounds() {
        let mut log = BrokerLog::new(10, true);
        // Distinct messages so each heartbeat is its own row — identical ones
        // would collapse into a single row and there would be nothing to
        // navigate.
        for i in 1..=3 {
            log.push(entry(i, status("a", "working", Some(&format!("step {i}")))));
        }
        assert_eq!(log.selected(), 0);
        log.select_up(); // already at top, stays
        assert_eq!(log.selected(), 0);
        log.select_down();
        log.select_down();
        assert_eq!(log.selected(), 2);
        log.select_down(); // at bottom, stays
        assert_eq!(log.selected(), 2);
    }

    #[test]
    fn selection_clamps_when_filter_shrinks_visible_set() {
        let mut log = BrokerLog::new(10, true);
        log.push(entry(1, status("a", "working", None)));
        log.push(entry(
            2,
            BrokerMessage::Blocked {
                agent_id: "b".to_string(),
                payload: BlockedPayload {
                    needs: "x".to_string(),
                    from: "c".to_string(),
                },
            },
        ));
        log.select_down(); // selected = 1
        assert_eq!(log.selected(), 1);
        // Filter to only status (1 visible) — selection must clamp to 0.
        handle_key(&mut log, KeyCode::Char('1'));
        assert_eq!(log.visible_count(), 1);
        assert_eq!(log.selected(), 0);
    }

    // -- Key handling (section 6) ----------------------------------------

    #[test]
    fn key_l_toggles_visibility() {
        let mut log = BrokerLog::new(10, true);
        assert!(log.visible);
        assert_eq!(
            handle_key(&mut log, KeyCode::Char('l')),
            LogKeyAction::Handled
        );
        assert!(!log.visible);
        handle_key(&mut log, KeyCode::Char('l'));
        assert!(log.visible);
    }

    #[test]
    fn key_a_resets_filter() {
        let mut log = BrokerLog::new(10, true);
        handle_key(&mut log, KeyCode::Char('1')); // narrow to status
        assert!(!log.filter().is_all());
        handle_key(&mut log, KeyCode::Char('a'));
        assert!(log.filter().is_all());
    }

    #[test]
    fn key_digits_map_to_chips_in_order() {
        for (i, (bit, _)) in CHIPS.iter().enumerate() {
            let mut log = BrokerLog::new(10, true);
            // Chips 0-8 map to `1`-`9`; the tenth chip (index 9) maps to `0`.
            let key = if i == 9 {
                '0'
            } else {
                char::from(b'1' + u8::try_from(i).unwrap())
            };
            handle_key(&mut log, KeyCode::Char(key));
            assert!(
                log.filter().is_chip_active(*bit),
                "digit {key} must toggle chip index {i}"
            );
        }
    }

    /// agent-answer-variant (dashboard-broker-log MODIFIED chips requirement):
    /// message types beyond the ten digit-keyed chips — currently
    /// `agent.answer` — carry their own filter bit, render under the default
    /// `All` filter, and have no dedicated chip.
    #[test]
    fn answer_visible_under_all_without_dedicated_chip() {
        let mut log = BrokerLog::new(10, true);
        log.push(entry(
            1,
            BrokerMessage::Answer {
                agent_id: "feat-a".to_string(),
                payload: AnswerPayload {
                    from: "supervisor".to_string(),
                    answer: "use rs256".to_string(),
                    re: None,
                },
            },
        ));
        assert!(log.filter().is_all(), "fresh panel defaults to All");
        assert_eq!(log.visible_count(), 1, "answer row renders under All");
        let visible: Vec<_> = log.iter_visible().collect();
        assert_eq!(type_short(&visible[0].2), "answer");
        assert!(
            CHIPS
                .iter()
                .all(|(bit, label)| *bit != BIT_ANSWER && *label != "answer"),
            "the chip row must not carry a dedicated answer chip"
        );
    }

    #[test]
    fn key_enter_opens_overlay_only_when_a_row_exists() {
        let mut empty = BrokerLog::new(10, true);
        assert_eq!(
            handle_key(&mut empty, KeyCode::Enter),
            LogKeyAction::Ignored
        );
        assert!(!empty.overlay_open());

        let mut log = BrokerLog::new(10, true);
        log.push(entry(1, status("a", "working", None)));
        assert_eq!(handle_key(&mut log, KeyCode::Enter), LogKeyAction::Handled);
        assert!(log.overlay_open());
    }

    #[test]
    fn key_esc_closes_overlay_and_passes_other_keys_through() {
        let mut log = BrokerLog::new(10, true);
        log.push(entry(1, status("a", "working", None)));
        handle_key(&mut log, KeyCode::Enter);
        assert!(log.overlay_open());
        // While the overlay is open, non-Esc keys are not consumed so the
        // dashboard's `q`-to-quit keeps working.
        assert_eq!(
            handle_key(&mut log, KeyCode::Char('q')),
            LogKeyAction::Ignored
        );
        assert!(log.overlay_open(), "q must not close the overlay");
        assert_eq!(handle_key(&mut log, KeyCode::Esc), LogKeyAction::Handled);
        assert!(!log.overlay_open());
    }

    #[test]
    fn unhandled_key_is_ignored() {
        let mut log = BrokerLog::new(10, true);
        assert_eq!(
            handle_key(&mut log, KeyCode::Char('z')),
            LogKeyAction::Ignored
        );
        assert_eq!(
            handle_key(&mut log, KeyCode::Char('q')),
            LogKeyAction::Ignored
        );
    }

    // -- Pretty JSON (task 7.2) ------------------------------------------

    #[test]
    fn pretty_json_is_multiline_and_matches_message() {
        let msg = status("feat-auth", "working", Some("rebasing"));
        let json = pretty_json(&msg);
        assert!(
            json.contains('\n'),
            "pretty JSON must be indented/multiline"
        );
        assert!(json.contains("agent.status"));
        assert!(json.contains("feat-auth"));
        // Round-trips back to the same message.
        let back: BrokerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back, msg);
    }
}
