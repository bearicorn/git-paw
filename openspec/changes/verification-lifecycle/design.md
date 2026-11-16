## Context

All line references are at v0.20.0 (`e213707`). The broker emits its own
`supervisor.verify-now` nudge from `src/broker/delivery.rs::publish_message`. Root-side verdicts
come from `sweep.sh`, and worker-side verbs from `broker.sh`. `commit-event-truth` already
makes each commit event carry its `sha` and publishes a finished rebase as one `rewritten`
artifact.

### Defects, with their source

| Finding | Where | What the code does |
|---|---|---|
| DF-21 | `src/broker/delivery.rs:322-331`, `assets/agent-skills/coordination/SKILL.md:506-509` | The nudge fires on every `committed`. The skill calls each commit "the canonical done signal". A `done` artifact (`broker.sh artifact`) draws no nudge. |
| DF-21 | `src/broker/messages.rs:600-604` | `VerifyNow` carries only `branch_id`: no SHA and no trigger, so the orchestrator cannot tell a section commit from the last one. |
| DF-37/40, PK-12 | `assets/hooks/pre-commit.sh:7-20` | The only commit guard is the branch guard. Nothing marks a branch as under verification. |
| PK-16 | `src/commands/send.rs:189-190` | `__send` refuses a mid-response agent. That is correct, so the stop has to come through the broker and the hook instead (D5). |
| PK-13 | `src/broker/messages.rs:179-184` | `VerifiedPayload` has no SHA. |
| PK-13 | `sweep.sh:1040-1062`, `sweep.sh:1142-1175` | `verified` sends only a message. `advanced-main` sends the new main SHA but not the merged branch's tip, so nothing can compare the two. |
| PK-14 | `src/supervisor/drive.rs:3172-3185` | `merge_state` is "tip is an ancestor of default". One commit after the merge flips a merged branch to unmerged. |

## Goals / Non-Goals

**Goals:**
- explicit "complete" and "frozen" states;
- merge and verification checked against SHAs.

**Non-Goals:**
- interrupting a mid-response worker (PK-16's `tell` half; proposed as its own change);
- changing `finish` (noted as a follow-up).

## Decisions

Decision numbers are kept from `broker-event-semantics`.

### D4 — Change complete is the worker's explicit `done`
The coordination skill already has `broker.sh artifact`, which publishes `done`. The skill now
says to publish it once after the final commit, for code changes too. The broker emits the verify
nudge with `trigger: "complete"` and the artifact's `sha` for `done`, and with
`trigger: "commit"` for `committed`. Per-commit nudges stay, so the existing "verify on each
event" discipline and `verify_on_commit_nudge` keep working. The orchestrator skill runs the full
five gates on `complete` only.

### D5 — Freeze as a verdict, enforced by the pre-commit guard
`orchestrator.freeze { agent_id, sha }` is a verdict: it needs the authority credential at
`/publish`, like `agent.verified` and orchestrator feedback. It is routed to the agent's inbox.
`sweep.sh freeze <agent>` publishes it and also writes `paw-frozen` (holding the SHA) into the
worker's linked gitdir. The root checkout can write there; the worker's sandbox is not involved.

- The pre-commit hook refuses a commit while `paw-frozen` exists, naming the frozen SHA and the
  way out (wait for `agent.verified` or `agent.feedback`).
- `sweep.sh verified` and `sweep.sh feedback-gate` delete the marker before publishing, and the
  broker clears its frozen record when either verdict arrives for that agent.
- The marker is the enforcement and the message is the notice. A worker that never polls is
  still stopped, which closes PK-12 for commits.

### D10 — Verified and merged tips
- `agent.verified` gains optional `sha`. `sweep.sh verified` fills it with the agent's worktree
  `HEAD` unless `--sha` is given.
- `agent.advanced-main` gains optional `merged_sha`: `sweep.sh advanced-main` resolves
  `git rev-parse <merged-branch>`.
- On an advanced-main whose `merged_sha` differs from the last verified `sha` recorded for that
  branch's agent, or when no verified SHA is recorded, the broker puts one `[merge-check]`
  question into the orchestrator inbox.
- The drive loop records each advanced-main's `merged_sha` per branch. A branch counts as merged
  when its tip is an ancestor of the default branch, **or** its recorded `merged_sha` is.
  Commits after that SHA are reported as `+N after merge`, not as unmerged.

## Risks / Trade-offs

- [The worker never publishes `done`] → per-commit nudges remain. The drive loop's existing
  completion detection still sees the committed terminal state, and the orchestrator skill tells
  it to ask the worker for `done` when a branch has gone quiet.
- [A stale `paw-frozen` blocks a worker after an orchestrator crash] → the refusal message names
  the file and the verdicts that lift it (`sweep.sh verified` or `sweep.sh feedback-gate`). `doctor` lists frozen
  markers. `purge` removes them with the other markers.

## Migration Plan

1. Hook: the frozen guard. Hook e2e test first.
2. Wire: the optional fields and the `orchestrator.freeze` variant (after `wire-format-dtos` and
   `supervisor-vocabulary-split` if they have landed).
3. Delivery: nudge triggers, freeze state, the `[merge-check]` question.
4. `sweep.sh` and `broker.sh` verbs, both skills, the drive-summary rule, docs.

Rollback is a revert. No persisted state changes. An older broker ignores the new optional
fields.

## Open Questions

1. **Freeze on a rebase directive.** When the orchestrator asks a frozen worker to rebase,
   feedback lifts the freeze (D5). Should `feedback-gate` keep the freeze for a gate failure the
   orchestrator wants fixed in a fresh commit? This change says every feedback thaws.
2. **Should `rewritten` nudge verification when the worker rebased its own branch** (on the
   orchestrator's request)? D1 in `commit-event-truth` says no: the worker's following `done` is
   the trigger.
