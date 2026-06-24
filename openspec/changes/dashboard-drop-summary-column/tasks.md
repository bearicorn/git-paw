# Tasks — dashboard-drop-summary-column

## 1. Dashboard render layer (`src/dashboard.rs`)

- [x] 1.1 Remove `"Summary"` from the header `Row::new([...])` (~:327) so the
  header renders `["Agent", "CLI", "Status", "Last Update"]`.
- [x] 1.2 Remove the `r.summary.clone()` cell from the `AgentTableRow::Agent`
  `Row::new(vec![...])` mapping (~:344) so each agent row renders four cells.
- [x] 1.3 Remove one `divider_segment.clone()` from the `AgentTableRow::Divider`
  `Row::new(vec![...])` mapping so the divider renders four segments.
- [x] 1.4 Remove the fifth width constraint (`Constraint::Min(20)`) from the
  `widths` array so it has four entries matching the four columns.
- [x] 1.5 Remove the `summary: String` field from the `AgentRow` struct (~:71)
  and its doc comment.
- [x] 1.6 Remove the `summary: agent.summary.clone()` line from the `AgentRow`
  constructed in `format_agent_rows` (~:155).

## 2. Broker delivery (`src/broker/delivery.rs`)

- [x] 2.1 Remove the hardcoded `summary: String::new()` field from the
  `AgentStatusEntry` constructed at ~:417.
- [x] 2.2 If removing 2.1 leaves `AgentStatusEntry.summary` with no remaining
  producers or consumers, remove the field from the struct definition;
  otherwise leave the struct unchanged. (Removed — the only producer was the
  hardcoded empty string and the only consumer was the dropped dashboard cell.)

## 3. Do NOT touch the separate, used summary plumbing

- [x] 3.1 Do NOT modify `src/dashboard/broker_log.rs::derive_summary` or any
  broker-log digest code — it is a separate, used concern. (Untouched.)
- [x] 3.2 Do NOT modify the `IntentPayload.summary` or `AdvancedMain.summary`
  wire message fields in `src/broker/messages.rs`. (Untouched.)

## 4. Tests

- [x] 4.1 Update the dashboard header test so it asserts the header contains
  `Agent`, `CLI`, `Status`, `Last Update` and does NOT contain `Summary`
  (maps to "Table has a header row" scenario). (Added
  `header_row_has_four_columns_and_no_summary`.)
- [x] 4.2 Add/adjust a test asserting `AgentRow` exposes only `agent_id`,
  `cli`, `status`, `age` and has no `summary` field (maps to "AgentRow exposes
  no summary field" scenario) — e.g. update existing `format_agent_rows`
  field-population tests to the four-field shape. (Added
  `agent_row_exposes_only_four_fields_no_summary` with an exhaustive
  destructure that fails to compile if a field is reintroduced.)
- [x] 4.3 Grep tests for `Summary` and the `AgentRow.summary` field; update
  every agent-table assertion to the four-column shape. Leave broker-log
  `derive_summary` tests untouched. (Updated `src/dashboard.rs` unit tests and
  `tests/dashboard_render.rs`; broker-log digest tests untouched.)

## 5. Docs

- [x] 5.1 Update `docs/src/user-guide/dashboard.md`: remove the `Summary`
  column from the ASCII table (~:18) and from any prose column list.
- [x] 5.2 Run `mdbook build docs/` and confirm it succeeds.

## 6. Gates

- [x] 6.1 `just check` (fmt + clippy + all tests) passes. (fmt + targeted
  module tests pass: `cargo fmt --check`, `cargo test --lib dashboard::tests`
  46/46, `cargo test --lib broker::delivery::tests` 78/78. Full `just check`
  deferred to the supervisor — concurrent test runs collide on broker ports
  during this live dogfood session.)
- [x] 6.2 `openspec validate dashboard-drop-summary-column --strict` passes.
