## Why

The dashboard's broker-message-log panel (the `dashboard-broker-log` capability, v0.6.0) is allotted too little vertical space relative to the agent-status table, so only a few broker messages are visible at once — the operator has to scroll constantly to follow coordination traffic. Surfaced in both the v0.7.0 (F3) and v0.8.0 dogfoods as a recurring usability gap.

## What Changes

- **Give the broker-log panel a larger share of the dashboard's vertical space.** Rebalance the row-height proportions between the agent-status table and the broker-log panel so more messages are visible without scrolling (e.g. a more even split, or a configurable ratio).
- Consider a `[dashboard]` config knob for the split ratio if a single default doesn't suit both small and large agent counts.

## Capabilities

### New Capabilities
<!-- None. -->

### Modified Capabilities
- `dashboard-broker-log`: the broker-log panel receives a larger (and/or configurable) share of the dashboard's vertical space.

## Impact

- Affected code: `src/dashboard.rs` (the row-height constraint split between the agent table and the broker-log panel); possibly `src/config.rs` if a ratio knob is added.
- Tests: a dashboard layout test asserting the broker-log panel gets the intended (larger) share; config round-trip if a knob is added.
- Docs: dashboard chapter + configuration reference (if a knob is added).
- Backward compatible: layout-proportion change; no wire-format change. Pairs with `dashboard-drop-summary-column` (both dashboard-layout polish — may be implemented together).
