# broker-dashboard Specification

## Purpose
A ratatui TUI that observes broker state, rendering an agent-status table (with pinned supervisor row, status symbols, and relative-age formatting) plus a title and status line, driven by an event-based draw loop that shares `BrokerState` without holding locks during rendering. It manages terminal lifecycle across clean exit, error, and panic, and terminates cleanly when its session is torn down or it is orphaned rather than busy-looping. It also adds a scrolling, filterable Broker log panel that displays recent broker messages newest-first from a bounded ring buffer, with per-type filter chips, a toggle hotkey, compact rows, a JSON details overlay, and buffer resilience across watcher restarts.

## Requirements
### Requirement: Dashboard entry point

The system SHALL provide a public function with the signature:

```rust
pub fn run_dashboard(
    state: &Arc<BrokerState>,
    broker_handle: BrokerHandle,
    shutdown: &AtomicBool,
) -> Result<(), PawError>
```

This function SHALL:

1. Enter crossterm alternate screen and raw mode
2. Install a panic hook that restores the terminal before printing the panic
3. Run an event-driven draw loop reading from `&Arc<BrokerState>` so the broker state is shared with HTTP handlers, watcher tasks, and the dashboard without copying
4. Exit when the user presses `q` OR when `shutdown` is set to `true` by an external signal handler
5. Restore the terminal (raw mode off, leave alternate screen) on exit or error
6. Return `Ok(())` on clean exit

The function SHALL take ownership of `BrokerHandle` so that the broker shuts down automatically when the dashboard exits and the handle is dropped. The `shutdown` flag SHALL allow `cmd_supervisor` and `cmd_start` to request a clean dashboard exit when their own signal handlers fire.

#### Scenario: Dashboard starts and stops cleanly

- **GIVEN** a valid `&Arc<BrokerState>`, `BrokerHandle`, and a `shutdown: &AtomicBool` initialised to `false`
- **WHEN** `run_dashboard` is called and the user presses `q`
- **THEN** the function returns `Ok(())`
- **AND** the terminal is restored to its pre-dashboard state

#### Scenario: External shutdown flag exits the dashboard

- **GIVEN** a running dashboard
- **WHEN** another thread sets `shutdown.store(true, Ordering::Release)`
- **THEN** the dashboard exits cleanly within one input-poll interval
- **AND** the function returns `Ok(())`

#### Scenario: BrokerHandle is dropped on dashboard exit

- **GIVEN** a valid state, handle, and shutdown flag pointing to a running broker
- **WHEN** `run_dashboard` returns
- **THEN** the `BrokerHandle` is dropped
- **AND** the broker stops accepting HTTP requests

### Requirement: Terminal lifecycle management

The system SHALL manage terminal state transitions for the ratatui TUI. On entry, the system SHALL enable raw mode and enter the alternate screen. On exit — whether from a clean `q` press, an error, or a panic — the system SHALL disable raw mode and leave the alternate screen so the user's terminal is usable.

#### Scenario: Terminal restored after clean exit

- **WHEN** `run_dashboard` returns `Ok(())`
- **THEN** raw mode is disabled
- **AND** the alternate screen is exited

#### Scenario: Terminal restored after error

- **WHEN** `run_dashboard` encounters an error and returns `Err(...)`
- **THEN** raw mode is disabled
- **AND** the alternate screen is exited

#### Scenario: Terminal restored after panic

- **WHEN** a panic occurs inside the draw loop
- **THEN** the installed panic hook disables raw mode and exits the alternate screen before the panic message is printed
- **AND** the panic message is readable in the normal terminal

### Requirement: Periodic state polling

The system SHALL poll `BrokerState` via `agent_status_snapshot` and SHALL render a new frame whenever a draw is needed. The dashboard SHALL wait for input by polling its controlling terminal with a bounded idle timeout rather than busy-polling; the idle timeout SHALL be at least 800 milliseconds so that an idle dashboard consumes negligible CPU. Key input and redraw-triggering events SHALL wake the wait immediately, so responsiveness does not depend on the idle timeout. The wait SHALL be bounded even when the terminal is gone: a hung-up controlling terminal SHALL be detected and SHALL cause the loop to exit rather than trap the wait indefinitely.

The system SHALL NOT hold the `BrokerState` read lock across a draw call or a poll/wait. The lock SHALL be acquired, data cloned, and the lock released before any rendering or waiting occurs.

#### Scenario: Dashboard refreshes within one second of a state change

- **GIVEN** a running dashboard
- **WHEN** an agent's status changes in `BrokerState`
- **THEN** the dashboard displays the updated status within 1 second

#### Scenario: Lock is not held during rendering

- **GIVEN** a running dashboard
- **WHEN** the dashboard renders a frame
- **THEN** no `BrokerState` read lock is held during the ratatui draw call

#### Scenario: Input is responsive

- **GIVEN** a running dashboard awaiting input
- **WHEN** the user presses `q`
- **THEN** the dashboard reacts immediately — the terminal poll returns on the keypress rather than after the idle timeout

#### Scenario: Idle dashboard does not busy-loop

- **GIVEN** a running dashboard with no pending input and no state changes
- **THEN** it waits on the bounded terminal poll for the idle timeout (at least 800 ms) between wakeups instead of redrawing continuously, keeping idle CPU negligible

### Requirement: Quit keybind

The system SHALL exit the draw loop when the user presses the `q` key. No other keyboard input SHALL be processed in v0.3.0. The system SHALL poll for keyboard events with a non-blocking timeout so that key presses are detected promptly without blocking the tick cycle.

#### Scenario: Pressing q exits the dashboard

- **GIVEN** a running dashboard
- **WHEN** the user presses `q`
- **THEN** the draw loop exits
- **AND** `run_dashboard` returns `Ok(())`

#### Scenario: Other keys are ignored

- **GIVEN** a running dashboard
- **WHEN** the user presses any key other than `q`
- **THEN** the dashboard continues running

### Requirement: Agent status table rendering

The system SHALL render a table displaying all known agents with the following columns:

| Column | Content | Width |
|---|---|---|
| Agent | The `agent_id` (slugified branch name) | Flexible |
| CLI | The CLI name (e.g. `"claude"`) | Fixed ~10 |
| Status | A Unicode symbol + status label | Fixed ~15 |
| Last Update | Relative time since last message | Fixed ~10 |

The table SHALL NOT render a `Summary` column. The horizontal space formerly
occupied by the `Summary` column SHALL be reclaimed by the remaining columns.

The table SHALL have a header row with column labels. When no agents are known (e.g. at session start before any agent has posted), the table SHALL display a single row or message indicating "No agents connected yet".

**Supervisor row placement.** When the agent snapshot contains an entry with `agent_id == "supervisor"`, the table SHALL render that entry as the first data row (row 0 below the header), regardless of the alphabetical ordering of the other entries. A visually distinguishable divider row SHALL be rendered immediately below the supervisor row to separate it from the coding-agent rows. The coding-agent rows SHALL follow the divider in their existing alphabetical-by-`agent_id` order.

When no `agent_id == "supervisor"` entry is present in the snapshot, no divider SHALL be rendered, and the coding-agent rows SHALL render in their existing alphabetical order starting from row 0.

#### Scenario: Table displays agent rows

- **GIVEN** `agent_status_snapshot` returns two agents: `feat-errors` (status "done", 3 minutes ago) and `feat-detect` (status "working", 30 seconds ago)
- **WHEN** the dashboard renders a frame
- **THEN** the table contains two data rows with the correct agent IDs, statuses, and relative times

#### Scenario: Table displays empty state

- **GIVEN** `agent_status_snapshot` returns an empty list
- **WHEN** the dashboard renders a frame
- **THEN** the table area displays "No agents connected yet"

#### Scenario: Table has a header row

- **WHEN** the dashboard renders a frame with at least one agent
- **THEN** the first row of the table contains column labels: Agent, CLI, Status, Last Update
- **AND** the header row does NOT contain a `Summary` column label

#### Scenario: Supervisor row is pinned to the top of the data rows

- **GIVEN** `agent_status_snapshot` returns three agents: `feat-broker`, `supervisor`, `feat-dashboard` (in alphabetical order, that ordering is `feat-broker`, `feat-dashboard`, `supervisor`)
- **WHEN** the dashboard renders a frame
- **THEN** the first data row (below the header) is the `supervisor` row
- **AND** a visually distinguishable divider row is rendered immediately below the supervisor row
- **AND** the subsequent rows are `feat-broker` then `feat-dashboard` (alphabetical)

#### Scenario: No divider when supervisor row is absent

- **GIVEN** `agent_status_snapshot` returns two agents: `feat-broker` and `feat-dashboard`, neither of which is `supervisor`
- **WHEN** the dashboard renders a frame
- **THEN** the first data row is `feat-broker` and the second is `feat-dashboard`
- **AND** no divider row is rendered

### Requirement: Agent row formatting as pure functions

The system SHALL provide pure functions for formatting agent data into display-ready rows. These functions SHALL perform no I/O, hold no locks, and be deterministic given the same inputs.

- `pub fn format_agent_rows(agents: &[AgentStatusEntry], now: Instant) -> Vec<AgentRow>` — converts raw agent data into formatted row structs
- `pub fn format_status_line(total: usize, working: usize, done: usize, blocked: usize, committed: usize) -> String` — produces a summary line like `"5 agents: 2 working, 1 done, 1 blocked, 1 committed"`

`AgentRow` SHALL be a public struct with `String` fields: `agent_id`, `cli`, `status`, `age`. `AgentRow` SHALL NOT carry a `summary` field, because the agent-status table no longer renders a Summary column.

**Phase preference.** When `format_agent_rows` builds the row for an entry whose underlying snapshot carries a most-recent `BrokerMessage::Status` with `payload.phase = Some(p)`, the row's `status` field SHALL render `p` (with the same status-symbol prefixing applied as for any other label). When `payload.phase` is `None` (or the most-recent message is not a `Status` variant), the row's `status` field SHALL fall back to the existing message-type-derived label.

#### Scenario: format_agent_rows produces correct row count

- **GIVEN** a list of 3 `AgentStatusEntry` values
- **WHEN** `format_agent_rows(agents, now)` is called
- **THEN** the result contains exactly 3 `AgentRow` values

#### Scenario: format_agent_rows populates all fields

- **GIVEN** an `AgentStatusEntry` with `agent_id = "feat-errors"`, status `"done"`, last seen 180 seconds ago
- **WHEN** `format_agent_rows` is called
- **THEN** the resulting `AgentRow` has `agent_id = "feat-errors"`, a non-empty `status` field containing `"done"`, and `age = "3m ago"`

#### Scenario: AgentRow exposes no summary field

- **GIVEN** an `AgentStatusEntry` for any agent
- **WHEN** `format_agent_rows` is called
- **THEN** the resulting `AgentRow` exposes only the `agent_id`, `cli`, `status`, and `age` fields
- **AND** no `summary` field is present on the row

#### Scenario: format_status_line produces a summary

- **WHEN** `format_status_line(4, 2, 1, 1, 0)` is called
- **THEN** the result is `"4 agents: 2 working, 1 done, 1 blocked, 0 committed"`

#### Scenario: format_agent_rows prefers phase over status_label for supervisor

- **GIVEN** an `AgentStatusEntry` for `agent_id = "supervisor"` whose most-recent message is a `BrokerMessage::Status` with `payload.status = "feedback"`, `payload.phase = Some("merging")`
- **WHEN** `format_agent_rows` is called
- **THEN** the resulting supervisor `AgentRow`'s `status` field contains `"merging"`
- **AND** the `status` field does NOT contain `"feedback"`

#### Scenario: format_agent_rows falls back to status_label when phase is None

- **GIVEN** an `AgentStatusEntry` for `agent_id = "feat-broker"` whose most-recent message is a `BrokerMessage::Status` with `payload.status = "working"`, `payload.phase = None`
- **WHEN** `format_agent_rows` is called
- **THEN** the resulting `AgentRow`'s `status` field contains `"working"`

### Requirement: Human-readable age formatting

The system SHALL provide a pure function `pub fn format_age(elapsed: Duration) -> String` that converts a duration into a human-readable relative time string:

- Less than 60 seconds → `"Xs ago"` (e.g. `"30s ago"`)
- 1 to 59 minutes → `"Xm ago"` (e.g. `"3m ago"`)
- 60 minutes or more → `"Xh Ym ago"` (e.g. `"1h 15m ago"`)

#### Scenario: Seconds range

- **WHEN** `format_age(Duration::from_secs(30))` is called
- **THEN** the result is `"30s ago"`

#### Scenario: Zero seconds

- **WHEN** `format_age(Duration::from_secs(0))` is called
- **THEN** the result is `"0s ago"`

#### Scenario: Minutes range

- **WHEN** `format_age(Duration::from_secs(180))` is called
- **THEN** the result is `"3m ago"`

#### Scenario: Hours and minutes

- **WHEN** `format_age(Duration::from_secs(4500))` is called
- **THEN** the result is `"1h 15m ago"`

#### Scenario: Exact hour boundary

- **WHEN** `format_age(Duration::from_secs(3600))` is called
- **THEN** the result is `"1h 0m ago"`

### Requirement: Status symbol mapping

The system SHALL provide a pure function `pub fn status_symbol(status: &str) -> &'static str` that maps agent status labels to Unicode symbols:

| Input | Output |
|---|---|
| `"working"` | `"🔵"` |
| `"done"` | `"🟢"` |
| `"verified"` | `"🟢"` |
| `"blocked"` | `"🟡"` |
| `"idle"` | `"⚪"` |
| any other value | `"⚪"` |

#### Scenario: Working status symbol

- **WHEN** `status_symbol("working")` is called
- **THEN** the result is `"🔵"`

#### Scenario: Done status symbol

- **WHEN** `status_symbol("done")` is called
- **THEN** the result is `"🟢"`

#### Scenario: Verified status symbol

- **WHEN** `status_symbol("verified")` is called
- **THEN** the result is `"🟢"`

#### Scenario: Blocked status symbol

- **WHEN** `status_symbol("blocked")` is called
- **THEN** the result is `"🟡"`

#### Scenario: Idle status symbol

- **WHEN** `status_symbol("idle")` is called
- **THEN** the result is `"⚪"`

#### Scenario: Unknown status falls back to default

- **WHEN** `status_symbol("something-unexpected")` is called
- **THEN** the result is `"⚪"`

### Requirement: Dashboard title

The system SHALL display a title line above the agent table containing the text `"git-paw dashboard"`. The title SHALL be visible at all times while the dashboard is running.

#### Scenario: Title is displayed

- **WHEN** the dashboard renders a frame
- **THEN** the rendered output includes the text `"git-paw dashboard"`

### Requirement: Dashboard pane layout

The system SHALL arrange tmux panes with the dashboard in a full-width top row and worktree panes tiled below. This layout SHALL be applied automatically when the dashboard starts and maintained throughout the session.

#### Scenario: Dashboard takes full width at top

- **GIVEN** a tmux session with dashboard pane and 3 worktree panes
- **WHEN** the dashboard applies its layout
- **THEN** the dashboard pane occupies the full window width as the top row
- **AND** the worktree panes are arranged in a tiled layout below the dashboard

#### Scenario: Layout is maintained after tmux operations

- **GIVEN** dashboard with proper layout applied
- **WHEN** a tmux operation occurs that might disrupt layout (e.g., window resize)
- **THEN** the system re-applies the correct layout on next render cycle

### Requirement: Committed status in counter and symbols

The system SHALL extend agent status tracking to include the "committed" state in both the counter display and status symbol mapping.

#### Scenario: Status counter includes committed count

- **WHEN** `format_status_line(5, 2, 1, 1, 1)` is called (total, working, done, blocked, committed)
- **THEN** the result is `"5 agents: 2 working, 1 done, 1 blocked, 1 committed"`

#### Scenario: Status symbol for committed

- **WHEN** `status_symbol("committed")` is called
- **THEN** the result is `"🟣"`

### Requirement: Broker messages panel (config-gated)

When the configuration option `[dashboard] show_message_log = true` is enabled, the system SHALL display an additional panel showing a scrolling tail of recent broker messages.

#### Scenario: Message log panel is hidden by default

- **GIVEN** default configuration (show_message_log not set)
- **WHEN** the dashboard renders
- **THEN** no message log panel is displayed

#### Scenario: Message log panel shows when enabled

- **GIVEN** configuration with `show_message_log = true`
- **WHEN** the dashboard renders
- **THEN** a message log panel is displayed showing recent broker messages

#### Scenario: Message log shows various message types

- **GIVEN** broker messages of types status, artifact, blocked, question
- **WHEN** the message log panel renders
- **THEN** all message types are displayed with appropriate formatting

### Requirement: No prompt-inbox panel

The dashboard SHALL NOT render a prompt-inbox panel for `agent.question` messages. Specifically, the dashboard layout SHALL NOT include:

- A "Questions (N pending)" titled block listing pending `agent.question` messages
- An input field for replying to questions
- A focused-question cursor or any keybindings (Tab, Enter, Backspace, printable characters) for navigating or composing replies in the dashboard

`agent.question` messages SHALL continue to flow through the broker per the `message-delivery` capability (routed to the `"supervisor"` inbox), and the supervisor agent SHALL read and respond to them via curl + the supervisor pane. The dashboard's role is observation only — agent status and (optionally, in v0.6.0+) a recent-messages log.

The `q` keybind SHALL remain the sole keyboard input handled by the dashboard.

#### Scenario: Dashboard layout has no Questions panel

- **GIVEN** a running dashboard with at least one `agent.question` in the broker's supervisor inbox
- **WHEN** the dashboard renders a frame
- **THEN** the rendered output does NOT contain the substring `"Questions ("`
- **AND** the rendered output does NOT contain a "Reply to" input prompt

#### Scenario: Tab key is not handled

- **GIVEN** a running dashboard
- **WHEN** the user presses Tab
- **THEN** the dashboard continues running without changing its display state
- **AND** no focus indicator is shown

#### Scenario: Printable character keys do not enter an input buffer

- **GIVEN** a running dashboard
- **WHEN** the user presses any printable character (e.g. `a`, `1`, space) other than `q`
- **THEN** the dashboard continues running without buffering the character
- **AND** no input field is updated or rendered

#### Scenario: Vertical layout collapses to the non-inbox shape

- **GIVEN** a running dashboard with `show_message_log = false`
- **WHEN** the dashboard renders a frame
- **THEN** the vertical layout contains, in order: a title chunk, the agent table chunk, and the status line chunk
- **AND** no prompts-section or input-field chunk is allocated

### Requirement: Exit when orphaned

The dashboard SHALL terminate when its session is gone, and SHALL NOT busy-loop after that point regardless of how the session ended or whether its in-process broker started. Specifically:

- The dashboard SHALL exit when its parent process is no longer present — on Unix, when it has been reparented to init (`getppid() == 1`) — so it never outlives a session torn down without SIGHUP (an abrupt `tmux kill-server`, a crash, or machine sleep). On platforms without this signal the prior SIGHUP-based shutdown behavior is retained.
- The dashboard SHALL additionally exit when its controlling terminal is gone — detected by polling the controlling terminal for hang-up (`POLLHUP`/`POLLERR`/`POLLNVAL`) or by a failed write to the terminal — so a dashboard reparented to a lingering shell (a parent that is alive but is not init) still terminates once its pane is gone. The terminal wait SHALL be bounded so a hung-up terminal cannot trap the loop before the lifecycle check runs; the dashboard SHALL NOT rely on the event-input poll returning an error to notice the hang-up, because a hung-up terminal is perpetually readable and would otherwise busy-loop the input poll without ever returning.
- If the in-process broker fails to bind its port, the dashboard SHALL emit a diagnostic and exit rather than enter a render/retry busy-loop.
- The shutdown / orphan / tty-gone check SHALL be evaluated on every loop path, including any error or degraded path, so no branch can bypass it and busy-loop.

#### Scenario: Orphaned-to-init dashboard exits

- **GIVEN** a running dashboard whose parent process has terminated, so it is reparented to init
- **WHEN** the draw loop next checks its lifecycle
- **THEN** the dashboard exits instead of continuing to render

#### Scenario: Dashboard keeps running while its parent is alive and its pane is present

- **GIVEN** a running dashboard whose parent process is alive and whose controlling terminal is present
- **WHEN** the draw loop checks its lifecycle
- **THEN** the dashboard continues running

#### Scenario: Reparented-to-shell dashboard exits when its pane is gone

- **GIVEN** a dashboard whose tmux pane was killed but whose parent is a lingering shell (not init), so `getppid()` is a live but unrelated process
- **WHEN** the draw loop next interacts with the (now-gone) controlling terminal
- **THEN** the tty-gone condition is detected and the dashboard exits rather than busy-looping

#### Scenario: Broker-bind failure exits instead of busy-looping

- **GIVEN** a dashboard whose in-process broker cannot bind its port (for example, a stale dashboard is still holding it)
- **WHEN** the dashboard starts
- **THEN** it emits a diagnostic and exits, rather than entering a high-CPU render/retry loop

#### Scenario: Lifecycle check is not bypassed on error paths

- **GIVEN** a running dashboard that takes an error or degraded branch of its loop
- **WHEN** that branch executes
- **THEN** the same shutdown / orphan / tty-gone check applies, so the dashboard cannot busy-loop on any path

#### Scenario: Session teardown releases the dashboard and its broker port

- **GIVEN** a running supervisor dashboard hosting the in-process broker
- **WHEN** the session is stopped and the dashboard's tmux pane pty hangs up
- **THEN** the dashboard's terminal wait returns (rather than trapping on the perpetually-readable hung-up pty), the lifecycle check exits the loop, and the in-process broker's port is released — a fresh connection to it is refused rather than the port remaining bound by a spinning orphan

### Requirement: Broker log panel exists in the dashboard

When the broker is enabled, the dashboard SHALL render a Broker log
panel. The panel SHALL display a scrolling list of broker messages,
newest first, within the screen region freed by the v0.5.0 prompt-inbox
removal.

The list SHALL scroll its viewport to keep the selected row visible:
when the user moves the selection with `Up`/`k` or `Down`/`j` past the
edge of the visible area, the viewport SHALL scroll so the selected
row stays on screen. Every retained message that passes the active
filter SHALL be reachable by scrolling — the panel SHALL NOT be capped
to only the first screenful of rows.

When visible, the panel SHALL be allotted a vertical share larger than
the v0.6.0 fixed twelve rows, so more messages are visible without
scrolling. The panel's height SHALL be a fixed number of rows whose
default is strictly greater than `12`, and SHALL be configurable via
`[dashboard.broker_log] height_lines`. The agent-status table SHALL
retain a positive minimum height that it absorbs the terminal's slack
into, so enlarging the panel does not starve the table: on a terminal
too short to grant both their full heights, the panel SHALL yield space
before the table collapses below its minimum. The hidden-panel layout
(panel toggled off) SHALL be unchanged and remain byte-equivalent to the
v0.5.0 three-segment shape.

#### Scenario: Panel renders when broker enabled

- **GIVEN** an active dashboard with the panel visible
- **WHEN** broker messages have been observed
- **THEN** the dashboard SHALL render the Broker log panel showing the
  most recent messages newest-first

#### Scenario: Scrolling reaches messages beyond the first screen

- **GIVEN** a visible panel holding more filter-passing messages than
  fit in the panel's row area
- **WHEN** the user presses `Down`/`j` repeatedly past the bottom of
  the visible area
- **THEN** the viewport SHALL scroll so later (older) messages become
  visible and the selected row stays on screen — every retained
  message is reachable, not just the first screenful

#### Scenario: Visible panel gets more than twelve rows by default

- **GIVEN** a dashboard with no `[dashboard.broker_log] height_lines`
  configured and the panel visible
- **WHEN** the dashboard computes its vertical layout
- **THEN** the Broker log panel segment SHALL be a fixed-height segment
  whose row count is strictly greater than `12`

#### Scenario: Configured height_lines sets the panel height

- **GIVEN** `[dashboard.broker_log] height_lines = 24` and the panel
  visible
- **WHEN** the dashboard computes its vertical layout
- **THEN** the Broker log panel segment SHALL be allotted exactly `24`
  rows

#### Scenario: Agent table keeps a positive minimum height

- **GIVEN** a dashboard with the panel visible
- **WHEN** the dashboard computes its vertical layout
- **THEN** the agent-status-table segment SHALL be a minimum-height
  segment with a positive lower bound, so the enlarged panel SHALL NOT
  reduce the table below that minimum

### Requirement: Bounded ring buffer with configurable cap

The dashboard SHALL retain at most `max_messages` messages in
the panel's ring buffer. The value SHALL be configurable via
`[dashboard.broker_log] max_messages` (default 500). When the
buffer reaches capacity, the oldest message SHALL drop off as
new messages arrive.

#### Scenario: Default cap is 500 messages

- **GIVEN** a dashboard launched with no
  `[dashboard.broker_log]` configuration
- **WHEN** the panel's buffer is queried at runtime
- **THEN** the configured capacity SHALL be 500

#### Scenario: Configured cap is respected

- **GIVEN** `[dashboard.broker_log] max_messages = 100` in
  `.git-paw/config.toml`
- **WHEN** the dashboard observes more than 100 messages
- **THEN** the panel SHALL retain exactly the most recent 100
  messages and SHALL drop earlier ones

### Requirement: Per-type filter chips

The panel SHALL render a header row of filter chips covering the ten
digit-hotkey message types (toggled with `1`-`9` then `0`), plus an
`All` reset chip. The user SHALL toggle individual chips with hotkeys
without leaving the keyboard. Filtering SHALL be a render-time view
operation; the underlying ring buffer SHALL retain all messages
regardless of active filters.

Message types beyond the tenth (currently `agent.answer`) SHALL carry
their own filter-mask bit, be retained in the ring buffer, and render
under the `All` filter without a dedicated chip — the digit hotkey
scheme is exhausted at ten chips, and a future change MAY extend the
hotkey scheme to give them chips.

#### Scenario: All chip is the default

- **GIVEN** a freshly opened dashboard
- **WHEN** the panel renders
- **THEN** the `All` filter SHALL be active and every retained
  message SHALL be visible

#### Scenario: Toggling a chip narrows the visible set

- **GIVEN** the panel with messages of multiple types in the
  buffer
- **WHEN** the user presses the hotkey for the `status` chip
- **THEN** only `agent.status` messages SHALL be visible while
  the chip is active, and the ring buffer SHALL still contain
  all messages

#### Scenario: Multiple chips combine inclusively

- **GIVEN** the panel
- **WHEN** the user toggles both `status` and `intent` chips
- **THEN** messages of either type SHALL be visible and all
  other types SHALL be hidden

#### Scenario: All chip resets the filter

- **GIVEN** any active filter state
- **WHEN** the user presses the `All` chip hotkey
- **THEN** every retained message SHALL be visible again

#### Scenario: Answer rows are visible under All without a dedicated chip

- **GIVEN** the panel with an `agent.answer` message in the buffer
- **WHEN** the `All` filter is active
- **THEN** the answer row SHALL be visible with type label `answer`
- **AND** the chip row SHALL contain no `answer` chip

### Requirement: Panel toggle hotkey

The dashboard SHALL provide a global hotkey that toggles the
Broker log panel's visibility. The hotkey SHALL work in every
dashboard mode (supervisor / non-supervisor / read-only). When
the panel is hidden, the dashboard's agent-table/segment layout SHALL
match its v0.5.0 post-inbox-removal state, and the always-present
status line SHALL carry a one-line hint that the toggle hotkey (`l`)
shows the panel again, so the hidden state is recoverable.

#### Scenario: Hotkey toggles visibility

- **GIVEN** a dashboard with the panel visible
- **WHEN** the user presses the toggle hotkey (`l`)
- **THEN** the panel SHALL be hidden on the next frame, and
  pressing again SHALL restore it

#### Scenario: Toggle works in supervisor mode

- **GIVEN** a supervisor-mode dashboard
- **WHEN** the user presses the toggle hotkey
- **THEN** the panel SHALL hide and re-show consistently with
  the non-supervisor case

#### Scenario: Hidden state shows a restore hint

- **GIVEN** a dashboard with the panel hidden
- **WHEN** the dashboard renders
- **THEN** the agent-table/segment layout SHALL match the v0.5.0
  post-inbox-removal layout, AND the status line SHALL include a hint
  naming the `l` hotkey as the way to show the panel

### Requirement: Compact row format

Each rendered row in the panel SHALL display, on a single line,
the message timestamp (`HH:MM:SS`), type short form, agent or
publisher identifier, and a one-line summary derived from the
message body. Summaries exceeding the row width SHALL be
truncated with an ellipsis (`…`).

#### Scenario: Row contains the four documented fields

- **WHEN** any broker message is rendered
- **THEN** the rendered row SHALL contain the timestamp in
  `HH:MM:SS` form, the type short form, the agent or publisher
  identifier, and a derived summary

#### Scenario: Long summary truncates with ellipsis

- **WHEN** a message's derived summary exceeds the available
  row width
- **THEN** the rendered row SHALL truncate the summary with
  `…` so the line fits the panel width without wrapping

### Requirement: Details overlay

The dashboard SHALL provide a details overlay accessed by
pressing Enter on a highlighted row. The overlay SHALL display
the message's pretty-printed JSON body in a scrollable view.
Pressing Esc SHALL close the overlay.

#### Scenario: Enter opens the overlay

- **GIVEN** a row highlighted in the panel
- **WHEN** the user presses Enter
- **THEN** the dashboard SHALL render a modal overlay
  containing the message's full JSON, pretty-printed

#### Scenario: Esc closes the overlay

- **GIVEN** an open details overlay
- **WHEN** the user presses Esc
- **THEN** the overlay SHALL close and the dashboard SHALL
  return to its prior view

### Requirement: Watcher restart resilience

The panel SHALL NOT clear its ring buffer on broker watcher
restarts within the dashboard process. Historical messages
SHALL remain visible across a transient broker outage; new
messages after restart SHALL appear at the top of the buffer
when they arrive.

#### Scenario: Buffer survives a transient watcher restart

- **GIVEN** a panel with N messages in its buffer
- **WHEN** the broker watcher restarts mid-session without the
  dashboard process exiting
- **THEN** the panel SHALL still show the N historical
  messages and SHALL continue to display new messages as they
  arrive after the watcher restart

### Requirement: Dashboard chapter reflects the supervisor-as-pane state

`docs/src/user-guide/dashboard.md` SHALL describe the dashboard
in its current state: it lives in pane 1 of supervisor sessions,
shows the agents status table, and does NOT include an
interactive prompt inbox panel. The chapter SHALL NOT contain
forward-looking statements claiming that v0.4 (or later) will add
features that have since either shipped and been removed
(prompt inbox) or shipped already (conflict detection,
learnings mode).

#### Scenario: Dashboard chapter does not promise v0.4 prompt inbox

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** it does NOT contain forward-looking text claiming v0.4 will add an interactive prompt inbox panel

#### Scenario: Dashboard chapter places the dashboard at pane 1 in supervisor mode

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** any reference to the dashboard's pane location in supervisor mode states that it is at pane 1
