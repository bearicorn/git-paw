## Why

git-paw scans three spec formats today — OpenSpec, plain Markdown, and Spec Kit
— to derive branches, agents, and boot prompts. A growing number of teams drive
their planning through [obra/superpowers](https://github.com/obra/superpowers),
a multi-CLI methodology plugin whose `writing-plans` skill produces scannable
implementation plans: bite-sized, checkbox-tracked tasks with exact file paths
and verification commands, saved to `docs/superpowers/plans/`. git-paw should
scan those plans the way it scans OpenSpec changes and Spec Kit features, so a
superpowers user gets the same one-command "spec → parallel worktrees" flow.
Slotted before the v1.0.0 CLI freeze so the frozen `--specs-format` contract
already covers the new value.

## What Changes

- **ADD** a new capability `superpowers-integration`: a `SuperpowersBackend`
  implementing the existing `SpecBackend` trait. It scans a directory of plan
  **files** (`docs/superpowers/plans/*.md`), parses each plan's header +
  `### Task N` sections + `- [ ]`/`- [x]` steps, identifies plans with
  incomplete work, and produces **one `SpecEntry` per incomplete plan** (a
  superpowers plan is a sequential TDD chain for a single worktree — there is no
  `[P]` parallel-task fan-out as in Spec Kit).
- **ADD** to `spec-scanning`: backend dispatch for the `superpowers` type;
  auto-detection of superpowers projects (`docs/superpowers/plans/` present);
  and `superpowers` as a valid `--specs-format` value. All additive — the
  existing `openspec` / `markdown` / `speckit` dispatch, auto-detect, and flag
  values are unchanged.

## Capabilities

### New Capabilities
- `superpowers-integration`: scans obra/superpowers `writing-plans` documents
  (`docs/superpowers/plans/*.md`) into `SpecEntry` values — one per incomplete
  plan — with boot prompts assembled from the plan header and its remaining
  tasks, and mid-flight `- [ ]` → `- [x]` writeback (mirroring the Spec Kit
  checkbox contract).

### Modified Capabilities
- `spec-scanning`: additive backend dispatch, auto-detection, and
  `--specs-format` acceptance for the `superpowers` type.

## Impact

- **Code:** new `src/specs/superpowers.rs` (`SuperpowersBackend` + plan parser),
  a `SpecBackendKind::Superpowers` variant and dispatch arm in `src/specs/mod.rs`,
  the `--specs-format` value list in CLI parsing, and the auto-detect probe.
  Reuses `slugify_branch` and the existing `- [ ]`/`- [x]` writeback machinery
  built for Spec Kit.
- **Export-agnostic:** the format is CLI-neutral markdown; the scanner MUST make
  no Claude-specific assumptions (per the "exported assets are project-agnostic"
  design principle).
- **Docs:** a mdBook chapter for the superpowers backend + a row in the
  spec-format table; `--specs-format` help text gains `superpowers`.
- **Backward compatible:** no change to existing backends; a project without
  `docs/superpowers/plans/` behaves exactly as before.
