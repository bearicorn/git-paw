## Context

All line references are at v0.20.0 (`e213707`). Workers read their inbox with
`GET /messages/<id>?since=N`; the orchestrator publishes verdicts and feedback through
`sweep.sh`. The wave status in `/status` decides `stalled` from idle time.

### Defects, with their source

| Finding | Where | What the code does |
|---|---|---|
| #33 | `src/broker/server.rs:388` | `POST /publish` answers a bare `202`, so a sender never learns its message's sequence number. |
| #33 | `src/broker/delivery.rs:493-517` | `poll_messages` is stateless (the cursor is the client's), so the broker cannot say whether an agent ever read a message. |
| #33 | `sweep.sh:1015-1038` | `feedback-gate` prints only `feedback (<gate>) -> <agent>`. |
| DV-17 | `src/broker/mod.rs:495`, `src/broker/mod.rs:514-522` | `stalled` is `idle > threshold`. Idle counts only phase changes, artifacts, verdicts and `HEAD` moves, so a turn that reads, thinks or runs a long test reads as a stall. |

## Goals / Non-Goals

**Goals:**
- read state for every orchestrator message;
- a stall signal that tells "busy" from "dead".

**Non-Goals:**
- delivery receipts pushed to the sender;
- replaying missed events after a broker restart.

## Decisions

Decision numbers are kept from `broker-event-semantics`.

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

### D9 — Pane activity is not progress, but it is not a stall either
The broker host samples each agent's surface every 30 s through the terminal backend. It applies
the prompt profile's responsiveness check (the one `__send` uses) and hashes the capture. A pane
that is mid-response *and* whose capture changed since the last sample marks **activity**.

- `wave` gains `last_activity_unix_secs` and `busy_agents`.
- `stalled` becomes `threshold > 0 && idle > threshold && since_activity > threshold`.
- `idle_seconds` and progress are unchanged, so the existing "not progress" rules hold.
- A hung stream shows a static pane, so it still stalls.

## Risks / Trade-offs

- [Pane sampling costs a capture per agent every 30 s] → it is the same capture the drive loop
  takes every few seconds already, and the backend call is off the async workers (`broker-server`
  rule).
- [A `[delivery]` question for feedback the worker read but did not poll past] → the cursor is
  the worker's `since`. The coordination skill's poll-before-commit rule closes the gap.

## Migration Plan

1. Delivery: the publish seq, the read cursor, the `[delivery]` question.
2. Wave activity sampling.
3. `sweep.sh` verbs, both skills, docs.

Rollback is a revert. No persisted state changes. Clients that ignore the publish body keep
working.

## Resolved Questions (2026-11-17)

1. **Pane sampling:** in the broker host, as D9 says, so attended sessions get the activity
   signal too. `__broker` takes the terminal-backend dependency.
