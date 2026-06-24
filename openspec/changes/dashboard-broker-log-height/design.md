# Design — dashboard-broker-log-height

## Context

The dashboard's vertical layout is computed in
`src/dashboard.rs::build_layout_constraints` (dashboard.rs:279). When the
Broker log panel is visible the frame is split into four stacked segments
with these constraints:

```
Constraint::Length(1)    // title
Constraint::Min(0)       // agent-status table  (absorbs all slack)
Constraint::Length(1)    // status line
Constraint::Length(12)   // broker log panel    (fixed 12 rows)
```

The agent table uses `Constraint::Min(0)`, so it absorbs *all* the terminal's
slack height, while the Broker log panel is pinned to a fixed `Length(12)`.
Subtracting the panel's title/border chrome, that leaves only a handful of
message rows visible. On a tall terminal the agent table (often just a few
agents plus the supervisor row and divider) sits on a sea of empty rows while
the operator has to scroll the Broker log constantly to follow coordination
traffic. This was surfaced as F3 in the v0.7.0 dogfood and recurred in the
v0.8.0 dogfood.

This is the `dashboard-broker-log` capability (v0.6.0). The capability already
owns the panel's existence, ring buffer, filter chips, toggle hotkey, row
format, and details overlay. The only thing this change touches is **how much
vertical space the panel is allotted** relative to the agent table — the
constraint at dashboard.rs:285. The hidden-panel layout
(`show_panel = false`, the v0.5.0 three-segment shape) is unchanged.

The Broker log panel's initial visibility and ring-buffer cap are already
configurable via `[dashboard.broker_log]` (`max_messages`, `default_visible`)
in `BrokerLogConfig` (config.rs:237). This change adds one sibling field to the
same table for the panel's height.

## Goals

- Give the Broker log panel a materially larger share of the dashboard's
  vertical space when visible, so more messages are visible without scrolling.
- Make the panel's height configurable via an optional
  `[dashboard.broker_log] height_lines` field, so small-agent-count sessions
  (lots of slack) and large-agent-count sessions (table needs room) can both
  be tuned. The default SHALL be larger than the previous fixed `12`.
- Keep the agent table comfortable: it must still get a reasonable minimum
  height even when the panel grows.

## Non-Goals

- **Do NOT touch** `src/dashboard/broker_log.rs` content rendering:
  `derive_summary`, the compact row format, filter chips, the details overlay,
  scrolling, or the ring buffer. This change is purely the *outer* layout
  split, not what the panel draws inside its region.
- No change to the `[dashboard.broker_log] max_messages` or `default_visible`
  fields, the toggle hotkey, or the panel's hidden state.
- No change to the agent-table columns (that is the sibling
  `dashboard-drop-summary-column` change — these two may land together but are
  independent).
- No change to the broker wire format, HTTP API, or any other config section.
- No change to the hidden-panel (`show_panel = false`) three-segment layout —
  it stays byte-equivalent to v0.5.0.

## Decisions

1. **Rebalance toward the panel via a configurable fixed height with a larger
   default.** The simplest, most predictable rebalance keeps the existing
   structure — title `Length(1)`, table `Min(...)`, status `Length(1)`, panel
   `Length(N)` — but raises the panel's `N` from the hard-coded `12` to a
   configurable value defaulting to a larger number (e.g. `20`). The agent
   table keeps `Min(...)` so it still absorbs slack on tall terminals, but the
   panel now claims a bigger fixed slice. This avoids a percentage split that
   would shrink the table to a sliver on short terminals.

2. **Give the agent table a non-zero minimum.** To keep the table usable when
   the panel grows on a short terminal, the table constraint becomes
   `Constraint::Min(M)` with a small positive `M` (the title + header + a few
   rows) rather than `Min(0)`. Ratatui resolves `Min` constraints before the
   panel's fixed `Length`, so the table is not starved; on a short terminal the
   panel yields rather than the table collapsing to nothing.

3. **Add `height_lines` to the existing `[dashboard.broker_log]` table.** The
   knob lives next to `max_messages` and `default_visible` in `BrokerLogConfig`
   (config.rs:237). It carries `#[serde(default = ...)]` so a config without
   the field — including every v0.5.0/v0.6.0/v0.7.0 config — loads the new
   default unchanged. The value is plumbed from `BrokerLogConfig` through the
   dashboard launcher into `build_layout_constraints`.

4. **Assert the computed constraint split, not pixels.** The TUI draw loop is
   coverage-exempt and pixel assertions are brittle across terminal sizes. The
   behavioural test therefore asserts the *constraints* returned by
   `build_layout_constraints` (the public-within-crate seam already used by the
   existing `layout_collapses_without_message_log` test): with the panel
   visible the panel segment's `Length` SHALL equal the configured height and
   SHALL be strictly greater than the previous fixed `12`. A second test
   round-trips the config field. We do not assert a specific rendered row
   count.

## Risks

- **Risk:** on a short terminal the enlarged panel could squeeze the agent
  table to nothing. *Mitigation:* decision 2 gives the table a positive
  `Min(...)`; ratatui honours `Min` before the panel's `Length`, so the panel
  shrinks first. A test asserts the table constraint is a `Min` with a
  positive lower bound.
- **Risk:** an existing test pins the panel segment to `Length(12)` and would
  fail. *Mitigation:* the tasks include updating any such assertion to the new
  default while keeping the segment-count assertions
  (`layout_collapses_without_message_log`) intact.
- **Risk:** the new config field breaks round-trip or v0.5.0 load
  compatibility. *Mitigation:* `#[serde(default)]` plus a round-trip test and a
  "bare `[dashboard.broker_log]` / no field still parses" test, mirroring the
  existing `max_messages`/`default_visible` coverage.
- **Risk:** scope creep into the panel's content. *Mitigation:* explicit
  non-goal — `broker_log.rs` and `derive_summary` are untouched; this change
  edits only the outer constraint vector and the config struct.
