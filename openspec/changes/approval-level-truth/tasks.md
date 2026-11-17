## 0. Gates before code

- [ ] 0.1 Merge order: rebase onto `config-schema-versioning` and `supervisor-vocabulary-split`. Do not merge this change in a different release from the version-2 bump (design D2, "Same-release constraint").
- [ ] 0.2 Fact gate (design D6), on Claude Code ≥ 2.1.325. Launch a worker exactly as git-paw does (isolation env, `--setting-sources project,local`, `--permission-mode auto`) in a `.git-paw/worktrees/` worktree. Record steps 1–6:
  1. the mode is on;
  2. the test command, `git add` and `git commit` run without a prompt;
  3. a read-only `grep` of a worktree file runs without a prompt;
  4. `setup-cli`'s `approve` and `tell` ask rules still prompt;
  5. what happens when auto mode is unavailable;
  6. `-p` at `--permission-mode auto`.

  Write the version and date beside the Claude rows in `src/agent_cli/mod.rs`. If step 1 or 2 fails, stop and amend the spec.
- [ ] 0.3 Freeze the current table: rename `pre_provider_approval_flags` in `src/agent_cli/tests.rs:61-70` (or copy it) as the version-1 `auto` table that the `accept-edits` and migration tests compare against.

## 1. Level set

- [ ] 1.1 In `src/config/supervisor.rs`:
  - add `ApprovalLevel::AcceptEdits` between `Manual` and `Auto`;
  - keep `#[default]` on `Auto`, the version-2 default (design D3); the version-1 fallback lives in config loading (3.1);
  - make `KEBAB_NAMES` four entries;
  - add the `kebab_name` arm;
  - update the doc comment's level descriptions.
- [ ] 1.2 Rewrite the profile `approval` parser (`src/config/profiles.rs:203-215`), `validate_approval_args` (`src/config/mod.rs:827-833`) and the serde error path so that each lists the four names.
- [ ] 1.3 Fix every exhaustive match the compiler reports (provider rows, doctor probes, delegate admission). `src/delegate.rs:309-315` already compares against `Manual` and should need no logic change.

## 2. Rows

- [ ] 2.1 Claude: `AcceptEdits` → `--permission-mode acceptEdits`, `Auto` → `--permission-mode auto` (`src/agent_cli/mod.rs:410-416`).
- [ ] 2.2 Codex: `AcceptEdits` → `--sandbox workspace-write` (`:484-489`). `agy` and `qwen`: `AcceptEdits` → `""`.
- [ ] 2.3 Update the `approval_flags` doc example (`src/config/supervisor.rs:1026-1045`).
- [ ] 2.4 `widening_arg` (`src/agent_cli/profile.rs:209-214`) reads the `AcceptEdits`, `Auto` and `FullAuto` rows.

## 3. Migration and warning

- [ ] 3.1 In the v1→v2 step (`src/config/migrate.rs`), add the four rewrites from design D2 (`agent_approval`, `approval`, `profiles.*.approval`, and the `approval_args` key `auto`). Return the paths that used `auto`. Record whether a version-1 document set a worker approval.
- [ ] 3.1a Worker approval resolution (design D3): a set value in any loaded document wins by merge precedence; otherwise `auto` when the highest-precedence loaded document is version 2; otherwise `accept-edits` (version-1 top document, or no file). An unset orchestrator approval inherits the result. Delegates keep their `manual` default.
- [ ] 3.2 Add a `core-deprecation` registry entry for the version-1 `auto` value with the new kind `configuration value` (resolved question 4; the kind itself comes from `supervisor-vocabulary-split`), and print one helper line per file that names every collected path.
- [ ] 3.3 `init`: stamp no existing unversioned file. Treat it like one with `[supervisor]`: no `version = 2` backfill and the one pointer line. A fresh file still declares `version = 2`. Coordinate with `supervisor-vocabulary-split`, whose "init stamps a layout-neutral file" scenario this overrides.
- [ ] 3.4 `add-cli` and `remove-cli`: write `[workers] approval = "accept-edits"` when the version-1 global file set no worker approval. The rewrite notice names each renamed or added approval value or key.
- [ ] 3.5 Template comment (`src/config/mod.rs:1280`): list the four levels, name `auto` as the default, and give the Claude flags of `accept-edits` and `auto`.

## 4. Showing the truth

- [ ] 4.1 `dry_run_launch_line` (`src/commands/launch.rs:378-389`): add the `approval:` line (level, flags or `no flags (…)`, override tag). Use the resolved profile's `approval_flags`, the same call the command uses.
- [ ] 4.2 `src/doctor.rs`: add `check_approval_levels` and a probe over the CLIs that `check_sandbox_mode` already enumerates, plus the profile and orchestrator CLIs. Add the ✓/⚠ rules, the auto-mode hint line and `--json` rows.

## 5. Tests (one or more per scenario)

- [ ] 5.1 Provider rows: four levels × every family. `accept-edits` equals the frozen version-1 `auto` table for every roster CLI and for an unknown CLI. A level never borrows another level's row (`src/agent_cli/tests.rs`, `src/config/tests.rs:1985-2012`).
- [ ] 5.2 Golden launch command at `src/commands/launch.rs:1782-1786`: a version-1 `auto` stays byte-identical, and a version-2 `auto` contains `--permission-mode auto`.
- [ ] 5.3 Migration fixtures:
  - every path migrates;
  - the #28 override keeps launching `--permission-mode auto`;
  - the file's bytes and mtime are unchanged;
  - a version-2 document is not rewritten;
  - default resolution: a version-1 file with no worker approval launches `acceptEdits` byte for byte, a version-2 file with none launches `--permission-mode auto`, no file gives `accept-edits`, a set global `manual` wins over a version-1 repository file with nothing set, and the orchestrator inherits each result.
- [ ] 5.4 Warning: one line per file naming every path, nothing on stdout, exit code unchanged, and no line when approvals are unset. `config check` finding.
- [ ] 5.5 `init` does not stamp a file that has `[profiles.*] approval = "auto"`, nor one with only `[broker] enabled = true`. `add-cli` writes `accept-edits` for a renamed value and for an unset worker approval, and gives the notice.
- [ ] 5.6 Profiles: `accept-edits` loads. A delegate denylist entry from an `accept-edits` override is denied. An `accept-edits` delegate is a writer.
- [ ] 5.7 Dry-run: version-2 default (`auto`), version-1 default (`accept-edits`), override tag, no-flags wording, per-profile level.
- [ ] 5.8 Doctor: rows and marks for a version-2 and a version-1 default, the auto hint, override tag, ⚠ for a level in effect with no flags, JSON.
- [ ] 5.9 Safety: an `auto` worker carries no `--allowedTools` or `--settings`. Starting at `auto` creates no `.claude/` and no record. A prompt in an `auto` pane is escalated with no keystroke (extend `tests/cli_setup_launch_guards.rs` and the drive-loop escalation tests).
- [ ] 5.10 Update the escalation hand-off tests (`src/supervisor/escalation.rs:1194-1349,1696`) so that `accept-edits` keeps `acceptEdits` and a version-2 `auto`, set or by default, gives `--permission-mode auto -p`.

## 6. Docs

- [ ] 6.1 `docs/src/configuration/README.md`: the four levels with an exact-flags table, `auto` as the version-2 default, the version-1 rule (an unset or `auto` worker approval stays `accept-edits`), the default-resolution order, and the migration-table row for the version-1 `auto`.
- [ ] 6.2 `docs/src/supported-clis.md`: per-family flags for the four levels, with the verification date.
- [ ] 6.3 `docs/src/user-guide/security-posture.md`: `auto` is the default, the CLI's own judgement, wider than `accept-edits`, and not a sandbox. If the CLI's auto mode is unavailable it may fall back silently, and `doctor` does not probe for it. git-paw still answers no prompt.
- [ ] 6.4 `coordination.md` and `supervisor.md`: how to choose between `accept-edits` and `auto` for an unattended wave (#23, #29). `agent-profiles.md`: the four values, and delegates at `auto`. `doctor.md`: the new check.
- [ ] 6.5 Worker-only rules today (design D7): the `[clis.claude-worker] args = "--settings …"` recipe, published only if the D6 gate confirmed `--settings` alongside `--setting-sources`. Otherwise leave it out.
- [ ] 6.6 CHANGELOG: the new level, `auto` now means the CLI's auto mode in version-2 files and is their default, and version-1 files are unchanged (including an unset worker approval) and warn when they write `auto`.

## 7. Close-out

- [ ] 7.1 `openspec validate approval-level-truth --type change --strict` passes. `just check` is green.
- [x] 7.2 Record the open questions' answers in design.md (Resolved Questions, 2026-11-17).
