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

- **Code:** `src/skills.rs` `render_spec_path_doctrine` — add a `SpecBackendKind::SpecKit`
  arm naming `specs/<feature>/` (or the sidecar). **`SpecBackendKind` variant-ripple
  checklist applies** — this change is scoped to the doctrine rendering, but grep
  `SpecBackendKind::SpecKit` across `src/` (`render_spec_path_doctrine`, `build_task_prompt`,
  `mcp/query/specs.rs`, `backend_for_type` in `specs/mod.rs`) to confirm no other site
  silently equates SpecKit with the OpenSpec path. No variant is added or removed.
- **Enum-variant ripple:** touches `SpecBackendKind` match arms (doctrine only); no
  variant set change.
- **Backward compatibility:** OpenSpec / Markdown / Superpowers doctrine is unchanged;
  additive SpecKit arm.
- **Docs:** internal boot prompt — no user-facing doc; the doctrine test gains a SpecKit
  case.
