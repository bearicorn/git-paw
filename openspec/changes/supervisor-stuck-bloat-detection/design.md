## Context

The supervisor already has stuck-detection machinery, all of it living in
`assets/scripts/sweep.sh` and surfaced by the bundled supervisor skill:

- `sweep.sh detect-stuck` captures every coding-agent pane, resolves each
  pane to its `agent_id` via `pane_current_path`, and calls the per-agent
  `stuck_eval` core.
- `stuck_eval` flags a pane **stuck-on-prompt** when a permission/paste
  marker is present AND the agent's broker `last_seen_seconds` is past
  `STUCK_THRESHOLD_SECONDS` (30s). It publishes a synthetic `agent.status`
  with `phase: "stuck-on-prompt"` and dedups per `(agent_id, prompt-shape)`
  within `STUCK_DEDUP_WINDOW_SECONDS` (300s), writing dedup state to
  `.git-paw/.sweep-stuck-dedup`.
- The `agent.status` wire format already carries an open `phase` enum plus a
  free-form `detail` object (`supervisor-introspection`); the broker does
  not validate phase values, so new phases need no broker change.
- `supervisor-stream-timeout-recovery` teaches the supervisor to recover
  from *its own* API stream timeout mid-sweep (checkpoint → replay).

The v0.8.0 dogfood exposed three gaps this change closes. (1) A coding
agent's CLI hit `Request timed out` and sat dead — nothing detected it
because no permission marker was present. (2) An agent's context bloated
past ~250k tokens; the CLI showed `/clear to save 250k tokens` and the
agent degraded until it froze — again, no marker matched. (3) An agent made
zero progress for a long window but looked merely "idle," and the
supervisor mis-classified a *prompt-blocked* agent as idle from branch-tip
and uncommitted-file counts, making a wrong "wind down" call. One agent sat
**blocked waiting on the supervisor for 1054 minutes**.

This detection is consumed by the sibling `unattended-drive-loop` change,
which decides what action (nudge, approve, restart, escalate) each emitted
phase triggers. This change is responsible only for *detecting and
publishing* the states; the drive loop owns the *response policy*.

## Goals / Non-Goals

**Goals:**

- Detect a coding agent's `Request timed out` / stream-failure state from
  its pane and surface it as a distinct phase.
- Detect context bloat proactively from the `/clear to save <N>k tokens`
  marker against a configurable token threshold.
- Detect no-progress over a heartbeat window using BOTH task-checkbox count
  and commit count, so a slow-but-moving agent is not flagged.
- Detect a blocked-on-supervisor state that has exceeded its own timeout.
- Make the detector read LIVE pane state so a prompt-blocked agent is
  classified blocked-on-prompt, never no-progress.
- Encode "N feedback→fix→re-verify cycles is normal, not stuck" in the
  supervisor skill.
- Keep all new state dedup'd and all new config fields backward compatible.

**Non-Goals:**

- The *response policy* (when to restart vs nudge vs escalate) — owned by
  `unattended-drive-loop`. This change emits phases; it does not decide
  remediation beyond the existing approve/nudge affordances.
- Restarting/respawning an agent process (no new CLI surface here).
- Changing the broker wire format or phase validation (open enum already
  accepts new values).
- Detecting the supervisor's *own* stream timeout — that remains
  `supervisor-stream-timeout-recovery`'s existing checkpoint/replay flow.

## Decisions

### Decision 1: Extend `stuck_eval`, do not add a parallel detector

The single `stuck_eval` core already owns capture-marker matching, the
heartbeat gate, dedup, and synthetic publish. The new shapes
(stream-timeout, context-bloat, no-progress, blocked-on-supervisor) are
added as additional classification branches inside the same core so they
inherit dedup and the publish path. **Alternative considered:** a separate
`detect-bloat`/`detect-progress` subcommand per shape — rejected because it
would duplicate pane capture, agent resolution, and dedup, and three
overlapping monitors is exactly the inline-bash reinvention the skill
forbids.

### Decision 2: Read the pane FIRST, then classify (hard constraint)

The detector capture is the source of truth for *why* an agent looks idle.
Classification order in `stuck_eval` is:

1. If a stream-timeout marker is present → **stuck-stream-timeout**.
2. Else if a permission/paste marker is present → **stuck-on-prompt**
   (existing path; route to approval classifier).
3. Else if a `/clear to save <N>k tokens` marker with `N ≥ threshold` is
   present → **context-bloat**.
4. Else (no marker) consult the no-progress heartbeat: if checkbox count
   AND commit count are both unchanged across the window → **no-progress**.

Because the marker check runs before the no-progress check, an agent
sitting on a prompt can never be classified no-progress. **Alternative
considered:** classify from broker `last_seen` + uncommitted-file counts
alone (what the dogfood supervisor did manually) — rejected: it produced
the wrong "wind down" call. The spec encodes "reads pane state" as a
testable requirement.

### Decision 3: No-progress uses checkbox count AND commit count, both unchanged

A no-progress signal requires BOTH the agent's `tasks.md` completed-checkbox
count and its branch commit count to be unchanged across the window. Using
either alone produces false positives — an agent can commit without ticking
a checkbox (refactor) or tick a checkbox without an extra commit
(amend/squash cadence). The detector snapshots `(checkbox_count,
commit_count, timestamp)` per agent in a progress-state file
(`.git-paw/.sweep-progress`) and compares on the next sweep. The window
default is ~25 min (`NO_PROGRESS_WINDOW_SECONDS = 1500`), longer than the
stuck-on-prompt heartbeat threshold because real edits take minutes.
**Alternative considered:** broker `last_seen` only — rejected, heartbeats
keep advancing while an agent spins on a hard problem; that is progress on
thinking, not a stall.

### Decision 4: Context-bloat threshold from the `/clear` hint, configurable

The CLI's own `/clear to save <N>k tokens` hint is the cheapest reliable
bloat signal — git-paw cannot read the CLI's token accounting directly. The
detector parses `N` (in thousands) from the marker and compares against
`CONTEXT_BLOAT_THRESHOLD_K` (default 250, matching the observed freeze
point). Flagging is *proactive*: the agent is still responsive at the
threshold, so the phase is advisory, letting the drive loop pre-empt the
freeze. **Alternative considered:** a fixed 250k constant — rejected;
different models/CLIs surface different numbers, so the threshold is a
`[supervisor]` config field.

### Decision 5: Blocked-on-supervisor is a timed state keyed off `agent.blocked`

An `agent.blocked` event whose `payload.from` is `supervisor` (or whose pane
shows it is awaiting supervisor input) starts a timer. When the unanswered
duration exceeds `BLOCKED_ON_SUPERVISOR_WINDOW_SECONDS` (default 900s / 15
min — far below the 1054-minute dogfood failure), the detector emits a
distinct phase so the supervisor (or the drive loop) is forced to act. The
broker already records `agent.blocked` events and their timestamps, so the
detector reads the agent's message stream rather than tracking its own
timer. **Alternative considered:** treat blocked-on-supervisor as ordinary
no-progress — rejected; the remediation differs (the supervisor must
*answer*, not nudge the agent), so it warrants its own phase.

### Decision 6: New phases reuse the open `phase` enum; dedup per `(agent, shape)`

Each new shape publishes `agent.status` with a new `phase` value
(`stuck-stream-timeout`, `context-bloat`, `no-progress`,
`blocked-on-supervisor`) and a `detail` object documenting the captured
evidence (e.g. `detail.captured_prompt`, `detail.tokens_k`,
`detail.unchanged_for_seconds`). The existing dedup file keys on
`(agent_id, shape)` so a persistently-stuck agent emits exactly one publish
per window per shape. No broker change is needed: `supervisor-introspection`
already specifies the broker accepts unknown phase values.

### Decision 7: "N re-verify cycles is not stuck" lives in the supervisor skill

This is an LLM-behaviour rule, not a detector branch — the detector has no
notion of verify cycles. The `supervisor-stream-timeout-recovery` skill
prose gains an explicit statement that multiple feedback→fix→re-verify
cycles per agent are normal progress (mcp-server: 7, dev-allowlist: 6) and
SHALL NOT be counted toward a stall. The spec asserts the skill prose
contains this rule.

### Decision 8: Thresholds are `[supervisor]` config, defaulted in the script

The three new windows/thresholds become optional `SupervisorConfig` fields
(`#[serde(default, skip_serializing_if = "Option::is_none")]`), mirroring
the existing gate-command fields, so older configs load unchanged. `sweep.sh`
reads them from `[supervisor]` via its existing TOML-discovery helper and
falls back to the documented defaults when unset.

## Risks / Trade-offs

- **[Pane-text heuristics are CLI-specific]** → The `Request timed out` and
  `/clear to save Nk tokens` markers are observed strings. Mitigation:
  match a small set of generic patterns (mid-stream cutoff / transport
  error; `/clear` + token hint), keep them in named regex variables next to
  the existing `STUCK_MARKERS_REGEX`, and phrase the spec generically (≥2
  symptom patterns, no single CLI's exact string baked into a SHALL).

- **[No-progress false positive on a long-but-legitimate task]** → A 30-min
  research/build step trips the window. Mitigation: require BOTH checkbox
  and commit unchanged, set the window well above the prompt threshold
  (~25 min), and make the emitted phase a *nudge* trigger, not an auto-kill.

- **[Context-bloat threshold too aggressive]** → Flagging at 250k may
  pre-empt an agent that would have finished. Mitigation: the phase is
  advisory/proactive and the threshold is configurable; the drive loop owns
  whether to act.

- **[Progress-state file races across sweeps]** → Concurrent sweeps could
  read/write `.git-paw/.sweep-progress`. Mitigation: same single-writer
  cadence as the existing dedup file (one sweep at a time), write the whole
  file atomically, and key per-agent so a partial read degrades to "no prior
  snapshot" (no false stall).

- **[Blocked-on-supervisor window misfires during a busy sweep]** → The
  supervisor may be mid-merge when the timer trips. Mitigation: 15-min
  default is generous; the phase surfaces the state, and the supervisor/drive
  loop decides — detection never auto-resolves the block.

## Migration Plan

1. Add the three optional `SupervisorConfig` fields with `serde(default)`;
   existing configs and sessions load unchanged (no field → documented
   default in the script).
2. Extend `sweep.sh` `stuck_eval`/`detect-stuck` with the new branches and
   the `.git-paw/.sweep-progress` snapshot file; `stuck-eval` stays
   fixture-drivable (stdin capture + args) so the new branches are testable
   without tmux.
3. Extend the supervisor and coordination skill prose; re-run the
   no-language-leak audit.
4. Rollback: the new phases are additive and dedup'd; reverting the script
   to the prior version simply stops emitting the new phases (the existing
   stuck-on-prompt path is unchanged). Config fields, being optional, are
   inert when the script does not read them.

## Open Questions

- Should the no-progress window be model-aware (a fast model finishing in
  seconds vs a slow one)? Deferred — single configurable window for v0.9.0;
  revisit if dogfood shows per-model tuning is needed.
- Should context-bloat trigger an automatic commit-before-clear nudge
  (tying into `coordination-context-budget`'s commit-before-compact rule)?
  The detection emits the phase; the auto-nudge wiring is left to
  `unattended-drive-loop`.
