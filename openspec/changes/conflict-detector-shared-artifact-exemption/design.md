## Context

git-paw's writeback protocol (every agent ticks its own checkbox in one shared
tasks file) structurally guarantees ownership-violation false positives, because the
ownership model gives that file a single owner. The noise masked a real collision in the
peer-4-poker run. Fixing it is a scoped exemption, mirroring GP-04a's `.git-paw/` overlap
exclusion.

## Goals / Non-Goals

**Goals:** writebacks to the shared task-tracking file never fire ownership violations;
real source-file collisions still do.

**Non-Goals:** per-line ownership of individual checkboxes (heavier; the file-level
exemption is sufficient and simpler); changing the writeback protocol itself.

## Decisions

- **Exempt the task-tracking file, not per-line ownership.** git-paw already knows each
  agent's spec path, so it can resolve the active spec backend's task-tracking file and
  exempt exactly that path from ownership-violation detection. Per-line ownership was
  considered and rejected as disproportionate — the file-level exemption removes the
  manufactured noise without weakening source-file detection.
- **Scope tightly.** Only the designated task-tracking artifact is exempt; forward-,
  in-flight-, and source-file ownership detection are unchanged, so a real collision is
  never masked (the peer-4-poker run had exactly one buried in the noise).

## Risks / Trade-offs

- **A genuine conflict inside the tasks file is now unflagged** → acceptable: git-paw
  itself directs concurrent writes there, and checkbox writebacks are line-local and
  merge trivially; source-file detection (where real conflicts live) is untouched.

## Migration Plan

Additive filter; no config or state change. Rollback is a straight revert.

## Open Questions

None.
