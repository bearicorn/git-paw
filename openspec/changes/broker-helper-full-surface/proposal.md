## Why

G4 from the v0.8.0 dogfood: the supervisor still falls back to **raw curl**
to publish a rich `agent.status` (with `phase` + a structured
`detail.{branch,audit_step,intended_targets,…}` body) because the bundled
supervisor helper's `sweep.sh status-publish` subcommand only accepts a
PLAIN message string — it does not cover the full `agent.status` payload the
introspection skill actually emits. This is the same anti-pattern as F1
(agents' raw boot-curl), now supervisor-side: the helper's surface is
narrower than the messages the skill emits, so raw `curl …/publish` leaks
back into the supervisor skill on every phase transition. That is verbose,
error-prone, and pressures the allowlist toward a broad `curl *` grant
instead of the least-privilege, by-path grant the project mandates.

## What Changes

- **Extend `sweep.sh status-publish` to cover the full `agent.status`
  surface.** The subcommand SHALL accept an optional `phase` and an optional
  structured `detail` (a JSON object), in addition to the existing message
  string, and SHALL shape the complete `agent.status` payload internally so
  the supervisor never hand-rolls JSON. The plain `status-publish <msg…>`
  form keeps working unchanged (backward compatible).
- **Route ALL supervisor `agent.status` publishes through the helper.** The
  bundled supervisor skill (`supervisor.md`) SHALL replace every raw
  `curl …/publish` example that emits an `agent.status` — boot self-register,
  the phase-taxonomy examples, the audit example, the `checkpoint` example,
  and the final-summary status — with the `sweep.sh status-publish` form.
  The supervisor skill SHALL contain NO raw `curl …/publish` example for
  `agent.status`.
- **Keep the least-privilege, by-path allowlist intact.** The helper
  invocation SHALL be covered by the existing `.git-paw/scripts/sweep.sh`
  by-path grant; this change SHALL NOT seed (or require) a broad `curl *`
  grant.

No broker wire-format change: `StatusPayload` already carries the optional
`phase` and `detail` fields ([[supervisor-introspection]]); this change only
widens the bundled helper's surface and updates the skill prose so the
existing fields are emitted through the helper rather than raw curl.

This is explicitly **not** the `sweep.sh learn` work — that rich
`agent.learning` surface is owned by the sibling change
`learnings-supervisor-observation-channel` and is out of scope here.

## Capabilities

### New Capabilities

<!-- None. The agent.status wire fields and the supervisor publish helper both
     already exist; this change widens the helper's status surface and rewires
     the skill to route through it. -->

### Modified Capabilities

- `agent-broker-helper`: extend the bundled-helper publish surface so the
  supervisor-side status publish accepts the FULL `agent.status` payload
  (`phase` + structured `detail`) and so the supervisor/coordination skills
  route every broker publish through the helper, never raw curl.
- `supervisor-introspection`: require the bundled supervisor skill to emit
  its phase-tagged `agent.status` (and the `checkpoint` emission) through the
  bundled helper rather than a raw `curl …/publish`, so the documented phase
  taxonomy reaches the broker by the least-privilege path.

## Impact

- `assets/scripts/sweep.sh` — `cmd_status_publish` widened to accept optional
  `phase` and `detail` (JSON object) and shape the full payload; the plain
  one-arg form is preserved.
- `assets/agent-skills/supervisor.md` — boot self-register, the introspection
  phase-taxonomy examples, the audit example, the `checkpoint` example, and
  the final-summary status rewritten to use `sweep.sh status-publish`; no raw
  `curl …/publish` for `agent.status` remains.
- `src/supervisor/curl_allowlist.rs` — verify the existing by-path grant for
  `.git-paw/scripts/sweep.sh` already covers the widened subcommand (no broad
  `curl *` grant added).
- Tests: `tests/sweep_sh_*.rs` (helper subcommand behaviour + conventions),
  a supervisor-skill-content test asserting no raw `agent.status` curl,
  and a curl-allowlist test asserting the by-path grant covers the helper.
- No change to `StatusPayload` (`src/broker/messages.rs`), the broker wire
  format, or the agent-side `broker.sh` helper.
