## Context

`resolve_specs_config` resolved the spec system with three tiers: `--specs-format`
→ `[specs]` config → filesystem auto-detection (`.specify/specs/` → speckit; and,
in-flight, `docs/superpowers/plans/` → superpowers). `git paw init` mirrored the
detection to pre-fill `[specs]`. This makes the config non-authoritative and adds
precedence rules. The fix removes the detection tier entirely.

## Goals / Non-Goals

**Goals:**
- Config `[specs]` or `--specs-format` is the sole source of the spec system.
- Unconfigured → actionable error, never a filesystem guess.
- `git paw init` records an explicit choice (prompt) or a commented template.

**Non-Goals:**
- No change to the backends themselves or to `--specs-format`'s conventional
  per-format default dir (a CLI-driven convenience, not detection).
- No change to the `.specify/memory/constitution.md` probe or governance
  constitution auto-wiring — those are gated on an already-configured
  `type = "speckit"` and do not select the spec system.

## Decisions

- **Drop the detection tier, keep the `--specs-format` default-dir.** Supplying
  `.specify/specs` / `docs/superpowers/plans` when the CLI names a format is a
  convenience tied to the explicit choice, not filesystem probing.
- **Init prompts (interactive) or writes a commented template (non-interactive).**
  Mirrors the existing `prompt_supervisor_section` TTY gating, so init stays
  scriptable and never blocks in CI.
- **`resolve_specs_config` loses its `repo_root` parameter** — with no probing,
  it no longer touches the filesystem.

## Risks / Trade-offs

- [Breaking: `.specify/`-only Spec Kit users with no `[specs]` now error] →
  Mitigation: the error names both remedies; `git paw init` (re-run) or a
  one-line `[specs]` addition restores operation. Only shipped Spec Kit
  auto-detection is affected; Superpowers auto-detection never shipped.

## Open Questions

- None.
