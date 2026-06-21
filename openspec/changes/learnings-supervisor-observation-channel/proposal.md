## Why

The `qualitative-learnings` capability (v0.6.0) defined four `agent.learning`
categories, publish heuristics, dedup discipline, and a file renderer — but in
practice the supervisor never captures qualitative learnings during a run.
Four concrete gaps:

1. **Publishing is hand-rolled raw curl.** The supervisor skill publishes
   `agent.learning` via a raw `curl -X POST …/publish -d '{…}'` example
   (`supervisor.md`). This is the G4 anti-pattern from the v0.8.0 dogfood:
   it forces a broad curl allowlist and is error-prone. The bundled
   `sweep.sh` has `status-publish`, `verified`, and `feedback-gate` — but
   **no `learn` subcommand**, so there is no least-privilege path to publish a
   learning.
2. **The heuristics are never invoked operationally.** The qualitative-learnings
   guidance lives in a standalone skill section that the continuous sweep loop
   (`supervisor.md` §1.5/§2) never references — the loop snapshots, classifies,
   approves, and detects-stuck, but has no "consider recording a learning" step.
3. **There is no session-end synthesis moment.** The existing heuristics are
   purely event-triggered; nothing prompts the supervisor to reflect over the
   whole run at wind-down and record the durable learnings.
4. **No category captures friction with git-paw *itself*.** The four existing
   categories describe friction in the *user's project* (failure shapes, doc
   gaps, ADR drift, scope mistakes). None capture the prime tool-improvement
   signal: friction the supervisor *absorbs* about git-paw the tool (a prompt
   cleared every cycle, a helper too narrow so raw curl leaks back, a detector
   that over-escalates). Today that signal only surfaces when a human
   hand-writes a findings file.

This matters most for v0.9.0 unattended operation: in an `--unattended` wave
there is no human babysitter to hand-write findings, so the supervisor
self-capturing qualitative learnings is the *only* way the dogfood signal
survives the run.

## What Changes

- Add a bundled **`sweep.sh learn <category> <title> <body-json>`** subcommand
  that publishes an `agent.learning` through the helper (no raw curl),
  allowlisted by precise script path (same least-privilege model as the other
  `sweep.sh` subcommands).
- Add a fifth qualitative category **`tooling_friction`** — friction the
  supervisor absorbs about git-paw itself — with a documented body shape, a
  publish heuristic (with an explicit "do not publish unless…" gate), and a
  renderer section.
- **Wire opportunistic capture into the operational sweep loop**: when the
  continuous sweep observes or absorbs friction, the supervisor records a
  one-line learning in the moment via `sweep.sh learn`.
- **Add a session-end synthesis pass**: at wind-down the supervisor reflects
  over the run and publishes the durable qualitative learnings, deduped against
  what it already published in-session.
- **Route all qualitative-learning publishing through the helper**: the
  supervisor skill SHALL NOT hand-roll raw curl for `agent.learning`; the
  raw-curl publish example is replaced with `sweep.sh learn`.

No broker wire-format change: `agent-learning-variant`'s open-enum contract
absorbs the new category transparently.

## Capabilities

### New Capabilities

<!-- None. The qualitative-learnings machinery (categories, heuristics,
     renderer, dedup) already shipped in v0.6.0; this change wires it into the
     sweep loop, adds one category, and adds the bundled helper subcommand. -->

### Modified Capabilities

- `qualitative-learnings`: add a bundled `sweep.sh learn` subcommand as the
  publish mechanism (no raw curl); add the `tooling_friction` category (body
  shape + publish heuristic + renderer section); add the operational
  sweep-loop capture step and the session-end synthesis pass to the supervisor
  skill; require qualitative-learning publishing to route through the helper.

  *(The bundled-helper subcommand lives under this capability rather than a
  sweep.sh-surface capability: `shared-helper` governs `build_boot_block()`,
  `stuck-prompt-detection` owns only `detect-stuck`, and the existing publish
  subcommands have no single owning capability. The `learn` subcommand exists
  solely to publish qualitative learnings, so it is cohesive here and keeps the
  change in one delta for clean validation + archive.)*

## Impact

- `assets/scripts/sweep.sh` — new `learn` subcommand (publishes
  `agent.learning`, reusing the existing `publish()` + broker-URL discovery).
- `assets/agent-skills/supervisor.md` — sweep-loop capture step (§2),
  session-end synthesis pass (final-summary section), `tooling_friction`
  heuristic + dedup identifier, and replacement of the raw-curl publish
  example with `sweep.sh learn`.
- `src/broker/learnings.rs` — renderer: a new "Tooling friction" section for
  the `tooling_friction` category (the existing open-category → "Other
  learnings" fallback already prevents drops; this gives the category its own
  section).
- `src/supervisor/curl_allowlist.rs` — verify the existing by-path grant for
  `sweep.sh` already covers the new subcommand (no broad `curl *` grant added).
- No change to `agent-learning-variant` (open-enum) or the broker wire format.
- No change to `shared-helper` (`build_boot_block`) or
  `stuck-prompt-detection` (`detect-stuck`).
