## Context

The `qualitative-learnings` capability (v0.6.0) already ships the hard parts:
the `agent.learning` open-enum broker variant ([[agent-learning-variant]]),
four qualitative categories with documented body shapes and publish
heuristics, within-session dedup discipline, and a file renderer that writes
per-category sections into `.git-paw/session-learnings.md` (with an
"Other learnings" fallback that prevents drops). The broker-internal
aggregator (`src/broker/learnings.rs`) flushes both the deterministic
(telemetry-derived) and the qualitative (publisher-emitted) records.

What is missing is the *operational path* that makes the supervisor actually
emit qualitative learnings during a run — verified against current code:

- `assets/scripts/sweep.sh` exposes `status-publish`, `verified`, and
  `feedback-gate`, but **no `learn` subcommand**. The supervisor skill's
  publish example is a raw `curl …/publish` (the G4 anti-pattern), which
  needs a broad curl allowlist and is error-prone.
- The qualitative-learnings guidance is a standalone section in
  `supervisor.md`; the continuous sweep loop (§1.5/§2) never references it.
- There is no wind-down synthesis step.
- The four categories cover the *user's project*; none cover friction with
  *git-paw itself* — the prime tool-improvement signal (see
  `feedback_learnings_mode_signals`).

This change is MCP-independent and adds no broker wire-format change.

## Goals / Non-Goals

**Goals:**
- A least-privilege, by-path helper subcommand for publishing learnings
  (`sweep.sh learn`), eliminating the raw-curl publish path.
- A fifth `tooling_friction` category capturing git-paw-self friction, with a
  heuristic gate and a renderer section.
- Two operationally-wired capture moments in the supervisor skill:
  opportunistic (mid-sweep) and a session-end synthesis pass.
- Preserve all existing qualitative + deterministic behaviour byte-for-byte.

**Non-Goals:**
- No new broker message type (reuse `agent.learning`).
- No change to the deterministic aggregator signals (stuck/recovery/conflict/
  permission) or their rendered sections.
- No telemetry / no network egress beyond the existing local broker (the
  no-telemetry stance from v0.7.0 is unchanged; learnings stay local).
- No automatic LLM publishing without a heuristic gate — a noisy qualitative
  signal is worse than none.

## Decisions

### D1 — Publish via `sweep.sh learn`, not raw curl

`sweep.sh learn <category> <title> <body-json>` reuses the script's existing
`publish()` helper + broker-URL discovery (`.git-paw/config.toml [broker]`,
default `127.0.0.1:9119`) to POST an `agent.learning` with
`agent_id = "supervisor"`. The body-json argument is passed through verbatim
(the skill is responsible for the documented per-category body shape).

*Why over alternatives:* a raw-curl example (status quo) forces a broad curl
allowlist and re-introduces the G4 leak. A new `git paw learn` subcommand was
rejected for the same reason the broker helper is a script not a subcommand
(a human might run it and hit errors; the script is the agent-facing surface).
Keeping it on `sweep.sh` (rather than `broker.sh`) co-locates it with the
supervisor's other publish verbs (`status-publish`/`verified`/`feedback-gate`)
which are also supervisor-role helpers.

### D2 — `tooling_friction` is a fifth open-enum category, not a schema change

`agent-learning-variant`'s category is an open string enum; the renderer's
"Other learnings" fallback already prevents drops for unknown categories. So
`tooling_friction` needs only: a documented body shape, a heuristic gate in
the skill, and a dedicated renderer section. Body shape:
`{ "friction": "<what git-paw made me do>", "occurrences": <n>,
"suggestion": "<proposed tool change>" }`. Primary dedup identifier:
`friction`.

*Why over alternatives:* reusing `doc_gap` was rejected — `doc_gap` is
explicitly about the *user's project conventions* missing from `[governance]`
docs, semantically distinct from "git-paw the tool made me repeat work."

### D3 — Heuristic gate mirrors the existing four (explicit "do not publish unless…")

`tooling_friction` publishes only when the same friction was absorbed **at
least twice in the session** (e.g. the same prompt cleared on ≥2 sweeps, or
the same helper-gap worked around ≥2 times). One-off friction is not
publish-worthy — consistent with the existing "no speculative records" gate
and the absence of a `confidence` field.

### D4 — Two capture moments, both routed through D1

- *Opportunistic* (continuous sweep §2): a new loop step — "if this sweep
  observed or absorbed friction matching a category gate, publish via
  `sweep.sh learn` (consult in-session records first for dedup)."
- *Session-end synthesis* (wind-down): a reflective pass over the run that
  publishes the durable learnings not already captured, deduped against
  in-session records by each category's primary identifier.

*Why both:* opportunistic catches perishable specifics (exact prompt, which
pane) that are gone by wind-down; synthesis catches patterns that only emerge
across the whole run. (User decision, 2026-06-29.)

## Risks / Trade-offs

- **Noisy qualitative signal floods the file** → the per-category heuristic
  gates + in-session dedup + the ≥2-occurrence `tooling_friction` gate keep
  volume low; the renderer already dedups by deterministic `id`.
- **Opportunistic capture distracts the sweep from approvals/stuck-detection**
  → capture is a *terminal, low-priority* step of the loop iteration (after
  approve/detect-stuck), one short publish, never blocking.
- **`sweep.sh learn` mis-targets pane 0 / sends keys** → it does not send keys
  at all; it only POSTs to the broker, so the W15-13 "never send-keys to pane
  0" constraint is not in play here.
- **Allowlist regression** → if `sweep.sh` is granted by exact argv rather than
  path, a new subcommand re-prompts. Verify the existing grant is path-based
  (`bash .git-paw/scripts/sweep.sh *`) and add a test.

## Migration Plan

Additive and backward-compatible. Existing sessions that never call
`sweep.sh learn` behave identically; the renderer change only adds a new
section that appears when a `tooling_friction` record exists. No config
migration. Rollback = revert the skill/script/renderer edits; the
`agent.learning` variant is untouched so no data is stranded.

## Open Questions

- Should the session-end synthesis also roll up the *deterministic* signals
  (e.g. "3 agents averaged 5 recovery cycles") into a one-line qualitative
  summary, or stay strictly qualitative? Leaning strictly qualitative for
  v0.9.0 (the deterministic sections already render those counts).
- Does `tooling_friction` warrant surfacing in the dashboard broker-log, or is
  the file the only sink? Default: file only (matches the other qualitative
  categories); revisit if dogfood shows it's missed.
