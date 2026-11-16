## Why

The orchestrator cannot tell whether a worker read a message, or whether a quiet worker is busy
or dead:

- **Feedback can go unread** (external feedback #33, v0.20.0 run). A rebase instruction sent
  with `feedback-gate` was never read, the worker committed without rebasing, and nothing told
  the orchestrator.
- **A busy wave reads `stalled`** (DV-17). A worker visibly mid-turn for 30 minutes, editing
  nothing, is indistinguishable from a dead one.

This is the third of three changes split from `broker-event-semantics` (2026-11-16). It lands
after `verification-lifecycle`, whose `freeze` verb and `orchestrator.freeze` message its
requirements name.

## What Changes

- **Delivery is visible.** `POST /publish` returns the message's sequence number. The broker
  records how far each agent has read its inbox, and `/status` shows unread messages.
  `sweep.sh delivery` answers "was it read?". A commit made while orchestrator feedback is unread
  raises a question.
- **A busy agent is not a stall.** The broker host samples each agent's pane. A mid-response pane
  whose content is changing is *activity*: it keeps `stalled` false and is listed, but it is not
  progress.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-runtime`: ADDED the sequence number in the publish response, per-agent read cursors,
  escalation of a commit made while feedback is unread, and pane activity in the wave status.
- `broker-agent-helper`: ADDED sequence numbers in the verdict verbs' output, and
  `sweep.sh delivery`.
- `skill-agent`: ADDED an inbox poll before each commit.

## Impact

- **Code:**
  - `src/broker/delivery.rs` (read cursors, unread-commit escalation, `publish_message` returns
    the seq);
  - `src/broker/server.rs` (publish response body, `/status` fields);
  - `src/broker/mod.rs` (wave activity);
  - `src/commands/broker_host.rs` (pane sampling);
  - `assets/agent-skills/orchestrator/scripts/sweep.sh` (`delivery`, sequence numbers);
  - `assets/agent-skills/coordination/SKILL.md` and `assets/agent-skills/orchestrator/SKILL.md`.
- **Wire:** additive only. A `{"seq": N}` body on the `202` response to `POST /publish`, and new
  `/status` fields. Old publishers and readers keep working.
- **Tests:** delivery tests (read cursor, unread escalation, the publish seq), a wave-activity test
  with a fake pane backend, and `sweep.sh` tests for `seq` output and `delivery` exit codes.
- **Docs:** the coordination chapter (the read cursor) and the orchestrator skill.
- **Dependencies:**
  - **`supervisor-vocabulary-split`** and **`wire-format-dtos`** (this release): new text uses the
    orchestrator vocabulary, and the `/status` fields go into `broker::wire`.
  - **`verification-lifecycle`**: `sweep.sh freeze` prints its `seq` like the other verdict verbs,
    and an unread `orchestrator.freeze` counts as unread orchestrator feedback. Land that change
    first; `commit-event-truth` supplies the artifact `sha` the `[delivery]` question names.
