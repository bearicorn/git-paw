## Context

`supervisor-as-pane-followups` archived on 2026-05-20 with §8c (drift 68
bundled helper) and §8d (drift 69 broker validation) **specced but not
implemented**. The spec deltas propagated to `openspec/specs/agent-skills/spec.md`
and `openspec/specs/broker-messages/spec.md` describing behaviour the
binary does not exhibit. Two new bugs also surfaced immediately after the
archive landed on `feat/v0.5.0-specs`.

This change closes both gaps in a single follow-up before the v0.5.0
release tag. Bundling the four items respects the change-cluster pattern
the v0.5.0 cycle has used: one OpenSpec change per coherent surface area,
not five micro-changes for the same supervisor flow.

## Decisions

### D1 — `cmd_supervisor` falls back to `SupervisorConfig::default()` rather than writing config

Two viable paths for Bug A:

- **D1a (chosen): in-memory default fallback.** `cmd_supervisor`
  unwraps `config.supervisor` to a `&SupervisorConfig::default()` when
  missing. No file write, no surprise persistence. The existing
  `[supervisor].cli > default_cli > error` resolution chain still
  errors if neither CLI is available — guaranteeing the binary doesn't
  launch with an unresolvable CLI.

- **D1b (rejected): interactive setup wizard writes `[supervisor]` to
  the user's `.git-paw/config.toml`** on prompt acceptance. Adds
  side-effecting file writes to the start path, which the project
  generally avoids; would also need a rollback path on failure. D1a is
  the minimal-blast-radius fix; if a setup-wizard flow is wanted later,
  it's a separate change.

The synthesized default has `enabled = false` (the prompt is the
imperative; the field documents persistence policy, not the current
session's mode). All other fields are their respective `Default`
values which are documented as safe.

### D2 — Bug B fix is `-c` on every split, no `cd && cli` send-keys race

Two RCA candidates for Bug B:

- **D2a (chosen): unify all splits to `-c <worktree>`**. Each
  `split-window` that creates an agent pane SHALL pass `-c
  <agent.worktree>` so the pane is born in the correct cwd. The
  follow-up `send-keys` then only invokes the CLI command, not a `cd
  <worktree> && <cli>` chain. Removes the race entirely (the pane's
  shell starts in the right cwd before send-keys can fire).

- **D2b (rejected): keep `cd && cli` send-keys but add a `sleep`
  before it** to lose the race the other way. Fragile (timing-dependent
  on shell startup speed), violates the existing -c-avoids-the-race
  comment in `src/tmux.rs:246-247`.

The build path already does this for pane 0 of bare sessions
(`new-session -c first_worktree`) and for subsequent splits in
`build_supervisor_session` (lines 738-748). The change makes it
consistent across the first-agent split in supervisor mode and the
subsequent-pane splits in the bare builder, which currently use
`cd <worktree> && <cli>` and trip the race.

### D3 — Helper script ships at `<repo>/.git-paw/scripts/sweep.sh`, not user-global

Two install locations:

- **D3a (chosen): per-repo at `<repo>/.git-paw/scripts/sweep.sh`.**
  Co-located with `<repo>/.git-paw/config.toml` and
  `<repo>/.git-paw/sessions/`. The supervisor pane's cwd is the repo
  root at boot, so relative invocation `.git-paw/scripts/sweep.sh` from
  the skill works without further path resolution. Survives `git paw
  purge` (which only touches `.git-paw/sessions/`). Per-repo isolation
  prevents one repo's config from leaking into another's helper.

- **D3b (rejected): user-global at `~/.git-paw/bin/sweep.sh`** with a
  PATH entry. Cross-repo behaviour gets harder to reason about
  (broker URL discovery has to fall back through several config
  paths). Not worth the complexity for v0.5.x.

### D4 — Broker `agent_id` validation is regex, not allowlist against session JSON

Two enforcement levels:

- **D4a (chosen): regex `^(supervisor|feat/[a-z0-9][a-z0-9-]+|feat-[a-z0-9][a-z0-9-]+)$`**.
  Stateless, cheap, catches the observed garbage (`a`, `b`,
  `<agent-id>`, empty strings). Allows arbitrary `feat-*` branches —
  including any a tester might use outside a paw session — but rejects
  obviously-invalid shapes. Pattern matches the `slugify_branch`
  output the launcher produces.

- **D4b (rejected): cross-reference against the current session
  JSON's worktree list**. Strictly correct (only known agents accepted)
  but introduces stateful coupling between the broker and session JSON
  that complicates testing and breaks the broker's
  fresh-launch / no-session edge case. The regex approach is good
  enough for v0.5.x; a stricter validation can layer on later.

The placeholder-rejection regex `^<.*>$` is exact-match — `<foo>`
is rejected but `partial <foo> embedded` is accepted (real human
content sometimes uses angle brackets inline).

### D5 — Placeholder-rejection: only check known string fields, not all

`BrokerMessage` payload variants have many string fields. Checking ALL
of them risks false positives (`StatusPayload.message` legitimately
contains arbitrary user-visible text). The validation SHALL check only
the four fields named in the spec delta: `payload.question` (on
`AgentQuestion`), `payload.message` (on `Status` and `Feedback`),
`payload.needs` (on `Blocked`), and each string in `payload.errors[]`
(on `Feedback`). These are the fields the supervisor skill's example
curls use placeholder syntax in.

The risk is a future feedback message containing `<example>` text being
rejected. Acceptable trade-off: the supervisor agent writing real
feedback uses prose, not angle-bracket wrappers; the false-positive
rate is low and the failure mode is HTTP 400 with a clear error.

## Risks / Trade-offs

- **Cherry-pick order constraint for the v0.5.0 cycle.** This change
  MUST land before `git paw v0.5.0` tags so the spec/code state is
  consistent. If it slips, ship as v0.5.1 instead of v0.5.0 — do NOT
  tag v0.5.0 with the §8c/§8d spec drift.

- **Broker validation rejecting tests.** Existing tests that POST to
  `/publish` with simplified `agent_id` strings (e.g.
  `tests/broker_integration.rs::"test-agent"`) MAY fail. Audit
  every existing `/publish` test caller, update to use the
  `^(supervisor|feat[-/].+)$` shape, and document the change in the
  v0.5.0 release notes if any test required content edits beyond
  trivial renames.

- **Skill-content test surface.** Rewriting the skill examples
  changes 500+ lines of `assets/agent-skills/supervisor.md`. The
  existing `coordination-skill-followups` test base asserts on
  specific substrings; some of those assertions MAY need updating.
  Treat skill-content tests as part of this change's scope, not as a
  separate sweep.

## Migration / Rollout

- v0.4-saved session JSONs work unchanged after this change lands —
  no schema migration. The fixes are runtime-only.

- Users who hit Bug A previously would have edited their
  `.git-paw/config.toml` to add a `[supervisor]` section as a
  workaround. Those configs continue to work; the new fallback only
  fires when the section is absent. No deprecation cycle.

- Users with custom curl tools that POST to `/publish` with
  unconventional `agent_id` values: their requests SHALL be rejected
  (HTTP 400) after this change lands. Documented in release notes.
  Mitigation: rename the offending IDs to match `feat-*`, or wrap the
  curl in a script that uses the supervisor scheme.

- `.git-paw/scripts/sweep.sh` is overwritten on `git paw init`.
  Users with local edits SHALL back up before re-running init.
  Documented in release notes.
