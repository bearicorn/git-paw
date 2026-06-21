## Context

git-paw bundles two broker helper scripts installed by `git paw init` under
`<repo>/.git-paw/scripts/`:

- **`broker.sh`** — the agent-side helper (capability `agent-broker-helper`).
  It wraps every agent→broker `curl` with positional-argument subcommands
  (`status`, `artifact`, `blocked`, `question`, `intent`, `poll`) and shapes
  the JSON internally. F1 introduced it so coding agents never hand-roll raw
  curl.
- **`sweep.sh`** — the supervisor-side helper (its publish verbs —
  `status-publish`, `verified`, `feedback-gate` — and its observe verbs —
  `snapshot`, `capture`, `approve`, `status`, `inbox`, `detect-stuck` — have
  no single owning capability today; pane-driving discipline is governed by
  `supervisor-skill-discipline`, the boot block by `shared-helper`, and stuck
  detection by `stuck-prompt-detection`).

The broker wire format already supports a rich `agent.status`: `StatusPayload`
in `src/broker/messages.rs` carries optional `phase: Option<String>` and
`detail: Option<serde_json::Value>` fields ([[supervisor-introspection]],
[[broker-messages]]). The supervisor skill documents a full phase taxonomy
(`sweep`, `audit`, `merge`, `feedback`, `intent_watch`, `learnings`, `idle`,
`checkpoint`, `baseline`) each with a structured `detail` body, and an
`audit_step` enumeration over the five verification gates.

The gap (G4): `sweep.sh status-publish` only accepts a plain message string —
its payload is hardcoded `{"status":"working","modified_files":[],"message":<msg>}`
with **no `phase` and no `detail`**. So every time the supervisor wants to
emit a phase-tagged status (which the skill says to do on *every* phase
transition), the skill falls back to a raw `curl …/publish -d '{…}'`. The
helper's surface is narrower than the messages the skill emits, so raw curl
leaks back in — exactly the F1 anti-pattern, supervisor-side.

Notably, `sweep.sh` already *constructs* a full `phase` + `detail`
`agent.status` internally inside `stuck_eval` (for the synthetic
`phase: "stuck-on-prompt"` publish), proving the shape is expressible in the
helper; it is simply not exposed on the user-facing `status-publish` verb.

## Goals / Non-Goals

**Goals:**

- Widen `sweep.sh status-publish` so it can emit the FULL `agent.status`
  payload the introspection skill documents: message + optional `phase` +
  optional structured `detail` object.
- Make the bundled supervisor skill route EVERY `agent.status` publish through
  the helper, leaving no raw `curl …/publish` example for `agent.status`.
- Keep the seeded allowlist least-privilege and by-path
  (`.git-paw/scripts/sweep.sh`); add no broad `curl *` grant.
- Preserve backward compatibility: the plain `status-publish <msg…>` form
  keeps producing the same v0.5.0-shape payload.

**Non-Goals:**

- The `sweep.sh learn` / `agent.learning` rich surface — owned by the sibling
  change `learnings-supervisor-observation-channel`. Not specced here.
- Any change to the broker wire format or `StatusPayload` struct — the
  `phase`/`detail` fields already exist.
- Any change to the agent-side `broker.sh` helper or its `status` subcommand
  (coding agents do not emit `phase`/`detail`; the filesystem watcher and
  lightweight heartbeats cover them).
- Read-side broker verbs (`/status`, `/messages/*`) — out of scope; this
  change is the publish (write) surface for `agent.status`.

## Decisions

### Decision 1 — Extend the EXISTING supervisor verb, do not add a new one

Widen `cmd_status_publish` to accept two optional trailing arguments — a
`--phase <phase>` value and a `--detail <json-object>` value — rather than
introduce a separate `status-publish-rich` subcommand. The plain positional
form `status-publish <msg…>` is preserved verbatim.

Concretely the surface becomes:

```
sweep.sh status-publish <message…>
sweep.sh status-publish --phase <phase> [--detail '<json-object>'] <message…>
```

When `--phase`/`--detail` are absent the emitted payload is byte-identical to
today's `{"status":"working","modified_files":[],"message":<msg>}`. When
`--phase` is present the payload gains `"phase":<phase>`; when `--detail` is
present it gains `"detail":<parsed-json-object>`. The helper shapes the JSON
internally (via the same Python `publish()` path used by `verified` and
`feedback-gate`), so the caller passes only the phase string and a detail
JSON object — never a full envelope.

*Why:* the introspection skill already calls this verb by name
(`status-publish`) for the final-summary status; widening it keeps one verb
the supervisor must remember and avoids a name proliferation. It also matches
the sibling change's principle of a cohesive per-purpose subcommand. The
`--phase`/`--detail` flags (parsed before the positional message) keep the
common plain form unchanged for backward compatibility.

*Alternative considered — a new `status-rich` subcommand:* rejected. It would
fork the supervisor's mental model (two status verbs) and leave the narrow
verb as a trap that re-invites raw curl for the rich case.

*Alternative considered — accept a full JSON envelope argument:* rejected.
That re-creates the raw-curl ergonomics (caller assembles the envelope) and
risks malformed `type`/`agent_id`, which the broker's agent_id/placeholder
validation ([[broker-messages]]) would 400. Passing only `phase` + `detail`
and shaping internally keeps the helper authoritative for the envelope.

### Decision 2 — Capability placement: `agent-broker-helper` is the primary owner

The G4 gap physically lives on `sweep.sh` (supervisor-side), but the
supervisor publish verbs have **no single owning capability**. Two candidate
homes were weighed:

- **`agent-broker-helper`** — the capability that establishes the bundled-
  helper contract ("the helper wraps every broker curl, shapes JSON
  internally, callers pass positional args, allowlisted by path"). Its
  "Helper publish subcommands" requirement already enumerates the publish
  verbs and their payload shapes. Although its named script is `broker.sh`,
  the *contract* it owns — bundled helper wraps all broker publishes, no raw
  curl, by-path allowlist — is exactly the contract G4 violates on the
  supervisor side.
- **`supervisor-introspection`** — owns the `phase`/`detail` wire fields, the
  phase taxonomy, and the emission cadence in the skill.

**Decision:** make `agent-broker-helper` the primary owner and add a delta
requirement there that (a) extends the helper's status-publish surface to the
full `agent.status` shape, and (b) mandates the supervisor/coordination skills
route every `agent.status` publish through the helper (no raw curl), under the
existing by-path allowlist. Separately, MODIFY `supervisor-introspection`'s
"Supervisor phase taxonomy" requirement so the skill's documented phase
emission is *delivered through the helper* (closing the loop between the
taxonomy it documents and the publish path).

This mirrors the sibling `learnings-supervisor-observation-channel` decision,
which placed `sweep.sh learn` under the cohesive `qualitative-learnings`
capability for the same reason (no dedicated sweep.sh-surface capability
exists). Here the cohesive owner for a *broker publish helper surface* is
`agent-broker-helper`. We deliberately do NOT touch `curl-allowlist`'s
requirements — the existing by-path grant already covers `sweep.sh`; a
scenario asserts that coverage rather than changing the allowlist contract.

### Decision 3 — `detail` is passed as a JSON object string, parsed by the helper

`--detail` takes a JSON object (e.g. `'{"branch":"feat/auth","audit_step":"tests"}'`).
The helper parses it with the same Python interpreter it already uses, embeds
it as the payload's `detail` value, and fails loudly (non-zero exit, stderr
message) if the argument is not a JSON object. This keeps the broker's
`detail: Option<serde_json::Value>` contract intact and avoids the helper
having to model every per-phase detail shape (the taxonomy is open-ended).

*Why a string rather than per-field flags:* the `detail` shape varies per
phase (`branch`+`audit_step` for audit, `intended_targets` for checkpoint,
`pass`+`agents_checked` for sweep, …). A single JSON-object argument keeps the
helper phase-agnostic and forward-compatible as the taxonomy grows — matching
the open-enum spirit of `phase`.

### Decision 4 — Convention discipline carries over

The widened `cmd_status_publish` SHALL keep using the `-c "$(cat <<'EOF' … EOF)"`
interpreter-invocation shape (never the stdin-claiming `interpreter - <<`
heredoc), so the existing convention test in `tests/sweep_sh_conventions.rs`
continues to pass. The JSON shaping moves into a `python3 -c` block consistent
with `cmd_feedback_gate` / `cmd_verified`.

## Risks / Trade-offs

- **[Malformed `--detail` JSON silently dropped]** → The helper SHALL validate
  that `--detail` parses to a JSON object and exit non-zero with a clear
  stderr message otherwise, rather than publishing a status with a string or
  null `detail`. A test covers the reject path.
- **[Skill rewrite misses a raw-curl example]** → A supervisor-skill-content
  test asserts there is NO `curl …/publish` line emitting an `agent.status` in
  `supervisor.md` (i.e. no `/publish` line whose body contains
  `"type":"agent.status"`). This makes the "no raw curl for agent.status"
  requirement enforceable rather than aspirational. Note: raw curls for OTHER
  message types the supervisor genuinely owns are out of scope for this change
  (e.g. a hand-written `agent.blocked` nudge embedded in a send-keys example);
  the test is scoped to `agent.status` bodies only.
- **[Broadening the allowlist by accident]** → A curl-allowlist test asserts
  the seeded grant still authorises `.git-paw/scripts/sweep.sh` by path and
  does NOT contain a broad `curl *` grant after this change.
- **[Backward-compat regression on the plain form]** → A test asserts
  `status-publish <msg>` (no flags) produces a payload with no `phase` and no
  `detail` keys, matching the v0.5.0 shape.

## Migration Plan

No data migration. `git paw init` overwrites `.git-paw/scripts/sweep.sh` with
the bundled asset on the next run; sessions already running pick up the wider
verb the next time the supervisor reloads the skill. The plain `status-publish`
form is unchanged, so any in-flight supervisor that has not learned the flags
keeps working. Rollback is reverting the asset + skill edits — the wire format
is untouched, so no broker or stored-message compatibility concerns arise.

## Open Questions

None. The wire fields exist, the helper already proves it can build the shape
internally (`stuck_eval`), and the capability placement follows an established
sibling precedent.
