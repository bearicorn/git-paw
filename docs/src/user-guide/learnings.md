# Learnings Mode

Learnings mode records deterministic friction signals from a supervisor
session into a markdown file you can review after the run. It is an opt-in
v0.5.0 feature: it requires supervisor mode to be active and the
`[supervisor] learnings = true` flag to be set explicitly. The output is
file-only in v0.5.0; a programmatic `agent.learning` broker variant is
deferred to v0.6.0.

## Contents

- [Why](#why)
- [Enabling Learnings Mode](#enabling-learnings-mode)
- [Output File](#output-file)
- [The Five Categories](#the-five-categories)
- [Sample Output](#sample-output)
- [Flush Cadence](#flush-cadence)
- [Roadmap: `agent.learning` (v0.6.0)](#roadmap-agentlearning-v060)

## Why

Supervisor runs absorb a lot of recurring friction silently: sandbox
warnings the agent retries past, approvals you reflexively click, brief
stuck states, conflicts caught and resolved before they hit your eyes.
Each event is too small to interrupt for, but the *pattern* is the most
useful signal git-paw can surface for tool, prompt, or process improvement.

Learnings mode aggregates those events into five deterministic categories
and writes them to a markdown file you can read between sessions —
turning silent friction into something you can act on.

## Enabling Learnings Mode

Set the master switch on the `[supervisor]` table (default `false`):

```toml
[supervisor]
enabled = true
learnings = true
```

The subsystem only activates when supervisor mode itself is active. With
`[supervisor] enabled = false` (or no `[supervisor]` section), the
`learnings = true` value is parsed and ignored — no aggregation runs and
no file is written.

## Output File

When active, the learnings subsystem writes to:

```
.git-paw/session-learnings.md
```

The file lives at the repository root (next to `.git-paw/broker.log` and
the session state). It is:

- **Append-only across sessions.** Subsequent supervisor runs add new
  entries below previous ones; nothing is overwritten or pruned.
- **Human-readable markdown.** No JSON or binary; you can `tail`, `grep`,
  or open it in any editor between sessions.
- **Not committed by default.** `git paw init` adds it to the project's
  `.gitignore` alongside the other `.git-paw/` runtime files.

## The Five Categories

v0.5.0 tracks five deterministic categories. Each entry includes a
timestamp, the agent involved (when applicable), and a one-line summary.

### 1. Stuck duration

An agent's `last_seen` exceeds the configured stall threshold without
producing a new `agent.status`, `agent.artifact`, or `agent.intent`
message. Records *how long* the agent was stuck and *what* the supervisor
or auto-approver did to recover it (sweep, pane capture, no-op).

Trigger condition: `(now - agent.last_seen) > stall_threshold` AND the
agent's most recent status is non-terminal (`done`, `verified`, `blocked`,
and `committed` are excluded; those are intentional resting states).

### 2. Recovery-cycle count

How many auto-approve sweeps (or supervisor-driven `tmux send-keys`
recovery actions) were needed before the agent published a fresh status
message. A high count for a single agent across a run usually means the
auto-approve allowlist is missing a prefix that agent's CLI keeps tripping
on.

Trigger condition: incremented once per sweep dispatch against a pane;
flushed when the agent finally publishes a non-stale message.

### 3. Forward conflicts

Two agents declared `agent.intent` payloads with overlapping `files`
before either committed. Records the agent pair and the overlapping paths.
Use this category to spot specs that were decomposed too coarsely (two
parallel agents both expected to own the same file).

Trigger condition: `[supervisor.conflict] warn_on_intent_overlap = true`
AND the broker conflict detector emitted `[conflict-detector]`-tagged
`agent.feedback` for the overlap.

### 4. In-flight conflicts

Two agents have overlapping `modified_files` in active `agent.status` or
`agent.artifact` payloads — the second agent committed (or is about to)
while the first still considers the path active. Records the agent pair,
the overlapping paths, and whether the conflict resolved within
`[supervisor.conflict] window_seconds` or escalated to the supervisor
inbox via `agent.question`.

Trigger condition: any in-flight overlap (forward intent already missing
or expired). Escalation is recorded separately when
`window_seconds` elapses without resolution.

### 5. Ownership violations

An agent's `modified_files` includes a path the spec marks as owned by
another change. Records the violating agent, the touched path, and the
owning change ID. Use this category to spot agents that drifted out of
their declared scope — usually a sign the spec body in `AGENTS.md` did
not make the ownership boundary obvious enough.

Trigger condition: ownership match against the OpenSpec / Markdown /
Spec Kit ownership declaration parsed at session start.

## Sample Output

A short illustrative excerpt (timestamps abbreviated for readability):

```markdown
# Session learnings — paw-myproject

## 2026-05-13 14:30 — supervisor run start

### Stuck duration

- 14:32:18 — feat-auth — stuck for 42s after permission prompt;
  recovered by auto-approve sweep (`cargo test` allowlist hit).
- 14:38:51 — feat-api — stuck for 118s on rebase conflict prompt;
  recovered manually after supervisor `tmux send-keys`.

### Recovery-cycle count

- feat-auth — 3 sweeps before fresh status.
- feat-api — 7 sweeps; investigate auto-approve prefixes.

### Forward conflicts

- feat-auth ↔ feat-api — overlap on `src/auth/middleware.rs`
  (warned via agent.feedback; both retracted before commit).

### In-flight conflicts

- feat-api ↔ feat-billing — overlap on `src/router.rs`;
  resolved by feat-billing pause within window_seconds.

### Ownership violations

- feat-api modified `src/auth/jwt.rs` (owned by add-auth);
  blocked at agent.feedback; escalated to supervisor inbox.

## 2026-05-13 16:05 — supervisor run start
...
```

The exact section ordering and bullet shape may evolve across patch
releases; the category set and the underlying triggers above are stable
for the v0.5.0 release.

## Flush Cadence

The aggregator buffers entries in memory and flushes to disk on an
interval (default 60s):

```toml
[supervisor.learnings_config]
flush_interval_seconds = 60
```

A shorter interval makes the file fresher (useful when you want to
`tail -f` the file in another terminal); a longer interval batches more
entries per write. The file is also flushed at supervisor shutdown so
nothing is lost between sessions even if the interval has not elapsed.

See [Configuration → Learnings mode tuning](../configuration/README.md#learnings-mode-tuning)
for the field reference.

## Roadmap: `agent.learning` (v0.6.0)

v0.5.0 ships learnings as file output only. The `agent.learning` broker
variant — a wire-format message agents and tools can publish and consume
via the broker — is intentionally deferred to v0.6.0 alongside MCP-mediated
inbox access. Until then, downstream consumers should parse the markdown
file rather than poll the broker.
