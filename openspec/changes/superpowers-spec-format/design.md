## Context

git-paw already has a `SpecBackend` trait with three implementations
(`OpenSpecBackend`, `MarkdownBackend`, `SpecKitBackend`) dispatched by
`specs.type`, plus auto-detection and a `--specs-format` flag. Adding
obra/superpowers is a fourth backend of the same shape. The superpowers
`writing-plans` skill saves plan files to `docs/superpowers/plans/` with a fixed
header (`REQUIRED SUB-SKILL: …`, `Goal`/`Architecture`/`Tech Stack`) and
`### Task N` sections whose steps are `- [ ]` checkboxes with `Files:` and
`Run:` metadata — the same checkbox-writeback contract git-paw already handles
for Spec Kit.

## Goals / Non-Goals

**Goals:**
- A `SuperpowersBackend` that turns each incomplete plan file into one
  `SpecEntry`, reusing `slugify_branch` and the Spec Kit checkbox/writeback code.
- Additive dispatch + `--specs-format` support.
- Fully CLI-agnostic parsing (export policy).

**Non-Goals:**
- No `[P]` parallel-task fan-out — a superpowers plan is one worktree's work.
- No filesystem auto-detection of the spec system — selection is config/CLI only
  (see the `spec-selection-explicit-only` change).
- No scanning of the `docs/superpowers/specs/*-design.md` design docs — those are
  context/why, not the task-bearing artifact.

## Decisions

- **Flat plan files, one entry per plan.** Superpowers plans are self-contained
  sequential TDD chains; fanning them out would break the intended
  single-worktree, ordered execution. Alternative considered: per-`### Task`
  entries — rejected because tasks in a plan are ordered and interdependent.
- **Reuse the Spec Kit checkbox + writeback machinery.** The `- [ ]`/`- [x]`
  parsing and mid-flight writeback already exist; the superpowers parser differs
  only in the surrounding structure (flat files, `### Task N`, `Files:`/`Run:`).
- **No auto-detection.** The spec system is chosen explicitly via config
  `[specs]` or `--specs-format`; `--specs-format superpowers` supplies the
  conventional `docs/superpowers/plans` dir when none is configured. (The
  `spec-selection-explicit-only` change removes filesystem auto-detection across
  all backends.)

## Risks / Trade-offs

- [The `writing-plans` header/structure is a convention, not a pinned schema, and
  may drift upstream] → Mitigation: parse leniently (ignore unrecognised lines),
  anchor on the header marker + `### Task`/checkbox shapes, and skip files that
  don't look like plans rather than erroring. Track the upstream skill.

## Open Questions

- Exact robustness of the header-marker match (upstream wording variants) —
  resolve at apply by inspecting the installed `writing-plans` skill source and
  a couple of real plan files; default to matching the stable
  `subagent-driven-development` sub-skill phrase.
- Whether completed (`- [x]`) steps are echoed into the boot prompt for context
  or omitted — default to including them marked-done; resolve at apply.
