## MODIFIED Requirements

### Requirement: The plan is printed before anything is written

Before prompting and before any write, the command SHALL print the plan: the CLI and item, the
item's verification status, every file it would create or modify by absolute path and marked as
created or modified, every entry it would add to each file with its key path and value (or, for a
line file, the line), and the setup-record file. The plan SHALL NOT print any existing content of a
target file. With `--dry-run`, the command SHALL print the plan, write nothing (including the setup
record) and exit `0`, even when `--yes` is also given. When every planned entry is already present,
the command SHALL report that there is nothing to do, write nothing and exit `0` without prompting.

#### Scenario: The plan lists files and entries

- **GIVEN** a repository with no `.claude/settings.local.json` at its root
- **WHEN** `git paw setup-cli claude --allowlist --dry-run` is run
- **THEN** stdout SHALL name the absolute path of `<repo>/.claude/settings.local.json` as created
- **AND** it SHALL list the entry `Bash(.git-paw/skills/coordination/scripts/broker.sh *)` under `permissions.allow`
- **AND** it SHALL list the entries `Edit(**/.git-paw/skills/**)` and `Edit(**/.git-paw/scripts/**)` under `permissions.deny`
- **AND** it SHALL name the setup-record file

Test: `tests/cli_setup_e2e.rs::the_plan_names_the_main_checkout_file_and_its_entries, cli_setup::render::tests::a_verified_plan_renders_exactly_the_golden`

#### Scenario: The plan never echoes existing settings

- **GIVEN** a `.claude/settings.local.json` whose `env` object holds `"API_TOKEN": "s3cret"`
- **WHEN** `git paw setup-cli claude --allowlist --dry-run` is run
- **THEN** stdout and stderr SHALL NOT contain `s3cret`

Test: `tests/cli_setup_e2e.rs::the_plan_never_echoes_an_existing_secret, cli_setup::render::tests::a_secret_in_the_target_file_never_reaches_the_rendered_plan`

#### Scenario: Dry run writes nothing

- **WHEN** `git paw setup-cli claude --allowlist --dry-run --yes` is run
- **THEN** it SHALL exit `0`
- **AND** no target file and no setup record SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::a_dry_run_writes_nothing_even_with_yes`

#### Scenario: Nothing to do

- **GIVEN** every planned allowlist entry is already present in the target file
- **WHEN** `git paw setup-cli claude --allowlist` is run on a terminal
- **THEN** it SHALL report that there is nothing to do and exit `0` without prompting
- **AND** no file SHALL be modified

Test: `tests/cli_setup_e2e.rs::nothing_to_do_exits_zero_without_writing, cli_setup::plan::tests::nothing_to_do_when_every_entry_is_present`

### Requirement: Allowlist entries are path-scoped to git-paw's helper scripts

For the `claude` row, the allowlist item SHALL add to `permissions.allow` only entries that name a
git-paw helper script by its package invocation path — `.git-paw/skills/<package>/scripts/<helper>`,
relative and derived from the bundled package registry (`skill-packages`) — from a fixed grant table
keyed by the names of the helper scripts the binary bundles:

- `broker.sh` (`coordination` package): every subcommand, as
  `Bash(.git-paw/skills/coordination/scripts/broker.sh *)`;
- `docs-fetch.sh` (`docs-fetch` package): only `find`, as
  `Bash(.git-paw/skills/docs-fetch/scripts/docs-fetch.sh find *)`;
- `sweep.sh` (`orchestrator` package): only the subcommands that send no keystrokes and publish no
  verdict — `snapshot`, `capture`, `status`, `worktrees-status`, `inbox`, `detect-stuck`,
  `stuck-eval`, `status-publish`, `learn` and `events` — each as
  `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh <subcommand> *)`; `events` is granted
  read-only, because it only reads the broker's log and status and sends no keystroke and publishes
  nothing;
- `_paw_common.sh`: no entry (it is sourced, never invoked).

The item SHALL NOT add an allow entry naming the deprecated compatibility path
`.git-paw/scripts/<helper>`; nothing bundled invokes it.

The item SHALL add `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh approve *)` and
`Bash(.git-paw/skills/orchestrator/scripts/sweep.sh tell *)` to `permissions.ask`, together with the
same two rules at the compatibility path (`Bash(.git-paw/scripts/sweep.sh approve *)` and
`Bash(.git-paw/scripts/sweep.sh tell *)`), so the keystroke-sending `approve` and `tell`
subcommands always prompt at either path even when another rule would allow them. The item SHALL NOT
add an allow entry for `sweep.sh approve`, `sweep.sh tell`, `sweep.sh verified`,
`sweep.sh feedback-gate`, `sweep.sh conflict-ack`, `sweep.sh answer` or `sweep.sh advanced-main`, for
`docs-fetch.sh get`, an interpreter-prefixed form, an absolute-path form, a wildcard over a directory,
or any entry naming a command other than a git-paw helper script. Every bundled helper script SHALL
appear in the grant table, either with its granted subcommands or as granted nothing, and every
`sweep.sh` subcommand SHALL be either granted or listed as never granted.

#### Scenario: The allow entries are exactly the grant table

- **WHEN** the allowlist item is planned for `claude`
- **THEN** its `permissions.allow` entries SHALL be exactly the twelve grant-table entries
- **AND** its `permissions.ask` entries SHALL include `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh approve *)`, `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh tell *)`, `Bash(.git-paw/scripts/sweep.sh approve *)` and `Bash(.git-paw/scripts/sweep.sh tell *)`

Test: `cli_setup::facts::tests::allow_entries_are_exactly_the_grant_table, cli_setup::plan::tests::allow_entries_are_exactly_the_grant_table`

#### Scenario: The read-only events subcommand is granted

- **WHEN** the allowlist item is planned for `claude`
- **THEN** its `permissions.allow` entries SHALL include `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh events *)`

#### Scenario: Keystroke and verdict subcommands are never allowed

- **WHEN** the allowlist item is planned for `claude`
- **THEN** no `permissions.allow` entry SHALL name `sweep.sh approve`, `sweep.sh tell`, `sweep.sh verified`, `sweep.sh feedback-gate` or `docs-fetch.sh get`
- **AND** no `permissions.allow` entry SHALL begin `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh *` or `Bash(.git-paw/scripts/sweep.sh *`

Test: `cli_setup::facts::tests::no_allow_entry_names_a_keystroke_or_verdict_subcommand, cli_setup::facts::tests::the_keystroke_subcommand_is_asked_not_allowed`

#### Scenario: The message-typing tell subcommand always prompts

- **WHEN** the allowlist item is planned for `claude`
- **THEN** its `permissions.ask` entries SHALL include `Bash(.git-paw/skills/orchestrator/scripts/sweep.sh tell *)` and `Bash(.git-paw/scripts/sweep.sh tell *)`
- **AND** the grant table SHALL list `sweep.sh tell` as never granted

#### Scenario: Every planned allow entry names a bundled helper

- **WHEN** the allowlist item is planned
- **THEN** every `permissions.allow` entry SHALL begin `Bash(.git-paw/skills/<package>/scripts/` followed by the name of a helper that registered package bundles
- **AND** no entry SHALL contain `curl`, `bash `, a leading `/` path, or `Bash(*`

Test: `cli_setup::facts::tests::every_allow_entry_is_a_path_scoped_bundled_helper_invocation`

#### Scenario: The compatibility path is never allowed

- **WHEN** the allowlist item is planned for `claude`
- **THEN** no `permissions.allow` entry SHALL begin `Bash(.git-paw/scripts/`

#### Scenario: The grant table covers every helper and subcommand

- **WHEN** the bundled helper scripts and the subcommands `sweep.sh` dispatches are enumerated
- **THEN** each helper SHALL have a grant-table row
- **AND** each `sweep.sh` subcommand SHALL be either granted or listed as never granted

Test: `cli_setup::facts::tests::every_bundled_helper_has_a_grant_table_row, cli_setup::facts::tests::every_dispatched_sweep_subcommand_is_granted_or_never_granted`

### Requirement: The allowlist item guards the granted helpers against silent modification

Because an allowed helper runs without a prompt, the allowlist item SHALL, in the same file and the
same write, add the protection rules the fact row declares, so that an agent session anywhere in the
repository cannot change what a granted entry runs without a prompt:

- `Edit(**/.git-paw/skills/**)` in `permissions.deny`, refusing the CLI's file-editing tools on any
  installed skill package — the granted helpers under its `scripts/` included — at any depth under
  the session's working directory;
- `Edit(**/.git-paw/scripts/**)` in `permissions.deny`, refusing the same tools on any compatibility
  helper-script directory at any depth;
- `Edit(**/.git-paw/config.toml)` in `permissions.ask`, so a change to the configuration the helpers
  read (for example the broker address) always prompts.

The allowlist entries SHALL NOT be planned without these protection rules, and the fact row SHALL be
marked verified only after the protection has been confirmed against the CLI for the file-editing
tools and for the filesystem commands the CLI auto-approves in its edit-accepting mode. That
confirmation SHALL cover the package-path entries and both deny rules above; a confirmation made
only for the pre-package `.git-paw/scripts/` entries and rule SHALL NOT count, and without a
confirmation that covers them the item is plan-only (*An item whose facts are unverified is
plan-only*).

#### Scenario: Protection is planned with the grants

- **WHEN** the allowlist item is planned for `claude`
- **THEN** the plan SHALL list `Edit(**/.git-paw/skills/**)` and `Edit(**/.git-paw/scripts/**)` under `permissions.deny`
- **AND** it SHALL list `Edit(**/.git-paw/config.toml)` under `permissions.ask`
- **AND** the protection entries SHALL be written in the same file replacement as the allow entries

Test: `cli_setup::plan::tests::protection_is_planned_with_the_grants_in_one_file, cli_setup::facts::tests::protection_rules_accompany_the_grants`

#### Scenario: A protection rule the user already has is not claimed

- **GIVEN** `permissions.deny` already contains `Edit(**/.git-paw/scripts/**)`
- **WHEN** the allowlist item is applied
- **THEN** that entry SHALL appear exactly once
- **AND** the setup record SHALL NOT list it

Test: `cli_setup::plan::tests::a_protection_rule_already_present_is_not_planned`

### Requirement: Writes add only the planned entries

The command SHALL write only the planned entries. In a JSON target file, every key and value it does
not add SHALL keep its value and its order; missing containers on the key path SHALL be created, and
added array items SHALL follow the existing items. An entry that is already present SHALL NOT be added
again and SHALL NOT be recorded as written by git-paw. Each file SHALL be replaced atomically; an
existing file SHALL keep its permissions and a created file SHALL be readable and writable by its owner
only. Before replacing a file, the command SHALL read it again, and if it changed since the plan was
made, the command SHALL stop with an error naming the file and write no further file.

#### Scenario: Unrelated settings are preserved

- **GIVEN** a `.claude/settings.local.json` holding `{"model": "x", "permissions": {"deny": ["y"], "allow": ["z"]}}`
- **WHEN** the allowlist item is applied to it
- **THEN** `model`, the existing `y` and `z` entries and the order of every existing key SHALL be unchanged
- **AND** the helper entries SHALL follow `z` in `permissions.allow`

Test: `tests/cli_setup_e2e.rs::unrelated_settings_keep_their_value_and_order, cli_setup::shell::tests::unrelated_settings_keep_their_value_and_their_order`

#### Scenario: An entry the user already had is not claimed

- **GIVEN** `permissions.allow` already contains `Bash(.git-paw/skills/coordination/scripts/broker.sh *)`
- **WHEN** the allowlist item is applied
- **THEN** that entry SHALL appear exactly once
- **AND** the setup record SHALL NOT list it

Test: `cli_setup::shell::tests::an_entry_already_present_is_neither_written_twice_nor_recorded, tests/cli_setup_e2e.rs::an_entry_the_user_already_had_is_never_claimed_by_undo`

#### Scenario: A concurrent change stops the run

- **GIVEN** a target file that changes between planning and writing
- **WHEN** the command reaches that file
- **THEN** it SHALL stop with an error naming the file
- **AND** that file and every later file in the plan SHALL be left unmodified

Test: `cli_setup::shell::tests::a_concurrent_change_stops_the_run_and_leaves_later_files_untouched`

#### Scenario: A created file is owner-only

- **GIVEN** no `.claude/settings.local.json` at the repository root
- **WHEN** the allowlist item creates it
- **THEN** the file SHALL be readable and writable by its owner only

Test: `cli_setup::shell::tests::a_created_file_is_owner_only_and_an_existing_file_keeps_its_mode`

### Requirement: Undo removes exactly what git-paw added

`git paw setup-cli <CLI> --undo` SHALL plan the removal of the recorded entries for `<CLI>`, limited to
the named items when item flags are given and covering every recorded item otherwise, and SHALL apply
it through the same plan output and confirmation gate as setup. An entry SHALL be removed only when it
is still present, equal to its recorded value, and of a shape its item produces: in the item's declared
target file, an allow or ask value `Bash(.git-paw/scripts/<name>…)` or
`Bash(.git-paw/skills/<package>/scripts/<name>…)` naming a helper-script file name, or a file-tool rule
whose path lies under `.git-paw/`; in the shared `info/exclude` file, the anchored exclusion line for
the item's target file. A recorded entry that is absent, changed or not of a valid shape SHALL NOT be
removed from disk; it SHALL be reported and dropped from the record. Protection entries SHALL be
removed only when, after the removal, the file holds no allow entry naming a path under
`.git-paw/scripts/` or `.git-paw/skills/`. A file git-paw created SHALL be deleted only when what
remains equals the empty structure git-paw created, and a directory git-paw created SHALL be removed
only when it is empty. Undo SHALL NOT modify any other content and SHALL NOT touch entries recorded
for another CLI.

#### Scenario: Undo removes only recorded entries

- **GIVEN** recorded allowlist entries for `claude` in a file that also holds a user entry `Bash(make test *)`
- **WHEN** `git paw setup-cli claude --undo --yes` is run
- **THEN** the recorded entries SHALL be removed
- **AND** `Bash(make test *)` and every other key SHALL be unchanged

Test: `cli_setup::undo::tests::removes_recorded_present_equal_shape_valid_entries_only, cli_setup::shell::tests::undo_removes_git_paws_entries_and_leaves_the_users_alone`

#### Scenario: A created file is deleted when only git-paw's skeleton remains

- **GIVEN** a target file git-paw created and nobody else edited
- **WHEN** undo removes its entries
- **THEN** the file SHALL be deleted, and its `.claude` directory too if git-paw created it and it is now empty

Test: `cli_setup::undo::tests::a_skeleton_only_created_file_is_deleted_with_its_directory, cli_setup::shell::tests::a_skeleton_only_created_file_is_deleted_with_its_directory`

#### Scenario: A created file the user extended is kept

- **GIVEN** a target file git-paw created to which the user added `"model": "x"`
- **WHEN** undo removes git-paw's entries
- **THEN** the file SHALL remain with `"model": "x"`

Test: `cli_setup::undo::tests::a_created_file_the_user_extended_is_kept, cli_setup::shell::tests::a_created_file_the_user_extended_is_kept`

#### Scenario: A changed entry is left in place

- **GIVEN** a recorded exclusion line that the user has since edited
- **WHEN** undo runs
- **THEN** the edited line SHALL be left in place and reported
- **AND** it SHALL be dropped from the record

Test: `cli_setup::undo::tests::an_edited_exclusion_line_is_left_in_place_and_reported`

#### Scenario: A forged record entry is not removed

- **GIVEN** a setup record edited to list `Bash(make test *)` as a `claude` allowlist entry
- **WHEN** undo runs
- **THEN** `Bash(make test *)` SHALL NOT be removed
- **AND** the entry SHALL be reported as invalid and dropped from the record

Test: `cli_setup::undo::tests::a_forged_entry_is_reported_dropped_and_left_on_disk, cli_setup::shell::tests::a_forged_record_entry_is_dropped_but_left_on_disk`

#### Scenario: Protection outlives a remaining helper grant

- **GIVEN** recorded allowlist entries where the user has since edited the recorded `Bash(.git-paw/scripts/broker.sh *)` entry to `Bash(.git-paw/scripts/broker.sh*)`
- **WHEN** `git paw setup-cli claude --undo --yes` is run
- **THEN** the edited entry SHALL be left in place and reported
- **AND** `Edit(**/.git-paw/scripts/**)` SHALL remain in `permissions.deny`

Test: `cli_setup::undo::tests::protection_is_kept_while_an_edited_helper_grant_remains, cli_setup::shell::tests::protection_survives_a_remaining_helper_grant`

#### Scenario: Entries recorded before the package move are undone

- **GIVEN** a setup record written by an earlier version whose `claude` allowlist entries name `.git-paw/scripts/` paths, all still present and unchanged
- **WHEN** `git paw setup-cli claude --undo --yes` is run
- **THEN** those entries and their protection rules SHALL be removed
- **AND** no other entry in the file SHALL change

#### Scenario: Package-path entries are undone

- **GIVEN** recorded allowlist entries naming `.git-paw/skills/<package>/scripts/` paths and both deny rules, all unchanged
- **WHEN** `git paw setup-cli claude --undo --yes` is run
- **THEN** the allow, ask and deny entries git-paw recorded SHALL be removed

#### Scenario: Another CLI's entries are untouched

- **GIVEN** a setup record that also holds entries recorded for another CLI identity
- **WHEN** `git paw setup-cli claude --undo --yes` is run
- **THEN** every entry recorded for the other CLI SHALL remain on disk and in the record

Test: `cli_setup::undo::tests::another_clis_entries_are_untouched_on_disk_and_in_the_record, cli_setup::shell::tests::another_clis_entries_are_untouched`
