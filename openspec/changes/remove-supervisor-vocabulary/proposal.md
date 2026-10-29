## Why

v0.21.0's `supervisor-vocabulary-split` shipped the coordinated-mode vocabulary — `--coordinated` /
`--bare`, `default_mode`, `[orchestrator]` / `[workers]` / `[drive]`, the broker root identity
`orchestrator`, the `orchestrator` skill name — and kept every old spelling working with a
deprecation warning, scheduled for removal in 1.0.0. 1.0.0 freezes the surface; anything still
accepted then would have to be carried for the whole 1.x line. This change removes the old
spellings so the frozen surface has no "supervisor" in it, and settles what happens to configs
that still use the version-1 `[supervisor]` table.

## What Changes

- **BREAKING:** `git paw start --supervisor` and `--no-supervisor` fail before any side effect with
  exit code `2` and a message naming the flag, the word "renamed", and `--coordinated` / `--bare`
  (the `remove-from-specs-alias` pattern).
- **BREAKING:** the broker no longer accepts `supervisor` as the root identity — in `agent_id`, the
  sender fields or the `/messages/supervisor` path — nor publishes of `supervisor.verify-now`; it
  answers `400` naming `orchestrator`. Event records written by 0.x releases are still read.
- **BREAKING:** the skill name `supervisor` no longer resolves (unknown-skill result naming
  `orchestrator`); a leftover `supervisor` override directory is no longer read and is reported.
- **BREAKING (machine output):** MCP `get_session_status` drops the deprecated `mode` field;
  `session_mode` remains.
- Session files are now written with `mode: "coordinated"`; files written by 0.x (`"supervisor"`)
  still load.
- `git paw remove supervisor` is no longer special (only `orchestrator` is refused).
- **Config files keep loading (decision (a)):** a version-1 file with `[supervisor]` still migrates
  in memory with its deprecation warning for the whole 1.x line, and the helper scripts keep their
  version-1 fallback. The no-effect `[supervisor.auto_approve]` and
  `[supervisor.common_dev_allowlist]` tables (deprecated in v0.17.0) still load and are dropped by
  the migration, never renamed. The `[supervisor]` keys are not entries of the stable surface: the manifest
  lists only version-2 keys, and a `version = 2` file still refuses `[supervisor]`.
- **Vocabulary sweep:** a standing guard keeps "supervisor" out of help, README, the user docs,
  exported assets, the default config, MCP descriptions and doctor output (except the migration
  table, the changelog and the stability chapter's removal list). The spec set is swept verbatim to
  the new spellings and the `supervisor-*` capabilities are renamed; `user-guide/supervisor.md`
  becomes `user-guide/coordinated-mode.md` with a redirect.
- Every registry entry scheduled for 1.0.0 is deleted (the `core-deprecation` guard enforces it).

**Archive order within v1.0.0:** `remove-from-specs-alias` archives first, then this change. Both
modify `cli-parsing` on disjoint requirements; this change's sweep is a direct reorganization of
`openspec/specs/`, which runs after deltas are folded in (spec-organization §6). It therefore
carries `remove-from-specs-alias`'s archived text — `--from-specs` stays removed (only its
rejection requirement names it), and *Spec mode + coordinated mode dispatch*, *Non-TTY launch
handling* and *--from-all-specs flag* already use the new vocabulary.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-parsing`: REMOVED *--supervisor and --no-supervisor are deprecated aliases*; ADDED the
  rejection with a migration message.
- `broker-protocol`: MODIFIED *The root session's broker identity is orchestrator* (no alias; the
  old spelling is refused on the live protocol).
- `session-state`: MODIFIED *The session mode is read in either spelling* (written `coordinated`).
- `mcp-server`: MODIFIED *Session status reports the session mode in coordinated terms* (`mode`
  removed).
- `skill-agent`: MODIFIED *Skill resolution order* (alias removed); REMOVED *The supervisor skill
  name is a deprecated alias*; ADDED the retired-name result and the stale-override notice.
- `git-add-remove-branch`: MODIFIED *remove refuses the root identity*.
- `core-configuration`: ADDED *Version-1 configuration files load throughout 1.x*.
- `core-project-conventions`: ADDED *User-facing surfaces carry no retired supervisor vocabulary*.

## Impact

- **Code:** `src/cli.rs` + `src/main.rs` (hidden rejected args; two rows added to the removed-flag
  table `remove-from-specs-alias` introduces for `--from-specs`); `src/broker/{wire,server}.rs` (alias removed,
  rejection; legacy decode kept for persisted records); `src/session.rs` (write `coordinated`);
  `src/mcp/query/session.rs`; `src/skills.rs` and `src/mcp/tools/project.rs`;
  `src/commands/remove.rs`; `src/deprecation.rs` (delete the 1.0.0 entries); `assets/scripts/sweep.sh`
  (drop the `supervisor` root-spelling acceptance; keep the config fallback).
- **Enum-variant ripple:** none (`BrokerMessage` variant set unchanged; only the alias in the wire
  mapper goes). `SpecBackendKind` untouched.
- **Specs and docs:** direct verbatim sweep of `openspec/specs/` (spec-organization §2/§6) with the
  `supervisor-*` capabilities renamed and `docs/src/specifications/` includes fixed in the same
  wave; `docs/src/user-guide/supervisor.md` renamed with an mdBook redirect; stability chapter's
  removal list; configuration reference keeps the migration table.
- **Cross-version dependencies:**
  - v0.21.0 `supervisor-vocabulary-split` (every requirement modified or removed here is one it
    added; it must be archived first) and its `core-deprecation` registry.
  - v0.24.0 `error-exit-code-contract` (registers `--from-specs` in the same registry).
  - v0.17.0 `classifier-retirement` deprecated `[supervisor.auto_approve]` and
    `[supervisor.common_dev_allowlist]` (parsed, no effect, one warning per file); v0.21.0 routes
    that warning through `deprecation::warn`, drops both tables in the version-1 → version-2
    migration and registers them with no removal release. This change keeps them loading, adds them
    to the registry table it relies on, and keeps their docs mentions confined to the migration
    table under the no-mention guard.
  - v0.20.0 `skill-override-resolution` MODIFIES *Skill resolution order* (three levels,
    `repo_root`, and the bundled `cli-setup` skill is never overridable). This change's MODIFIED
    block is written on that text: it keeps the three levels, the `cli-setup` paragraph and its
    scenarios, and drops only the `supervisor` alias. Re-diff it against the archived v0.20.0 text
    before apply (task 1.1).
  - v1.0.0 `cli-contract-freeze` (edited alongside this change: it freezes the new vocabulary and
    adopts the registry), `layer-isolation` (edited: its Supervised layer is the Coordinated
    layer), `remove-from-specs-alias` (archived before this change; same rejection pattern; it
    deletes its own registry entry, introduces the removed-flag table, writes its `cli-parsing` and
    `boot-block` copies in the coordinated-mode vocabulary, renames *Spec mode + supervisor mode
    dispatch* to *Spec mode + coordinated mode dispatch*, and leaves *Start subcommand with
    optional flags* as v0.21.0 restated it).
  - Later drafts that used the old names, v1.1.0 `registry-repo-profiles` (`[supervisor].cli`)
    and v1.2.0 `a2a-wire-model` (`agent_id = "supervisor"`, `supervisor.verify-now`), have adopted
    `[orchestrator] cli`, `orchestrator` and `orchestrator.verify-now`.
- **Security / safety:** one root spelling on the live wire removes the normalization path from
  the verdict-authority surface; the rejection is explicit, never a silent drop.
