## Context

Broker events come from four sources. All line references are at v0.20.0 (`e213707`).

- **The shared post-commit hook** (`assets/hooks/post-commit.sh`, installed by
  `src/agents.rs:571` into the common git dir). It sources the worktree's `paw-agent-id` marker
  and publishes `agent.artifact { status: "committed" }` for every commit.
- **The watcher** (`src/broker/watcher.rs`). Every 2 s it runs `git rev-parse HEAD` and
  `git status --porcelain` per watch target. It marks progress on a moved `HEAD` and publishes
  `agent.status working` with the dirty-file set when that set changes.
- **The delivery layer** (`src/broker/delivery.rs::publish_message`). It routes each message and
  emits the broker's own `supervisor.verify-now` nudge.
- **The conflict detector** (`src/broker/conflict.rs`). It turns intents and statuses into
  `agent.feedback` and `agent.question`.

Root-side verbs come from `sweep.sh`, and worker-side verbs from `broker.sh`.

### Defects, with their source

| Finding | Where | What the code does |
|---|---|---|
| DF-35/50/56, DV-13 | `assets/hooks/post-commit.sh:7-13` | Publishes for every commit, with no check for a rebase in progress (`rebase-merge`/`rebase-apply` under the gitdir), so a rebase of N commits publishes N artifacts. |
| DF-35/50/56, DV-13 | `src/broker/delivery.rs:322-331` | Every `committed` artifact emits a `VerifyNow`, so each replayed commit draws a nudge. |
| DF-56 (`index.lock`) | `assets/hooks/post-commit.sh:9` | `git diff HEAD~1 --name-only` compares `HEAD~1` with the **working tree**, not with `HEAD`. It lists uncommitted files too, refreshes the index (taking `index.lock` opportunistically), and prints nothing on a root commit. |
| DF-56 (`index.lock`) | `src/broker/watcher.rs:80-87` | `git status --porcelain` runs without `--no-optional-locks`, so every 2 s it may take `index.lock` to refresh the index. A rebase that needs the lock at that moment stops. |
| DF-35, DV-08, DV-10 | `assets/hooks/post-commit.sh:12` | `agent_id` is always the marker's agent. An orchestrator committing with `git -C <worker-worktree>` is published as the worker. No environment marker distinguishes the orchestrator's processes today. |
| DV-08 (second reading) | `src/broker/conflict.rs:270` | `CONFLICT_DETECTOR_SENDER = "supervisor"`: detector warnings and questions carry the orchestrator's identity, so they read as edits or rulings by it. See Open Questions. |
| DF-21 | `src/broker/delivery.rs:322-331`, `assets/agent-skills/coordination/SKILL.md:506-509` | The nudge fires on every `committed`. The skill calls each commit "the canonical done signal". A `done` artifact (`broker.sh artifact`) draws no nudge. |
| DF-21 | `src/broker/messages.rs:600-604` | `VerifyNow` carries only `branch_id`: no SHA and no trigger, so the orchestrator cannot tell a section commit from the last one. |
| DF-37/40, PK-12 | `assets/hooks/pre-commit.sh:7-20` | The only commit guard is the branch guard. Nothing marks a branch as under verification. |
| PK-16 | `src/commands/send.rs:189-190` | `__send` refuses a mid-response agent. That is correct, so the stop has to come through the broker and the hook instead (D5). |
| DV-09/10 | `src/broker/conflict.rs:836-850` | `record_in_flight_pair` re-warns whenever the triple's decision record is missing. |
| DV-09/10 | `src/broker/conflict.rs:635-656` | `sweep_in_flight_pairs` deletes **acknowledged** records once their overlap has been absent for the retention (60 s floor). A recurring overlap then warns both agents again and later re-escalates. |
| DV-09/10 | `src/broker/conflict.rs:874-883` | `acknowledge` suppresses only the escalation. |
| DV-15 | `src/commands/remove.rs:184-197` | `detach_worktree` tears the worktree down while its watcher is still live. |
| DV-15 | `src/broker/watcher.rs:166-176` | The watcher deregisters only once the path is gone. Until then it publishes the deletions as the departed agent's `working` set. |
| DV-15 | `src/broker/conflict.rs:595-602` | `update_status` replaces an agent's set, but nothing removes an agent, so its last set lingers and overlaps every later status. |
| DV-17 | `src/broker/mod.rs:495`, `src/broker/mod.rs:514-522` | `stalled` is `idle > threshold`. Idle counts only phase changes, artifacts, verdicts and `HEAD` moves, so a turn that reads, thinks or runs a long test reads as a stall. |
| PK-13 | `src/broker/messages.rs:179-184` | `VerifiedPayload` has no SHA. |
| PK-13 | `sweep.sh:1040-1062`, `sweep.sh:1142-1175` | `verified` sends only a message. `advanced-main` sends the new main SHA but not the merged branch's tip, so nothing can compare the two. |
| PK-14 | `src/supervisor/drive.rs:3172-3185` | `merge_state` is "tip is an ancestor of default". One commit after the merge flips a merged branch to unmerged. |
| #33 | `src/broker/server.rs:388` | `POST /publish` answers a bare `202`, so a sender never learns its message's sequence number. |
| #33 | `src/broker/delivery.rs:493-517` | `poll_messages` is stateless (the cursor is the client's), so the broker cannot say whether an agent ever read a message. |
| #33 | `sweep.sh:1015-1038` | `feedback-gate` prints only `feedback (<gate>) -> <agent>`. |

## Goals / Non-Goals

**Goals:**
- one event per real change, attributed to whoever made it;
- explicit "complete" and "frozen" states;
- conflict acknowledgements that hold;
- removal that leaves no ghost;
- a stall signal that tells "busy" from "dead";
- merge and verification checked against SHAs;
- read state for every orchestrator message.

**Non-Goals:**
- interrupting a mid-response worker (PK-16's `tell` half; proposed as its own change);
- authenticating worker senders;
- changing `finish` (noted as a follow-up);
- replaying missed events after a broker restart.

## Decisions

### D1 — Replay detection in the hook: rebase only
The post-commit hook returns early when `$(git rev-parse --git-path rebase-merge)` or
`…/rebase-apply` exists, the states `git rebase` and `git am` hold while replaying.

- A single `git cherry-pick` or `git commit --amend` is new work by whoever runs it, so it still
  publishes. The branch-guard remedy itself tells a worker to cherry-pick.
- A new `post-rewrite` hook, installed with the same managed-block logic, publishes **one**
  `agent.artifact { status: "rewritten" }` when git passes it `rebase`. It carries the new tip
  `sha` and the union of the rewritten commits' files.
- `rewritten` is not `committed`, so no nudge fires and the roster's sticky-terminal rule is
  unaffected.

*Alternative:* debounce in the broker (collapse N artifacts in T seconds). Rejected: it guesses,
and it still holds the lock during the replay.

### D2 — Exact, lock-free commit file list
`git diff-tree --no-commit-id --name-only -r --root HEAD` lists exactly the commit's files,
handles a root commit, and reads no index. The hook also sends `sha` (`git rev-parse HEAD`). The
watcher's git calls run as `git --no-optional-locks …`, which makes `git status` skip its
opportunistic index refresh-and-write (available since git 2.15; check it against the minimum
git version `doctor` enforces).

The watcher also skips publishing for a worktree whose gitdir holds `rebase-merge`,
`rebase-apply`, `MERGE_HEAD` or `CHERRY_PICK_HEAD`. Mid-operation dirty sets are conflict
markers and replays, not edits. The `HEAD` observation still runs, so progress is unaffected.

### D3 — Attribution through an environment marker
git-paw exports `GIT_PAW_ACTOR=orchestrator` into the orchestrator CLI's environment, both in
the foreground launch and in the root pane. Worker panes never get it. The post-commit and
post-rewrite hooks publish **nothing** when it is set: the orchestrator knows its own commits,
and publishing them under any id draws false nudges or broadcasts. The watcher still sees the
moved `HEAD`, so wave progress is kept.

*Alternative:* compare the committer identity. Rejected: workers and the orchestrator share the
user's git identity.

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

### D6 — Read cursor, not delivery receipts
`GET /messages/<id>?since=N` is the only read path. The broker records
`read_through[id] = max(read_through, N)`: a poll from `N` proves the agent processed everything
up to `N`.

- `/status` gains, per agent, `read_through` and `unread`. `unread` counts the messages in its
  inbox with `seq > read_through` that came from the orchestrator, and gives the oldest one's
  `seq` and age.
- `POST /publish` answers `202 {"seq": N}`, so `sweep.sh` can print `seq N` and
  `sweep.sh delivery <agent> <seq>` can answer `read` or `unread`.
- When a `committed` artifact arrives from an agent with unread orchestrator feedback, the
  broker puts one `agent.question` into the orchestrator inbox, tagged `[delivery]` and naming
  the agent, the commit SHA and the unread `seq`s. This is the #33 case.

### D7 — Acknowledgement scope and lifetime
`acknowledge` marks the record `warned`, so no in-flight warning follows. The retention sweep
skips acknowledged records. An acknowledged record ends only when:
- the overlap shape changes materially (the existing rule);
- either agent is withdrawn (D8);
- or the session ends.

The memory bound is the number of acknowledged pairs, which is small.

### D8 — Withdrawal before teardown
A new `DELETE /watch` (body `{"worktree_path"}`) does three things: it forgets the target, stops
its watcher task through a per-target cancel token, and calls `ConflictTracker::withdraw(agent)`.
`withdraw` drops the agent's current files, its intent and every in-flight triple and decision
involving it.

`remove` calls `DELETE /watch` after killing the pane and before `detach_worktree`. If the broker
is unreachable, it warns and continues, because removal never depends on the broker.
`git-add-remove-branch`'s "discovers removal passively" rule is about the orchestrator and the
drive loop, which are still not signalled.

### D9 — Pane activity is not progress, but it is not a stall either
The broker host samples each agent's surface every 30 s through the terminal backend. It applies
the prompt profile's responsiveness check (the one `__send` uses) and hashes the capture. A pane
that is mid-response *and* whose capture changed since the last sample marks **activity**.

- `wave` gains `last_activity_unix_secs` and `busy_agents`.
- `stalled` becomes `threshold > 0 && idle > threshold && since_activity > threshold`.
- `idle_seconds` and progress are unchanged, so the existing "not progress" rules hold.
- A hung stream shows a static pane, so it still stalls.

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
- [Pane sampling costs a capture per agent every 30 s] → it is the same capture the drive loop
  takes every few seconds already, and the backend call is off the async workers (`broker-server`
  rule).
- [`--no-optional-locks` means the watcher's status may run on a stale stat cache] → it is
  correct, only slower on huge trees, and the next real git command refreshes the index.
- [A `[delivery]` question for feedback the worker read but did not poll past] → the cursor is
  the worker's `since`. The coordination skill's poll-before-commit rule closes the gap.

## Migration Plan

1. Hooks: replay guard, `diff-tree`, `sha`, `post-rewrite`, actor check. Hook e2e tests first.
2. Watcher: `--no-optional-locks` and the in-progress check.
3. Wire: the optional fields and the `orchestrator.freeze` variant (after `wire-format-dtos` and
   `supervisor-vocabulary-split` if they have landed).
4. Delivery: nudge triggers, read cursor, `[delivery]` and `[merge-check]` questions.
5. Conflict detector: acknowledgement scope and lifetime, `withdraw`. Then `DELETE /watch` and
   `remove`.
6. Wave activity sampling.
7. `sweep.sh` verbs, both skills, the drive-summary rule, docs.

Rollback is a revert. No persisted state changes. An older broker ignores the new optional
fields, and an older hook body is replaced on the next `start`.

## Open Questions

1. **DV-08's second reading.** Detector messages carry `from = "supervisor"`
   (`conflict.rs:270`), so a reader can take them for the orchestrator's own edits or rulings.
   Should the detector get its own sender id (`conflict-detector`)? The acknowledgement filter
   would then check the root identity, and the re-entrancy guard the detector id. This change
   assumes the attribution in DV-08 is the orchestrator-commit case (D3). The maintainer should
   confirm from the DV-08 transcript.
2. **Freeze on a rebase directive.** When the orchestrator asks a frozen worker to rebase,
   feedback lifts the freeze (D5). Should `feedback-gate` keep the freeze for a gate failure the
   orchestrator wants fixed in a fresh commit? This change says every feedback thaws.
3. **Pane sampling in the broker host, or a drive-loop heartbeat?** D9 puts it in the host, so
   attended sessions get it too. The cost is a terminal-backend dependency in `__broker`, which
   so far runs without `$TMUX`. It could instead be its own change if the maintainer prefers.
4. **Should `rewritten` nudge verification when the worker rebased its own branch** (on the
   orchestrator's request)? D1 says no: the worker's following `done` is the trigger.
5. **Split.** If this change is too large for one build, the natural seams are:
   - commit-event truth (D1–D3, D7, D8);
   - the verification lifecycle (D4, D5, D10);
   - delivery and activity (D6, D9).
