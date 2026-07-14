# dashboard-broker-log Specification

## Purpose
Adds a scrolling, filterable Broker log panel to the dashboard that displays recent broker messages newest-first from a bounded ring buffer, with per-type filter chips, a toggle hotkey, compact rows, a JSON details overlay, and buffer resilience across watcher restarts.
## Requirements
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

