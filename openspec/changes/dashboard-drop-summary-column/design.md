# Design — dashboard-drop-summary-column

## Context

The dashboard's agent-status table (`src/dashboard.rs`) renders five columns:
`Agent`, `CLI`, `Status`, `Last Update`, and `Summary`. The `Summary` column
is sourced from `AgentRow.summary` (dashboard.rs:71), which is copied from
`AgentStatusEntry.summary` in `format_agent_rows` (dashboard.rs:155).

In production that field is **always empty**:

- The broker builds each `/status` roster entry in
  `src/broker/delivery.rs` and hardcodes `summary: String::new()`
  (delivery.rs:417). Nothing else ever writes a non-empty value into the
  `AgentStatusEntry.summary` that feeds the dashboard table.
- The `agent.status` wire message (`StatusPayload`) carries **no** summary
  field, so there is no upstream source the broker could populate the column
  from even if it wanted to.

The net effect: the `Summary` column header renders, every cell is blank, and
the column's flexible width (`Constraint::Min(20)`) consumes horizontal space
on every redraw while never displaying data. This is dead UI weight.

There is a separately-named, genuinely-used "summary" concern that this change
must leave untouched: `src/dashboard/broker_log.rs::derive_summary` produces
the one-line digests shown in the broker-log panel, and the
`IntentPayload.summary` / `AdvancedMain.summary` wire fields feed that digest.
These are not the dead agent-table column and are out of scope.

## Goals

- Remove the always-blank `Summary` column from the dashboard agent-status
  table: the header label, the per-row cell, the divider segment, and the
  width constraint.
- Remove the now-unused `summary` field on the `AgentRow` struct and the line
  in `format_agent_rows` that copies it.
- Drop the hardcoded `summary: String::new()` at `delivery.rs:417` (and, if it
  becomes unused as a result, the `AgentStatusEntry.summary` field).
- Reclaim the horizontal space for the four columns that carry real data:
  `Agent`, `CLI`, `Status`, `Last Update`.

## Non-Goals

- **Do NOT touch** `src/dashboard/broker_log.rs::derive_summary` or any of the
  broker-log digest plumbing — it is a separate, used concern.
- **Do NOT touch** the `IntentPayload.summary` or `AdvancedMain.summary` wire
  message fields (`src/broker/messages.rs`) — they feed `derive_summary` and
  remain in use.
- No change to the `agent.status` wire format, broker HTTP API, or any config
  field. This is a purely cosmetic dashboard render change.
- No change to the broker-log panel, its toggle, or its layout.

## Decisions

1. **Drop the column at the render layer and the row struct together.** The
   header (`dashboard.rs:327`), the `Row::new` cell (`:344`), the divider
   segment, and the width constraint are removed so the table renders four
   columns. The `AgentRow.summary` field (`:71`) and its copy in
   `format_agent_rows` (`:155`) are removed so no dead field lingers behind a
   removed column.

2. **Drop the broker-side hardcoded empty summary.** `delivery.rs:417`'s
   `summary: String::new()` is removed. If removing it leaves
   `AgentStatusEntry.summary` with no remaining producers or consumers, the
   field itself is removed; otherwise it is left as-is. Either way the
   dashboard no longer reads it.

3. **Behavioural tests assert column absence.** The dashboard test that
   currently asserts the header contains `Summary` is updated so the header
   row contains exactly `Agent`, `CLI`, `Status`, `Last Update` and does NOT
   contain `Summary`. Row-render tests that referenced the summary field/cell
   are updated to the four-column shape.

## Risks

- **Risk:** a test or doc elsewhere still asserts the five-column shape and
  would fail after the change. *Mitigation:* the tasks include grepping for
  `Summary` across tests and docs and updating every agent-table assertion;
  the `derive_summary`/broker-log occurrences are explicitly excluded.
- **Risk:** accidentally removing the still-used `derive_summary` /
  `IntentPayload.summary` / `AdvancedMain.summary` plumbing while chasing the
  word "summary". *Mitigation:* explicit non-goal and a dedicated task stating
  these MUST NOT be modified; the broker-log panel and its tests must stay
  byte-identical.
- **Risk:** layout regression — the four remaining columns reflow. *Mitigation:*
  this is the intended outcome (reclaimed space); the dashboard docs ASCII
  table is updated to match, and the empty-state / divider rows are verified to
  still render with four cells.
