## Why

After v0.18.0 there is no supervisor agent in a session. The operator's own driving agent is the
orchestrator, every pane is a worker, and git-paw's own part is the drive loop plus "coordinated"
mode (broker, boot prompts, dashboard). The word "supervisor" still names the start flags, the
`[supervisor]` config table, the broker's root identity, a doctor group, a skill name and dozens of
help and prose strings, so the surface describes a role that no longer exists. v1.0.0 freezes that
surface. The new names have to ship now, with the old names still working and warning for at least
two minor releases, so v1.0.0 can remove them (`remove-supervisor-vocabulary`) and freeze a surface
with no "supervisor" in it. The user chose the split-by-concern naming on 2026-10-19.

## What Changes

- **A deprecation registry and one warning helper** (new `core-deprecation`): every deprecated item
  is recorded once (item, kind, since, replacement, removal version); using one prints a single
  stderr line per item per process; stdout and exit codes never change; a build guard fails when an
  entry outlives its removal version. v0.24.0's `--from-specs` warning reuses it.
- **Start-mode flags:** `--coordinated` and `--bare` replace `--supervisor` and `--no-supervisor`.
  The old flags stay as hidden aliases that parse identically and warn (removal: 1.0.0).
- **Config schema version 2** (on top of `config-schema-versioning`): the `[supervisor]` table is
  split by concern into top-level `default_mode`, `[orchestrator]` (+ `.gates`, `.escalation`),
  `[workers]` and `[drive]`. A version-1 file (every config written before this release) is migrated
  in memory and prints one deprecation warning per key family; it is never rewritten. Setting an old
  key and its new counterpart is a config error naming both; a `version = 2` file may not carry
  `[supervisor]`. `git paw config check` and `doctor` report each old key with its new name. The
  bundled scripts read the new keys first and fall back to the old ones. The two tables v0.17.0
  deprecated with no effect, `[supervisor.auto_approve]` and `[supervisor.common_dev_allowlist]`,
  are dropped by the migration, never renamed into `[workers]`; their v0.17.0 per-file warning goes
  through the same deprecation helper, and a rewrite of the global file by `add-cli` / `remove-cli`
  drops them and says so.
- **Broker root identity `orchestrator`:** the root session publishes and is addressed as
  `orchestrator`; the broker accepts `supervisor` wherever the root identity appears and normalizes
  it at the wire edge (removal: 1.0.0). The verify nudge is `orchestrator.verify-now` (the old type
  still decodes). The sweep stuck flag becomes `blocked-on-orchestrator`.
- **Persisted values and machine output, additively:** session files written by any release are read
  with `mode` `"supervisor"` or `"coordinated"` (writers keep `"supervisor"` until 1.0.0); MCP
  `get_session_status` gains `session_mode` beside the deprecated `mode`; doctor's "Supervisor" group
  becomes "Coordination" (its check ids are born `coordination.*`).
- **The `supervisor` skill name** (v0.18.0 alias of `orchestrator`) warns when used (removal: 1.0.0).
- **Prose and docs:** help text, prompts, hooks, boot blocks, inter-agent rules, skills, MCP
  descriptions, README, configuration reference (new tables + migration table), CLI reference
  (flags + deprecations), and the user guide speak of coordinated mode, the orchestrator and workers.
- Not breaking: every existing config, flag, script, session file and broker client keeps working.

## Capabilities

### New Capabilities

- `core-deprecation`: the deprecation registry (including no-effect entries for the 0.17.0
  auto-approval tables), the single stderr warning helper, deprecated-flag detection from the raw
  arguments, the removal-version build guard, and the published registry.
  Purpose for the archived spec: "Record every deprecated git-paw surface in one registry, warn once
  per use on stderr without changing output or exit codes, and keep deprecated items from outliving
  their scheduled removal."

### Modified Capabilities

- `core-configuration`: ADDED schema version 2 and its key relocation, version-1 warnings, the
  old/new conflict rule, refusal of `[supervisor]` in a version-2 file, one-unit merge of the
  coordination tables, init's version guard, and the helper scripts' key fallback.
- `supervisor-config`: RENAMED + MODIFIED the enabled-mode resolution chain, the init prompt and the
  init commented block into start-mode / coordination-table terms; MODIFIED v0.17.0's *Deprecated
  auto-approval tables are parsed but have no effect* (written on its text): the tables are dropped
  by the version-2 migration instead of round-tripping, and the warning goes through the shared
  helper.
- `cli-parsing`: RENAMED + MODIFIED `--no-supervisor flag` and the mutual-exclusion requirement;
  MODIFIED *Start subcommand with optional flags* and *--unattended flag* (the latter written on
  v0.17.0 `classifier-retirement`'s text, keeping its escalate-not-approve help); ADDED the
  deprecated aliases (with the reading rule for requirements that still name them).
- `broker-protocol`: ADDED the root identity `orchestrator` with the `supervisor` inbound alias and
  the `orchestrator.verify-now` type.
- `session-state`: ADDED both session-mode spellings are read.
- `mcp-server`: ADDED `session_mode` in `get_session_status`.
- `preflight-diagnostics`: RENAMED + MODIFIED *Supervisor check* → *Coordination check*; MODIFIED the
  grouped-report requirement; ADDED reporting of version-1 coordination keys.
- `skill-agent`: ADDED the `supervisor` skill-name warning.
- `git-add-remove-branch`: ADDED `remove` refuses the root identity under either spelling.

## Impact

- **Code:** new `src/deprecation.rs`; `src/config/{migrate,wire,mod,supervisor}.rs` (v1→v2 step,
  v2 wire tables, mapper into the existing internal model, merge unit, error texts); `src/init.rs`
  (prompt, template, version guard); `src/cli.rs` + `src/main.rs` (flags, aliases, raw-arg scan);
  `src/commands/{supervisor,start,recover,add,remove}.rs` (prompt and resolution text);
  `src/broker/{wire,server,delivery,messages}.rs` (root identity normalization, agent-id rule,
  verify-now spelling); `src/supervisor/drive.rs`, `src/dashboard/view.rs`,
  `src/coordination/inventory.rs` (one root-identity constant); `src/session.rs` (session wire mode
  alias); `src/mcp/query/session.rs`, `src/mcp/tools/*`; `src/doctor.rs`; `src/skills.rs`;
  `src/opsx/role_guard.rs`; `src/broker/learnings.rs`.
- **Scripts and exported assets:** `assets/scripts/sweep.sh` (+ tracked `.git-paw/scripts/sweep.sh`),
  `assets/hooks/*.sh`, `assets/boot-block*.md`, `assets/inter-agent-rules.md`,
  `assets/governance-section-header.md`, `assets/agent-skills/{coordination,orchestrator}.md`, the
  escalation-framing asset, the `init` default config.
- **Enum-variant ripple:** `BrokerMessage`'s variant set is unchanged; only `VerifyNow`'s wire
  spelling changes, in the `broker::wire` mapper (from `wire-format-dtos`), with a decode alias.
  `SpecBackendKind` untouched. `SessionMode` keeps two variants (the coordinated one is renamed
  internally; its serialized spellings are pinned by the session wire type).
- **Dependencies:** none new.
- **Cross-version dependencies:**
  - v0.17.0 `classifier-retirement` (archived first): this change's *--unattended flag* delta and its
    *Deprecated auto-approval tables are parsed but have no effect* delta copy that change's text and
    only apply the vocabulary and the version-2 drop, so archiving here never reverts v0.17.0; its
    `deprecated_table_warning` is re-routed through `deprecation::warn`; its parse-only
    `AutoApproveConfig` / `CommonDevAllowlistConfig` stay for version-1 reading
    (`AutoApproveConfig::resolved` is already gone).
  - v0.21.0 `config-schema-versioning` (must land first: version 2 is a step in its chain, and its
    *Script-read configuration keys* list changes), `wire-format-dtos` (the session and broker wire
    types carry the aliases), `uniform-json-output` (doctor ids are born `coordination.*`),
    `config-check-command` (its D4 deprecation table becomes this registry's config entries),
    `session-state-reconciliation` (also MODIFIES *`git paw doctor` runs read-only preflight
    checks*; archive it first — this delta is written on its text). Siblings that name `[supervisor]`
    keys, `--supervisor` or `from: "supervisor"` (`configurable-agent-grid`, `finish-command`,
    `finish-merge-queue`, `readme-landing-page`, `worktree-shared-dep-cache`) adopt the new names
    in whichever lands second.
  - v0.18.0: `supervisor-pane-removal` (no pane; mode meaning), `orchestrator-escalation-handoff`
    (`[supervisor].cli/approval`, `[supervisor.escalation]`), `orchestrator-skill-export` (the
    `supervisor` skill alias), `broker-verdict-authority` (its gate on feedback claiming the root
    identity must see the normalized identity), `orchestrator-read-surface` (`[broker]
    wave_stall_minutes` stays where it is).
  - v0.19.0 `worker-usage-budget` (`budget_max_tokens` / `budget_max_cost` land in `[drive]`).
  - v0.24.0 `error-exit-code-contract` (edited to register `--from-specs` here instead of defining
    its own warn function).
  - v1.0.0 `remove-supervisor-vocabulary` removes every alias this change adds; `cli-contract-freeze`
    adopts this registry; `remove-from-specs-alias` must rebase its copies of *Start subcommand with
    optional flags* and the dispatch requirements on this change's text.
- **Security / safety:** the verdict-authority gate and the correction loop see one canonical root
  identity, so the alias cannot be used to bypass either. No new input surface; the config migration
  is pure and never writes the user's file.
