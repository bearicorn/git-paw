# dashboard-broker-log Specification Delta — dashboard-broker-log-height

## MODIFIED Requirements

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
