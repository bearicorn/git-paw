## Context

The `2026-05-13-supervisor-as-pane` change moved the supervisor agent into a tmux pane and reshaped the launch flow so `cmd_supervisor` returns immediately after `tmux_session.execute()` plus boot-prompt injection. Dogfood on 2026-05-12 against `feat/v0.5.0-specs` surfaced five concrete defects in the resulting surface (drift 30, 31, 33, 39, 40 in MILESTONE.md). The agent that implemented `supervisor-as-pane` had already published its `agent.verified` message and merged before the drifts were added, so none of them were specced or implemented.

This change is the follow-up. It is intentionally narrow — five defects, no broader supervisor restructure, no new capabilities. Where each defect has more than one reasonable fix, this design selects the one that minimises wire-format churn and matches the existing surface conventions.

## Goals / Non-Goals

**Goals:**

- Eliminate phantom supervisor rows on aborted launches (D1).
- Make the supervisor row's CLI column populate correctly (D2).
- Remove the prompt-inbox panel from the dashboard (D3).
- Pin the supervisor row to row 0 of the agent table with a divider (D4).
- Replace the misleading `feedback` status label on the supervisor row with an explicit phase field (D5).
- Preserve wire-format backward compatibility on `StatusPayload`.

**Non-Goals:**

- Re-introducing an inbox-style human-input surface elsewhere in the dashboard. The supervisor pane is the human's input surface; no replacement panel ships in this change. (v0.6.0 issue #10 reclaims the freed space for a recent-messages panel.)
- Re-architecting the agent status table (column set, widths, sorting outside the supervisor pin). The drift items don't call for it.
- Changing the wire-format discriminator for any `BrokerMessage` variant. `agent.feedback` stays as-is; the supervisor's misleading "status=feedback" symptom is fixed via the new `phase` field, not by adding a new variant.
- Adding `cli` resolution to the broker side. Drift 31's "OR have the broker resolve `cli` for the `supervisor` agent_id from `[supervisor].cli` config" alternative is explicitly rejected — the broker stays config-unaware.
- Reading agent inbox messages for `agent.feedback` reply routing (the inbox-panel's original use case). Coding-agent inbox handling for feedback is a separate concern, addressed in `dashboard-broker-log` (v0.6.0 #10) and the agent-skills coordination flow.

## Decisions

### D1. Self-registration moves into the supervisor pane (drift 30)

**Decision:** Delete the `publish_to_broker_http(broker_url, build_status_message("supervisor", "working", Some("Supervisor booting")))` call from `cmd_supervisor`. The supervisor pane's Claude publishes its own initial `agent.status` via the existing skill-driven curl flow (the supervisor skill's bootstrap instructions already include the canonical curl POST template; the only change is that it actually executes that template before doing anything else).

**Mechanism:** the supervisor pane's boot block (built in `cmd_supervisor` at ~line 1036 via `build_boot_block("supervisor", ...)`) instructs the agent to publish a self-registration `agent.status`. The boot block is already injected into the supervisor pane via `submit_prompt_to_pane(... 0 ...)`. After this change, the boot block is the sole source of supervisor self-registration; nothing publishes on the launcher side.

**Effect on the non-TTY skip path:** under any aborted launch (a non-TTY skip return path, a `PawError` raised after `tmux_session.execute()` but before send-keys completes, or a system-level failure to spawn the supervisor pane), the broker has no `agent_id = "supervisor"` record. The dashboard's `agent_status_snapshot` skips the supervisor row entirely, which is the correct behaviour: no supervisor process exists, so no supervisor row should render.

The supervisor-launch spec's existing "Supervisor self-registration" requirement currently states that `cmd_supervisor` publishes the initial status. That language is updated by this change's `specs/supervisor-launch/spec.md` delta to specify that publication happens from inside the supervisor pane (via the skill-driven curl), not from `cmd_supervisor` pre-return.

**Alternatives considered:**

- Keep the launcher-side publish but only fire it after a successful `tmux capture-pane` check confirming the supervisor pane has reached an interactive prompt. **Rejected** — adds polling complexity to `cmd_supervisor`, doesn't fix the underlying conceptual mismatch ("a process that hasn't started shouldn't publish its own status").
- Have the supervisor pane publish on every boot block (idempotent re-registration). **Rejected** — adds noise to the broker message log; the boot block runs once, so a single self-registration is sufficient.

### D2. Explicit `cli` parameter on `build_status_message` (drift 31)

**Decision (option A from the task description):** add a `cli: Option<String>` field to `StatusPayload`. Update `build_status_message`'s signature to:

```rust
pub fn build_status_message(
    agent_id: &str,
    status: &str,
    message: Option<String>,
    cli: Option<&str>,
) -> BrokerMessage
```

The new field uses `#[serde(default, skip_serializing_if = "Option::is_none")]` so:

- Old JSON without the field deserialises as `cli: None`.
- New JSON with `cli: None` omits the field from the serialised bytes.
- New JSON with `cli: Some("claude")` includes `"cli": "claude"` in the payload object.

**Why option A:** the broker is intentionally config-unaware (its job is wire-format relay + queue management; reaching back into `.git-paw/config.toml` for special-case CLI resolution is a layering violation). Putting the CLI on the wire keeps the broker dumb.

**Effect on the broker's agent_clis map:** the broker continues to populate `inner.agent_clis` from `WatchTarget` for coding agents. When a status message arrives with `payload.cli = Some(...)`, the broker SHALL upsert the supplied value into `inner.agent_clis` for that `agent_id`. This means the supervisor's `cli` (published by its own self-registration in D1) lands in `agent_clis` and the dashboard's row renders correctly.

**Alternatives considered:**

- Option B: broker reads `[supervisor].cli` from config to fill the supervisor's CLI column. **Rejected** — couples the broker to the config layer; in remote-broker scenarios (v1.0.0+) the config isn't even on the same machine.
- Add a separate `agent.register` message variant. **Rejected** — over-engineering for a single optional field; bumps the variant count without benefit.

### D3. Delete the dashboard prompt-inbox panel (drift 33)

**Decision:** remove the prompt-inbox panel — including the `Questions (N pending)` Block, the `Reply to X> _` input field at the bottom, the `focused_question` index, the `input_buffer` state, all keybindings that touch them (Tab, Enter, Backspace, printable-character input outside `q`), and the `drive_question_tick` polling loop. The dashboard's vertical layout collapses from 5 or 6 chunks down to 3 or 4 (title + agent table + status line, plus optional message log).

**Why remove rather than fix:**

- Coding agents don't poll their inbox for `agent.feedback` replies (no skill instructs them to). Submissions through the input field never reach the agent.
- The panel doesn't track resolution — answered questions stay visible indefinitely, cluttering the panel.
- With the supervisor as a pane, the human's primary input surface is the supervisor pane (typed directly via tmux). The inbox panel duplicates that surface poorly.
- v0.6.0 issue #10 (`dashboard-broker-log`) is already specced to reuse this layout area for a recent-messages panel that surfaces broker events including `agent.question` more usefully.

**State that goes away:**

- `QuestionEntry` struct, the `questions: Vec<QuestionEntry>` field, the `focused_question: Option<usize>` cursor, `input_buffer: String`, the `MAX_VISIBLE_QUESTIONS` constant.
- `drive_question_tick`, all unit tests that exercise inbox behaviour, the keybinding cases for Tab/Enter/Backspace/printable-chars in `run_dashboard`'s event loop.

**State that stays:**

- The `q`-to-quit keybinding (`Quit keybind` requirement is unchanged).
- `agent.question` messages on the wire — the broker keeps routing them to the supervisor's inbox per `message-delivery::Question messages are routed to the supervisor inbox`. The supervisor agent reads them via curl and responds via tmux/curl.
- The `automatic-approval` spec's "the prompt SHALL be surfaced to the human via the dashboard prompts inbox" language — that requirement is amended by this change's `specs/dashboard/spec.md` delta to read "via the supervisor pane" since the inbox is gone.

**Alternative considered:**

- Keep the panel but fix the agent-side polling. **Rejected** — adds a polling loop to every coding agent's skill, which is exactly the kind of mechanism the supervisor-as-pane change removed from the launcher. The user's 2026-05-12 call was explicit: delete the panel.

### D4. Pin the supervisor row to row 0 with a divider (drift 39)

**Decision:** in the agent-table renderer (or in `format_agent_rows`), partition the input list into `(supervisor_entry, coding_entries)`. Render the supervisor entry as row 0, render a divider row beneath it, then render coding entries in their existing alphabetical-by-`agent_id` order.

**Divider mechanism:** insert a single ratatui `Row::new(["─".repeat(N), "─".repeat(N), "─".repeat(N), "─".repeat(N), "─".repeat(N)])` (one wide horizontal-line character per column) styled with `Style::default().fg(Color::DarkGray)` or similar. The exact character and color are implementation details; the spec requirement is that a visually distinguishable separator row appears between the supervisor row and the coding-agent rows.

**When the supervisor row is absent:** if no `agent_id == "supervisor"` entry exists in the snapshot (e.g. on a coding-only session, or before the supervisor self-registers), no divider is rendered and the coding agents render alphabetically from row 0 (existing behaviour preserved).

**Estimated diff size:** ~20 lines in `dashboard.rs` — a partition call, a divider row constructor, and a chained iterator that yields `[supervisor_row, divider_row, coding_row_1, ..., coding_row_N]` instead of the current alphabetical list.

**Alternative considered:**

- Pin the supervisor row via a custom sort key that gives `"supervisor"` a sentinel sort value of `""`. **Rejected** — works for sorting but doesn't produce the visual divider; the partition approach is more explicit about the layout intent.

### D5. Explicit `phase` field on `StatusPayload` (drift 40)

**Decision (recommended in the task description):** add a `phase: Option<String>` field to `StatusPayload` with `#[serde(default, skip_serializing_if = "Option::is_none")]`. The supervisor agent populates `phase` when publishing an `agent.status` during a phase transition (e.g. `baseline`, `watching`, `approving`, `answering`, `merging`, `summary`). The exact phase vocabulary is established by the supervisor skill, not by this spec — the spec requirement is that `phase` is a free-form `Option<String>`.

**Dashboard preference rule:** when `format_agent_rows` builds the row for an entry whose most recent message is a `BrokerMessage::Status { payload, .. }` with `payload.phase = Some(p)`, the row's status column SHALL render `p` (with status-symbol mapping applied to `p`). Otherwise, the row falls back to the message-type-derived label (`status_label()`) as today.

**Why this over `agent.supervisor_status`:** introducing a new wire variant would (a) require validation rules in `broker/messages.rs`, (b) require routing rules in `broker/delivery.rs`, (c) bloat the variant count to 9+ for a single optional field on a single existing variant. Adding an optional field to the existing `agent.status` payload is the minimum invasive change.

**Why coding agents don't use `phase`:** coding agents have a clear lifecycle (`working` → `done` → `committed` → `verified`) that maps cleanly to `status_label()`. Their `phase` stays `None` and they retain their existing rendering. If a future capability (e.g. test-runner pacing) introduces phases for coding agents, this field can carry that without further wire changes.

**Backward compatibility:**

- Old JSON without `phase`: deserialises as `phase: None` → dashboard falls back to current behaviour. No regression.
- Old broker binaries reading new JSON with `phase: Some(...)`: serde silently ignores the unknown field; dashboard renders status_label-derived label as before. Degrades gracefully.

**Alternative considered:**

- Compute the supervisor's phase in the dashboard from the message log (e.g. "if the last 3 messages are all `agent.feedback`, label phase as `verifying`"). **Rejected** — fragile heuristic, hard to evolve; explicit publisher-driven phase is much easier to reason about.

## Risks / Trade-offs

- **[Risk] D1 introduces a window where the dashboard has no supervisor row.** After `cmd_supervisor` returns, the supervisor pane's Claude takes a few seconds to boot and execute its self-registration curl. During that window the dashboard shows coding-agent rows but no supervisor row. **Mitigation:** documented behaviour; the user is already running `tmux attach -t paw-<project>` to interact, so they see the supervisor pane appear alongside the dashboard pane. Window is bounded by Claude boot time (~3-5s). If dogfood reveals friction, the supervisor skill's bootstrap section can be ordered so that the self-registration curl is the very first action.

- **[Risk] D2 adds a field to a serialised payload; older deployments mixing v0.4 and v0.5 binaries on the same broker may interleave messages with and without the field.** **Mitigation:** serde's default behaviour for owned structs is to ignore unknown fields on deserialisation and use defaults for missing fields — both directions degrade gracefully. The new tests pin this behaviour.

- **[Risk] D3 removes a documented user-facing feature.** The `automatic-approval` spec's "surfaced to the human via the dashboard prompts inbox" language was an architectural promise. **Mitigation:** the modified language redirects to the supervisor pane, which is the actual surface the human uses. Release notes call out the change.

- **[Risk] D4 partition assumes exactly one supervisor row.** If somehow two agents publish with `agent_id = "supervisor"` (e.g. a misconfigured session), the partition takes only the first; the second is dropped from the partition path and rendered alphabetically among coding agents. **Mitigation:** the broker's `agent_id` slug validation prevents two distinct agents from claiming the literal `"supervisor"` slug in practice; if it does happen, the visual effect is one supervisor on top and one in the alphabetical list, which is loud enough to debug.

- **[Risk] D5 phase vocabulary is unfixed.** Future supervisor skill iterations may add or rename phases; the dashboard accepts any string. **Mitigation:** the `status_symbol` function falls back to a default symbol for unknown labels, so the dashboard never crashes on an unfamiliar phase. The vocabulary is owned by the supervisor skill, which is versioned with the binary.

- **[Trade-off] All five drifts are bundled in one change.** A reviewer who wants to verify D1 in isolation has to read D2-D5 too. **Why bundled:** the drifts share the supervisor-row surface; reviewing them together produces a coherent picture. Splitting risks five micro-PRs that each touch overlapping files.

## Migration Plan

1. Land this change on the `feat/v0.5.0-specs` branch (or wherever supervisor-as-pane shipped).
2. Update the supervisor skill (`assets/agent-skills/supervisor.md`) to publish its self-registration `agent.status` as the very first action after reading AGENTS.md. Verify the resolved skill output contains an explicit curl POST template that the agent runs unconditionally.
3. Document in release notes: (a) phantom supervisor rows no longer appear on aborted launches; (b) the dashboard's "Questions" panel is removed; (c) the supervisor row is now pinned to the top of the dashboard table.
4. Rollback path: revert. v0.4 behaviour returns — phantom rows on non-TTY skip, empty CLI column, prompt-inbox panel, alphabetical sort, status=feedback for supervisor. Wire format unchanged on revert (serde default-Nones stay valid).

## Open Questions

- **Should the supervisor skill establish a canonical phase vocabulary (e.g. `baseline`, `watching`, `approving`, `answering`, `merging`, `summary`) or leave it free-form?** The spec uses free-form; the recommendation here is that the skill ships a canonical list as suggested values but doesn't constrain the field. If dogfood reveals divergent vocabularies, formalise in v0.6.0.

- **Should the divider row (D4) be a true ratatui visual separator (a `Row` with a horizontal-line character) or a styled empty row?** Implementation detail; the spec only requires that the divider is visually distinguishable. Recommendation: horizontal-line characters with a dimmed style, matching `git status` conventions.

- **Does D5's `phase` field need a validator in `from_json`?** The proposal is unbounded free-form (matches the supervisor skill's freedom to evolve). If a future change adds a fixed vocabulary, the validator can be added then.

- **Should D1's deletion of the launcher-side publish be guarded behind a config flag for one release cycle?** Recommendation: no — the dogfood evidence is clear that the launcher-side publish is wrong, and a config flag would carry indefinite tech debt. Direct deletion + release-notes call-out is the right move.
