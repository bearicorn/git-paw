## Why

`forward-coordination` adds the `agent.intent` protocol but no logic acts on it: agents broadcast intents into the void, and overlaps go undetected. v0.4 has a manual section in `supervisor.md` that asks the CLI supervisor to compare `modified_files` across `agent.artifact` events and warn — but that's *post-commit* and human-paced. By the time the supervisor agent notices, the conflict already exists in two branches.

This change adds programmatic conflict detection in the broker process so warnings fire pre-edit (forward), during-edit (in-flight), and at first ownership violation. The supervisor *agent* (CLI in a tmux pane) keeps its observe-and-intervene role; the broker becomes the early-warning system.

## What Changes

- Add a new broker-internal subsystem (`src/broker/conflict.rs` or equivalent) that runs alongside the filesystem watcher and message delivery. The detector observes published messages and emits `agent.feedback` (and optionally `agent.question`) when one of three failure shapes is detected:
  - **Forward conflict** — two `agent.intent` messages from different agents list overlapping files. Both publishers are warned via `agent.feedback`.
  - **In-flight conflict** — two agents' `agent.status.modified_files` (auto-published by the watcher) include the same file. Both branches are warned. If neither pauses or commits within `window_seconds`, the detector escalates with `agent.question` to the supervisor inbox.
  - **Ownership violation** — an agent's `modified_files` include a file outside its own active `agent.intent` *and* inside another active agent's intent. The violator gets `agent.feedback` immediately. If `escalate_on_violation` is true (default), an `agent.question` also goes to the supervisor inbox.
- Add a `[supervisor.conflict]` TOML sub-table:
  - `window_seconds: u64` (default 120)
  - `warn_on_intent_overlap: bool` (default true)
  - `escalate_on_violation: bool` (default true)
- Add an in-memory active-intent tracker. On every `agent.intent` publish, the tracker stores `(agent_id, files, summary, valid_for_seconds, received_at)`. A sweeper task drops entries past TTL. The tracker is cleared per-agent when that agent publishes `agent.artifact` for any of the listed files (intent fulfilled) or when its TTL expires.
- Auto-emitted `agent.feedback` and `agent.question` messages SHALL use `from: "supervisor"` (matching the v0.4 convention that supervisor-originated messages carry that sender). The error text SHALL begin with a `[conflict-detector]` tag so the recipient (and dashboard) can distinguish auto-emitted warnings from human-typed supervisor feedback.
- Update the embedded `supervisor.md` skill: replace the v0.4 "### Conflict detection" section with a note that automatic forward / in-flight / ownership detection now runs in the broker. The CLI supervisor's job is to layer human judgment on top — for example, deciding when an `agent.question` escalation needs human intervention vs. waiting for the agents to self-resolve.
- The detector activates only when `[supervisor] enabled = true`. With supervisor disabled, intents are still broadcast (per `forward-coordination`) but no warnings fire — agents and humans see intents but no automated action is taken.

Not in scope:
- No CLI surface — no `git paw conflicts` command, no `--conflict-window` flag. All tuning via `[supervisor.conflict]`.
- No persistent conflict log. The active-intent tracker and in-flight tracker are in-memory only; broker restart resets state. (A broker restart implicitly invalidates all intents, which is the safe default.)
- No glob expansion. If an intent lists `src/**`, the detector treats it as a single literal entry; overlaps are computed by *string equality* against the entries (after normalization). Glob-aware overlap is deferred — current MILESTONE design note acknowledges this.
- No supervisor-side ML or heuristic ranking of conflict severity. Three rules, deterministic outcomes, configurable on/off.

## Capabilities

### New Capabilities
- `conflict-detection`: the broker-internal detector subsystem. Owns the active-intent tracker, the in-flight-conflict timer, the ownership-violation logic, and the auto-emission of `agent.feedback`/`agent.question`.

### Modified Capabilities
- `supervisor-config`: add a nested `ConflictConfig { window_seconds, warn_on_intent_overlap, escalate_on_violation }` field on `SupervisorConfig`, with TOML default-when-absent semantics matching the existing pattern.
- `agent-skills`: update the embedded supervisor skill to replace the v0.4 manual conflict-detection section with a note about the new automatic detection and the human-layered role on top.

## Impact

**Code**:
- New module `src/broker/conflict.rs` (or `src/broker/conflict/mod.rs` if it grows large): tracker structs, detector loop, overlap algorithm, auto-emission helpers.
- `src/broker/mod.rs` — wire the detector into broker startup behind a `[supervisor]` enabled check.
- `src/broker/server.rs` or `src/broker/publish.rs` — hook the detector into the publish path so it sees every `agent.intent` and `agent.status` message before/after delivery.
- `src/config.rs` — add `ConflictConfig` to `SupervisorConfig`.
- `assets/agent-skills/supervisor.md` — section rewrite.
- `docs/src/user-guide/supervisor.md` (if it exists) and `docs/src/configuration.md` — document the new config table.

**Tests**:
- Forward-conflict: two agents publish intents listing the same file → both receive `agent.feedback` from `"supervisor"` with `[conflict-detector]` tag.
- Forward-conflict skipped when `warn_on_intent_overlap = false`.
- In-flight-conflict: two agents' watcher-published `agent.status.modified_files` overlap → both warned; if no commit/pause within window, supervisor receives `agent.question`.
- In-flight escalation honours `window_seconds`.
- Ownership violation: agent A intends `src/a.rs`; agent B intends `src/b.rs`; agent B's watcher publishes `modified_files = ["src/a.rs"]` → B receives `agent.feedback`; if `escalate_on_violation`, supervisor receives `agent.question`.
- TTL sweep: intent older than `valid_for_seconds` is dropped from the tracker; subsequent overlaps don't fire warnings against the expired entry.
- Detector inactive when `[supervisor]` is disabled — intents broadcast but no warnings fire.

**Backward compatibility**: fully additive. Configs without `[supervisor.conflict]` use defaults. The detector only runs in supervisor mode, so v0.4 setups that haven't enabled supervisor are byte-for-byte unchanged. The auto-emitted `agent.feedback` messages reuse the existing variant — no new wire format.

**Mismatches surfaced (in addition to those tracked under MILESTONE.md items 12-14)**:
- `supervisor.md` line 132-135 has a v0.4 manual "Conflict detection" section. This change *replaces* it. After the rewrite, the skill description in `agent-skills/spec.md` will need an additional scenario covering the new wording. **Folded into this change** — handled in the agent-skills delta.
