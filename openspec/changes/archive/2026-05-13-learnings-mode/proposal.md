## Why

v0.4.0 sessions produce a lot of signal that disappears at session end: how long agents were stuck, how many feedback cycles each task took, where conflicts emerged, what the supervisor escalated. v0.5.0's `conflict-detection` adds even more — forward, in-flight, ownership events that are valuable across sessions for spotting bad spec splits or recurring coordination failure shapes.

This change adds a *learnings* subsystem that observes the session, summarises actionable patterns, and writes them as a human-readable markdown file (`.git-paw/session-learnings.md`). v0.5.0 ships file-only; v0.6.0 (when MCP lands) will add a structured broker variant for on-demand programmatic access. Splitting that way avoids baking a wire format we'd be locked into before there's an actual consumer.

## What Changes

- **Opt-in config flag.** Add `[supervisor] learnings: bool` (default `false`). When `true` and supervisor mode is also active, the learnings subsystem starts. When supervisor mode is off, the flag has no effect (the subsystem requires the supervisor's existence).
- **Broker-internal aggregator.** Add a learnings aggregator subsystem to the broker process, peer to the watcher and the conflict detector. It subscribes to the publish-completed event stream and accumulates per-session counters and event lists keyed by signal type. The aggregator's only output sink in v0.5.0 is the markdown file.
- **Tracked signals (v0.5.0 scope — the deterministic ones).**
  - **Stuck duration** — derived from `agent.blocked` published-time vs. the unblock event (next `agent.artifact` from the same agent that resolves the stated `payload.from` dependency).
  - **Recovery-cycle count** — number of `agent.feedback` events received per agent before the agent's eventual `agent.verified`.
  - **Forward conflicts** — `agent.intent` overlap events from the conflict detector. Distinguish *intra-spec* (both agents from the same `SpecEntry` family) and *cross-spec* (different `SpecEntry` families) using the `agent_id` → `SpecEntry` mapping the session already tracks.
  - **In-flight conflicts** — initial-warning events from the conflict detector.
  - **Ownership violations** — events from the conflict detector.
  - **Permission-prompt patterns** — count of auto-approve hits per command-class label (the existing supervisor-config auto-approve telemetry, when present in v0.4 message logs).
- **Deferred signals (v0.5.0 marks them out-of-scope).** "Recurring failure shapes", "doc gaps", "undocumented architectural patterns", and "scope-mistake signals" require LLM-level reasoning rather than pattern-matching. v0.5.0 ships the deterministic signals only; supervisor-skill-driven qualitative observations (which require an `agent.learning` wire format to publish) are deferred to v0.6.0 alongside the broker variant.
- **No `agent.learning` broker variant in v0.5.0.** Earlier drafts proposed shipping the variant now to make v0.6.0's MCP `get_learnings()` cheap. After scope review, deferred to v0.6.0:
  - v0.5.0's value is the human-readable markdown file. The wire format isn't consumed by anything in v0.5.0.
  - Defining the wire format ahead of its consumer risks baking in fields/categories that turn out wrong once MCP is implemented.
  - The aggregator's internal data model is preserved (so v0.6.0 can serialise it to a broker variant without re-deriving from messages); v0.5.0 just doesn't surface that data on the wire.
- **Markdown output format.** Append-only to `.git-paw/session-learnings.md`. New session = new top-level section dated with the current ISO date. Section structure: H3 per category present in this session (e.g. `### Conflict events`, `### Where agents got stuck`, `### Recovery cycles`, `### Permission patterns`). Empty categories are omitted.
- **Triggering writes.** The aggregator flushes (a) periodically (every 60s, configurable as `[supervisor.learnings] flush_interval_seconds`) to keep the file roughly current during long sessions, and (b) at session end (broker shutdown). Each flush appends new entries since last flush; nothing is rewritten.
- **No supervisor skill update for end-of-session qualitative observations.** Earlier drafts had the supervisor agent publish qualitative learnings via `agent.learning` curl at end-of-session. That's deferred to v0.6.0 along with the variant. v0.5.0's supervisor skill is unchanged with respect to learnings.

Not in scope:
- The `agent.learning` broker variant (deferred to v0.6.0).
- Supervisor-skill-driven qualitative observations (deferred to v0.6.0).
- LLM-level pattern reasoning (recurring failures, doc gaps, ADR drift, scope mistakes — deferred along with the variant since they require it for publication).
- Real-time UI for learnings (dashboard display is a follow-up).
- Cross-session aggregation. v0.5.0 markdown file is per-session.
- Privacy/redaction for learnings content. v0.5.0 logs raw observations including agent IDs and file paths; users with sensitive paths set their existing `.gitignore` patterns to keep the file out of git (it's under `.git-paw/` which is conventionally already gitignored).

## Capabilities

### New Capabilities
- `learnings-mode`: the broker-internal aggregator subsystem, signal-tracking logic, and markdown writer.

### Modified Capabilities
- `supervisor-config`: add `learnings: bool` (default `false`) and a nested `LearningsConfig` with `flush_interval_seconds: u64` (default `60`).

## Impact

**Code**:
- `src/broker/learnings.rs` (or `src/broker/learnings/mod.rs`): aggregator type, signal-trackers, flush logic. Wires into broker startup behind a `[supervisor] learnings = true` check.
- `src/config.rs`: `SupervisorConfig` gains `learnings: bool` and `LearningsConfig`. Both default to disabled / 60s.
- `src/broker/mod.rs`: spawn the aggregator alongside the watcher and conflict detector when supervisor + learnings both enabled.
- `docs/src/user-guide/supervisor.md` (or wherever supervisor docs live): document the markdown file and the deterministic signal categories. Note that v0.6.0 adds programmatic access via MCP.

**Tests**:
- Aggregator unit tests for each deterministic signal: stuck-duration computed from blocked/unblock event pair; recovery-cycle count per agent; intra-vs-cross-spec classification of forward conflicts; in-flight and ownership counts.
- Markdown writer test: empty session produces no output; populated session produces well-formed sections with H3 headings; subsequent flushes append rather than overwriting.
- Integration test: enable learnings + supervisor mode, run a fixture session that triggers each tracked signal, assert the markdown file contains the expected sections.

**Backward compatibility**: fully additive. `learnings: bool` defaults to `false`, so v0.4 configs / v0.5.0 configs that don't opt in produce zero new behaviour. Disabling the broker subsystem entirely produces zero overhead.

**Mismatches surfaced**: none new. The deferred `agent.learning` broker variant moves to v0.6.0's MCP work, where it's consumed directly.
