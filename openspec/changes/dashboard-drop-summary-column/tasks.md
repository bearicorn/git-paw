# Tasks — dashboard-drop-summary-column

## 1. Dashboard render layer (`src/dashboard.rs`)

- [ ] 1.1 Remove `"Summary"` from the header `Row::new([...])` (~:327) so the
  header renders `["Agent", "CLI", "Status", "Last Update"]`.
- [ ] 1.2 Remove the `r.summary.clone()` cell from the `AgentTableRow::Agent`
  `Row::new(vec![...])` mapping (~:344) so each agent row renders four cells.
- [ ] 1.3 Remove one `divider_segment.clone()` from the `AgentTableRow::Divider`
  `Row::new(vec![...])` mapping so the divider renders four segments.
- [ ] 1.4 Remove the fifth width constraint (`Constraint::Min(20)`) from the
  `widths` array so it has four entries matching the four columns.
- [ ] 1.5 Remove the `summary: String` field from the `AgentRow` struct (~:71)
  and its doc comment.
- [ ] 1.6 Remove the `summary: agent.summary.clone()` line from the `AgentRow`
  constructed in `format_agent_rows` (~:155).

## 2. Broker delivery (`src/broker/delivery.rs`)

- [ ] 2.1 Remove the hardcoded `summary: String::new()` field from the
  `AgentStatusEntry` constructed at ~:417.
- [ ] 2.2 If removing 2.1 leaves `AgentStatusEntry.summary` with no remaining
  producers or consumers, remove the field from the struct definition;
  otherwise leave the struct unchanged.

## 3. Do NOT touch the separate, used summary plumbing

- [ ] 3.1 Do NOT modify `src/dashboard/broker_log.rs::derive_summary` or any
  broker-log digest code — it is a separate, used concern.
- [ ] 3.2 Do NOT modify the `IntentPayload.summary` or `AdvancedMain.summary`
  wire message fields in `src/broker/messages.rs`.

## 4. Tests

- [ ] 4.1 Update the dashboard header test so it asserts the header contains
  `Agent`, `CLI`, `Status`, `Last Update` and does NOT contain `Summary`
  (maps to "Table has a header row" scenario).
- [ ] 4.2 Add/adjust a test asserting `AgentRow` exposes only `agent_id`,
  `cli`, `status`, `age` and has no `summary` field (maps to "AgentRow exposes
  no summary field" scenario) — e.g. update existing `format_agent_rows`
  field-population tests to the four-field shape.
- [ ] 4.3 Grep tests for `Summary` and the `AgentRow.summary` field; update
  every agent-table assertion to the four-column shape. Leave broker-log
  `derive_summary` tests untouched.

## 5. Docs

- [ ] 5.1 Update `docs/src/user-guide/dashboard.md`: remove the `Summary`
  column from the ASCII table (~:18) and from any prose column list.
- [ ] 5.2 Run `mdbook build docs/` and confirm it succeeds.

## 6. Gates

- [ ] 6.1 `just check` (fmt + clippy + all tests) passes.
- [ ] 6.2 `openspec validate dashboard-drop-summary-column --strict` passes.
