## Why

Several requirements in the main specs describe behaviour that was removed, or never shipped,
and some contradict other requirements in the same spec. This is DS-7.

- In v0.16.0 git-paw stopped writing permission grants into CLI settings files (CHANGELOG
  0.16.0: "Stop seeding inert command allowlists into cli settings"). No code under `src/`
  writes `settings.json`, `allowed_bash_prefixes` or a `.claude/` directory. A test
  (`src/main.rs` `attach_agent_leaves_a_pre_existing_settings_file_byte_identical`) pins that.
  Even so, `approval-command-safety` still has ten requirements that mandate seeding. They
  contradict that spec's own *No permission grants are seeded into agent CLI settings files*.
- In `supervisor-learnings`, *MCP get_learnings consumes the variant* requires `get_learnings`
  to prefer broker records and return a `source` field. The shipped tool
  (`src/mcp/tools/session.rs` ~:94) only parses the file and has no `source` field. That matches
  `mcp-server`'s scenario "get_learnings parses the session-learnings.md file" and the user guide
  (`docs/src/user-guide/learnings.md` ~:325). The *User guide includes a Learnings Mode chapter*
  requirement right after it still says the chapter must describe `agent.learning` as "deferred to
  v0.6.0", although it shipped and the chapter documents it.
- A few other requirements still describe seeding: the stack-preset requirements say "the
  seeder", `supervisor-config` says `enabled` "controls the seeder", `cli-resolution`'s boot
  registration depends on "broker-curl seeding", and a `sweep.sh learn` scenario starts with
  "WHEN the supervisor's permission allowlist is seeded".

- **DS-7 addition — the learning id is not stable across toolchains.** `supervisor-learnings`
  *Deterministic id for idempotent re-emission* specifies the `agent.learning` id as a SHA-256
  prefix, but `LearningRecord::deterministic_id` (`src/broker/learnings.rs` ~:172) hashes with
  std's `DefaultHasher`, whose algorithm is unspecified and may change between Rust releases — a
  toolchain bump can silently change every id, defeating the dedup the id exists for. SHA-256 is
  not an option either: no `sha2` crate is in the approved dependency table (AGENTS.md).

For the stale requirements, the code is right and the specs need to catch up before the v1.0
contract freeze. For the learning id, neither the spec nor the code is right: the fix pins a
stable, dependency-free hash in both.

## What Changes

- REMOVE the ten seeding requirements from `approval-command-safety`.
- MODIFY *Standard preset content*, *Named stack presets* and *User-extensible allowlist via
  `extra` field* so they describe what the presets do today. `effective_patterns(stacks, extra)`
  composes them into the auto-approve whitelist
  (`SupervisorConfig::effective_whitelist`, `src/config/supervisor.rs` ~:927), and the universal
  preset is rendered into the supervisor skill's `{{DEV_ALLOWLIST_PRESET}}` guidance. The preset
  contents, exclusions and dedup/ordering rules stay word-for-word.
- REMOVE *MCP get_learnings consumes the variant*. `mcp-server` remains the only owner of the
  file-only `get_learnings` contract.
- MODIFY *User guide includes a Learnings Mode chapter*: drop the "deferred to v0.6.0" clause and
  its scenario. Require the chapter to document broker publication and that `get_learnings()`
  reads the file.
- MODIFY *Bundled `sweep.sh learn` subcommand*: its "no broad curl grant" scenario is now stated
  through the managed-helper classifier rule instead of a seeded allowlist.
- MODIFY `supervisor-config` *Common dev allowlist sub-table*: `enabled` is parsed for
  compatibility and has no effect; `extra` feeds the auto-approve whitelist.
- MODIFY `cli-resolution` *End-to-end boot registration*: no "broker-curl seeding"
  precondition.
- MODIFY `supervisor-learnings` *Deterministic id for idempotent re-emission*: the id is the
  64-bit FNV-1a hash (hand-rolled, no dependency) of the existing canonical serialisation, as 16
  lowercase hex characters, with a golden-value scenario. This is the one place the scheme is
  specified. Ids change once on upgrade (they were `DefaultHasher` output). `sweep.sh learn` is
  brought onto the same scheme by `bundled-helper-discovery` (wave 4), which ADDs a requirement
  that points at this one rather than restating it.
- One behaviour change: the learning-id hash (`LearningRecord::deterministic_id`). Everything
  else is spec deltas, tests for scenarios that have none yet, and a comment-only fix to the stale
  "seed a grant" header comments in the `broker.sh` and `docs-fetch.sh` assets. Code changes beyond
  that happen only if a test still asserts removed behaviour; none was found.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `approval-command-safety`: REMOVED ×10. MODIFIED *Standard preset content*, *Named stack
  presets*, *User-extensible allowlist via `extra` field*.
- `supervisor-learnings`: REMOVED *MCP get_learnings consumes the variant*. MODIFIED *User guide
  includes a Learnings Mode chapter*, *Bundled `sweep.sh learn` subcommand* and *Deterministic id
  for idempotent re-emission*.
- `supervisor-config`: MODIFIED *Common dev allowlist sub-table*.
- `cli-resolution`: MODIFIED *End-to-end boot registration*.

## Impact

- `src/broker/learnings.rs` — `LearningRecord::deterministic_id` switches from `DefaultHasher`
  to a hand-rolled FNV-1a 64-bit hash over the unchanged canonical string (`canonical_value`,
  `hour_bucket`); its doc comment is updated; a golden-value test pins a known id. No new
  dependency. `docs/src/user-guide/learnings.md` (~:307) names the hash and the one-time id change.
- Otherwise specs only, plus doc prose that no longer matches. `docs/src` already describes no seeding
  (`configuration/README.md` ~:905-948, `user-guide/coordination.md` ~:125-143,
  `user-guide/supervisor.md` ~:585-610) and a file-only `get_learnings`. Tasks re-check this and
  fix anything left over.
- `assets/scripts/broker.sh` (~:18-22) and `assets/scripts/docs-fetch.sh` (~:15-18): header
  comments only. Installed copies will show as content drift in `doctor` until `git paw init`
  refreshes them, which is the normal path for asset updates.
- `{{#include}}` coupling: `docs/src/specifications/README.md` includes `cli-resolution/spec.md`
  by path, so the archived text appears there. It only links `approval-command-safety`,
  `supervisor-learnings` and `supervisor-config`. No paths change, so no include breaks.
- Tests: no test asserts removed behaviour. The preset scenarios are covered by
  `src/supervisor/dev_allowlist.rs` tests and `src/config/tests.rs` `effective_whitelist_*`.
  Tasks add any scenario test that is missing, plus a doc-guard test for the learnings chapter.
- The `## Purpose` paragraph of `approval-command-safety` still describes seeding. A delta cannot
  edit it, so the archiver rewrites it after `openspec archive` (see tasks).
- **Collisions**: none. No pending change MODIFIES or REMOVES any requirement touched here.
  Pending changes that touch the same capabilities touch different requirements:
  `start-spec-launch-parity` ADDs to `supervisor-config`; `broad-grant-scope-safety` and
  `live-prompt-tail-window` touch `approval-auto`, not `approval-command-safety`;
  `mcp-superpowers-support` touches other `mcp-server` requirements; `cli-launch-identity`
  modifies other `cli-resolution` requirements (*Auto-detect known AI CLIs on PATH*, *README
  Supported AI CLIs table…*); `supervisor-boot-wiring` modifies `supervisor-learnings`
  *Conflict-event signal*; `bundled-helper-discovery` only ADDs a `supervisor-learnings`
  requirement (`sweep.sh learn` ids follow this change's scheme) and depends on this change
  landing first. The sibling change
  `status-and-picker-truth` touches `session-state`, `preflight-diagnostics` and
  `cli-interactive-selection`.
