## Context

`render_spec_path_doctrine` (`src/skills.rs`) renders the boot-prompt spec-path pointer
per backend; the OpenSpec arm names `openspec/changes/…`, and Spec Kit falls through to
it, so every Spec Kit worker is pointed at a nonexistent path. A single missing match arm.

## Goals / Non-Goals

**Goals:** Spec Kit workers are pointed at their real spec location; other backends
unchanged.

**Non-Goals:** reworking the boot prompt or the sidecar mechanism; the broader per-CLI
work (v0.18).

## Decisions

- **Add a SpecKit arm to the doctrine, sourced from the Spec Kit backend's known
  layout.** Point at `specs/<feature>/` (the Spec Kit feature directory) or the injected
  sidecar the worktree already carries. Prefer whichever the SpecKit backend already
  resolves, to avoid a second source of truth.
- **Respect the `SpecBackendKind` variant-ripple checklist.** The fix is in the doctrine,
  but confirm no sibling consumer (`build_task_prompt`, `mcp/query/specs.rs`) assumes
  SpecKit shares the OpenSpec path.

## Risks / Trade-offs

- **The exact SpecKit pointer (`specs/<feature>/` vs sidecar) is wrong** → the sidecar is
  always present in the worktree, so pointing at it is the safe default; the feature
  directory is the nicer pointer where resolvable.

## Migration Plan

Additive match arm; no config or state change. Rollback is a straight revert.

## Open Questions

- Whether to name `specs/<feature>/` or the sidecar as the primary pointer — resolved at
  implementation from what the SpecKit backend already exposes.
