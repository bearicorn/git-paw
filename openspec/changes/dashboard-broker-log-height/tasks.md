# Tasks — dashboard-broker-log-height

## 1. Config field (`src/config.rs`)

- [ ] 1.1 Add a `height_lines: u16` (or `usize`) field to `BrokerLogConfig`
  (~:237) next to `max_messages` / `default_visible`, with
  `#[serde(default = "BrokerLogConfig::default_height_lines")]` and a doc
  comment naming the default.
- [ ] 1.2 Add `BrokerLogConfig::default_height_lines()` returning a value
  strictly greater than `12` (e.g. `20`), and include `height_lines` in the
  `Default` impl.
- [ ] 1.3 Confirm the field participates in the repo-overrides-global merge as
  a scalar (repo value wins), consistent with the other `[dashboard.broker_log]`
  fields.

## 2. Layout rebalance (`src/dashboard.rs`)

- [ ] 2.1 Change `build_layout_constraints` (~:279) so the visible-panel branch
  takes the panel height as a parameter (threaded from config) instead of the
  hard-coded `Constraint::Length(12)` (~:285): the panel segment becomes
  `Constraint::Length(height_lines)`.
- [ ] 2.2 Change the agent-table constraint from `Constraint::Min(0)` to
  `Constraint::Min(M)` with a small positive `M` (title + header + a few rows)
  so the enlarged panel cannot starve the table; the table still absorbs slack
  on tall terminals (maps to "Agent table keeps a positive minimum height").
- [ ] 2.3 Plumb `height_lines` from `BrokerLogConfig` through the dashboard
  launcher (`run_dashboard` / `run_dashboard_with_panes`, ~:402/:426) into
  `build_layout_constraints` / `draw_frame`, alongside the existing
  `max_messages` / `default_visible` plumbing.
- [ ] 2.4 Keep the hidden-panel (`show_panel = false`) branch a three-segment
  layout, byte-equivalent to v0.5.0 (unchanged).

## 3. Tests

- [ ] 3.1 Layout test: with the panel visible and the default height,
  `build_layout_constraints` returns a panel segment whose `Length` is strictly
  greater than `12` (maps to "Visible panel gets more than twelve rows by
  default"). Assert the computed constraint split, not pixels (TUI draw loop is
  coverage-exempt).
- [ ] 3.2 Layout test: with an explicit configured height (e.g. `24`) the panel
  segment's `Length` equals that value (maps to "Configured height_lines sets
  the panel height").
- [ ] 3.3 Layout test: the agent-table segment is a `Min` constraint with a
  positive lower bound (maps to "Agent table keeps a positive minimum height").
- [ ] 3.4 Update `layout_collapses_without_message_log` (~:1250) so the segment
  *count* assertions (3 hidden / 4 visible) still hold and any pinned
  `Length(12)` expectation is replaced with the new default.
- [ ] 3.5 Config tests: `height_lines` parses from
  `[dashboard.broker_log] height_lines = 24`; a bare `[dashboard.broker_log]`
  table and a v0.5.0 `[dashboard]` section both fall back to the default; the
  field round-trips through save/load (maps to the four `configuration`
  scenarios). Extend the existing `BrokerLogConfig` tests (~:2851).

## 4. Docs

- [ ] 4.1 Update the dashboard chapter (`docs/src/user-guide/dashboard.md`) to
  note the Broker log panel's larger default height.
- [ ] 4.2 Update the configuration reference to document
  `[dashboard.broker_log] height_lines` (default, meaning) alongside
  `max_messages` / `default_visible`.
- [ ] 4.3 Run `mdbook build docs/` and confirm it succeeds.

## 5. Gates

- [ ] 5.1 `just check` (fmt + clippy + all tests) passes.
- [ ] 5.2 `just deny` passes (no new dependencies expected).
- [ ] 5.3 `openspec validate dashboard-broker-log-height --strict` passes.
