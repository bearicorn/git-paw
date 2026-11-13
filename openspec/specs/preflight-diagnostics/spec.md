# preflight-diagnostics Specification

## Purpose
The read-only `git paw doctor` preflight command: grouped ✓/⚠/✗ diagnostics across environment, CLIs, config, spec system, bundled scripts, broker, supervisor, and hygiene — each non-passing check carrying a remedy — with a `--json` renderer, an optional `--live` session-lifecycle smoke arm, and an exit code that reflects the worst check. Doctor diagnoses but never repairs.

## Requirements

### Requirement: `git paw doctor` runs read-only preflight checks

The system SHALL provide a `git paw doctor` command that inspects the environment,
configuration, and repository state and prints a grouped diagnostic report. Each check
SHALL be reported with one of three statuses — ✓ (pass), ⚠ (warning), or ✗ (failure) —
and every non-✓ check SHALL print an actionable remedy. The command SHALL NOT mutate any
file, config, session, or other persistent state.

#### Scenario: Doctor prints a grouped report

- **GIVEN** a git repository
- **WHEN** `git paw doctor` is run
- **THEN** it SHALL print checks grouped under Environment, CLIs, Config, Spec system, Bundled scripts, Broker, Supervisor, and Hygiene
- **AND** each check SHALL carry a ✓, ⚠, or ✗ status
- **AND** every ⚠ or ✗ check SHALL include a remedy line

#### Scenario: Doctor does not mutate state

- **GIVEN** a repository with a `.git-paw/` directory
- **WHEN** `git paw doctor` is run
- **THEN** no file under `.git-paw/` (config, scripts, session state) SHALL be created, modified, or deleted

### Requirement: Exit code reflects the worst check

The command SHALL exit with a non-zero status when any check is ✗, and with zero when all
checks are ✓ or ⚠. A ⚠ alone SHALL NOT cause a non-zero exit.

#### Scenario: All checks pass

- **GIVEN** an environment where every check resolves to ✓
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit 0

#### Scenario: A hard failure is present

- **GIVEN** an environment with at least one ✗ check
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit non-zero

#### Scenario: Only warnings are present

- **GIVEN** an environment with ⚠ checks but no ✗ checks
- **WHEN** `git paw doctor` is run
- **THEN** the process SHALL exit 0

### Requirement: Machine-readable `--json` output

When `--json` is passed, the command SHALL emit a single JSON document enumerating every
check with at least its group, name, status, detail, and remedy fields, and SHALL suppress
the human-readable rendering. The exit-code contract SHALL be identical to the
human-readable mode.

#### Scenario: `--json` emits parseable output

- **WHEN** `git paw doctor --json` is run
- **THEN** stdout SHALL be a single parseable JSON document
- **AND** each check entry SHALL include `group`, `name`, `status`, and (for non-✓ checks) `remedy`
- **AND** the exit code SHALL match what the human-readable mode would return for the same environment

### Requirement: Environment checks

The command SHALL verify that `git` and `tmux` are present on PATH and meet minimum
versions, and that the working directory is inside a git repository. Any missing tool or a
non-repository directory SHALL be reported ✗ with a remedy.

#### Scenario: tmux missing

- **GIVEN** `tmux` is not on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the Environment group SHALL report a ✗ for tmux with a remedy to install it

#### Scenario: Not a git repository

- **GIVEN** the working directory is not inside a git repository
- **WHEN** `git paw doctor` is run
- **THEN** the Environment group SHALL report a ✗ with a remedy

### Requirement: CLI availability check

The command SHALL report the AI CLIs detected on PATH (the known roster plus any
`[clis.*]` custom entries). When no CLI resolves, it SHALL report ⚠ (surfacing the
`NoCLIsFound` launch condition early) with a remedy to install a CLI or add a `[clis.*]`
entry.

#### Scenario: No CLIs resolve

- **GIVEN** no known or custom CLI resolves on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the CLIs group SHALL report ⚠ with a remedy

#### Scenario: CLIs detected

- **GIVEN** at least one CLI resolves
- **WHEN** `git paw doctor` is run
- **THEN** the CLIs group SHALL report ✓ and list the detected CLIs

### Requirement: Config check

The command SHALL report whether `.git-paw/config.toml` is present and parses, report the
resolved `worktree_placement`, and flag unknown or deprecated fields as ⚠. An unparseable
config SHALL be ✗.

#### Scenario: Config parses

- **GIVEN** a valid `.git-paw/config.toml`
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✓ and state the resolved `worktree_placement`

#### Scenario: Config does not parse

- **GIVEN** a `.git-paw/config.toml` with invalid TOML
- **WHEN** `git paw doctor` is run
- **THEN** the Config group SHALL report ✗ with a remedy

### Requirement: Spec-system check

The command SHALL report the explicitly resolved spec system (from `--specs-format` /
`[specs]`, per the v0.12.0 explicit-only rule) and the count of specs scanned. When no spec
system is configured, it SHALL report ⚠ with the actionable "add `[specs]` or pass
`--specs-format`" guidance rather than a bare error.

#### Scenario: Spec system configured

- **GIVEN** `[specs]` is configured (or the equivalent flag would be passed)
- **WHEN** `git paw doctor` is run
- **THEN** the Spec-system group SHALL report ✓ with the resolved format and scanned spec count

#### Scenario: Spec system unconfigured

- **GIVEN** no `[specs]` configuration
- **WHEN** `git paw doctor` is run
- **THEN** the Spec-system group SHALL report ⚠ with guidance to add `[specs]` or pass `--specs-format`

### Requirement: Bundled-scripts check

The command SHALL verify, for every package in the bundled package registry, that the package is installed at the repository root under `.git-paw/skills/<package>/` with its `SKILL.md` present, and that each of the package's helper scripts exists under the package's `scripts/`, is executable, and matches the running binary's embedded version. A missing package, a missing or non-executable helper, or a stale helper SHALL be reported with a remedy to run `git paw init`. A root package's `SKILL.md` or reference whose content differs from what the running binary renders from the current config SHALL be reported ⚠ as stale, with a remedy to run `git paw start` or `git paw init`. When the repository is not initialised (no `.git-paw/config.toml`), the command SHALL report the packages, helpers and compatibility copies as one ⚠ line stating the repository is not initialised, with the remedy `git paw init`, instead of one line per file, and SHALL NOT report ✗ for them. The command SHALL also check each helper's compatibility copy under `.git-paw/scripts/`; because nothing bundled invokes that path, a missing or stale compatibility copy SHALL be reported as ⚠ and SHALL NOT be ✗. The command SHALL also verify that a Python 3 interpreter is available on PATH (`python3`, or `python` reporting major version 3), since every bundled helper script requires one to run; when none is found it SHALL report ⚠ with a remedy to install Python 3. Because core git-paw orchestration (start/add/remove) does not require Python, a missing interpreter SHALL be ⚠ and SHALL NOT be ✗.

#### Scenario: A bundled script is missing

- **GIVEN** `.git-paw/skills/orchestrator/scripts/sweep.sh` does not exist
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ✗ for `sweep.sh` with a remedy to run `git paw init`

Test: `doctor::tests::bundled_script_check_grades_each_on_disk_state`, `tests/doctor_e2e.rs::a_missing_package_helper_fails_naming_init`

#### Scenario: A bundled script is stale

- **GIVEN** a package helper exists but its content differs from the running binary's embedded version
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report a non-✓ status with a remedy to run `git paw init`

Test: `doctor::tests::bundled_script_check_grades_each_on_disk_state`

#### Scenario: A package's SKILL.md is missing

- **GIVEN** `.git-paw/skills/coordination/SKILL.md` does not exist
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ✗ for the `coordination` package with a remedy to run `git paw init`

Test: `doctor::tests::a_missing_package_skill_md_fails_naming_the_package`

#### Scenario: A missing compatibility copy is a warning

- **GIVEN** every package is installed and intact but `.git-paw/scripts/broker.sh` is missing
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ⚠ for the compatibility copy
- **AND** SHALL NOT report ✗

Test: `doctor::tests::a_missing_compatibility_copy_is_a_warning`

#### Scenario: A root skill rendered before a config change is stale

- **GIVEN** an initialised repository whose `.git-paw/skills/orchestrator/SKILL.md` renders `[supervisor] test_command` as `(not configured)`
- **AND** `[supervisor] test_command = "make test"` was then added to `.git-paw/config.toml`
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ⚠ for the `orchestrator` package as stale with a remedy naming `git paw start` or `git paw init`
- **AND** `.git-paw/skills/orchestrator/SKILL.md` SHALL be byte-identical afterwards

Test: `doctor::tests::a_stale_rendered_root_skill_warns_naming_start_or_init`, `tests/doctor_e2e.rs::a_root_skill_rendered_before_a_config_change_is_stale`

#### Scenario: An uninitialised repository is reported once

- **GIVEN** a git repository where `git paw init` never ran, so there is no `.git-paw/config.toml`
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report exactly one package line, ⚠, stating the repository is not initialised with the remedy `git paw init`
- **AND** SHALL report no ✗ for a package, helper or compatibility copy

Test: `doctor::tests::an_uninitialised_repository_is_reported_once`, `tests/doctor_e2e.rs::an_uninitialised_repository_is_reported_once`

#### Scenario: No Python 3 interpreter on PATH

- **GIVEN** neither `python3` nor a version-3 `python` is on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ⚠ for the Python 3 interpreter with a remedy to install Python 3 (the bundled helper scripts require it)

Test: `doctor::tests::missing_python3_warns_rather_than_fails`

### Requirement: Broker check

When `[broker]` is enabled, the command SHALL check that the configured `bind`/`port` is
free or reachable. The reachability probe SHALL be made **with the repository being
diagnosed**, so the check distinguishes this repository's own broker from a git-paw broker
on the same port that serves a different — or unreported — repository. The latter SHALL
report ⚠ with the `[broker] port` remedy, because it is the clash that would refuse the next
`git paw start`, and SHALL be reported distinctly from a non-git-paw service holding the
port (a different cause with a different fix). When the broker is disabled, it SHALL note
the pure-manual baseline as an informational ✓.

#### Scenario: Broker enabled and port free

- **GIVEN** `[broker] enabled = true` and the configured port is free
- **WHEN** `git paw doctor` is run
- **THEN** the Broker group SHALL report ✓

#### Scenario: Broker disabled

- **GIVEN** `[broker] enabled = false`
- **WHEN** `git paw doctor` is run
- **THEN** the Broker group SHALL report an informational ✓ noting the pure-manual baseline

#### Scenario: Port held by another repository's broker

- **GIVEN** `[broker] enabled = true` and the configured port is held by a git-paw broker serving a different repository
- **WHEN** `git paw doctor` is run
- **THEN** the Broker group SHALL report ⚠
- **AND** the detail SHALL say the port is held by another repository
- **AND** the remedy SHALL name the `[broker] port` setting

#### Scenario: A foreign service is not reported as another repository's broker

- **GIVEN** `[broker] enabled = true` and the configured port is held by a non-git-paw service
- **WHEN** `git paw doctor` is run
- **THEN** the Broker group SHALL report ⚠ with a detail distinct from the another-repository case

### Requirement: Supervisor check

When `[supervisor]` is enabled, the command SHALL verify that the configured gate commands
resolve to binaries on PATH and that `sweep.sh` is installed. The gate-command verbs SHALL
be sourced from the resolved stack preset, NOT from a hard-coded git-paw toolchain, so the
check stays project-agnostic.

#### Scenario: A gate-command binary is missing

- **GIVEN** `[supervisor] enabled = true` and a configured gate command whose binary is not on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the Supervisor group SHALL report ✗ for that gate command with a remedy

#### Scenario: Supervisor disabled

- **GIVEN** `[supervisor] enabled = false`
- **WHEN** `git paw doctor` is run
- **THEN** the Supervisor group SHALL report an informational ✓

### Requirement: Hygiene check

The command SHALL verify required `.gitignore` entries are present (including
`.git-paw/worktrees/` being ignored under child placement) and detect stale session state or
orphaned worktree registrations, reporting ⚠ with a remedy to run `git paw purge --stale`.

Stale session state SHALL mean a receipt that claims active while its tmux session is absent.
A session whose tmux session exists but whose agent panes have all exited (past the launch
grace period) SHALL NOT be reported as stale and SHALL NOT be given the `git paw purge --stale`
remedy, because `purge --stale` does not purge it. The command SHALL instead report it as ⚠.
The finding SHALL name the session, state that its agent panes have exited, and give the remedy
`git paw start`, which recovers the session.

#### Scenario: Missing gitignore entry

- **GIVEN** a required `.gitignore` entry is absent
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ with a remedy

#### Scenario: Stale session detected

- **GIVEN** session state that claims active but whose tmux session is gone
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ with a remedy to run `git paw purge --stale`

#### Scenario: Session whose agent panes exited

- **GIVEN** session state that claims active, whose tmux session exists, and whose agent panes have all exited past the launch grace period
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ naming the session and stating that its agent panes exited
- **AND** the remedy SHALL be to run `git paw start`
- **AND** the finding SHALL NOT describe the tmux session as gone and SHALL NOT recommend `git paw purge --stale`

### Requirement: `--live` smoke arm

The command SHALL accept a `--live` flag that additionally runs the `selftest`
session-lifecycle harness and folds its verdict into the report as a Live-smoke check,
so static preflight and live smoke sit under one diagnostic surface. Without `--live` the
command SHALL remain static — it SHALL NOT spawn a tmux session or an AI CLI, and the
Live-smoke group SHALL be absent. The `selftest` command SHALL remain available as the
harness's direct entry point, with its own output unchanged. Because the static checks
already report a missing prerequisite (notably tmux) as ✗, a smoke run that could not
start SHALL be reported ⚠ rather than ✗.

#### Scenario: `--live` adds the smoke verdict

- **GIVEN** an environment where the session lifecycle completes
- **WHEN** `git paw doctor --live` is run
- **THEN** the report SHALL carry a Live-smoke check reporting ✓

#### Scenario: A failing lifecycle is a hard failure

- **GIVEN** an environment where a lifecycle step fails
- **WHEN** `git paw doctor --live` is run
- **THEN** the Live-smoke check SHALL report ✗ naming the failing step
- **AND** the process SHALL exit non-zero

#### Scenario: A smoke run that cannot start warns

- **GIVEN** the lifecycle cannot run because tmux is unavailable
- **WHEN** `git paw doctor --live` is run
- **THEN** the Live-smoke check SHALL report ⚠ with a remedy rather than ✗

#### Scenario: Static runs carry no Live-smoke group

- **WHEN** `git paw doctor` is run without `--live`
- **THEN** the report SHALL NOT contain a Live-smoke group

#### Scenario: `--json --live` stays parseable

- **WHEN** `git paw doctor --json --live` is run
- **THEN** stdout SHALL remain a single parseable JSON document, with the harness's
  per-step progress output suppressed

### Requirement: Diagnose-only (no `--fix` in v0.13.0)

The command SHALL be diagnose-only in v0.13.0; it SHALL NOT expose a repair mode. (`--fix`
is deferred to a later cycle.)

#### Scenario: No repair flag

- **WHEN** `git paw doctor --help` is inspected
- **THEN** it SHALL NOT advertise a `--fix` or other repair option

### Requirement: Sandbox-availability check

`git paw doctor` SHALL include a check that detects whether an FS-scoped sandbox
backend is available for the current operating system — `sandbox-exec` on macOS,
`bwrap` on Linux/WSL — and report it with a ✓/⚠ status and, when unavailable or
degraded, an actionable remedy pointing at the sandbox setup documentation. The
check MUST be read-only (it MUST NOT invoke the sandbox or mutate any file) and
MUST follow the existing doctor conventions (grouped output, status glyph,
remedy line), degrading gracefully like the tmux availability check rather than
failing the command.

**Test:** `tests/doctor_sandbox_check.rs`

#### Scenario: Backend present reports pass
- **WHEN** `git paw doctor` runs on a system where the OS sandbox backend (`sandbox-exec` on macOS or `bwrap` on Linux) is on `PATH`
- **THEN** the sandbox check reports ✓ and the command's exit code is unaffected by this check

#### Scenario: Backend absent warns with a remedy
- **WHEN** `git paw doctor` runs on a system where the OS sandbox backend is not available
- **THEN** the sandbox check reports ⚠ with a remedy line pointing at the sandbox setup documentation, and does not report ✗ (absence is not a failure)

#### Scenario: Check is read-only
- **WHEN** the sandbox-availability check runs
- **THEN** no file under `.git-paw/` is created, modified, or deleted, and the sandbox is not launched

### Requirement: Doctor surfaces the effective spec-driven worker CLI

`git paw doctor` SHALL report the CLI that spec-driven worker panes would resolve to (per the spec-driven CLI resolution chain, evaluated for the current config and any discoverable specs) and SHALL warn when that effective CLI does not match a configured `default_spec_cli`. This surfaces a silent mis-resolution — such as supervisor-mode workers launching with an unexpected, unsandboxed CLI — rather than passing green. The check SHALL be read-only and SHALL degrade to informational (not a hard failure) when no specs or no `default_spec_cli` are configured.

#### Scenario: Doctor warns when the effective spec CLI differs from default_spec_cli

- **GIVEN** a config with `default_spec_cli = "claude-oss"` and a resolution path that would launch spec workers with a different CLI
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL warn that the effective spec-worker CLI does not match `default_spec_cli`

#### Scenario: Doctor passes when the effective spec CLI matches

- **GIVEN** a config with `default_spec_cli = "claude-oss"` and a resolution path that launches spec workers with `"claude-oss"`
- **WHEN** `git paw doctor` runs
- **THEN** the spec-CLI check SHALL pass (no warning)

### Requirement: Doctor reports CLI setup state

`git paw doctor` SHALL report, in the CLIs group, one setup check for each CLI identity that has a row in the setup fact table or entries in the setup record. The check SHALL be read-only: it reads the setup record, the files the record names, the item's target settings file, the helper scripts in the main checkout and in the current session's worktrees, and SHALL NOT create, modify or delete any of them. An entry the running binary would plan SHALL count as applied when the item's target settings file holds that value at the entry's key path, whether git-paw wrote it or it was there before the item was applied. The comparison against the running binary's plan SHALL be made whenever the item is applied — that is, whenever the record holds any entry for it — and SHALL NOT depend on the record holding a permission entry, because an item applied to a file that already held every planned entry records none. It SHALL report:

- ✓ when no item is applied, with a detail listing the available items and whether each is verified or plan-only;
- ✓ when every applied item's recorded entries are present and unchanged and every entry the running binary would plan is present in the target file;
- ⚠ when a recorded entry is missing or changed, with a remedy naming `git paw setup-cli <CLI> --undo` or re-applying the item;
- ⚠ when the applied entries differ from the entries the running binary would plan — an entry it would plan is absent from the target file, or the record holds an entry it no longer plans — with a remedy naming `git paw setup-cli <CLI> --undo` followed by re-applying the item;
- ⚠ when an allowlist item is applied and a helper script at a path an allow entry in the target file or the record names — its package path, or the compatibility path `.git-paw/scripts/` for entries applied before the package move — or any other bundled script in that helper's directory (such as the shared `_paw_common.sh` preamble it sources), in the main checkout or in a worktree of the current session, differs from the copy the binary bundles, naming that script's path, because the CLI would run the modified script without a prompt, with a remedy to restore the helper (`git paw init` for the main checkout, restarting the session for a worktree) or undo the item.

Entries applied by an earlier version for the pre-package `.git-paw/scripts/` paths differ from the entries the running binary plans, so they SHALL be reported as out of date under the rule above. Doctor and every other command SHALL leave them in place: only the user, by running `git paw setup-cli <CLI> --undo` and then re-applying the item, replaces them.

The check SHALL NOT report ✗.

#### Scenario: Available but not applied

- **GIVEN** no setup record
- **WHEN** `git paw doctor` runs
- **THEN** the CLIs group SHALL contain a ✓ setup check for `claude` whose detail names the allowlist item and its verification status

Test: `cli_setup::doctor::tests::not_applied_reports_ok_with_the_items_and_their_verification_status, tests/doctor_e2e.rs::doctor_reports_setup_availability_before_anything_is_applied`

#### Scenario: A recorded entry was removed by hand

- **GIVEN** a recorded allowlist entry that is no longer in its file
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ with a remedy naming `--undo`

Test: `cli_setup::doctor::tests::a_recorded_entry_removed_by_hand_warns_naming_undo`

#### Scenario: Applied entries are out of date

- **GIVEN** a setup record whose recorded allowlist entries lack one entry the running binary would plan
- **AND** the target settings file does not hold that entry
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ with a remedy naming `git paw setup-cli claude --undo` and re-applying `--allowlist`

Test: `cli_setup::doctor::tests::applied_entries_out_of_date_warn_naming_undo_and_reapplying`

#### Scenario: An entry the user already had counts as applied

- **GIVEN** a `.claude/settings.local.json` that already held `Bash(.git-paw/skills/coordination/scripts/broker.sh *)` before `git paw setup-cli claude --allowlist --yes` was run
- **AND** the setup record therefore does not list that entry
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ✓

Test: `cli_setup::doctor::tests::an_entry_the_user_already_had_counts_as_applied`

#### Scenario: Undo and re-apply converge when entries pre-existed

- **GIVEN** the repository from "An entry the user already had counts as applied"
- **WHEN** `git paw setup-cli claude --undo --yes` and then `git paw setup-cli claude --allowlist --yes` are run
- **THEN** `Bash(.git-paw/skills/coordination/scripts/broker.sh *)` SHALL still be present in `permissions.allow`
- **AND** `git paw doctor` SHALL report the setup check ✓

Test: `tests/doctor_e2e.rs::undo_and_reapply_converge_when_entries_pre_existed`

#### Scenario: A recorded entry the binary no longer plans is reported

- **GIVEN** a setup record listing an allowlist entry, present in the target file, that the running binary would not plan
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ with a remedy naming `git paw setup-cli claude --undo` and re-applying `--allowlist`

Test: `cli_setup::doctor::tests::a_recorded_entry_the_binary_no_longer_plans_is_reported`

#### Scenario: A hand deletion is caught when every planned entry pre-existed

- **GIVEN** a target settings file that already held every entry the running binary plans before the item was applied
- **AND** a setup record that therefore lists no permission entry for the item
- **WHEN** one of those entries is removed by hand and `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠

Test: `cli_setup::doctor::tests::a_hand_deletion_is_caught_when_every_planned_entry_pre_existed`

#### Scenario: A granted helper was modified

- **GIVEN** the allowlist item applied and a session worktree whose `.git-paw/skills/coordination/scripts/broker.sh` differs from the bundled copy
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ naming that helper's path

Test: `cli_setup::doctor::tests::a_modified_granted_helper_warns_naming_its_path`

#### Scenario: A modified preamble beside a granted helper is reported

- **GIVEN** the allowlist item applied and a `.git-paw/skills/coordination/scripts/_paw_common.sh` that differs from the bundled copy
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ naming that preamble's path

Test: `cli_setup::doctor::tests::a_tampered_preamble_beside_a_granted_helper_is_checked`, `cli_setup::doctor::tests::helper_paths_follow_the_applied_allow_entries`, `cli_setup::doctor::tests::granted_helper_path_normalises_the_spellings_a_grant_can_use`, `tests/doctor_e2e.rs::a_modified_preamble_beside_a_granted_helper_is_reported_at_both_paths`, `tests/doctor_e2e.rs::an_unplanned_allow_value_over_a_tampered_helper_is_reported`

#### Scenario: Entries applied before the package move are out of date

- **GIVEN** a setup record, written by an earlier version, whose applied allowlist entries name `.git-paw/scripts/` paths, all present and unchanged
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ with a remedy naming `git paw setup-cli claude --undo` followed by re-applying `--allowlist`
- **AND** the settings file and the setup record SHALL be byte-identical afterwards

Test: `cli_setup::doctor::tests::entries_applied_before_the_package_move_are_out_of_date`, `tests/doctor_e2e.rs::entries_applied_before_the_package_move_are_out_of_date`

#### Scenario: The setup check is read-only and never fails doctor

- **GIVEN** a repository with a setup record
- **WHEN** `git paw doctor` runs
- **THEN** the setup record and every file it names SHALL be byte-identical afterwards
- **AND** no setup check SHALL be reported ✗

Test: `cli_setup::doctor::tests::the_setup_check_never_reports_fail, tests/doctor_e2e.rs::doctor_leaves_the_record_the_settings_and_every_helper_byte_identical`

#### Scenario: A CLI with nothing to report gets no setup check

- **GIVEN** a `[clis.my-agent]` entry with no fact row and no record entries
- **WHEN** `git paw doctor` runs
- **THEN** no setup check SHALL be reported for `my-agent`

Test: `cli_setup::doctor::tests::an_identity_with_no_row_and_no_record_gets_no_check`

### Requirement: Doctor lists broad durable grants in the project's CLI settings

For each CLI identity whose fact row declares an allowlist item, when that item's target settings file exists, `git paw doctor` SHALL report in the CLIs group one advisory check listing every `permissions.allow` entry in that file whose shape grants more than one specific command. The shapes SHALL be a closed list:

1. a tool rule with no specifier or a bare wildcard specifier (for example `Bash`, `Bash(*)`, `Bash(:*)`);
2. a single-word prefix grant (for example `Bash(cargo:*)`, `Bash(git *)`, `Bash(rm*)`);
3. a prefix grant whose first word is a shell, an interpreter or a command wrapper (`sh`, `bash`, `zsh`, `fish`, `dash`, `ksh`, `python`, `python3`, `node`, `deno`, `bun`, `ruby`, `perl`, `php`, `lua`, `env`, `xargs`, `timeout`, `nohup`, `nice`, `sudo`, `su`, `doas`, `eval`, `exec`, `ssh`);
4. a prefix grant for a git subcommand that publishes, rewrites or discards history or working-tree state (`push`, `reset`, `clean`, `rebase`, `branch`, `checkout`, `restore`, `stash`, `worktree`, `update-ref`, `reflog`, `gc`, `filter-branch`, `config`);
5. a prefix grant for git-paw itself (`git paw` or `git-paw`) with no subcommand or with a subcommand that changes state (`start`, `add`, `remove`, `stop`, `pause`, `purge`, `init`, `add-cli`, `remove-cli`, `setup-cli`);
6. a prefix grant whose first word is `rm`, `chmod`, `chown`, `dd`, `curl` or `wget`, except a `curl` grant to a loopback address;
7. a file-tool rule (`Read`, `Edit`, `Write`) anchored at the filesystem root or the home directory and ending in a wildcard;
8. a grant naming a git-paw helper script, at its package path or at its compatibility path, that is broader than the setup item's own entry for that helper (for example `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh *)` or `Bash(.git-paw/scripts/sweep.sh *)`).

The check SHALL be ⚠ when at least one entry matches, listing the file and each matching entry — abbreviated to its tool name and the first two words of its specifier, followed by `…` when anything was cut, so a credential embedded later in a rule is not echoed — with a remedy stating that git-paw does not edit these entries and that the user should review and remove the ones they did not intend; it SHALL be ✓ when none matches, and ⚠ stating the file was not inspected when the file is not valid JSON. The check SHALL NOT report ✗, SHALL NOT print any other content of the file, SHALL NOT create, modify or delete the file, and SHALL NOT influence how any permission prompt is handled. An entry equal to one of the setup item's own planned entries SHALL NOT be listed, whoever added it.

#### Scenario: Broad grants left by earlier approvals are listed

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(git-paw purge:*)`, `Bash(git push:*)`, `Bash(cargo:*)` and `Bash(npm run test:*)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ listing `Bash(git-paw purge:*)`, `Bash(git push:*)` and `Bash(cargo:*)`
- **AND** it SHALL NOT list `Bash(npm run test:*)`

Test: `cli_setup::doctor::tests::broad_grants_left_by_earlier_approvals_are_listed, tests/doctor_e2e.rs::doctor_lists_a_broad_grant_the_user_already_had`

#### Scenario: git-paw's own setup entries are not listed

- **GIVEN** the allowlist item applied and no other entry in `permissions.allow`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ✓

Test: `cli_setup::doctor::tests::git_paws_own_planned_entries_are_not_listed`

#### Scenario: A long rule is abbreviated

- **GIVEN** `permissions.allow` holds `Bash(curl -H "Authorization: Bearer abc123" https://api.example.com/*)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL list the entry abbreviated as `Bash(curl -H …)`
- **AND** the output SHALL NOT contain `abc123`

Test: `cli_setup::doctor::tests::a_long_rule_is_abbreviated_and_never_echoes_a_credential`

#### Scenario: A wide helper grant is listed

- **GIVEN** `permissions.allow` holds `Bash(.git-paw/scripts/sweep.sh *)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ listing `Bash(.git-paw/scripts/sweep.sh *)`

Test: `cli_setup::doctor::tests::a_wide_helper_grant_is_listed`

#### Scenario: A wide grant at the package path is listed whole

- **GIVEN** `permissions.allow` holds `Bash(.git-paw/skills/docs-fetch/scripts/docs-fetch.sh *)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ listing `Bash(.git-paw/skills/docs-fetch/scripts/docs-fetch.sh *)` with the helper path unabbreviated

Test: `cli_setup::doctor::tests::a_wide_grant_at_the_package_path_is_listed_whole`, `cli_setup::doctor::tests::a_long_non_helper_word_is_still_cut`

#### Scenario: The advisory never edits the file

- **GIVEN** a settings file with broad grants
- **WHEN** `git paw doctor` runs
- **THEN** the file SHALL be byte-identical afterwards
- **AND** no check SHALL be reported ✗ because of it

Test: `cli_setup::doctor::tests::the_advisory_never_reports_fail, tests/doctor_e2e.rs::doctor_leaves_the_record_the_settings_and_every_helper_byte_identical`

#### Scenario: An unparseable file is reported, not inspected

- **GIVEN** a main-checkout `.claude/settings.local.json` that is not valid JSON
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ stating the file was not inspected
- **AND** it SHALL NOT print the file's content

Test: `cli_setup::doctor::tests::an_unparseable_file_is_reported_without_its_content`

### Requirement: Spec-workflow reachability check

`git paw doctor` SHALL include, in its "Spec system" group, a check that the workflow command the boot task prompt instructs spec-driven agents to run is reachable from the repository's worktrees. The check SHALL be a pure function of a probe recording: the resolved spec format, the workflow command that format's task prompt names (if any), the configured worktree placement (`PawConfig::worktree_placement()`), the effective spec-driven worker CLIs — the deduplicated list the discovered specs resolve to, as already surfaced by the spec-worker CLI check, empty when no specs are discovered — and, for each of those CLIs and each location where that CLI's definition of the command can live (as recorded in a table keyed by the CLI's provider family — so a `[clis.<name>]` entry declaring a `family` uses that family's locations — with a generic unknown fallback), whether the file exists at the main checkout and whether it is committed there. "Committed" SHALL mean present at the main checkout's `HEAD`, because `git worktree add` populates a new worktree from a commit: a definition that is ignored, untracked, or merely staged does not travel. Each resolved CLI SHALL be judged on its own:

- reachable when a definition is committed, or when placement is `child` and a definition exists at the main checkout;
- unreachable when its locations are known and either placement is `sibling` and every definition found is uncommitted, or no definition is found;
- unverified when its locations are unknown.

The verdict SHALL be:

- `Ok` (not applicable) when the format's task prompt names no workflow command, or when no spec-driven worker CLI resolves (an empty list);
- `Warn` when at least one resolved CLI is unreachable — naming the command, every unreachable CLI (and only those), the locations checked, and a remedy appropriate to the placement: under `sibling` placement both fixes (commit the definitions, or set `worktree_placement = "child"`), and under `child` placement — where the definitions are simply absent, so naming `child` would contradict the finding — installing the definitions at the repository root and committing them;
- `Warn` when no resolved CLI is unreachable but at least one is unverified — the detail stating that reachability could not be verified for each unverified CLI and scoping any reachability claim to the CLIs it covers, and the remedy naming `[worktree.env] copy` in `.git-paw/config.toml` as the way to provision that CLI's workflow command files into each new worktree;
- otherwise `Ok`.

The check SHALL never be `Fail`, SHALL be read-only, and SHALL appear in `--json` output.

#### Scenario: Sibling placement with an uncommitted definition warns

- **GIVEN** an OpenSpec repository with `worktree_placement = "sibling"`, spec-driven worker CLI `claude`, and `.claude/commands/opsx/apply.md` present at the root but ignored by git
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`
- **AND** its detail SHALL name `/opsx:apply` and the location checked
- **AND** its remedy SHALL mention committing the definitions and `worktree_placement = "child"`

Test: `doctor::tests::spec_workflow_check_warns_on_sibling_placement_with_an_uncommitted_definition, tests/doctor_e2e.rs::an_uncommitted_workflow_definition_warns_under_sibling_placement`

#### Scenario: A staged but uncommitted definition does not travel

- **GIVEN** the same repository where the definition has been `git add`ed but never committed
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`, because a new worktree is populated from a commit and would not carry it

Test: `doctor::tests::a_staged_but_uncommitted_definition_does_not_reach_a_sibling_worktree, tests/doctor_e2e.rs::a_staged_but_uncommitted_definition_warns_under_sibling_placement`

#### Scenario: Child placement reaches the root definition

- **GIVEN** the same repository with `worktree_placement = "child"`
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok`

Test: `doctor::tests::spec_workflow_check_passes_on_child_placement_with_a_root_definition, tests/doctor_e2e.rs::the_same_uncommitted_definition_passes_under_child_placement, tests/doctor_e2e.rs::child_placement_still_passes_when_doctor_runs_inside_the_worktree`

#### Scenario: A committed definition is reachable under any placement

- **GIVEN** an OpenSpec repository with sibling placement whose `.claude/commands/opsx/apply.md` is committed
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok`

Test: `doctor::tests::spec_workflow_check_passes_when_the_definition_is_committed, tests/doctor_e2e.rs::a_committed_workflow_definition_passes_under_sibling_placement`

#### Scenario: A missing definition warns

- **GIVEN** an OpenSpec repository whose spec-driven worker CLI is `claude` and which has no definition of the command at any known location
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn` and name the locations checked

Test: `doctor::tests::spec_workflow_check_warns_on_sibling_placement_with_no_definition_at_all, doctor::tests::spec_workflow_check_warns_when_no_definition_exists_anywhere, tests/doctor_e2e.rs::a_missing_definition_warns_under_sibling_placement`

#### Scenario: Under child placement the remedy does not name child placement

- **GIVEN** an OpenSpec repository with `worktree_placement = "child"` and no definition of the command at any known location
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`
- **AND** its remedy SHALL ask for the definitions to be installed and committed
- **AND** its remedy SHALL NOT ask for `worktree_placement = "child"`, which is already set

Test: `doctor::tests::spec_workflow_check_warns_when_no_definition_exists_anywhere`

#### Scenario: A format without a workflow command is not applicable

- **GIVEN** a repository whose resolved spec format is Markdown, Spec Kit or Superpowers
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` and state that the format names no workflow command

Test: `doctor::tests::spec_workflow_check_is_not_applicable_for_a_format_without_a_workflow_command, doctor::tests::only_the_openspec_format_instructs_a_workflow_command, tests/doctor_e2e.rs::a_repository_without_a_spec_system_reports_no_workflow_command`

#### Scenario: No resolved worker CLI is not applicable

- **GIVEN** an OpenSpec repository in which no specs are discovered, so no spec-driven worker CLI resolves
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` and state that there is no spec-driven worker CLI to check

Test: `doctor::tests::spec_workflow_check_is_not_applicable_without_a_resolved_worker_cli`

#### Scenario: Several worker CLIs warn naming only the unreachable ones

- **GIVEN** an OpenSpec repository with `worktree_placement = "sibling"` whose discovered specs resolve to two CLIs with known locations, `claude` and a second CLI, where only `claude`'s definition is committed
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn`
- **AND** its detail SHALL name the second CLI and its locations checked
- **AND** its detail SHALL NOT name `claude` as unreachable

Test: `doctor::tests::spec_workflow_check_warns_naming_only_the_unreachable_cli`

#### Scenario: An unknown CLI is reported, not warned

- **GIVEN** an OpenSpec repository whose spec-driven worker CLI has no entry in the location table
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Warn` (not `Ok`) with a detail stating reachability could not be verified
- **AND** its remedy SHALL name `[worktree.env] copy`
- **AND** when another resolved CLI is reachable, the detail SHALL name that CLI as the one the reachability claim covers

Test: `doctor::tests::spec_workflow_check_warns_on_an_unknown_cli, doctor::tests::a_reachable_and_an_unknown_cli_each_scope_the_detail_to_themselves, tests/doctor_e2e.rs::an_unverifiable_workflow_cli_warns_with_a_provisioning_remedy`

#### Scenario: An unverified CLI never shows as passing

- **GIVEN** an OpenSpec repository whose only spec-driven worker CLI has no entry in the location table
- **WHEN** `git paw doctor --json` runs
- **THEN** the spec-workflow check's status SHALL be the warning status, not the ok status

Test: `tests/doctor_e2e.rs::an_unverifiable_workflow_cli_warns_with_a_provisioning_remedy`

#### Scenario: The check is in the JSON report

- **WHEN** `git paw doctor --json` runs
- **THEN** the output SHALL contain the spec-workflow check with its status and detail

Test: `tests/doctor_e2e.rs::an_uncommitted_workflow_definition_warns_under_sibling_placement, tests/doctor_e2e.rs::json_document_carries_the_required_fields_on_every_check`

#### Scenario: A family variant uses its family's locations

- **GIVEN** an OpenSpec repository whose spec-driven worker CLI is `[clis.claude-opus]` with `family = "claude"`, and whose `.claude/commands/opsx/apply.md` is committed
- **WHEN** `git paw doctor` runs
- **THEN** the spec-workflow check SHALL be `Ok` and name `claude-opus` as reachable

Test: `tests/doctor_e2e.rs::a_family_variant_finds_its_familys_workflow_definitions`

### Requirement: Doctor warns about dangerous grant residue in the project's CLI settings

`git paw doctor` SHALL report in the CLIs group one grant-residue check over the project-shared and project-local settings files, in the main checkout, of every CLI identity whose fact row declares an allowlist item and records them (for `claude`: `.claude/settings.json` and `.claude/settings.local.json`). The check SHALL be reported when at least one of those files exists, and SHALL NOT be reported at all when none does. The check SHALL be ⚠ when a file holds any of:

1. a `permissions.allow` entry that grants `curl` with any arguments — a `Bash` rule whose specifier is `curl` followed only by a wildcard (for example `Bash(curl *)`, `Bash(curl:*)`, `Bash(curl*)`);
2. a `permissions.allow` `Bash` rule that names a git-paw helper script **for which the grant table lists at least one never-granted subcommand**, followed by one of those subcommands, by a wildcard standing in the subcommand position, or by a word that a wildcard completes into one of them — whatever path form names the script (relative, absolute or interpreter-prefixed), for example `Bash(.git-paw/scripts/sweep.sh approve *)`, `Bash(.git-paw/scripts/sweep.sh verified *)`, `Bash(.git-paw/scripts/sweep.sh ap*)` or `Bash(.git-paw/scripts/sweep.sh *)`. A rule naming a helper the grant table grants whole, such as `Bash(.git-paw/scripts/broker.sh *)`, withholds nothing and SHALL NOT be reported;
3. a top-level `allowed_bash_prefixes` key, the inert seed earlier git-paw versions wrote.

The ⚠ detail SHALL name each file and each matching entry, and SHALL say, for each matching entry, why that entry is dangerous: for a network-client grant, an unrestricted network client; for a helper grant, what the subcommands it reaches actually hand over — a subcommand that presses an approval keystroke or publishes a supervisor verdict SHALL be reported as letting an agent session approve its own prompts or publish a verdict without a prompt, a subcommand that passes an arbitrary URL through SHALL be reported as a read-anywhere primitive, and any other withheld subcommand SHALL be reported as one the grant table deliberately withholds; for the inert key, that it grants nothing in the current CLI but is left over from an earlier version. A rule reaching several withheld subcommands SHALL be reported by the gravest of their reasons.

Each named entry SHALL be abbreviated so that a credential embedded in the rule is not echoed: a network-client or inert-key entry as the broad-grant advisory abbreviates entries, and a helper grant rendered from the word that names the helper, so that anything preceding it — an environment assignment, which is where a secret appears in such a rule — is replaced by an elision rather than printed.

The remedy SHALL state that git-paw never rewrites grants it did not write and that the user should remove these entries by hand. The check SHALL be ✓ when no file holds a match, and ⚠ stating the file was not inspected when an existing file is not valid JSON. The check SHALL be read-only, SHALL NOT report ✗, SHALL NOT print any other content of the files, and SHALL NOT influence how any permission prompt is handled.

#### Scenario: A durable self-approval grant is flagged

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(.git-paw/scripts/sweep.sh approve *)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL be ⚠ naming that file and that entry
- **AND** its remedy SHALL say to remove the entry by hand and that git-paw does not rewrite it

Test: `cli_setup::doctor::tests::a_durable_self_approval_grant_is_flagged, tests/doctor_e2e.rs::doctor_warns_about_grant_residue_in_both_settings_files_and_edits_neither`

#### Scenario: A blanket curl grant is flagged

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(curl *)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL be ⚠ naming `Bash(curl *)`

Test: `cli_setup::doctor::tests::a_blanket_curl_grant_is_flagged`

#### Scenario: A wildcard that completes a withheld subcommand is flagged

- **GIVEN** `permissions.allow` holds `Bash(.git-paw/scripts/sweep.sh ap*)`, which covers `approve` without naming it
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL report that entry

Test: `cli_setup::doctor::tests::every_never_granted_helper_shape_is_flagged, cli_setup::doctor::tests::a_partial_wildcard_that_completes_only_granted_subcommands_is_not_residue`

#### Scenario: A matched entry is named without echoing a preceding secret

- **GIVEN** `permissions.allow` holds `Bash(GH_TOKEN=<secret> .git-paw/scripts/sweep.sh approve *)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL name the entry from the helper word onward
- **AND** the detail and the remedy SHALL NOT contain the assignment or its value

Test: `cli_setup::doctor::tests::a_residue_entry_carrying_a_credential_is_rendered_from_the_helper`

#### Scenario: The reason names what the matched subcommand hands over

- **GIVEN** `permissions.allow` holds `Bash(.git-paw/scripts/docs-fetch.sh get *)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL report it as a read-anywhere primitive, not as a self-approval grant

Test: `cli_setup::doctor::tests::the_reason_names_what_the_matched_subcommand_hands_over`

#### Scenario: The inert seed in the shared settings file is flagged

- **GIVEN** a main-checkout `.claude/settings.json` holding a top-level `allowed_bash_prefixes` array and no `.claude/settings.local.json`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL be ⚠ naming `.claude/settings.json` and `allowed_bash_prefixes`

Test: `cli_setup::doctor::tests::the_inert_seed_in_the_shared_settings_file_is_flagged`

#### Scenario: git-paw's own entries are not residue

- **GIVEN** the allowlist item applied and no other entry in either settings file
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL be ✓

Test: `cli_setup::doctor::tests::git_paws_own_entries_are_not_residue, cli_setup::doctor::tests::a_granted_helper_invocation_is_not_residue, tests/doctor_e2e.rs::undo_and_reapply_converge_when_entries_pre_existed`

#### Scenario: A loopback-scoped curl grant is not residue

- **GIVEN** `permissions.allow` holds `Bash(curl -s http://127.0.0.1:9119/status)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL NOT list that entry

Test: `cli_setup::doctor::tests::a_loopback_scoped_curl_grant_is_not_residue`

#### Scenario: Only a command rule can be residue

- **GIVEN** `permissions.allow` holds `Read(.git-paw/scripts/sweep.sh *)`, which grants reading the helper rather than running it
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL NOT list that entry

Test: `cli_setup::doctor::tests::only_a_command_rule_can_be_residue`

#### Scenario: The residue check never edits the files

- **GIVEN** both settings files holding residue
- **WHEN** `git paw doctor` runs
- **THEN** both files SHALL be byte-identical afterwards
- **AND** no check SHALL be reported ✗ because of them

Test: `cli_setup::doctor::tests::the_residue_check_never_reports_fail, tests/doctor_e2e.rs::doctor_warns_about_grant_residue_in_both_settings_files_and_edits_neither`

### Requirement: Doctor reports each worker CLI's declared sandbox mode

`git paw doctor` SHALL report, for every CLI a worker of the current configuration can resolve to
(the configured default, spec-driven and preset CLIs), its declared sandbox mode — `native`,
`wrapper` or `none` — labelled as declared, never as verified. It SHALL be read-only and SHALL NOT
invoke any CLI or sandbox.

#### Scenario: A declared sandbox is reported

- **GIVEN** `default_cli = "sbx"` and `[clis.sbx] sandbox = "wrapper"`
- **WHEN** `git paw doctor` runs
- **THEN** it SHALL report `sbx` with sandbox mode `wrapper (declared)`

### Requirement: Doctor never passes a full-auto worker on a declaration alone

When the effective worker approval level is `full-auto`, the sandbox-mode check SHALL report ⚠ with
a remedy pointing at the sandbox documentation for every reported CLI: one declaring no sandbox, and
one declaring a sandbox, labelled `declared, not verified`. Neither SHALL be ✗.

#### Scenario: Unsandboxed full-auto warns

- **GIVEN** `agent_approval = "full-auto"` and `default_cli = "claude"` with no `[clis.claude].sandbox`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ naming `claude` with a remedy pointing at the
  sandbox documentation
- **AND** it SHALL NOT report ✗

#### Scenario: A declared sandbox at full-auto is not shown as verified

- **GIVEN** `agent_approval = "full-auto"` and `default_cli = "sbx"` with `[clis.sbx] sandbox = "wrapper"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ for `sbx` labelled `declared, not verified`

### Requirement: Doctor flags sandbox declarations a launch cannot honour

The sandbox-mode check SHALL report ⚠ for a `wrapper` CLI whose `command`, by name or by the file it
resolves to on `PATH`, is a known agent CLI binary, and ✗ with the refusal for a CLI whose launch at
the effective level the native-sandbox rule would refuse.

#### Scenario: A wrapper that is the agent CLI itself warns

- **GIVEN** `default_cli = "sbx"` and `[clis.sbx] command = "claude"`, `sandbox = "wrapper"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ for `sbx` naming `claude`

#### Scenario: Doctor reports a launch the sandbox rule would refuse

- **GIVEN** `agent_approval = "full-auto"`, `default_cli = "codex"` and `[clis.codex] sandbox = "native"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ✗ naming `[clis.codex].approval_args`

### Requirement: Doctor flags an out-of-date orchestrator pointer

`git paw doctor` SHALL include in its "Hygiene" group an "orchestrator pointer" check that is `Warn` when the root `AGENTS.md` carries the orchestrator pointer section but not the section the running binary writes (for example one written before the section was scoped to the root session), with a remedy to re-run `git paw init`; and `Ok` when the section is current or absent. It SHALL be read-only and never `Fail`.

#### Scenario: An out-of-date pointer section warns

- **GIVEN** a root `AGENTS.md` whose pointer section was written by an earlier binary
- **WHEN** `git paw doctor` runs
- **THEN** the orchestrator-pointer check SHALL be `Warn` with a remedy naming `git paw init`

Test: `doctor::tests::an_out_of_date_orchestrator_pointer_warns_with_the_init_remedy`, `tests/doctor_e2e.rs::an_out_of_date_orchestrator_pointer_warns`

#### Scenario: A current or absent pointer passes

- **GIVEN** a repository just initialised by the running binary, or one with no pointer section
- **WHEN** `git paw doctor` runs
- **THEN** the orchestrator-pointer check SHALL be `Ok`

Test: `agents::tests::orchestrator_pointer_currency_is_detected`, `doctor::tests::a_clean_repository_passes_every_hygiene_check`

### Requirement: Doctor reports where each bundled skill resolves from

`git paw doctor` SHALL report a read-only `skill sources` check in the Bundled-scripts group that resolves every registered package through the production resolver and names each one's source: `embedded`, `repo <SKILL.md path>` or `user <SKILL.md path>`. It SHALL be ✓ when all resolve, ⚠ naming an ignored `cli-setup` override directory (remedy: remove it), and ✗ naming the package and override path when one fails to resolve (remedy: fix or remove it).

#### Scenario: All bundled skills embedded

- **GIVEN** no repository or user override for any bundled package
- **WHEN** `git paw doctor` is run
- **THEN** the `skill sources` check SHALL be ✓
- **AND** its detail SHALL name every registered package with `embedded`

#### Scenario: A user override is named with its path

- **GIVEN** a user override for `coordination` and no repository override
- **WHEN** `git paw doctor` is run
- **THEN** the `skill sources` detail SHALL name `coordination` as `user` with the override's `SKILL.md` path
- **AND** the check SHALL be ✓

#### Scenario: An ignored cli-setup override warns

- **GIVEN** a repository override directory `<main>/.agents/skills/cli-setup/` containing a `SKILL.md`
- **WHEN** `git paw doctor` is run
- **THEN** the `skill sources` check SHALL be ⚠
- **AND** its detail SHALL name `cli-setup` as `embedded` and name the override directory as ignored
- **AND** it SHALL NOT be ✗

#### Scenario: A broken override fails the check

- **GIVEN** a repository override for `coordination` whose frontmatter is invalid
- **WHEN** `git paw doctor` is run
- **THEN** the `skill sources` check SHALL be ✗
- **AND** its detail SHALL name `coordination` and the override's path
- **AND** its remedy SHALL say to fix or remove that override

#### Scenario: The check appears in JSON output

- **WHEN** `git paw doctor --json` is run
- **THEN** the output SHALL contain a check named `skill sources` with its status and detail

### Requirement: Doctor evaluates worker profiles at their own approval level

`git paw doctor`'s sandbox-mode check SHALL also report the CLI of every configured worker profile, evaluated at that profile's effective approval level (its `approval`, else the worker default) rather than only the session-wide level, and its spec-CLI check SHALL resolve a spec's `paw_profile` to that profile's CLI. Both SHALL stay read-only.

#### Scenario: A full-auto worker profile on an unsandboxed CLI warns

- **GIVEN** `agent_approval = "manual"`, no `[clis.claude].sandbox`, and `[profiles.trusted]` with `kind = "worker"`, `cli = "claude"`, `approval = "full-auto"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ naming `claude` with a remedy pointing at the sandbox documentation
- **AND** it SHALL NOT report ✗

#### Scenario: A spec's profile resolves to its CLI

- **GIVEN** a discovered spec with `paw_profile: routine` and `[profiles.routine]` with `kind = "worker"`, `cli = "beta"`
- **WHEN** `git paw doctor` runs
- **THEN** the spec-CLI check SHALL report `beta` for that spec
