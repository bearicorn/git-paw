## Context

A worker's durable state splits three ways: the worktree (on disk, survives anything), the CLI
conversation (restorable *if* the session id is known), and progress-while-detached (needs a live
process). git-paw persists the first, cannot restore the second, and the third is a separate question
(the tmux spike). Today `recover_session` relaunches every worker cold. Claude Code accepts
`--session-id <uuid>` at start-up and `--resume <uuid>` later; other CLIs vary (Codex generates its own
ids and resumes via a subcommand; several have no resume at all). `MILESTONE.md` lists this as confirmed
item 2 for v0.17.0 and notes it pairs with v0.18.0's per-CLI provider layer.

## Goals / Non-Goals

**Goals:**
- Know each worker's CLI session id deterministically at launch, for CLIs that let git-paw assign one.
- Persist it additively; use it on recovery; expose it in `status --json`.
- Keep per-CLI knowledge in the existing `[clis.<name>]` table with a built-in row for Claude.

**Non-Goals:**
- Scraping ids from pane output or from a CLI's session directory (Codex-style). Those CLIs get
  override templates only, and no built-in row until they can assign an id at launch.
- Resuming the *orchestrator* — there is no orchestrator pane after this cycle; the headless escalation
  change may reuse `resume_args` for continuity, but that is its decision.
- Any change to restart-from-pause (it never respawns CLIs) or to boot-prompt injection on recovery.

## Decisions

### D1 — Assign at launch rather than capture after

Assignment is deterministic, CLI-agnostic in mechanism (a template), needs no scraping and no
per-CLI filesystem knowledge, and gives the id *before* the CLI starts so it is persisted even if the
pane dies seconds later. Capture would be fragile (prompt profiles, log formats) and CLI-specific in
code. Cost: CLIs that cannot accept an assigned id (Codex today) are not covered — acceptable; the
template seam lets a consumer add one the day the CLI supports it.

### D2 — Templates beside `approval_args`, same resolution chain

`session_id_args` / `resume_args` mirror the shape and precedence of `approval_args` (override →
built-in → none), rendered with `{{SESSION_ID}}`. Consumers already know this pattern; the
configuration reference gains two rows in the same table. Load-time validation rejects a template
without the placeholder so a typo cannot silently launch a CLI with a dangling flag.

### D3 — v4 UUID from OS randomness, no `uuid` crate

`uuid` is not in the approved dependency set. Read 16 bytes from the OS (`/dev/urandom` on the
Unix platforms git-paw supports), set the version/variant nibbles, format as
`xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx`. A tiny `src/random.rs` (or `domain.rs`) helper; the later
`broker-verdict-authority` change reuses it for its credential.

### D4 — Recovery appends resume args; everything else unchanged

`recover_session` builds each pane's CLI command today from `entry.cli` + approval flags. With an id
and a resume template, append the rendered `resume_args`. Prompt handling on recovery is deliberately
left as it is — a resumed CLI receiving the boot block again is harmless and keeps this change to one
concern. Whether recovery should send a shorter "you were resumed" nudge is noted as an open question.

### D5 — Expose in `--json` only

The human table stays byte-identical (guarded by existing status tests); the id is operator/tooling
data, so `--json` is the right surface. The MCP `get_session_status` already reads the session JSON
and picks the field up for free.

## Risks / Trade-offs

- [A CLI rejects an assigned id (e.g. id already exists on disk)] → fresh v4 UUIDs do not collide;
  if a CLI still refuses, the launch-readiness gate already detects a dead pane and retries once.
- [Upstream flag drift (`--session-id` renamed)] → built-in rows are verified at implementation time
  and overridable per CLI via the template; no code change needed for a consumer to fix drift.
- [`ps` visibility] → the id is not a secret; it identifies a local conversation file readable only
  by the same user.
- [Two changes touching `recover_session`] → the pane-removal change modifies recovery's pane set;
  this change only appends arguments to an existing command string. Land this first; the merge is
  additive.

## Migration Plan

Single wave: config templates + validation → UUID helper → assign in `attach_agent` → persist →
recovery append → `status --json` → docs. Pre-existing session files need no migration.

## Open Questions

- Should recovery of a resumed worker inject a short resume nudge instead of the full boot + task
  prompt? Recommendation: defer; observe one dogfood cycle of resumed workers first.
