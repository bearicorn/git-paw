## Context

Five friction points from the v0.19 dogfood, each small, each in a different module. They share
one change because each costs workers or the orchestrator time in every session, and none needs a
new wire variant or struct field.

## Goals / Non-Goals

**Goals:** every worker broker call is a `broker.sh` call; workers stop reading orchestrator
instructions; doctor stops showing ✓ for a check it could not make; an emptied session can be
refilled; the orchestrator never archives a merge that did not land.

**Non-Goals:** git-paw writing any agent CLI's settings or command files; changing `start`'s
behaviour for sessions that have agents; new broker message types.

## Decisions

### D1 — Three helper additions, not a raw-JSON escape hatch (Group A)

Three coordination examples had no `broker.sh` form: the `activity` status, region-scoped intents,
and the `since` cursor (the skill tells agents to read `last_seq`, but `poll` never printed it). Add
`status --activity <phrase>`, repeatable `intent --region <path>#<kind>=<value>`, and a trailing
`last_seq: <n>` line from `poll`. A generic `--payload-json` option was rejected: it would let
workers hand-roll payloads again, which the helper exists to prevent.

Region syntax: `#` separates path from region because `:` and `,` already appear in the files
list. Values: `function=<name>`, `class=<name>`, `block=<anchor>`, `range=<start>-<end>`. A region
whose path is not in `<files>` adds that path. Unknown kinds or a bad range exit 2 before any
publish.

The bare-call instruction extends the skill's existing "Run dev commands bare" section to name the
helper explicitly and adds one line to both boot templates. It describes the call's shape only,
so it stays CLI-agnostic.

### D2 — Keep the pointer in the tracked AGENTS.md, scope it in prose (Group B)

Moving the pointer to an untracked file was rejected: the root agent CLI auto-loads `AGENTS.md`,
not an arbitrary untracked file, so the pointer would stop reaching the orchestrator. The section
stays where it is, between its markers, and its prose now opens with the condition: "If you are
the agent session running at the repository root (the main checkout) …"; a coding agent in a
worktree ignores the section. Marker handling, idempotence and existing tests are unchanged; only
the asset text changes.

### D3 — Unverified is a warning, remedy is `[worktree.env] copy` (Group C)

`WorkflowReach::Unverified` now produces `CheckResult::warn`. The remedy names
`[worktree.env] copy` and says that it copies files, so each command file is listed individually.
`[worktree.env] copy` takes files, not directories; the guide says so. Unreachable still takes
precedence over unverified, so the existing unreachable detail and remedy do not change.
The scenario "An unknown CLI is reported, not warned" keeps its name, because the validator matches
MODIFIED scenarios by name. Its body now asserts `Warn`. Rename it when the spec is next reorganised.

### D4 — "No agent panes" is alive; start with selection on an empty session (Group D)

Root cause of DF-43: `has_live_agent_pane_with` returns false both when every agent pane runs a
bare shell and when there is no agent pane at all, so an emptied session reads `AgentsExited` →
`Stopped`. Split the two: zero coding-agent panes → `Alive`. `add` then sees `Active` and works
with no change to `add.rs`. The launch grace period and the exited-agents behaviour are untouched.

DF-44: `start --specs` on such a session went through `revive_existing`, which ignores selection.
Decision: when the receipt records zero worktrees and a selection flag is given:
- **Stopped** → discard the receipt with the purge path (kill the leftover tmux session if any;
  there are no worktrees to remove), print `Session '<name>' had no agents; replacing it with the
  selection`, and continue into the fresh-launch path. Nothing live is lost.
- **Active / Paused** → refuse with `session '<name>' is running with no agents — add them with
  \`git paw add <branch>\` or \`git paw add --from-spec <name>\``. Tearing it down was rejected:
  its broker and dashboard (pane 0 since v0.18) are live and serving, and `add` is the supported
  path for putting agents into a running session; replacing it would discard the broker log and
  dashboard state for no gain.

This leaves the recover contract intact. A session with agents, or `start` without selection
flags, behaves exactly as before.

### D5 — Landed-merge check in the orchestrator skill (Group E)

Step 2 of "Merge orchestration" gains, right after `git merge --ff-only`:
`git merge-base --is-ancestor <tip> HEAD` (record `<tip>` with `git rev-parse` before merging); on
failure, the merge did not land: no archive, no `advanced-main`, investigate. The skill also adds
a rule that a must-succeed git or test step is never piped, and an archive step, placed after the
tests pass and the ancestor check succeeds, that re-runs `{{TEST_COMMAND}}` when the archive adds
a capability. The step says "your spec workflow's archive step (if it has one)", so the skill
names no spec tool.

## Risks / Trade-offs

- [Liveness change hides a real failure] → only a session with *zero* coding-agent panes is
  affected; one whose agents died still has panes running a shell and stays `AgentsExited`.
- [More ⚠ rows in doctor] → intended: the row was a false ✓; the remedy is actionable.
- [Text-pinning tests churn] → listed in the proposal's Impact; updated in the same task group as
  the asset change.
- [`#` in a real path] → paths containing `#` cannot use `--region`; the whole-file form still works.

## Open Questions

None blocking. The order of the per-group commits follows tasks.md.
