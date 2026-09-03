## Why

git-paw's own workflow tells every agent to tick its own `- [x]` in a single shared
task-tracking file (a Spec Kit `specs/<feature>/tasks.md`, or the OpenSpec `tasks.md`),
then the ownership model assigns that file a single owner and flags every other agent's
writeback as an ownership violation (peer-4-poker, 2026-09-03: eight violations against
one `tasks.md`, the supervisor itself flagged). The signal is pure noise — and it
trained the operator to ignore a detector that had a **real** collision buried in it
(one agent edited a source file owned by another). git-paw manufactures the false
positives through its own writeback protocol.

## What Changes

- The conflict detector exempts the shared spec-task-tracking artifact — the tasks file
  git-paw directs every agent to update with its own checkbox — from ownership-violation
  detection, because a writeback there is expected coordination, not a collision.
- The exemption is scoped to that designated artifact only; ownership violations on
  genuine source files are still reported, so a real collision is never masked.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `broker-conflict-detection`: the shared spec-task-tracking artifact is exempt from
  ownership-violation detection.

## Impact

- **Code:** `src/broker/conflict.rs` ownership-violation path — exempt the active spec
  backend's task-tracking file (resolved from the spec path git-paw already knows per
  agent; reuse `is_managed_path()` / the spec-backend task-file resolution). Source-file
  violations are unaffected.
- **Enum-variant ripple:** none.
- **Backward compatibility:** additive filter; genuine ownership violations still fire.
- **Docs:** the Conflict Detection chapter notes the task-artifact exemption.
