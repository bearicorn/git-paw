## ADDED Requirements

### Requirement: Dashboard shows attributed usage against the budget caps

The dashboard SHALL render a usage panel below its title line, built from the same attributed usage that `git paw status --json` reports: each worker's usage from its CLI's usage report, and the usage of each headless orchestrator invocation the session records. The panel SHALL show:

- one row per worker with its agent id, its CLI's provider family, its tier (the CLI id the worker was launched with), input tokens, output tokens, and cost. Cost SHALL be shown only when the CLI reports one; otherwise the cell SHALL show `—`. A worker whose usage is unknown SHALL show `unknown` and the reason instead of figures, and SHALL NOT be counted as zero;
- one subtotal row per tier, summing that tier's known figures;
- one row for the orchestrator's headless invocations, giving their count and their summed known figures;
- a total row over all known figures, labelled as covering known usage only, with the number of workers and invocations whose usage is unknown;
- for each configured budget cap (`budget_max_tokens` counting input plus output tokens, and `budget_max_cost`), the known total beside the cap, the percentage used and a text progress bar, marked as exceeded at or above 100%. A cost cap for which no attributed source reports cost SHALL be shown as not evaluable;
- a note that the figures exclude the operator's own session.

Token counts SHALL be shown in a compact form (for example `12.3k`, `1.2M`). The panel SHALL show how old the figures are, and SHALL show that usage is loading until the first read completes. The exceeded mark SHALL be a text marker plus a text attribute, so it is visible without colour. The panel SHALL NOT show agent status, phase, last-update or pane information.

#### Scenario: Workers, tiers and totals

- **GIVEN** workers `feat-a` (tier `sonnet`, 100 in / 20 out), `feat-b` (tier `sonnet`, 50 in / 10 out) and `feat-c` (tier `opus`, 200 in / 40 out), and no recorded orchestrator invocation
- **WHEN** the usage panel renders
- **THEN** it SHALL show three worker rows, a `sonnet` subtotal of 150 in / 30 out, an `opus` subtotal of 200 in / 40 out, and a total of 350 in / 70 out

#### Scenario: Unknown usage is not counted as zero

- **GIVEN** worker `feat-a` with known usage of 100 in / 20 out and worker `feat-g` running a `generic`-family CLI
- **WHEN** the usage panel renders
- **THEN** the `feat-g` row SHALL show `unknown` with a reason naming its CLI and no token figures
- **AND** the total SHALL be 100 in / 20 out, labelled as known usage, and SHALL name one worker with unknown usage

#### Scenario: Cost appears only when the CLI reports it

- **GIVEN** worker `feat-a` whose report carries a cost of 1.20 and worker `feat-b` whose report carries tokens but no cost
- **WHEN** the usage panel renders
- **THEN** `feat-a` SHALL show cost `1.20`, `feat-b` SHALL show `—`, and the total cost SHALL be `1.20`

#### Scenario: Orchestrator invocations have their own row

- **GIVEN** a session that records two headless orchestrator invocations with known usage of 30 in / 5 out and 10 in / 5 out
- **WHEN** the usage panel renders
- **THEN** the orchestrator row SHALL show 2 invocations and 40 in / 10 out
- **AND** the total SHALL include those figures

#### Scenario: Progress against the token cap

- **GIVEN** `budget_max_tokens = 1000` and known usage totalling 600 input plus output tokens
- **WHEN** the usage panel renders
- **THEN** it SHALL show 600 of 1000 tokens at 60% without the exceeded mark
- **AND** when the known total reaches 1100 it SHALL show 110% with the exceeded mark

#### Scenario: A cost cap without a cost source is not evaluable

- **GIVEN** `budget_max_cost = 25.0` and no worker or invocation reporting cost
- **WHEN** the usage panel renders
- **THEN** the cost cap SHALL be shown as not evaluable rather than as 0%

#### Scenario: No cap, no progress line

- **GIVEN** neither budget cap configured
- **WHEN** the usage panel renders
- **THEN** it SHALL show no cap progress line

#### Scenario: The roster is gone

- **GIVEN** a session whose agents publish `agent.status` messages
- **WHEN** the dashboard renders a frame
- **THEN** the frame SHALL contain no `Status` or `Last Update` column and no status symbol

#### Scenario: Usage loads after the first read

- **GIVEN** a dashboard whose first usage read has not completed
- **WHEN** it renders a frame
- **THEN** the usage panel SHALL state that usage is loading

### Requirement: Dashboard view state is a pure function of its model

The dashboard SHALL compute everything it draws with a pure view-model function of its model. That covers the usage rows, tier subtotals, orchestrator row, totals and cap progress; the visible broker-log rows after filtering and heartbeat collapse; the status line; and the focus and selection markers. The model consists of the latest usage snapshot and the time it was read, the configured caps, the retained broker-log entries, the active filter, focus and selection, the broker-reachability and stalled-wave indicators, and the current time, which is passed in as a value. The function SHALL perform no I/O, SHALL read no clock, SHALL take no lock, and SHALL return equal view states for equal models. The draw step SHALL only lay out the view state the function returns.

#### Scenario: Equal models yield equal view states

- **GIVEN** two models built from the same usage snapshot, caps, log entries, filter, focus and time
- **WHEN** the view state is computed for each
- **THEN** the two view states SHALL be equal

#### Scenario: The view state is computed without a terminal

- **GIVEN** a model holding a usage snapshot with two workers and three broker-log entries, built in a test with no terminal attached
- **WHEN** the view state is computed
- **THEN** it SHALL contain the two worker rows, their totals and the three log rows

### Requirement: Tab moves focus between the usage panel and the broker log

The dashboard SHALL keep keyboard focus on either the usage panel or the broker log, starting on the broker log. `Tab` SHALL move focus to the other panel when both are visible. With the broker log focused, every broker-log key SHALL behave as it did before this requirement. With the usage panel focused:

- `Up`/`k` and `Down`/`j` SHALL move the selected worker row by one;
- `PageUp`/`PageDown` SHALL move it by one screen;
- `Home`/`End` SHALL move it to the first and last worker.

The worker rows SHALL scroll to keep the selection visible while the orchestrator and total rows stay visible below them. When the broker log is hidden, focus SHALL be on the usage panel. The selected row SHALL be marked with a text attribute, so it is visible without colour.

#### Scenario: Default focus keeps the broker-log keys

- **GIVEN** a freshly opened dashboard
- **WHEN** the user presses `Down`
- **THEN** the broker-log selection SHALL move and the usage selection SHALL NOT change

#### Scenario: Tab moves focus to the usage panel

- **GIVEN** a dashboard with the broker log focused
- **WHEN** the user presses `Tab` and then `Down`
- **THEN** the usage selection SHALL move down and the broker-log selection SHALL NOT change

#### Scenario: Workers beyond the screen are reachable

- **GIVEN** 40 workers and a usage panel that shows 10 worker rows, with the usage panel focused
- **WHEN** the user presses `End`
- **THEN** the last worker SHALL be visible and selected
- **AND** the orchestrator and total rows SHALL still be visible

#### Scenario: Hiding the log focuses the usage panel

- **GIVEN** a dashboard with the broker log focused
- **WHEN** the user presses `l`
- **THEN** the broker log SHALL be hidden and `Down` SHALL move the usage selection

## MODIFIED Requirements

### Requirement: Dashboard entry point

The system SHALL provide a public function with the signature:

```rust
pub fn run_dashboard(
    client: &BrokerClient,
    usage: UsageReader,
    shutdown: &AtomicBool,
) -> Result<(), PawError>
```

`BrokerClient` is the shared HTTP client (`src/broker/client.rs`) bound to the session's broker URL. `UsageReader` is the read-only query that produces the session's usage snapshot, the same one `git paw status --json` uses. This function SHALL:

1. Enter crossterm alternate screen and raw mode
2. Install a panic hook that restores the terminal before printing the panic
3. Run an event-driven draw loop whose broker data comes only from `client` (`GET /status`, `GET /log?since=`) and whose usage data comes only from `usage`
4. Exit when the user presses `q` OR when `shutdown` is set to `true` by an external signal handler
5. Restore the terminal (raw mode off, leave alternate screen) on exit or error
6. Return `Ok(())` on clean exit

The function SHALL NOT take a `BrokerState` or a `BrokerHandle`, and SHALL NOT write session, configuration or broker state. Exiting the dashboard SHALL have no effect on the broker.

#### Scenario: Dashboard starts and stops cleanly

- **GIVEN** a `BrokerClient` for a live broker, a `UsageReader` for the session, and a `shutdown: &AtomicBool` initialised to `false`
- **WHEN** `run_dashboard` is called and the user presses `q`
- **THEN** the function returns `Ok(())`
- **AND** the terminal is restored to its pre-dashboard state
- **AND** the broker is still serving

#### Scenario: External shutdown flag exits the dashboard

- **GIVEN** a running dashboard
- **WHEN** another thread sets `shutdown.store(true, Ordering::Release)`
- **THEN** the dashboard exits cleanly within one input-poll interval
- **AND** the function returns `Ok(())`

### Requirement: Periodic state polling

On each wake-up the system SHALL refresh its model by polling the broker over HTTP: `GET /status` for broker reachability and the wave status, and `GET /log?since=<last_seq>` for the broker log. It SHALL request only log entries after the last sequence number it has already seen, and SHALL NOT request the full log again after its first read. It SHALL refresh the usage snapshot every 10 seconds on a thread separate from input handling, and the snapshot SHALL reach the draw loop as a message. It SHALL render a new frame whenever a draw is needed.

The dashboard SHALL wait for input by polling its controlling terminal with a bounded idle timeout rather than busy-polling. The idle timeout SHALL be at least 800 milliseconds and at most 1000 milliseconds, so that an idle dashboard uses negligible CPU and still reflects a broker event within one second. Key input SHALL wake the wait immediately. The wait SHALL be bounded even when the terminal is gone: a hung-up controlling terminal SHALL be detected and SHALL cause the loop to exit rather than trap the wait indefinitely. HTTP requests SHALL have a bounded timeout so a slow broker cannot freeze the draw loop, and a slow usage read SHALL NOT delay input handling.

#### Scenario: Dashboard shows a broker event within one second

- **GIVEN** a running dashboard
- **WHEN** an agent publishes a message to the broker
- **THEN** the broker log displays it within 1 second

#### Scenario: The log reads only new entries

- **GIVEN** a running dashboard that holds broker-log entries up to sequence 12
- **WHEN** it next polls the broker log
- **THEN** its request SHALL carry `since=12`

#### Scenario: Usage refreshes on its cadence

- **GIVEN** a running dashboard and a worker whose usage report grows
- **WHEN** 10 seconds pass
- **THEN** the usage panel SHALL show the new figures and a figure age below 10 seconds

#### Scenario: Input is responsive

- **GIVEN** a running dashboard awaiting input
- **WHEN** the user presses `q`
- **THEN** the dashboard reacts immediately — the terminal poll returns on the keypress rather than after the idle timeout

#### Scenario: Idle dashboard does not busy-loop

- **GIVEN** a running dashboard with no pending input and no state changes
- **THEN** it waits on the bounded terminal poll for the idle timeout between wakeups instead of redrawing continuously

#### Scenario: A slow broker does not freeze the loop

- **GIVEN** a broker that stops answering mid-session
- **WHEN** the dashboard polls it
- **THEN** each request SHALL time out within the bounded timeout and the loop SHALL continue to process input

#### Scenario: A slow usage read does not block input

- **GIVEN** a usage read that takes 5 seconds
- **WHEN** the user presses `q` while it runs
- **THEN** the dashboard SHALL quit without waiting for the read to finish

### Requirement: Quit keybind

The system SHALL exit the draw loop when the user presses the `q` key. Every other key SHALL be handled only as the focus rules and the broker log panel define, and any key that neither defines SHALL be ignored. The system SHALL poll for keyboard events with a non-blocking timeout so that key presses are detected promptly without blocking the tick cycle.

#### Scenario: Pressing q exits the dashboard

- **GIVEN** a running dashboard
- **WHEN** the user presses `q`
- **THEN** the draw loop exits
- **AND** `run_dashboard` returns `Ok(())`

#### Scenario: Other keys are ignored

- **GIVEN** a running dashboard
- **WHEN** the user presses a key that neither the focus rules nor the broker log panel define (for example `z`)
- **THEN** the dashboard continues running and its view state is unchanged

### Requirement: Dashboard title

The system SHALL display a title line above the usage panel containing the text `"git-paw dashboard"`. The title SHALL be visible at all times while the dashboard is running.

#### Scenario: Title is displayed

- **WHEN** the dashboard renders a frame
- **THEN** the rendered output includes the text `"git-paw dashboard"`

### Requirement: Dashboard pane layout

When a session has a dashboard (per `session-state`'s "Dashboard pane index persisted in session state"), the system SHALL arrange tmux panes with the dashboard in a full-width top row and the coding-agent panes in the agent grid below it. The grid is row-major, with at most `agents_per_row` per row and each row at equal widths, per `tmux-orchestration`'s "Every pane-less session uses the agent grid". This layout SHALL be applied when the session is built, re-applied when an agent is added or removed, and kept for the life of the session. When the session has no dashboard, no top row SHALL be reserved and the agent grid SHALL use the whole window.

#### Scenario: Dashboard takes full width at top

- **GIVEN** a tmux session with a dashboard pane and 3 coding-agent panes
- **WHEN** the session is built
- **THEN** the dashboard pane occupies the full window width as the top row
- **AND** the 3 coding-agent panes form one equal-width row below the dashboard

#### Scenario: Layout is maintained after add and remove

- **GIVEN** a dashboard session with the grid applied
- **WHEN** an agent is added or removed
- **THEN** the dashboard SHALL remain the full-width top row and the agent rows SHALL re-flow to the grid for the new count

#### Scenario: Without a dashboard the grid fills the window

- **GIVEN** a session with the broker running and no dashboard, with 3 coding-agent panes
- **WHEN** the session is built
- **THEN** pane 0 SHALL be the first coding agent and the single agent row SHALL span the full window height

### Requirement: No prompt-inbox panel

The dashboard SHALL NOT render a prompt-inbox panel for `agent.question` messages. Specifically, the dashboard layout SHALL NOT include:

- A "Questions (N pending)" titled block listing pending `agent.question` messages
- An input field for replying to questions
- A focused-question cursor, or any keybinding (Enter, Backspace, printable characters) for selecting a question or composing a reply in the dashboard

`agent.question` messages SHALL continue to flow through the broker per the `message-delivery` capability, routed to the root identity's inbox. The orchestrator — the operator's own agent session outside the tmux session, or a person — SHALL read and answer them through the bundled helpers or the MCP tools. The dashboard's role is observation only: the usage panel and the broker log.

#### Scenario: Dashboard layout has no Questions panel

- **GIVEN** a running dashboard with at least one `agent.question` in the broker's root inbox
- **WHEN** the dashboard renders a frame
- **THEN** the rendered output does NOT contain the substring `"Questions ("`
- **AND** the rendered output does NOT contain a "Reply to" input prompt

#### Scenario: Printable character keys do not enter an input buffer

- **GIVEN** a running dashboard
- **WHEN** the user presses any printable character (e.g. `a`, `1`, space) other than `q`
- **THEN** the dashboard continues running without buffering the character
- **AND** no input field is updated or rendered

#### Scenario: Vertical layout without the broker log

- **GIVEN** a running dashboard with the broker log hidden
- **WHEN** the dashboard renders a frame
- **THEN** the vertical layout contains, in order: a title chunk, the usage panel chunk, and the status line chunk
- **AND** no prompts-section or input-field chunk is allocated

### Requirement: Broker log panel exists in the dashboard

When the broker is enabled, the dashboard SHALL render a Broker log
panel. The panel SHALL display a scrolling list of broker messages,
newest first, below the usage panel and the status line.

The list SHALL scroll its viewport to keep the selected row visible:
when the user moves the selection with `Up`/`k` or `Down`/`j` past the
edge of the visible area, the viewport SHALL scroll so the selected
row stays on screen. Every retained message that passes the active
filter SHALL be reachable by scrolling — the panel SHALL NOT be capped
to only the first screenful of rows.

When visible, the panel's height SHALL be a fixed number of rows whose
default is strictly greater than `12`, and SHALL be configurable via
`[dashboard.broker_log] height_lines`. The usage panel SHALL
keep a positive minimum height and SHALL take up the terminal's spare rows,
so enlarging the panel does not starve the usage panel: on a terminal
too short to grant both their full heights, the panel SHALL yield space
before the usage panel collapses below its minimum. With the panel hidden
the layout SHALL be the three-segment shape: title, usage panel,
status line.

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

#### Scenario: Usage panel keeps a positive minimum height

- **GIVEN** a dashboard with the panel visible
- **WHEN** the dashboard computes its vertical layout
- **THEN** the usage-panel segment SHALL be a minimum-height
  segment with a positive lower bound, so the enlarged panel SHALL NOT
  reduce the usage panel below that minimum

### Requirement: Panel toggle hotkey

The dashboard SHALL provide a global hotkey (`l`) that toggles the
Broker log panel's visibility. The hotkey SHALL work in every session
mode (bare and coordinated). When the panel is hidden, the usage panel
SHALL take every row between the title line and the status line, and the
always-present status line SHALL carry a one-line hint that the toggle
hotkey (`l`) shows the panel again, so the hidden state is recoverable.

#### Scenario: Hotkey toggles visibility

- **GIVEN** a dashboard with the panel visible
- **WHEN** the user presses the toggle hotkey (`l`)
- **THEN** the panel SHALL be hidden on the next frame, and
  pressing again SHALL restore it

#### Scenario: Toggle works in a coordinated session

- **GIVEN** a dashboard in a coordinated session
- **WHEN** the user presses the toggle hotkey
- **THEN** the panel SHALL hide and re-show as it does in a bare session

#### Scenario: Hidden state shows a restore hint

- **GIVEN** a dashboard with the panel hidden
- **WHEN** the dashboard renders
- **THEN** the usage panel SHALL span every row between the title line
  and the status line, AND the status line SHALL include a hint
  naming the `l` hotkey as the way to show the panel

### Requirement: Dashboard chapter reflects the supervisor-as-pane state

`docs/src/user-guide/dashboard.md` SHALL describe the dashboard as it currently is:

- it is an opt-in view, turned on with `[dashboard] enabled = true` or `git paw start --dashboard`, and it needs the broker;
- when enabled, it is the full-width top row at pane 0 in every session mode;
- it shows the usage panel and the broker log panel;
- it includes no interactive prompt inbox panel and no agent roster.

For agent state the chapter SHALL point to `git paw status`. The chapter SHALL NOT contain forward-looking statements that a release will add features that have since shipped or been removed, and SHALL NOT place the dashboard at pane 1.

#### Scenario: Dashboard chapter does not promise a prompt inbox

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** it does NOT contain forward-looking text claiming a future version will add an interactive prompt inbox panel

#### Scenario: Dashboard chapter places the dashboard at pane 0 in every mode

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** any reference to the dashboard's pane location states that it is at pane 0
- **AND** no sentence states that the dashboard is at pane 1

#### Scenario: Dashboard chapter states how to turn the view on

- **WHEN** `docs/src/user-guide/dashboard.md` is inspected
- **THEN** it names `[dashboard] enabled` and `git paw start --dashboard`, states that the dashboard is off by default and needs the broker, and points to `git paw status` for agent state

### Requirement: Broker log dedups consecutive identical status heartbeats

The broker-log panel SHALL collapse a run of identical status heartbeats from one agent into a single row that shows the latest timestamp and a repeat count. Heartbeats are identical when they have the same agent, status, `activity` and `message`. This keeps frequent bare heartbeats from burying real state transitions. Messages from other agents that arrive between two of an agent's heartbeats SHALL NOT break that agent's run. A status from the same agent that differs in status, activity or message SHALL begin a new row, as SHALL any non-status message from that agent. The collapsed row SHALL be displayed at the position of its latest heartbeat, so the panel stays ordered newest-first by last-seen time. Collapsing is a render-time view operation; the ring buffer SHALL retain every message.

The key spans every field the row renders, so two heartbeats collapse only when the
line a reader sees is identical. Because the panel renders a status's `message`, a
narrower key would fold rows that display differently and lose their text.

#### Scenario: Repeated identical heartbeats collapse to one row

- **GIVEN** three consecutive `agent.status` messages from the same agent, all with status `working` and no activity
- **WHEN** the broker-log panel renders them
- **THEN** they SHALL appear as a single row
- **AND** that row SHALL show a repeat count of `3` and the latest of the three timestamps

#### Scenario: Interleaved agents still collapse per agent

- **GIVEN** heartbeats arriving A `working`, B `working`, A `working`, B `working`, A `working`
- **WHEN** the broker-log panel renders them
- **THEN** there SHALL be one row for A with a repeat count of `3` and one row for B with a repeat count of `2`

#### Scenario: A changed status breaks the run

- **GIVEN** two `working` heartbeats from an agent followed by a `verified` status from the same agent
- **WHEN** the broker-log panel renders them
- **THEN** the two `working` heartbeats SHALL collapse into one row
- **AND** the `verified` status SHALL render as a separate row

#### Scenario: Heartbeats differing only by message do not collapse

- **GIVEN** two consecutive `working` heartbeats from the same agent with no `activity`, whose `message` values differ
- **WHEN** the broker-log panel renders them
- **THEN** they SHALL render as two separate rows, each with a repeat count of `1`

### Requirement: Dashboard degrades when the broker is unreachable

Whenever its most recent poll failed to reach the broker, the dashboard SHALL render a visible "broker unreachable" indicator in its status line. While the broker is unreachable it SHALL:

- keep the broker log's history;
- keep refreshing the usage panel, whose figures do not come from the broker;
- keep polling on its idle cadence.

It SHALL clear the indicator on the first successful poll. The indicator SHALL be independent of the stalled-wave indicator. It SHALL NOT exit and SHALL NOT busy-loop while the broker is unreachable.

#### Scenario: Paused broker shows the indicator

- **GIVEN** a running dashboard and `git paw pause` stopping the broker
- **WHEN** the dashboard's next poll fails
- **THEN** the status line SHALL show the broker-unreachable indicator
- **AND** the broker log SHALL still show its earlier rows and the usage panel SHALL still render

#### Scenario: Recovered broker clears the indicator

- **GIVEN** a dashboard showing the broker-unreachable indicator
- **WHEN** `git paw start` restarts the broker and the next poll succeeds
- **THEN** the indicator SHALL disappear and new broker-log rows SHALL render

### Requirement: Dashboard renders a notice below its minimum size

The dashboard SHALL compute its minimum usable size from its fixed layout constraints: the usage panel's minimum column widths and height, the title and status lines, and the broker-log panel's minimum when it is visible. When the terminal is smaller in either dimension, the dashboard SHALL render a single notice stating the current size and the required minimum instead of the clipped layout, and SHALL keep handling keys. It SHALL render the full layout again on the first frame after the terminal grows past the minimum.

#### Scenario: Tiny terminal shows the notice

- **GIVEN** a 30×8 terminal
- **WHEN** the dashboard renders
- **THEN** the frame SHALL contain the too-small notice with `30×8` and the required minimum, and no usage panel

#### Scenario: Growing the terminal restores the layout

- **GIVEN** the dashboard showing the too-small notice
- **WHEN** the terminal is resized above the minimum
- **THEN** the next frame SHALL render the usage panel and status line

#### Scenario: Quit still works when too small

- **GIVEN** the dashboard showing the too-small notice
- **WHEN** the user presses `q`
- **THEN** the dashboard SHALL quit

## REMOVED Requirements

### Requirement: Agent status table rendering

**Reason**: The dashboard no longer shows an agent roster. With the orchestrator outside the session, per-agent state belongs to `git paw status`, and the dashboard is a usage-and-events view (user decision, 2026-10-19).

**Migration**: Run `git paw status` (or `git paw status --json`, field `worktrees[].agent`). It now shows each agent's broker-reported status or phase, its age and its pane (see `session-state`, "Status shows each agent's broker-reported state"). Programmatic readers use `GET /status`, MCP `get_session_status` or `sweep.sh status`, as before.

### Requirement: Agent row formatting as pure functions

**Reason**: `format_agent_rows`, `AgentRow` and `format_status_line` existed only to render the removed roster and its agent counters. The pure formatting of the new view lives in the view-model function ("Dashboard view state is a pure function of its model").

**Migration**: None for users. Library callers of `git_paw::dashboard::{format_agent_rows, format_status_line, AgentRow}` read the roster from `GET /status` (the `coordination::inventory` join) instead.

### Requirement: Status symbol mapping

**Reason**: The status symbols decorated the removed roster rows.

**Migration**: None. `git paw status` prints the status label as text.

### Requirement: Committed status in counter and symbols

**Reason**: The agent counter line and the status symbols were removed with the roster.

**Migration**: `git paw status` shows each agent's status, including `committed`.

### Requirement: Broker messages panel (config-gated)

**Reason**: This was superseded in v0.6.0 by "Broker log panel exists in the dashboard". The `show_message_log` key has no effect in the code, and this requirement's "hidden by default" scenario contradicts the broker log, which is visible by default.

**Migration**: Use the broker log panel and `[dashboard.broker_log] default_visible`. `show_message_log` is still accepted and still ignored (see `core-configuration`).
