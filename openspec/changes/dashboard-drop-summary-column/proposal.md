## Why

The dashboard's agent-status table has a "Summary" column that always renders blank in production: the broker hardcodes `summary: String::new()` when building each `/status` roster entry (`src/broker/delivery.rs:417`), and the `agent.status` message (`StatusPayload`) carries no summary field at all. So the column is dead weight — it consumes horizontal space in every dashboard render and never shows anything.

## What Changes

- **Remove the dead "Summary" column** from the dashboard agent table: drop the header + cell rendering, the `summary` field on the row struct and its copy, and the hardcoded empty `summary` at `delivery.rs:417`.
- Reclaim the horizontal space for the columns that carry real data (Agent / CLI / Status / Last Update).
- **Do NOT touch** `src/dashboard/broker_log.rs::derive_summary` nor the `IntentPayload.summary` / `AdvancedMain.summary` message fields — those are a separate, *used* concern (broker-log one-line digests).

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `dashboard`: the agent-status table no longer renders a "Summary" column.

## Impact

- Affected code: `src/dashboard.rs` (header `:327`, cell `:344`, row-struct field `:71` + copy `:155`), `src/broker/delivery.rs:417` (drop the hardcoded empty summary).
- Tests: behavioural dashboard test no longer asserts a Summary column; existing row-render tests updated.
- Docs: dashboard chapter column list updated.
- Backward compatible: cosmetic dashboard change; no wire-format or config change. The `agent.status` message never carried a summary, so nothing on the broker side changes.
