## Why

The `2026-05-13-supervisor-as-pane` change shipped on `feat/v0.5.0-specs` and turned the supervisor into a tmux pane. During the 2026-05-12 and 2026-05-15/16 dogfood sessions the user identified seven concrete defects in the supervisor-as-pane surface. Drift items 30, 31, 33, 39, 40 surfaced in the 2026-05-12 session and were appended to MILESTONE.md mid-cycle AFTER the `supervisor-as-pane` agent had finished and its branch was verified — so they were never specced, never implemented, and were dropped when `supervisor-as-pane` was merged. Drift items 65 and 66 surfaced during the 2026-05-15/16 batch-1 dogfood. Drift 65: the user (acting as supervisor stand-in because Claude Code in non-TTY can't run the supervisor pane — see drift 36) observed that absorbing routine approvals every sweep is the operationally-correct behaviour, but the supervisor skill's text doesn't codify it as a continuous-iteration responsibility. Drift 66: while moving into verification, the user explicitly named the five gates that constitute supervisor verification — testing, regression analysis, spec audit, doc audit, security audit — but the supervisor skill's Workflow currently lists only three (test, regression check, spec audit with governance verification as a sub-step), leaving doc audit and security audit either buried inside governance verification or absent entirely.

The fixes are small, mutually reinforcing, and all touch the same surface (the supervisor row on the dashboard + the supervisor-pane self-registration flow + the supervisor skill's doctrine for routine-approval absorption + the supervisor skill's verification gate enumeration). Bundling them into one follow-up change keeps the diff cohesive and lets the agent reason about all seven together instead of context-switching across seven micro-PRs.

The seven drift items, with the originating dogfood evidence each:

1. **Drift 30 — Phantom supervisor row on non-TTY launches.** `cmd_supervisor` publishes `agent.status { status: "working", message: "Supervisor booting" }` for `agent_id = "supervisor"` from the launcher process, BEFORE the supervisor pane's CLI process is spawned. On any path where the supervisor pane fails to come up (non-TTY, missing CLI, layout error after the publish), the broker's `/status` retains the phantom row and the dashboard shows a "working" supervisor that never existed.

2. **Drift 31 — Supervisor row has empty CLI column.** `build_status_message(agent_id, status, message)` has no `cli` parameter. The dashboard's CLI column is populated from `inner.agent_clis` (the watch-target map), which is keyed off coding-agent panes only. The supervisor pane is not a watch target, so its `cli` resolves to `""` and the dashboard renders `cli=''` for the supervisor row.

3. **Drift 33 — Prompt-inbox panel is dead code.** The dashboard's "Questions (N pending)" panel and the "Reply to X>" input field at the bottom were intended to let the human reply to `agent.question` events. In practice (a) coding agents don't poll their inbox for `agent.feedback` replies, so submissions never reach the agent; (b) the panel doesn't track resolution, so answered questions stay visible forever; (c) with the supervisor as a pane, the human types replies directly into the supervisor pane via tmux, which is the natural surface. The 2026-05-12 user call was explicit: delete the panel. The reclaimed screen real-estate is reused by v0.6.0 issue #10 (`dashboard-broker-log`) for a recent-messages panel.

4. **Drift 39 — Supervisor row has no visual prominence.** The supervisor is the operator's primary collaborator and the row most-watched on the dashboard, but `format_agent_rows` sorts by `agent_id` alphabetically. With coding-agent IDs like `feat-broker` and `feat-dashboard`, the `supervisor` row lands near the bottom of the table. The fix is to pin the supervisor row to row 0 of the agent table and render a visual divider separating it from the coding-agent rows.

5. **Drift 40 — Supervisor status column shows `feedback` instead of an actual phase.** `delivery::record_message` sets `record.status = msg.status_label().to_string()`. For coding agents, `status_label()` returns `"working"`, `"done"`, `"committed"`, etc. — all valid lifecycle phases. For the supervisor, when it publishes `agent.feedback` to a coding agent, `status_label()` returns `"feedback"`, which is the wire-message type, not the supervisor's phase. The dashboard then shows `status=feedback` for the supervisor row, which is misleading: the supervisor's phase is "watching" or "verifying", not "feedback". The fix is an explicit `phase: Option<String>` field that the supervisor populates when it transitions between phases; the dashboard prefers `phase` over the message-type-derived label when present.

7. **Drift 66 — Supervisor verification has five explicit gates, not three.** The current `Workflow` section in `assets/agent-skills/supervisor.md` lists Test (§4) → Regression check (§5) → Spec Audit (§6) → Verify or feedback (§7), with the Governance verification sub-step bundled inside Spec Audit. The 2026-05-16 dogfood-driven correction made explicit that supervisor verification has **five first-class gates** in order:

    1. **Testing** — `{{TEST_COMMAND}}` in the agent's worktree.
    2. **Regression analysis** — diff vs baseline; previously-passing-now-failing tests are regressions.
    3. **Spec audit** — every `### Requirement:` and `#### Scenario:` in `openspec/changes/<change>/specs/` is implemented + covered by tests.
    4. **Doc audit** — mdBook (`docs/src/`), README, `AGENTS.md`, `--help`, rustdoc — updated where the change's `Impact` section says they should be. Currently buried inside the governance-verification sub-step.
    5. **Security audit** — OWASP categories from `CLAUDE.md` (command injection, XSS, SQL injection, path traversal, unvalidated external input, secret leakage in logs/errors), plus new `unwrap()`/`expect()` outside test code (project-wide rule). Currently absent or implicit in one governance-verification example bullet.

    Findings flow through `agent.feedback` with each error prefixed by the gate name (e.g. `[doc audit] mdBook chapter X not updated`, `[security audit] new unwrap() in src/foo.rs:42 outside #[cfg(test)]`). The governance-verification sub-step is preserved as a doc-audit input source.

6. **Drift 65 — Supervisor skill missing explicit "continuous-sweep absorption" doctrine.** `assets/agent-skills/supervisor.md` covers a launch-time pane sweep in §1.5 (proactive, runs once at attach) and a reactive background-poll auto-approval thread in §"Auto-approve permission prompts" (fires per-agent on `stall_threshold_seconds`). Neither codifies the **continuous proactive sweep** that worked in practice during the 2026-05-15/16 batch-1 dogfood — the user-as-supervisor swept every coding-agent pane on every monitoring-loop iteration (~270s cadence) and absorbed dev-essential prompts (`git commit`, `cargo test`, `mdbook build`, `git stash`, `git restore`, `cargo fmt --check`, `awk`, `python3 -c '...'`) before they bubbled to the human. The user's mid-session feedback was explicit: *"seems like i dont need to approve then ... so take that as a learning."* Doctrinally, the supervisor IS the rubber-stamp gate and the human is the escalation audience; the skill's Rules section doesn't state this. The fix extends the workflow's `Watch` step to invoke the §1.5 safe-command policy every iteration and adds an explicit Rules bullet about absorbing routine approvals.

These seven items are deliberately bundled because they share a single subject (the supervisor surface's correctness — dashboard rendering + self-registration flow + skill doctrine + verification gates) and most touch `StatusPayload`, `build_status_message`, `cmd_supervisor`, `dashboard.rs`, or `assets/agent-skills/supervisor.md` directly.

## What Changes

Seven small modifications, all backward-compatible on the wire:

1. **Move supervisor self-registration into the supervisor pane.** Delete the `publish_to_broker_http(... build_status_message("supervisor", "working", Some("Supervisor booting")))` call from `cmd_supervisor`. The supervisor's first `agent.status` is published from inside the supervisor pane by the supervisor agent itself (via the existing post-prompt curl flow described in the supervisor skill). When no supervisor is actually running (e.g. an aborted launch, a non-TTY skip path, a missing CLI), the broker has no supervisor row and the dashboard correctly omits the supervisor entry.

2. **Add `cli: Option<String>` to `StatusPayload`.** Field is serde-defaulted to `None` and `skip_serializing_if = "Option::is_none"`. `build_status_message` gains an optional `cli: Option<&str>` parameter. Wire-format backward compat: old payloads without the field deserialise as `None`; new payloads with the field are accepted by older binaries (serde ignores unknown fields by default for owned structs — verify this in the test). Supervisor publishes its `cli` (resolved from `[supervisor].cli` config) when it self-registers; coding agents continue to omit the field (their CLI is populated from the watch-target map by the broker).

3. **Delete the dashboard prompt-inbox panel.** Remove the `Questions (N pending)` Block, the `Reply to X> _` input Block, the `focused_question` state, the keybindings for navigating + replying, and the `drive_question_tick` polling loop. Reclaim the bottom ~10 lines of the dashboard layout. The reclaimed space remains unused in this change (v0.6.0 issue #10 fills it with a recent-messages panel).

4. **Pin the supervisor row to row 0 of the agent table + add a divider.** `format_agent_rows` (or the table renderer in `draw_frame`) detects the entry with `agent_id == "supervisor"`, pulls it out of the alphabetical sort, and renders it as row 0. A styled divider row separates the supervisor from the coding-agent rows below.

5. **Add `phase: Option<String>` to `StatusPayload`.** Field is serde-defaulted to `None` and `skip_serializing_if = "Option::is_none"`. The supervisor publishes `phase` explicitly when it transitions between lifecycle phases (e.g. `baseline`, `watching`, `approving`, `answering`, `merging`, `summary`). The dashboard's `format_agent_rows` prefers `phase` over the message-type-derived label when the entry's last message has `phase: Some(_)`. Coding-agent rows continue to use the message-type-derived label (no behaviour change for them).

7. **Promote the supervisor's verification gates to five first-class steps.** Restructure `assets/agent-skills/supervisor.md`'s Workflow so steps 4-7 read as:
   - **§4 Testing** (renamed from "Test"),
   - **§5 Regression analysis** (renamed from "Regression check"),
   - **§6 Spec audit** (unchanged in name; governance verification stays as sub-step but is reframed as one input source for the doc audit, not the doc audit itself),
   - **§6a Doc audit** (NEW) — verify mdBook chapters under `docs/src/`, `README.md`, `AGENTS.md`, `--help` text, and rustdoc on changed public items are updated where the change's `Impact` section says they should be,
   - **§6b Security audit** (NEW) — review the diff for OWASP-relevant patterns from `CLAUDE.md` (command injection, XSS, SQL injection, path traversal, unvalidated external input, secret leakage), plus any new `unwrap()`/`expect()` calls outside test code (project-wide rule). On doc/text-only changes this gate is normally a fast noop.
   - **§7 Verify or feedback** (unchanged structurally; the `agent.verified` message body grows to enumerate all five gates' outcomes; `agent.feedback` errors prefix each entry with the originating gate name so the agent can route the fix correctly).

6. **Bake the "continuous-sweep absorption" doctrine into the supervisor skill.** Two textual changes to `assets/agent-skills/supervisor.md`:
   - **Workflow §2 ("Watch") extension** — explicitly call out that on every monitoring-loop iteration, the supervisor SHALL sweep every coding-agent pane via `tmux capture-pane` and apply the §1.5 safe-command policy (auto-approve dev-essential prompts; escalate unknown prompts via `agent.question`). Reframes §1.5 from "launch-time only" to "launch-time + every iteration." The reactive `[supervisor.auto_approve]` poll thread remains a fallback for when the supervisor is offline.
   - **New Rules bullet** — *"Absorb routine approvals. Dev-essential prompts (git commit/test/fmt/clippy/stash/restore, mdbook build, broker curls, common shell reads like awk/grep/python3) are auto-approved by you on every sweep. Escalate to the human only for: real cross-agent conflicts, scope/spec decisions, destructive ops outside an agent's worktree, and anything novel."* States the operationally-correct framing that the supervisor (not the human) is the rubber-stamp gate.

### Capabilities

#### New Capabilities

*(none — all changes modify existing capabilities)*

#### Modified Capabilities

- `broker-messages` — `StatusPayload` adds `cli: Option<String>` and `phase: Option<String>` fields with serde defaults. `build_status_message` signature grows an optional CLI parameter. Wire format is forward- and backward-compatible.
- `dashboard` — prompt-inbox panel is removed. The agent-table renderer pins the supervisor row to row 0 with a divider. Row formatting prefers an explicit `phase` field over the message-type-derived status label.
- `supervisor-launch` — self-registration moves out of `cmd_supervisor` into the supervisor pane. No supervisor `agent.status` is published until the supervisor pane's agent actually starts and curls the broker itself.
- `agent-skills` — the supervisor skill's workflow §2 covers continuous-iteration safe-command sweeps (not launch-time only); a new Rules bullet codifies that absorbing routine approvals is the supervisor's job and the human is the escalation audience; the verification sub-flow (§4-§7) is restructured into five first-class gates (testing, regression analysis, spec audit, doc audit, security audit) with the existing governance-verification sub-step reframed as an input source for the doc audit gate.

## Impact

**Code:**

- `src/broker/messages.rs::StatusPayload` — add two fields, both `Option<String>` with serde defaults and skip-serializing-if-none.
- `src/broker/publish.rs::build_status_message` — add an optional `cli: Option<&str>` parameter; populate `StatusPayload.cli` from it.
- `src/main.rs::cmd_supervisor` — delete the `publish_to_broker_http(... build_status_message("supervisor", ...))` block at ~line 1056-1065. The supervisor pane's Claude publishes its own status via the existing skill-driven curl flow.
- `assets/agent-skills/supervisor.md` — the existing boot section already tells the supervisor to publish a self-registration `agent.status`. Verify the language; if the skill currently relies on the launcher publishing on its behalf, update it to instruct the supervisor to publish first thing after reading AGENTS.md.
- `src/dashboard.rs::draw_frame` — remove the prompts section + input field; collapse the layout constraints. Remove `QuestionEntry`, `drive_question_tick`, the `focused_question` cursor, the `input_buffer`, and the related keybindings.
- `src/dashboard.rs::format_agent_rows` (or `draw_frame`'s table rendering) — pin the `agent_id == "supervisor"` entry to row 0; insert a visual divider row between it and the coding-agent rows; prefer `phase` over status_label when the most recent message has `phase: Some(_)`.
- `assets/agent-skills/supervisor.md` — extend workflow §2 ("Watch") to explicitly invoke the §1.5 safe-command policy on every iteration; add a new Rules bullet codifying the "absorb routine approvals" doctrine (cf. drift 65); restructure §4-§7 into the five verification gates (Testing, Regression analysis, Spec audit, Doc audit, Security audit) and update `agent.verified` / `agent.feedback` examples to use gate-name prefixes (cf. drift 66).

**Tests:**

- Serde round-trip for `StatusPayload` with both new fields populated.
- Serde round-trip for old-format `StatusPayload` JSON (without `cli`/`phase`) — deserialises with `cli: None, phase: None`.
- `build_status_message` with `Some("claude")` populates the CLI field; with `None` omits it from JSON.
- `format_agent_rows` with mixed input (supervisor + 3 coding agents) — first row is the supervisor; coding agents follow alphabetically.
- `format_agent_rows` with a `StatusPayload { phase: Some("merging"), status: "feedback", .. }` for the supervisor → supervisor row's status column shows "merging", not "feedback".
- Dashboard layout snapshot test (TestBackend buffer) — no "Questions (N pending)" text, no "Reply to" input field, no prompts/input chunk in the layout.
- Integration test: `cmd_supervisor` returns without publishing any supervisor `agent.status` (broker `/status` should contain coding agents only until the supervisor pane's Claude posts on its own).

**Backward compatibility:**

- Wire format: old broker binaries reading new `StatusPayload` JSON ignore the unknown `cli`/`phase` fields (default serde behaviour for owned structs). New broker binaries reading old JSON populate `cli: None, phase: None`.
- Saved session state: not affected (sessions don't serialise `StatusPayload`).
- Configuration: not affected (no new config fields).
- v0.4 deployments observing the supervisor row will see one fewer row at launch time (the supervisor row only appears AFTER its pane's Claude curls the broker, typically within 3-5 seconds of pane creation rather than from launcher-side publish). This is the intended behaviour change.

**Mismatches surfaced by this change:**

- The supervisor skill currently relies on the launcher publishing the initial `agent.status` so the dashboard immediately shows the supervisor. The skill needs to be updated to take ownership of the self-registration step. If the skill update is missed, the supervisor row will simply appear ~3-5 seconds later — not catastrophic.
- The dashboard's `MAX_VISIBLE_QUESTIONS` constant and related helpers in `src/dashboard.rs` become unused after panel removal. Sweep with grep to remove dead code.
