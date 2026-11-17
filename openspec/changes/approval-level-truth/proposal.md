## Why

git-paw's approval level `auto` does not launch Claude Code's own auto mode. For a Claude-family
worker it launches `--permission-mode acceptEdits` (`src/agent_cli/mod.rs:413`), which approves
file edits and stops on every shell command the CLI's rules do not allow. That mapping was right
when it was made on 2026-09-04: Claude Code's `--permission-mode` then had no `auto` choice. Claude
Code now ships one. The installed Claude Code 2.1.325 lists `acceptEdits`, `auto`,
`bypassPermissions`, `manual`, `dontAsk` and `plan`, and has a `claude auto-mode` command for
inspecting its rules. So the name `auto` now promises something the level does not do.

The v0.20.0 external run (2026-11-16) hit this three times:

- **#28.** A user had to find `[clis.<name>] approval_args = { auto = "--permission-mode auto" }`
  on their own to get Claude Code's auto mode. Neither `start --dry-run` nor `doctor` tells you
  what a level launches.
- **#23.** v0.17.0 retired git-paw's classifier, and `[supervisor.auto_approve]` and
  `[supervisor.common_dev_allowlist]` now do nothing. A wave at `auto` stalls on its first
  `npm test` or `git commit` unless the user writes allow rules into
  `.claude/settings.local.json`. Those rules then apply to the user's own sessions as well.
- **#29.** Under `acceptEdits`, even a read-only `grep` of the worker's own output file prompts.

Approval level names are part of the configuration contract that v1.0.0 freezes, so v0.21.0 is the
last release where the name can be fixed instead of documented around. The fix must not bring back
a git-paw classifier. v0.17.0 deliberately left the decision to the agent CLI's own permission
rules, and Claude Code's auto mode is exactly that: the CLI's own judgement.

## What Changes

- **Four approval levels: `manual`, `accept-edits`, `auto`, `full-auto`.** The new `accept-edits`
  level launches what `auto` launches today: Claude `--permission-mode acceptEdits`, Codex
  `--sandbox workspace-write`, no flags for `agy` and `qwen`.
- **`auto` launches the CLI's own automatic permission mode where the family has one.** For Claude
  that is `--permission-mode auto`. Codex keeps `--sandbox workspace-write`. Every other row is
  unchanged. git-paw still makes no decision about any command, and it still escalates every
  prompt the CLI leaves in place.
- **The default worker approval is `auto`.** Whenever no loaded configuration sets the worker
  approval, whatever the file's version and also with no configuration file at all, workers launch
  the CLI's own auto mode. A fresh `git paw init` does too. An unset orchestrator approval inherits
  the worker approval, as it does today, so it is `auto` too. A delegate profile still defaults to
  `manual`.
- **An explicit `auto` keeps its meaning on upgrade.**
  - The version-1 to version-2 configuration migration, added in this release by
    `supervisor-vocabulary-split`, rewrites every version-1 approval value `"auto"` to
    `"accept-edits"` and every `approval_args` key `auto` to `accept-edits`, in memory. A config
    that wrote `auto` therefore launches byte-identical commands.
  - A version-1 file that uses `auto` warns once through the shared deprecation helper
    (`core-deprecation`), filed under a new `configuration value` kind.
  - `git paw init` does not stamp `version = 2` onto a file that uses the version-1 `auto`.
    `add-cli` and `remove-cli` write `accept-edits` where such a file said `auto`.
- **`git paw start --dry-run` names each worker's level and its flags.** Below each `runs:` line it
  prints `approval: <level> → <flags>`, and names the `approval_args` override when one applies.
- **`git paw doctor` gains an `approval levels` check.** For every CLI a launch can use, it lists
  what each of the four levels launches, marks the levels in effect, and warns (⚠) when a level
  in effect other than `manual` launches no flags.
- **Delegates.** The `--args` widening denylist covers the flags of every level above `manual`.
  The single-writer admission rule applies to every level above `manual`, not only to `auto`.
- **Docs** (configuration reference, supported CLIs, security posture, coordination and
  orchestrator guides) describe the four levels, the exact flags, the new default and the
  behaviour change it brings to users who never set an approval, the migration,
  and how to choose between `accept-edits` and `auto`. They warn that an unavailable auto mode may
  fall back silently. `doctor` does not probe for it.

**Behaviour change for upgrading users who never set an approval.** Every existing file still
loads. A configuration that writes an approval value launches the same commands. But one that sets
no worker approval (version 1, version 2, or no file at all) now gets `auto`: its Claude workers
move from `--permission-mode acceptEdits` to Claude Code's auto mode, and its headless orchestrator
invocations follow. Codex, `agy` and `qwen` launch the same flags at both levels, so only Claude
workers change. git-paw prints no one-time notice; the CHANGELOG and upgrade notes say so and name
how to keep the old behaviour: `[workers] approval = "accept-edits"`, or in a version-1 file
`[supervisor] agent_approval = "accept-edits"`.

**Out of scope.** These items moved to the v0.23.0 per-CLI audit:
- #22: setup-cli refuses a custom CLI of the Claude family.
- #25 and #30: Claude first-launch dialogs.
- PK-11: re-verifying the `full-auto` row on Claude Code 2.1.322 and later.

Worker-only allow rules (alternative B in design.md) are recorded and deferred.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `supervisor-config`:
  - MODIFIED **SupervisorConfig struct with defaults**: four levels, default `auto` in every
    configuration version.
  - MODIFIED **Permission flag mapping**: an `accept-edits` row, Claude `auto` becomes
    `--permission-mode auto`, and the row is re-verified against Claude Code 2.1.325.
  - MODIFIED **CustomCli approval_args override field**: four keys.
- `core-configuration`: ADDED:
  - version-1 `auto` values migrate to `accept-edits`; an unset worker approval takes the
    default `auto`;
  - such a file warns once;
  - `init` and the global-file rewrite never change what an explicit `auto` means.
- `cli-agent-profiles`: MODIFIED:
  - **Agent profiles are declared in `[profiles.<name>]`**: four approval values;
  - **A delegate profile's args cannot widen its permissions**: every level above `manual`;
  - **Delegate runs are admitted four readers or one writer at a time**: every level above
    `manual` is a writer.
- `tmux-orchestration`: ADDED **The dry-run plan names each worker's approval level and flags**.
- `preflight-diagnostics`: ADDED **Doctor reports what each approval level launches**.
- `approval-command-safety`: ADDED **The auto level hands command judgement to the CLI's own auto
  mode**.

## Impact

- **Code:**
  - `src/config/supervisor.rs`: `ApprovalLevel` gains `AcceptEdits`, the `#[default]` stays on
    `Auto`, and `KEBAB_NAMES` has four entries.
  - `src/agent_cli/mod.rs`: the Claude and Codex rows, and the provider doc comment's verification
    date.
  - `src/agent_cli/profile.rs`: `widening_arg` reads every row above `manual`.
  - `src/config/profiles.rs` and the delegate admission code.
  - `src/config/mod.rs`: `validate_approval_args` and its message, and the template comment.
  - The v1→v2 step in `src/config/migrate.rs`, which `config-schema-versioning` and
    `supervisor-vocabulary-split` add.
  - `src/commands/launch.rs`: `dry_run_launch_line`.
  - `src/doctor.rs`: a new `check_approval_levels`.
  - `src/supervisor/escalation.rs` (tests only).
- **Exhaustive matches on `ApprovalLevel`** gain an arm: the provider rows in
  `src/agent_cli/mod.rs:410-559`, `kebab_name`, doctor probes and delegate admission. The compiler
  finds them.
- **Tests:**
  - `src/agent_cli/tests.rs:61-70` (`pre_provider_approval_flags`) becomes the frozen version-1
    table, and a new test asserts that a version-1 `auto` file launches it byte for byte.
  - `src/config/tests.rs:1985-2012`, the launch golden at `src/commands/launch.rs:1786`,
    migration fixtures, doctor, and dry-run.
- **Docs:** `docs/src/configuration/README.md`, `docs/src/supported-clis.md`,
  `docs/src/user-guide/security-posture.md`, `coordination.md`, `supervisor.md`,
  `agent-profiles.md`, `doctor.md`, and the CLI reference's Deprecations table (generated from the
  registry).
- **Cross-change dependencies:**
  - Rebases over `config-schema-versioning`, which provides the migration chain.
  - Rebases over `supervisor-vocabulary-split`, which provides version 2, the v1→v2 step, the
    `[workers] approval` and `[orchestrator] approval` paths, and the deprecation registry and
    helper. That change must add the `configuration value` registry kind this entry uses. Its
    `init` backfill rule gains one exclusion here: a file that uses the version-1 `auto`.
  - Must ship in the same release as the version-2 bump. If it slips, it needs its own v2→v3 step,
    because some version-2 files will already say `auto` and mean `acceptEdits`.
  - `config-check-command` reports the version-1 `auto` through the registry. It needs no change.
- **Dependencies:** none.
