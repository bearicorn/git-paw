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
- **Ripple finding (task 1.2): `render_spec_path_doctrine` already had a correct SpecKit
  arm** (added when the function was introduced, unrelated to this change) — task 1.1 is
  satisfied by existing code. The variant-ripple grep found the actual bug in
  `build_task_prompt` (`src/main.rs`): it grouped `SpecBackendKind::Markdown` and
  `SpecBackendKind::SpecKit` into one arm claiming sibling artifacts live under
  `openspec/changes/<id>/` — the literal path this proposal's "Why" describes workers
  chasing. Split into its own arm pointing at the sidecar only (the Spec Kit
  decomposition already embeds spec + plan + task phase inline, so no sibling directory
  exists to name); `mcp/query/specs.rs` and `specs/mod.rs::backend_for_type` already
  handle SpecKit distinctly. `openspec/changes/speckit-spec-path-pointer/specs/` gained a
  matching requirement/scenario for traceability.

## Risks / Trade-offs

- **The exact SpecKit pointer (`specs/<feature>/` vs sidecar) is wrong** → the sidecar is
  always present in the worktree, so pointing at it is the safe default; the feature
  directory is the nicer pointer where resolvable.

## Migration Plan

Additive match arm; no config or state change. Rollback is a straight revert.

## Open Questions

- Whether to name `specs/<feature>/` or the sidecar as the primary pointer — resolved at
  implementation from what the SpecKit backend already exposes.
