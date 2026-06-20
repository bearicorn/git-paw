# Learnings Mode

Learnings mode records deterministic friction signals from a supervisor
session into a markdown file you can review after the run. It is an opt-in
v0.5.0 feature: it requires supervisor mode to be active and the
`[supervisor] learnings = true` flag to be set explicitly. The output is
file-only in v0.5.0; a programmatic `agent.learning` broker variant is
deferred to v0.6.0.

## Contents

- [Why](#why)
- [Privacy & Sharing](#privacy--sharing)
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

## Privacy & Sharing

**Learnings mode performs no telemetry.** The aggregator writes to exactly
one place — the local `.git-paw/session-learnings.md` file in your repo. The
broker it observes binds to `127.0.0.1`, and git-paw ships no outbound HTTP
client: nothing about your session is collected, uploaded, or phoned home
under any configuration. The feature is also fully **opt-in** — it runs only
when you set `[supervisor] learnings = true` (default `false`), and a session
that has not opted in behaves exactly as if the feature did not exist.

That makes the learnings file yours: a purely local artifact you can read,
keep, or delete. When a session starts with learnings enabled, git-paw prints
a one-line reminder of where the file lives and that nothing leaves your
machine, so the privacy stance is visible at the point of use, not just here.

**Optional sharing.** If a session surfaces a recurring rough edge that looks
worth fixing in git-paw itself, the most useful thing you can do is share that
context with the maintainers. This is a deliberate, manual action — there is
no "share now" command and no automatic upload. To contribute, open an issue
on the [git-paw issue tracker](https://github.com/bearicorn/git-paw/issues)
and attach (or paste) the relevant part of your `session-learnings.md`.

> **Review before you share.** The file contains repo-specific details —
> branch names, file paths, spec IDs — that you may not want public. Read it
> first and strip or anonymise anything sensitive. git-paw deliberately does
> not scrub the file for you: only you know what is sensitive in your repo.
> Your own LLM or agent CLI can help here — ask it to redact paths and
> identifiers before you paste the result into an issue.

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

## Qualitative signals (v0.6.0)

The five categories above are *deterministic* — the broker derives them
mechanically from message traffic. As of v0.6.0 the supervisor also
records four *qualitative* signals: observations that require reasoning
over the whole session, which only the supervisor LLM can make during its
normal sweep and audit work. They ride the same `agent.learning` wire
format and land in their own sections of `session-learnings.md`.

Because these are LLM judgments, the supervisor skill gates each one
behind a heuristic and a "do not publish unless…" rule, and suppresses
near-duplicates within a session. The aim is a short, high-signal list,
not an exhaustive log — if a category is empty in a run, the supervisor
simply didn't see a confident instance.

### Recurring failure shapes

The same error shape recurring across multiple feedback cycles from
different branches (e.g. three branches all hitting the same import-cycle
pattern). **What to do:** treat it as a systemic signal — a missing lint,
an undocumented constraint, or a fixture/scaffold gap — rather than fixing
each branch in isolation.

### Documentation gaps

A spec audit found a convention the spec assumes but no checked-in doc
explains. The record names the `convention`, the doc paths checked, and a
suggested home for it. **What to do:** add the convention to the suggested
doc (git-paw does not write docs for you — see Non-goals below).

### ADR / architectural drift

Code introduced an architectural decision — a new dependency, framework,
or boundary — not reflected in your configured ADRs. The record names the
`decision_area`, the observed pattern, and a candidate ADR title. **What
to do:** decide whether the drift is intentional (write the ADR) or
accidental (revert or rework it).

### Scope-mistake signals

Two or more branches coordinated heavily because the original spec scope
drew the boundary in the wrong place. The record names the `branches`, the
shared files, and a suggestion. **What to do:** consider re-cutting the
scope for the next change so the same work lands in one branch.

### Unknown categories

Records whose category the aggregator doesn't recognise (e.g. a category
added by a newer supervisor skill) are never dropped — they appear under
an **Other learnings** section so nothing is lost across version skew.

> **Non-goals.** Qualitative signals *flag* issues; they do not fix them.
> git-paw does not auto-generate docs or ADRs, and it does not scan past
> sessions for cross-session patterns — each observation belongs to the
> session that saw it. The detection thresholds live in the supervisor
> skill prose, not in config (see below); if a category is too noisy or
> too quiet for your project, edit your local copy of the supervisor
> skill.

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

### Recurring failure shapes

- import cycle between the auth and session modules: 3 instances
  across feat-auth, feat-api, feat-billing

### Documentation gaps

- agents are expected to run the linter before committing — add a
  Conventions section to AGENTS.md naming the pre-commit lint step

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

## Programmatic consumption: `agent.learning` (v0.6.0)

v0.5.0 shipped learnings as file output only. As of v0.6.0 the aggregator
*also* publishes each flushed entry to the broker as an `agent.learning`
message, so tools no longer have to re-parse the Markdown. The file output is
unchanged — it remains the source of truth for cold-repo / no-broker
scenarios — and the broker publish is purely additive.

Each `agent.learning` record carries:

| Field        | Meaning                                                         |
|--------------|-----------------------------------------------------------------|
| `id`         | Deterministic 16-hex-char dedup hash (see below)                |
| `agent_id`   | Publishing agent (`supervisor` for aggregator-produced records) |
| `branch_id`  | Branch the record is scoped to; omitted for cross-cutting ones  |
| `category`   | `conflict_event`, `stuck_duration`, `recovery_cycles`, `permission_pattern` (open set) |
| `title`      | Short human-readable summary (mirrors the Markdown bullet)      |
| `body`       | Category-specific structured object                             |
| `timestamp`  | ISO 8601 UTC                                                    |

The `id` is a stable 64-bit hash (16 hex chars) over the category, branch,
body (keys sorted), and the UTC *hour bucket*. It is a dedup key, not a
security primitive. Re-publishing the same logical record within one
hour yields the same `id`, so a consumer can dedupe re-emissions; a genuine
recurrence in a later hour gets a fresh `id`. This lets the aggregator re-run
a sweep idempotently without flooding consumers with duplicates.

Branch-scoped records (`stuck_duration`, `recovery_cycles`) are routed to
that branch's inbox and are retrievable with:

```bash
curl -s http://127.0.0.1:9119/messages/<branch-id>
```

Cross-cutting records (`conflict_event`, `permission_pattern`) carry no
`branch_id` and land in the supervisor inbox. Every record is also retained
in the broker's full message log (`/log`).

The MCP `get_learnings()` tool consumes these records when the broker is
running and falls back to parsing the Markdown file when it is not — the same
structured shape either way, with a `source: "broker" | "file"` field telling
you which path produced them.

Publication is controlled by the `broker_publish` knob (default `auto`,
which follows `[broker] enabled`); see
[Configuration → Learnings mode tuning](../configuration/README.md#learnings-mode-tuning).
