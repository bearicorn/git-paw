## Context

Nothing in `src/` reads token usage or cost. The pieces that make it possible already exist: each
Claude-family worker is launched with a git-paw-assigned session id (`--session-id <uuid>`, recorded
as `WorktreeEntry.cli_session_id`), each CLI now has a provider record, and the drive loop sweeps on
a fixed cadence. The `[supervisor.correction] on_exhausted` policy (`escalate` | `abandon`) is the
roadmap's chosen response to a breach.

The concrete usage report of any CLI is a third-party fact the code does not encode. For Claude
Code it is expected to be the session transcript under the CLI's configuration directory, keyed by
session id, with per-message token counts — **to verify by the v0.23.0 audit** (location, record
shape, whether a cost figure is present, and how a relocated config directory such as
`CLAUDE_CONFIG_DIR` or a variant's `settings_path` parent changes the location).

## Goals / Non-Goals

**Goals:**

- Per-worker and per-tier token usage, and cost where the CLI itself reports it.
- A cap that stops git-paw from adding spend and surfaces the breach, for unattended runs.
- Honest "unknown" everywhere a figure cannot be attributed.

**Non-Goals:**

- A price table. Shipping per-model prices would go stale and couple git-paw to vendors' pricing;
  cost is shown only when the CLI reports it.
- Stopping a worker mid-turn, killing a pane, or any hard enforcement inside the CLI.
- Attributing the operator's own driving session.
- A dashboard view (the dashboard's future is an open v0.18.0 question).

## Decisions

**D1 — Facet shape.** `CliProvider::usage(&UsageQuery) -> UsageReport` where the query carries the
session id, worktree path and resolved config directory, and the report is `Known { input, output,
cache_read, cache_write, cost: Option }` or `Unknown(reason)`. The provider supplies a pure parser
from report text to figures; a thin adapter locates and reads the report. Default: `Unknown`.

**D2 — Roles.** `worker` (each coding agent; grouped by CLI id, which is how a tier is configured
after `cli-provider-record`) and `orchestrator` (each headless one-shot the drive loop starts; it is
assigned a session id when its provider has a `session_id_args` template, and the loop keeps those
ids for the run). The operator's session is out of git-paw's view; outputs say "excludes your own
session".

**D3 — Where it shows.** `git paw status --json`: a `usage` object (or `null`) per worktree and a
`usage_total` over known figures, additive. The drive loop exit summary: a usage section by role and
by CLI id. Human `git paw status` stays byte-identical, preserving the existing session-state rule.

**D4 — Breach handling reuses `on_exhausted`.** The loop re-reads usage on a slower cadence than the
sweep (every few sweeps; reading reports is file I/O, not a pane capture). On breach it publishes one
`budget-exhausted` escalation review item, then stops initiating spend: no nudges, re-engagement or
correction cycles, and no one-shots. Permission prompts need no change: since v0.17.0
`classifier-retirement` the loop escalates every prompt and answers none, so a worker stops at its
next prompt regardless of the budget. `escalate` keeps the loop running so
completion is still observed; `abandon` exits with outcome `budget-exhausted` and a non-zero status.
Interrupting workers was rejected: it risks losing uncommitted work and needs a per-CLI interrupt
contract nobody has verified. Consequence, documented: a worker that does not reach a prompt
(`full-auto`, or only edits and commands its CLI's own permission mode and allow rules let through)
keeps spending until its current task ends.

**D5 — Unknown is not zero.** At loop start, with a cap set, the loop prints which workers' usage is
unknown and, for `budget_max_cost`, whether any source reports cost. Totals are labelled
"of known usage". A cap is never silently treated as satisfied because figures are missing.

**D6 — Skill step.** The exported orchestrator skill's budget step tells the driving agent to read
`git paw status --json` usage before opening a new wave or dispatching new work, to stop and report
to the operator on breach, and never to stop or kill a worker itself. It names no CLI or vendor.

## Risks / Trade-offs

- **Report format churn** → the parser is per provider and fixture-tested; a parse failure yields
  `Unknown(reason)`, never a wrong number and never a failed command.
- **Stale figures** → reports are read when asked; a mid-turn worker's latest message may be missing.
  Acceptable for a guardrail that acts between turns.
- **Privacy** → transcripts contain conversation text. The adapter reads only the usage fields it
  needs and never logs or stores the rest.
- **Exit summary edited twice** → v0.18.0 `supervisor-pane-removal` also modifies it; this change
  lands later and rebases its delta onto that text (both add independent lines).
