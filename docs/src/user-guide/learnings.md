# Learnings Mode

Learnings mode records deterministic friction signals from a supervisor
session into a markdown file you can review after the run. It is an opt-in
feature: it requires supervisor mode to be active and the
`[supervisor] learnings = true` flag to be set explicitly. The markdown file
is always written; when the broker is enabled, each record is additionally
published as an `agent.learning` broker message (see
[Programmatic consumption](#programmatic-consumption-agentlearning-v060)).

## Contents

- [Why](#why)
- [Privacy & Sharing](#privacy--sharing)
- [Enabling Learnings Mode](#enabling-learnings-mode)
- [Output File](#output-file)
- [The Deterministic Categories](#the-deterministic-categories)
- [Qualitative signals (v0.6.0)](#qualitative-signals-v060)
- [Sample Output](#sample-output)
- [Flush Cadence](#flush-cadence)
- [Programmatic consumption: `agent.learning` (v0.6.0)](#programmatic-consumption-agentlearning-v060)

## Why

Supervisor runs absorb a lot of recurring friction silently: sandbox
warnings the agent retries past, approvals you reflexively click, brief
stuck states, conflicts caught and resolved before they hit your eyes.
Each event is too small to interrupt for, but the *pattern* is the most
useful signal git-paw can surface for tool, prompt, or process improvement.

Learnings mode aggregates those events into four deterministic categories
(plus the supervisor's qualitative signals) and writes them to a markdown
file you can read between sessions —
turning silent friction into something you can act on.

## Privacy & Sharing

**Learnings mode performs no telemetry.** The aggregator writes to exactly
one place — the local `.git-paw/session-learnings.md` file in your repo. The
broker it observes binds to `127.0.0.1` by default, and git-paw ships no outbound HTTP
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

The file lives in the repository root's `.git-paw/` directory. It is:

- **Append-only across sessions.** Each session's first flush adds a
  `## Session Learnings — <ISO-8601 UTC start time>` heading, and later
  flushes and later sessions append below it; nothing is overwritten or
  pruned.
- **Human-readable markdown.** No JSON or binary; you can `tail`, `grep`,
  or open it in any editor between sessions.
- **Not committed by default.** `git paw init` adds it to the project's
  `.gitignore` alongside the other `.git-paw/` runtime files.

## The Deterministic Categories

The aggregator tracks four deterministic categories, each written under its
own `###` heading in the session's block. Each bullet is a one-line summary;
a category with nothing to report is omitted from that flush.

### Conflict events

One bullet per conflict the [broker conflict detector](conflict-detection.md)
reported via a `[conflict-detector]`-tagged `agent.feedback` or
`agent.question`, classified as:

- **Forward conflicts** — two agents' `agent.intent` declarations overlap
  (recorded with a `forward-conflict-…` label). Use these to spot specs that
  were decomposed too coarsely (two parallel agents both expected to own the
  same file). A forward conflict is labelled `forward-conflict-intra-spec`
  when both agents were launched from the same spec and
  `forward-conflict-cross-spec` when they came from different ones, using the
  spec each agent's worktree recorded at launch. An agent started without a
  spec (a bare `git paw start`, or `git paw add` without `--from-spec`) has no
  recorded spec, so its forward conflicts classify cross-spec; only the specs
  that are known are named in the bullet.
- **In-flight conflicts** — two agents are modifying the same file at once.
- **Ownership violations** — an agent edited a file claimed by another
  agent's active `agent.intent`. Records the violator, the owner, and the
  file. Use these to spot agents that drifted out of their declared scope.

The detector's symmetric warnings (one per agent in the pair) collapse into a
single bullet.

### Where agents got stuck

Stuck duration: the time between an agent's `agent.blocked` (naming the agent
it waits on in `from`) and its next `agent.artifact`. A block still open when
the session ends is recorded as unresolved, with the duration measured up to
shutdown.

### Recovery cycles

The recovery-cycle count: how many `agent.feedback` messages an agent received
before its `agent.verified` (or before session end, if it never verified). A
high count marks an agent that needed many correction rounds. Zero cycles is
not recorded.

Conflict acknowledgements are excluded from the count. A `[conflict-ack]`
feedback (see [Conflict Detection](conflict-detection.md#in-flight-conflict))
tells an agent that an overlap it was warned about has been adjudicated — it
asks the agent for nothing, so counting it would report correction rounds
that never happened.

### Permission patterns

How often auto-approve cleared permission prompts for a given matched
allowlist entry (each auto-approval publishes an `agent.status` with status
`auto_approved`). An entry appears only once it reaches 5 hits, so the list
shows which commands your agents trip on most.

## Qualitative signals (v0.6.0)

The four categories above are *deterministic* — the broker derives them
mechanically from message traffic. As of v0.6.0 the supervisor also
records *qualitative* signals: observations that require reasoning
over the whole session, which only the supervisor LLM can make during its
normal sweep and audit work. They ride the same `agent.learning` wire
format and land in their own sections of `session-learnings.md`. The
supervisor publishes them through the bundled
`.git-paw/scripts/sweep.sh learn <category> <title> <body-json>` helper — a
least-privilege, by-path publish path, never a raw curl.

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

### Tooling friction

Friction the supervisor absorbed about **git-paw itself** — a tool behaviour
that made it repeat work or work around the tool (a permission prompt cleared
on every sweep, a helper too narrow so it fell back to raw curl, a detector
that over-escalated) — as distinct from the four project-scoped categories
above. The record names the `friction`, the `occurrences` (how many times it
was absorbed this session), and a `suggestion`. The supervisor publishes it
only when the same friction was absorbed **at least twice in the session**, so
a one-off is never recorded. **What to do:** treat it as the prime
tool-improvement signal — fold it into a git-paw issue or a config/allowlist
change.

### Correction-budget exhaustion

Emitted by the `--unattended` drive loop (not by the supervisor skill) when a
branch spends its whole
[`[supervisor.correction] max_cycles`](supervisor.md#unattended-correction-loop)
budget without converging on a passing gate. The record carries the `branch`,
the worker `cli`, and the `cycles` spent, under category
`correction_exhausted`. **What to do:** read it as a tiered-model tuning
signal — a branch that burns its whole budget is usually a task whose worker
model is under-powered for that class of work, so re-assign it to a stronger
CLI or split the spec. Like every learning it is emitted only when
`[supervisor] learnings = true`.

### Unknown categories

Records whose category the aggregator doesn't recognise (e.g. a category
added by a newer supervisor skill) are never dropped — they appear under
an **Other learnings** section so nothing is lost across version skew. The
`correction_exhausted` category above renders there too, as a title plus its
JSON body.

> **Non-goals.** Qualitative signals *flag* issues; they do not fix them.
> git-paw does not auto-generate docs or ADRs, and it does not scan past
> sessions for cross-session patterns — each observation belongs to the
> session that saw it. The detection thresholds live in the supervisor
> skill prose, not in config (see below); if a category is too noisy or
> too quiet for your project, edit your local copy of the supervisor
> skill.

## Sample Output

A short illustrative excerpt (agent names, paths, and counts are made up;
headings and bullet shapes are the ones the aggregator writes):

```markdown
## Session Learnings — 2026-05-13T14:30:05Z

### Conflict events
- forward-conflict-cross-spec: feat-api and feat-auth
- in-flight-conflict: feat-api and feat-billing
- ownership-violation: feat-api edited `src/auth/jwt.rs` owned by feat-auth

### Where agents got stuck
- feat-api: blocked 11m12s waiting on feat-auth
- feat-billing: blocked 4m03s waiting on feat-api (unresolved at session end)

### Recovery cycles
- feat-auth: 3 feedback cycles before verifying

### Permission patterns
- `cargo test` auto-approved 23 times

### Recurring failure shapes
- import cycle between the auth and session modules: 3 instances across feat-auth, feat-api, feat-billing

### Documentation gaps
- agents run the linter before committing — add a Conventions section to AGENTS.md naming the pre-commit lint step
## Session Learnings — 2026-05-13T16:05:41Z
...
```

The exact bullet wording may evolve across releases; the category set and
the underlying triggers above are stable.

## Flush Cadence

The aggregator buffers entries in memory and flushes to disk on an
interval (default 60s):

```toml
[supervisor.learnings_config]
flush_interval_seconds = 60
```

A shorter interval makes the file fresher (useful when you want to
`tail -f` the file in another terminal); a longer interval batches more
entries per write. The file is also flushed once more at broker shutdown so
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

The `id` is a 64-bit FNV-1a hash (16 lowercase hex characters) of the
canonical form (category, branch, body sorted, UTC *hour bucket*).
FNV-1a is stable across Rust toolchain versions — each `id` is deterministic
regardless of when or where it is computed. It is a dedup key, not a security
primitive. Re-publishing the same logical record within one hour yields the
same `id`, so a consumer can dedupe re-emissions; a genuine recurrence in a
later hour gets a fresh `id`. This lets the aggregator re-run a sweep
idempotently without flooding consumers with duplicates.

**The bundled `sweep.sh learn` helper uses the same scheme.** The supervisor
publishes qualitative learnings through
`.git-paw/scripts/sweep.sh learn <category> <title> <body-json>`, and that
helper computes its `id` with exactly the form above — over the category, an
empty branch id (supervisor learnings are cross-cutting), the canonical body,
and the current UTC hour bucket. The two publishers therefore assign the same
id to the same logical record, and a consumer can dedupe across both. The
title is NOT part of the canonical form, so rephrasing it does not mint a new
record.

Because the canonical form has to render byte-identically on both sides, the
helper accepts only numbers it can reproduce exactly: **64-bit integers**. A
body carrying a non-integer number (`0.5`, `1e16`) or an integer beyond the
64-bit range is refused with a diagnostic and nothing is published — the two
serialisers disagree on those (`1e+16` against `1e16`), and a record published
under an id the aggregator cannot reproduce would silently defeat the dedup.
Learning bodies carry counts and labels, so this costs nothing in practice.

**Note:** record ids changed once in this release (v0.17.0): the aggregator's
hash moved from platform-dependent `DefaultHasher` to stable FNV-1a, and
`sweep.sh learn` — which had been computing a truncated SHA-1 over a different
canonical form, giving the same record two different ids depending on which
publisher emitted it — moved onto the shared scheme. Existing ids from v0.16.x
are now invalid; re-run your aggregator to regenerate them.

Branch-scoped records (`stuck_duration`, `recovery_cycles`) are routed to
that branch's inbox and are retrievable with:

```bash
curl -s http://127.0.0.1:9119/messages/<branch-id>
```

Cross-cutting records (`conflict_event`, `permission_pattern`) carry no
`branch_id` and land in the supervisor inbox. Every record is also retained
in the broker's full message log (`/log`).

The MCP `get_learnings()` tool reads the Markdown file, not the broker: it
returns each `###` section as a `{ category, entries }` record, and returns the
four deterministic sections with empty `entries` when no file exists yet.

Publication is controlled by the `broker_publish` knob (default `auto`,
which follows `[broker] enabled`); see
[Configuration → Learnings mode tuning](../configuration/README.md#learnings-mode-tuning).
