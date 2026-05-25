## Context

The broker process in v0.4 owns three subsystems: an HTTP server (`server.rs`), a publish/delivery pipeline (`publish.rs`, `delivery.rs`), and a filesystem watcher (`watcher.rs`). The watcher is the closest precedent for what this change adds — a long-running task that observes events and auto-publishes broker messages on behalf of agents.

The supervisor *agent* (CLI in a tmux pane) is a peer in the broker — it reads its inbox, runs tests, and publishes `agent.feedback` / `agent.verified` / `agent.question`. Until v0.5.0 it was the only entity producing those messages.

`forward-coordination` adds the `agent.intent` variant. The wire is set up; nothing acts on intents yet.

This design specifies the broker-internal subsystem that:
- watches `agent.intent` and `agent.status` events as they pass through the publish pipeline,
- maintains an in-memory tracker of active intents and per-agent file claims,
- emits `agent.feedback` (and conditionally `agent.question`) when one of three failure shapes triggers.

Three constraints shape the design:

1. **Single source of truth for "what's an active intent."** Both the forward-conflict check (intent vs. intent) and the ownership-violation check (status modified_files vs. *other* agents' intents) need the same active-intent map. One tracker, two consumers.
2. **In-flight conflicts need a timer.** Forward and ownership are point-in-time predicates. In-flight needs "two agents have been touching the same file for ≥ window_seconds without one pausing." That requires per-pair state plus a tick-based escalation.
3. **No duplicate warnings.** If agents A and B publish overlapping intents, the detector should warn each *once*, not on every subsequent publish from the same pair. The tracker carries a `warned_pairs: HashSet<(agent_id, agent_id)>` to dedupe.

## Goals / Non-Goals

**Goals:**
- Detect three failure shapes (forward, in-flight, ownership) and emit the appropriate warnings within the same poll cycle as the triggering event.
- Drive everything from existing broker events (`agent.intent` from publish, `agent.status` from the watcher) — no new event source.
- Keep the supervisor *agent* untouched architecturally; the only skill change is documentation. The agent still polls, reasons, and publishes — it just shares the broker channel with auto-emitted messages.
- Use the existing `agent.feedback` and `agent.question` variants. No new wire format.
- Default-on for the warning paths (`warn_on_intent_overlap = true`, `escalate_on_violation = true`); default `window_seconds = 120`. Match MILESTONE.md's stated defaults exactly.
- Exit cleanly when supervisor mode is off — the detector simply doesn't start. Intent broadcast still works (per `forward-coordination`).

**Non-Goals:**
- No glob expansion. Intents listing `src/**` are stored as the literal string `"src/**"`; overlaps are computed by exact-equality on entries (after normalization). MILESTONE acknowledges this; glob-aware overlap is left to a follow-up.
- No persistent conflict log. The tracker is in-memory; a broker restart resets state. `learnings-mode` (later in v0.5.0) will record aggregate conflict counts via a different path (publishing `supervisor.learning` events).
- No CLI surface. No `git paw conflicts` subcommand. Tuning is via `[supervisor.conflict]` in config.
- No human-judgment ranking. The detector emits warnings deterministically; severity is implicit in which rule fired.
- No prevention. Detection only — agents that ignore warnings can still race. v1.0.0 may add advisory locks; v0.5.0 just warns.

## Decisions

### D1. Detector lives in the broker process, not the supervisor agent

Three options were considered:

| Option | Where | Pros | Cons |
|---|---|---|---|
| Skill instructions only | Supervisor CLI agent | Zero new code | Slow (CLI poll cycles), brittle (depends on the agent reasoning correctly), hard to dedupe warnings, can't run a TTL sweep on intents |
| Broker-internal subsystem (chosen) | New module under `src/broker/` | Fast, deterministic, has all the state it needs in one place, mirrors watcher pattern | New code path; need to be careful about ordering relative to publish/delivery |
| Separate sidecar process | Standalone binary | Process-level isolation | Operational overhead (lifecycle, IPC, config); buys nothing here |

Chose broker-internal. Sits next to the watcher in `src/broker/`. Consumes the same publish-event stream that delivery uses; emits via the same publish API the watcher uses.

### D2. Hook order in the publish pipeline

Publish flow today (v0.4):
1. HTTP handler validates the message via `from_json`.
2. Sender's agent record is updated.
3. Delivery enqueues to recipient inboxes (per the delivery rules in `message-delivery/spec.md`).

The detector hooks **after delivery completes**. Three reasons:

- A warning emitted in response to an intent must reach an inbox, which means the intent must be persisted/delivered first (recipients of the warning may be the publishers, who registered earlier).
- Hooking before delivery would mean the detector's own `agent.feedback` emit calls re-enter the publish pipeline before the original message has finished — a re-entrancy risk.
- Failures in the detector must not block delivery. Delivery is critical; detection is best-effort.

So: detector subscribes to a "publish completed" channel (or runs as a tail-task on the message log). Each tick: read new messages since last cursor, classify, update tracker, emit warnings.

### D3. Tracker data structures

```rust
struct IntentRecord {
    agent_id: String,
    files: HashSet<String>,           // normalized; one entry per file
    summary: String,
    received_at: Instant,
    valid_for: Duration,              // copied from valid_for_seconds
}

struct InFlightPair {
    a: String,                        // agent_ids ordered lexically
    b: String,
    file: String,                     // the shared file that triggered
    first_seen: Instant,
    escalated: bool,                  // already published agent.question?
}

pub struct ConflictTracker {
    intents: HashMap<String, IntentRecord>,           // by agent_id
    warned_intent_pairs: HashSet<(String, String)>,   // ordered pair → already warned
    in_flight_pairs: HashMap<(String, String, String), InFlightPair>, // (a, b, file)
    warned_violations: HashSet<(String, String)>,    // (violator, file) — already warned
}
```

Ordering pairs lexically (`min(a,b), max(a,b)`) keeps the dedupe sets symmetric.

### D4. Forward-conflict algorithm

On `agent.intent` from agent X:

1. Normalize entries in `payload.files`: trim, deduplicate, drop empties (already enforced by validation, but defense-in-depth).
2. Insert/replace `intents[X]` with a fresh `IntentRecord`.
3. For every *other* `intents[Y]` where TTL has not elapsed:
   - Compute `overlap = X.files ∩ Y.files`.
   - If `overlap` is non-empty AND `(min(X,Y), max(X,Y))` is not in `warned_intent_pairs`:
     - Emit `agent.feedback` to X with errors `["[conflict-detector] forward conflict: agent {Y} also intends to modify {N} of these files: {file_list}", ...]`.
     - Emit `agent.feedback` to Y with the symmetric message.
     - Insert the ordered pair into `warned_intent_pairs`.

If `warn_on_intent_overlap = false`: skip the emission, but still update the tracker (later in-flight / ownership checks may need the data).

### D5. In-flight-conflict algorithm

On `agent.status` from agent X (auto-published by the watcher; carries `modified_files`):

1. Sweep expired intents from the tracker (TTL).
2. Update an in-memory `current_files: HashMap<agent_id, HashSet<String>>` with X's `modified_files` (replacing — `modified_files` is always the full set, not a delta).
3. For every *other* agent Y with non-empty `current_files[Y]`:
   - Compute `overlap = current_files[X] ∩ current_files[Y]`.
   - For each `file` in overlap, ordered pair `(a,b) = (min(X,Y), max(X,Y))`:
     - If `(a, b, file)` not in `in_flight_pairs`: insert with `first_seen = now`, `escalated = false`. Emit `agent.feedback` to both X and Y with `["[conflict-detector] in-flight conflict: file {file} is being modified by both {X} and {Y}"]`.
     - If `(a, b, file)` already in `in_flight_pairs` AND not `escalated` AND `(now - first_seen) ≥ window_seconds`: emit `agent.question` to inbox `"supervisor"` with question text `"In-flight conflict on {file} between {X} and {Y} has not resolved within {window_seconds}s. Human input requested."`. Set `escalated = true`.
4. Cleanup: for any `in_flight_pairs[(a, b, file)]` where `file` no longer in `current_files[a] ∩ current_files[b]` (one of them stopped touching it), remove the entry. The agents have moved past the conflict; don't keep escalating.

### D6. Ownership-violation algorithm

On `agent.status` from agent X with `modified_files`:

1. For each `file` in X's `modified_files`:
   - If `intents[X]` exists and `file ∈ intents[X].files`: this is in-scope for X — no violation.
   - Else if some other `intents[Y]` exists with `file ∈ intents[Y].files` (and TTL not elapsed):
     - This is a violation: X edited a file outside its own intent and inside another agent's intent.
     - If `(X, file)` not in `warned_violations`:
       - Emit `agent.feedback` to X: `["[conflict-detector] ownership violation: you edited {file} but agent {Y} declared intent over it. Update your agent.intent to declare this file or back off."]`.
       - If `escalate_on_violation = true`: also emit `agent.question` to `"supervisor"`: `"Ownership violation: {X} edited {file} which is in {Y}'s intent. Human review requested."`.
       - Insert `(X, file)` into `warned_violations`.

The "X has no intent" case is *not* a violation — agents without active intents are uncoordinated, which is a different problem (and one the skill addresses by telling agents to publish intent before editing). Only files claimed by *some other* agent constitute a violation.

### D7. Auto-emitted message conventions

| Trigger | Emitted message | `agent_id` (recipient) | `payload.from` | Tag prefix |
|---|---|---|---|---|
| Forward conflict | `agent.feedback` × 2 | each publisher | `"supervisor"` | `[conflict-detector]` |
| In-flight conflict (initial) | `agent.feedback` × 2 | each toucher | `"supervisor"` | `[conflict-detector]` |
| In-flight escalation (after window) | `agent.question` × 1 | `"supervisor"` | `"supervisor"` | `[conflict-detector]` |
| Ownership violation | `agent.feedback` × 1 | violator | `"supervisor"` | `[conflict-detector]` |
| Ownership escalation (if config) | `agent.question` × 1 | `"supervisor"` | `"supervisor"` | `[conflict-detector]` |

Using `from: "supervisor"` keeps the dashboard / agent skill mental model: "feedback from supervisor is something to act on." The `[conflict-detector]` prefix in the error text is a convention readable by humans and skim-able by agents; future versions may make it structured (e.g. a sub-tag in the payload), but v0.5.0 keeps it text-only to avoid widening the wire format.

### D8. Configuration shape

```toml
[supervisor]
enabled = true

[supervisor.conflict]
window_seconds = 120
warn_on_intent_overlap = true
escalate_on_violation = true
```

Field choice rationale:
- `window_seconds` over `window_secs` / `escalation_window_seconds` — short, matches existing `last_seen` / `valid_for_seconds` style.
- `warn_on_intent_overlap` is a kill-switch for forward warnings only. In-flight and ownership are independent. (We considered three separate flags — `forward`, `in_flight`, `ownership` — but rejected as overkill for v0.5.0; aggregate flags can be added later if dogfood demands.)
- `escalate_on_violation` toggles the `agent.question` for ownership only. The `agent.feedback` to the violator always fires (silently dropping ownership violations would be worse than no detection).
- No `enabled` flag at the `[supervisor.conflict]` level — `[supervisor] enabled = false` already gates the whole detector. A nested `enabled` would be redundant and confusing (which one wins?).

### D9. Backward compatibility for `SupervisorConfig`

Today's `SupervisorConfig` is flat. Adding a nested `conflict: ConflictConfig` field:

```rust
#[derive(Default, Deserialize, Serialize)]
pub struct SupervisorConfig {
    // existing fields...
    #[serde(default)]
    pub conflict: ConflictConfig,
}

#[derive(Deserialize, Serialize)]
pub struct ConflictConfig {
    pub window_seconds: u64,
    pub warn_on_intent_overlap: bool,
    pub escalate_on_violation: bool,
}

impl Default for ConflictConfig {
    fn default() -> Self {
        Self {
            window_seconds: 120,
            warn_on_intent_overlap: true,
            escalate_on_violation: true,
        }
    }
}
```

The `#[serde(default)]` annotation means a config without `[supervisor.conflict]` loads `ConflictConfig::default()`. Existing v0.4 configs continue to load unchanged.

### D10. Skill update scope

`assets/agent-skills/supervisor.md` line 132-135 currently reads:

> ### Conflict detection
>
> Compare the `modified_files` arrays from every `agent.artifact` event. If two agents
> report overlapping paths, that is a merge conflict waiting to happen — publish
> `agent.feedback` to both agents asking who owns the file, or escalate to the human.

This change replaces it with an advisory section noting that the broker now auto-detects forward / in-flight / ownership conflicts, and the supervisor agent's role is to layer human judgment on the resulting `agent.question` messages (when, e.g., neither agent has resolved an in-flight conflict in time and the human needs to intervene). The supervisor skill SHALL NOT instruct the supervisor agent to also do manual `modified_files` comparison — that path is now redundant and would emit duplicate warnings.

## Risks / Trade-offs

- **[Risk] False-positive warnings on coarse intents.** Two agents both intending `src/lib.rs` (a popular module entry point) would warn even if their actual edits don't collide. → **Mitigation:** the skill teaches "be specific about file paths"; users with chronic false positives can set `warn_on_intent_overlap = false`. Long-term: glob/scope-aware overlap algorithm (post-v0.5.0).
- **[Risk] Tracker memory growth on long sessions.** Stale entries linger in `warned_intent_pairs` and `warned_violations` until broker restart. → **Mitigation:** entries keyed on `agent_id`s. v0.5.0 sessions have ≤ ~20 agents; the dedupe sets stay small. If dogfood shows growth, add periodic GC of pairs whose agents are gone.
- **[Risk] Race between intent publish and watcher status.** An agent publishes intent then immediately starts editing; the first `agent.status` arrives before the broker has finished processing the intent. The ownership check would fire even though the intent declares the file. → **Mitigation:** publish ordering is FIFO per the existing delivery spec, so intent is processed before any subsequent status from the same agent. Cross-agent races (Y's status arrives before X's intent has been processed) are possible but transient — the next status tick re-evaluates and the false-violation warning will not be re-emitted thanks to `warned_violations` dedupe. Net effect: at most one spurious warning per crossing race; acceptable.
- **[Risk] In-flight escalation fires while supervisor is asleep / between polls.** The escalation `agent.question` lands in the supervisor inbox, but the supervisor agent might be slow to read it. → **Mitigation:** that's the supervisor's existing latency, not specific to this change. The escalation message itself is durable in the inbox.
- **[Trade-off] Auto-emitted from `"supervisor"` blurs human vs. machine.** A user reading the dashboard can't distinguish broker-emitted feedback from human-typed supervisor feedback without reading the `[conflict-detector]` tag. → **Mitigation:** the tag is conspicuous; learnings-mode (later in v0.5.0) will attribute differently when it categorizes events.
- **[Trade-off] No glob support in v0.5.0.** Intents like `src/**` won't trigger overlap with literal entries. → Documented; users can list specific files, or wait for the glob-aware version.

## Migration Plan

Additive only — no migration step required.

1. Land `forward-coordination` first (delivers `agent.intent`).
2. Land this change. Existing v0.4 configs load unchanged (defaults applied).
3. With `[supervisor] enabled = true` (existing) and no opt-out, the detector starts on the next `git paw start` invocation.
4. Rollback: revert the change. The tracker subsystem disappears; intents continue to broadcast (via `forward-coordination`); no auto-warnings fire.

User-visible notes for release notes:
- New auto-emitted `agent.feedback` and `agent.question` traffic begins appearing on dashboards once supervisor mode is on.
- Users who previously relied on the v0.4 supervisor-skill manual conflict comparison should update any user-forked supervisor.md to drop the manual section (or accept that they'll see double warnings until they merge upstream).

## Open Questions

- **Should escalation `agent.question` text include a suggested resolution?** E.g. "Suggested: pause {Y} until {X} commits, or split scope." Decision deferred to first dogfood — a generic question is fine in v0.5.0; resolution suggestions can be added once we know what works.
- **Should the detector warn on a single agent re-publishing intent that overlaps its own previous intent?** I.e. agent A publishes intent for `[a.rs]`, then re-publishes for `[a.rs, b.rs]` — does the overlap with the *prior* A intent count? Decision: replace-not-merge. Treating an agent's later intent as authoritative (overwrite) avoids self-conflict warnings. This is also what `intents[X] = ...` (insert/replace) gets us automatically.
- **Should the broker persist `warned_intent_pairs` across restarts?** Decision: no for v0.5.0. Restart is rare and a fresh slate of warnings is fine. Revisit if learnings-mode shows users want continuity across long sessions.
