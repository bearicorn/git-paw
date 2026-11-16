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

### Defects, with their source

| Finding | Where | What the code does |
|---|---|---|
| DF-35/50/56, DV-13 | `assets/hooks/post-commit.sh:7-13` | Publishes for every commit, with no check for a rebase in progress (`rebase-merge`/`rebase-apply` under the gitdir), so a rebase of N commits publishes N artifacts. |
| DF-35/50/56, DV-13 | `src/broker/delivery.rs:322-331` | Every `committed` artifact emits a `VerifyNow`, so each replayed commit draws a nudge. |
| DF-56 (`index.lock`) | `assets/hooks/post-commit.sh:9` | `git diff HEAD~1 --name-only` compares `HEAD~1` with the **working tree**, not with `HEAD`. It lists uncommitted files too, refreshes the index (taking `index.lock` opportunistically), and prints nothing on a root commit. |
| DF-56 (`index.lock`) | `src/broker/watcher.rs:80-87` | `git status --porcelain` runs without `--no-optional-locks`, so every 2 s it may take `index.lock` to refresh the index. A rebase that needs the lock at that moment stops. |
| DF-35, DV-08, DV-10 | `assets/hooks/post-commit.sh:12` | `agent_id` is always the marker's agent. An orchestrator committing with `git -C <worker-worktree>` is published as the worker. No environment marker distinguishes the orchestrator's processes today. |
| DV-08 (second reading) | `src/broker/conflict.rs:270` | `CONFLICT_DETECTOR_SENDER = "supervisor"`: detector warnings and questions carry the orchestrator's identity, so they read as edits or rulings by it. See Open Questions. |
| DV-09/10 | `src/broker/conflict.rs:836-850` | `record_in_flight_pair` re-warns whenever the triple's decision record is missing. |
| DV-09/10 | `src/broker/conflict.rs:635-656` | `sweep_in_flight_pairs` deletes **acknowledged** records once their overlap has been absent for the retention (60 s floor). A recurring overlap then warns both agents again and later re-escalates. |
| DV-09/10 | `src/broker/conflict.rs:874-883` | `acknowledge` suppresses only the escalation. |
| DV-15 | `src/commands/remove.rs:184-197` | `detach_worktree` tears the worktree down while its watcher is still live. |
| DV-15 | `src/broker/watcher.rs:166-176` | The watcher deregisters only once the path is gone. Until then it publishes the deletions as the departed agent's `working` set. |
| DV-15 | `src/broker/conflict.rs:595-602` | `update_status` replaces an agent's set, but nothing removes an agent, so its last set lingers and overlaps every later status. |

## Goals / Non-Goals

**Goals:**
- one event per real change, attributed to whoever made it;
- conflict acknowledgements that hold;
- removal that leaves no ghost.

**Non-Goals:**
- the change-complete, freeze and merge-truth signals (`verification-lifecycle`);
- read state and pane activity (`broker-delivery-and-activity`);
- authenticating worker senders;
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

Decision numbers are kept from `broker-event-semantics`; D4, D5 and D10 are in
`verification-lifecycle`, D6 and D9 in `broker-delivery-and-activity`.

## Risks / Trade-offs

- [`--no-optional-locks` means the watcher's status may run on a stale stat cache] → it is
  correct, only slower on huge trees, and the next real git command refreshes the index.

## Migration Plan

1. Hooks: replay guard, `diff-tree`, `sha`, `post-rewrite`, actor check. Hook e2e tests first.
2. Watcher: `--no-optional-locks` and the in-progress check.
3. Wire: the optional `sha` and the `rewritten` status (after `wire-format-dtos` and
   `supervisor-vocabulary-split` if they have landed).
4. Conflict detector: acknowledgement scope and lifetime, `withdraw`. Then `DELETE /watch` and
   `remove`.

Rollback is a revert. No persisted state changes. An older broker ignores the new optional
field, and an older hook body is replaced on the next `start`.

## Open Questions

1. **DV-08's second reading.** Detector messages carry `from = "supervisor"`
   (`conflict.rs:270`), so a reader can take them for the orchestrator's own edits or rulings.
   Should the detector get its own sender id (`conflict-detector`)? The acknowledgement filter
   would then check the root identity, and the re-entrancy guard the detector id. This change
   assumes the attribution in DV-08 is the orchestrator-commit case (D3). The maintainer should
   confirm from the DV-08 transcript.
