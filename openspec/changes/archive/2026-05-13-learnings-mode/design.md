## Context

The broker process now hosts three long-running subsystems: the watcher (auto-publishes `agent.status` from filesystem events), the conflict detector (auto-publishes `agent.feedback`/`agent.question` from intent/status overlap analysis), and the message delivery pipeline. The conflict detector pattern is the closest precedent for `learnings-mode`: a broker-internal task that subscribes to publish events and maintains in-memory state.

The learnings aggregator is functionally similar but with two key differences:
- **No broker output.** v0.5.0's aggregator only writes to a markdown file. Earlier drafts had it also publish `agent.learning` broker messages so v0.6.0's MCP server could query them; that wire format is deferred to v0.6.0 along with the consumer. Defining the variant before MCP exists risks baking in fields/categories that turn out wrong.
- **Some signals require historical context.** Stuck duration = block-time minus unblock-time. The aggregator carries longer-lived per-agent state than the conflict detector's pair-keyed dedupe sets.

The MILESTONE description originally listed 11 signal categories. After scope review:
- 7 are deterministic (computable from event-stream patterns) — shipped in v0.5.0.
- 4 are qualitative (LLM-level interpretation, requiring publication via the deferred broker variant) — deferred to v0.6.0.

v0.5.0 ships only the deterministic 7 to the markdown file.

## Goals / Non-Goals

**Goals:**
- Run the aggregator entirely inside the broker process, peer to watcher / conflict-detector. Same lifecycle (start when supervisor + learnings both enabled; exit cleanly on broker shutdown).
- Compute the 7 deterministic signal categories purely from event streams the broker already has (publish events, conflict-detector events). No new event sources.
- Produce one output: the markdown file (`.git-paw/session-learnings.md`).
- Append-only markdown: each session's learnings are added at the bottom of the file under a dated H2; never rewrite or shuffle prior content.
- Cleanly skip emission when the broker has no observed events for a signal (no empty sections, no spurious "0 conflicts" rows).
- Design the aggregator's internal data model so v0.6.0 can serialise it to a broker variant without re-deriving from messages.

**Non-Goals:**
- An `agent.learning` broker variant. Deferred to v0.6.0.
- Implementing the 4 qualitative signals (recurring failure shapes, doc gaps, ADR drift, scope mistakes). These require LLM reasoning + a publish path; deferred.
- Supervisor skill changes for end-of-session qualitative observations. Deferred.
- Cross-session aggregation. v0.5.0 markdown is one section per session.
- Privacy / redaction.
- Real-time learnings UI.
- Persistent learnings across broker restarts within a session.

## Decisions

### D1. Subsystem placement: broker-internal, peer to conflict-detector

Same architecture as the conflict detector — a long-running task in the broker process subscribed to the publish-completed event stream. Direct access to the broker's `Arc<State>`. Lifecycle keyed on `[supervisor] enabled = true` AND `[supervisor] learnings = true`.

### D2. Per-signal tracker data structures

```rust
pub struct LearningsAggregator {
    pending_blocks: HashMap<String, (Instant, String)>,         // stuck-duration
    feedback_counts: HashMap<String, u32>,                      // recovery-cycles
    conflict_events: Vec<ConflictEvent>,                        // structured event list
    permission_counts: HashMap<String, u64>,                    // command-class -> count
    last_flushed_md_idx: usize,                                 // markdown writer cursor
    session_start: Instant,
}
```

The aggregator owns one instance behind `Arc<Mutex<_>>`. All input methods (`record_*`) take the lock, append/update, release.

The data shapes are designed for future serialisation: each `ConflictEvent`, stuck-duration tuple, recovery-cycle entry, and permission-count entry has the structured fields v0.6.0's `agent.learning` payload would carry. v0.5.0 just doesn't put them on the wire — only into the markdown.

### D3. Stuck-duration algorithm

On `agent.blocked` from agent X with `payload.from = Y`: insert `pending_blocks[X] = (now, Y)`.

On X's next `agent.artifact`: compute duration, mark resolved, clear the entry, accumulate into the markdown's "Where agents got stuck" bucket. Conservative interpretation: any next artifact resolves the block; if the agent gave up and did unrelated work, the duration is high-but-uninformative — surfaced as a learning signal in itself.

Session end with open block → unresolved entry with duration up to shutdown.

### D4. Recovery-cycle algorithm

For each agent X, count `agent.feedback` messages addressed to X (`Feedback.agent_id = X`) between successive `agent.artifact` events from X. The count is recorded in the markdown when X is verified or at session end. Zero-count agents produce no entry.

### D5. Conflict event source: re-use conflict-detector outputs

Subscribe to `agent.feedback` and `agent.question` messages whose error/question text begins with the `[conflict-detector]` tag. Classify into one of: `forward-conflict-intra-spec`, `forward-conflict-cross-spec`, `in-flight-conflict`, `ownership-violation`. Intra-vs-cross-spec uses the agent → `SpecEntry` mapping the broker session tracks at event time.

### D6. Permission-pattern tracker

Subscribe to `agent.status` messages tagged `auto_approved`. Increment per-command-class counters. At each flush, classes with `count >= threshold` (default 5) produce one entry; lower-count classes are silently held in the counter for next flush.

### D7. Markdown file format

```markdown
## Session Learnings — 2026-04-22T14:35:09Z

### Conflict events
- forward-conflict-cross-spec: task/T010 (spec 003-user-list) and
  task/T002 (spec 004-error-handling) both intended `src/main.rs`.

### Where agents got stuck
- task/T002: blocked 11m12s waiting for task/T001 to expose AuthClient.

### Recovery cycles
- task/T015 needed 4 feedback cycles before verifying.

### Permission patterns
- `cargo check` auto-approved 23 times across 4 agents.
```

Format choices:
- ISO timestamp in the H2 so multiple same-day sessions don't collide.
- One H3 per signal category present *in this session*. Empty categories are omitted.
- Bullet list per H3.
- Append-only: prior session content unchanged across runs.

### D8. Flush triggers

- **Timer-based flush**: every `flush_interval_seconds` (default 60s, configurable).
- **Shutdown flush**: when the broker stops, one final flush.
- **Burst flush after detector events**: NOT eager. Bursts batch into the next periodic / shutdown flush.

### D9. No broker variant; no supervisor skill update

v0.5.0 ships file-only. The aggregator does NOT publish `agent.learning` (or any other variant) on the wire. The supervisor skill is NOT modified by this change.

The reason: v0.5.0's value is the markdown. The wire format is consumed by v0.6.0's MCP server, which doesn't exist yet. Defining the variant ahead of its consumer risks shape mismatches when MCP is built. The aggregator's internal data model is preserved (D2) so v0.6.0 can wrap it in a variant without re-deriving from messages.

The 4 qualitative signals (recurring failures, doc gaps, ADR drift, scope mistakes) that previously relied on supervisor-skill-driven `agent.learning` publishing are deferred along with the variant. v0.6.0 picks them up.

### D10. Configuration

```toml
[supervisor]
enabled = true
learnings = true                       # opt-in

[supervisor.learnings_config]
flush_interval_seconds = 60            # default; tuning knob
```

`learnings = false` (or absent) → aggregator does not start. `[supervisor.learnings_config]` is fully optional.

## Risks / Trade-offs

- **[Risk] Stuck-duration false positives.** "Agent gave up and did other work" looks like "agent unblocked." → Surfaces as high-but-uninformative duration; human reading the markdown makes the call.
- **[Risk] Markdown file bloat over many sessions.** Append-only growth → typical sessions add a few KB; users can prune manually.
- **[Trade-off] Deferring the broker variant means v0.6.0 has to re-decide the wire format.** Acceptable: that decision is better made *with* the MCP consumer in scope.
- **[Trade-off] No qualitative signals in v0.5.0.** The most "interesting" signals (recurring patterns, doc gaps) require LLM analysis + a publish path. Deferring keeps v0.5.0 shippable with deterministic value while the qualitative path matures alongside MCP.

## Migration Plan

Additive. No migration step.

1. Land `forward-coordination` (provides `agent.intent`) and `conflict-detection` (provides the conflict events the aggregator depends on) before this change.
2. Land this change. Existing v0.4 / early-v0.5 sessions don't opt in; zero behaviour change.
3. Users who want learnings: add `[supervisor] learnings = true` to `.git-paw/config.toml`. New sessions populate `.git-paw/session-learnings.md`.
4. Rollback: revert. The aggregator subsystem disappears; `.git-paw/session-learnings.md` is left in place (already-written content is harmless).

Release-notes call-outs:
- New opt-in `[supervisor] learnings = true` config field.
- New file `.git-paw/session-learnings.md` (gitignored by convention; users with custom `.gitignore` should ensure `.git-paw/` is covered).
- Programmatic access (broker variant + MCP query) is a v0.6.0 follow-up.

## Open Questions

- **Should the markdown file path be configurable?** Decision: not in v0.5.0. Hard-coded to `.git-paw/session-learnings.md`. Configurability is dogfood-driven.
- **Should a session with zero learnings still produce a section in the markdown?** Decision: no. Empty sessions write nothing.
- **What's the v0.6.0 wire format for `agent.learning`?** Open — to be decided alongside MCP. The aggregator's internal data model (D2) gives v0.6.0 a starting point.
