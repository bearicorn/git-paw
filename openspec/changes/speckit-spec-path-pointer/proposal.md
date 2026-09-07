## Why

Every Spec Kit agent, every wave, burns a boot detour discovering where its spec
actually lives (peer-4-poker, quantified: all three agent boot prompts each wave).
`SpecBackendKind::SpecKit` falls through to the OpenSpec/Markdown branch of
`render_spec_path_doctrine`, so the boot prompt tells the agent to read siblings under
`openspec/changes/<id>/` — a path that does not exist under Spec Kit. The injected
sidecar carries the real content, so it is harmless but wastes a worker cycle, and a
cheap worker is the most likely to chase the dead path.

## What Changes

- The spec-path doctrine (the `SPEC_PATH_DOCTRINE` skill substitution rendered by
  `render_spec_path_doctrine`) points Spec Kit workers at the Spec Kit spec location
  (`specs/<feature>/`) — or the injected per-worktree sidecar — and no longer directs a
  Spec Kit worker to `openspec/changes/<id>/`.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `core-lang-agnostic`: the spec-path doctrine emits a Spec-Kit-specific spec-location
  pointer for a Spec Kit backend instead of the OpenSpec path.

## Impact

- **Code:** `src/skills.rs` `render_spec_path_doctrine` already names `.specify/specs/<feature>/`
  for `SpecBackendKind::SpecKit` (pre-existing, unrelated to this change — task 1.1 is a
  no-op confirmation). **`SpecBackendKind` variant-ripple checklist applies** — grepping
  `SpecBackendKind::SpecKit` across `src/` (`render_spec_path_doctrine`, `build_task_prompt`,
  `mcp/query/specs.rs`, `backend_for_type` in `specs/mod.rs`) surfaced the actual bug in
  `build_task_prompt` (`src/main.rs`): it grouped SpecKit with `Markdown` and claimed sibling
  artifacts live under `openspec/changes/<id>/`, the nonexistent path this proposal's "Why"
  describes. Fixed with a dedicated SpecKit arm pointing at the sidecar only. No variant is
  added or removed.
- **Enum-variant ripple:** touches `SpecBackendKind` match arms in both `render_spec_path_doctrine`
  (unchanged) and `build_task_prompt` (new dedicated arm); no variant set change.
- **Backward compatibility:** OpenSpec / Markdown / Superpowers doctrine and task prompts are
  unchanged; additive SpecKit arm in `build_task_prompt`.
- **Docs:** internal boot prompt — no user-facing doc; the doctrine test gains a SpecKit
  case.
