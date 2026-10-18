## MODIFIED Requirements

### Requirement: Allowlist entries are path-scoped to git-paw's helper scripts

For the `claude` row, the allowlist item SHALL add to `permissions.allow` only entries that name a
git-paw helper script by its relative invocation path under `.git-paw/scripts/`, from a fixed grant
table keyed by the names of the helper scripts the binary bundles:

- `broker.sh`: every subcommand, as `Bash(.git-paw/scripts/broker.sh *)`;
- `docs-fetch.sh`: only `find`, as `Bash(.git-paw/scripts/docs-fetch.sh find *)`;
- `sweep.sh`: only the subcommands that send no keystrokes and publish no verdict — `snapshot`,
  `capture`, `status`, `worktrees-status`, `inbox`, `detect-stuck`, `stuck-eval`, `status-publish`,
  `learn` and `events` — each as `Bash(.git-paw/scripts/sweep.sh <subcommand> *)`; `events` is
  granted read-only, because it only reads the broker's log and status and sends no keystroke and
  publishes nothing;
- `_paw_common.sh`: no entry (it is sourced, never invoked).

The item SHALL add `Bash(.git-paw/scripts/sweep.sh approve *)` to `permissions.ask`, so the
keystroke-sending `approve` subcommand always prompts even when another rule would allow it. The
item SHALL also add to `permissions.ask` these guard rules, so that an agent session cannot, without
a prompt, bypass the git hooks git-paw installs or drive the session's tmux panes, even when another
rule would allow the command:

- git hook bypass: `Bash(git -c *)`, `Bash(git * --no-verify*)` and `Bash(git commit -n *)`;
- tmux keystroke and command injection: `Bash(tmux send-keys *)`, `Bash(tmux send *)`,
  `Bash(tmux run-shell *)`, `Bash(tmux run *)`, `Bash(tmux pipe-pane *)` and `Bash(tmux pipep *)`.

Each guard rule SHALL be confirmed against the CLI to prompt for the invocation it names, in the
permission modes the fact row's evidence records, before the row lists it; a form the CLI does not
match as intended SHALL NOT be shipped. The item SHALL NOT add an allow entry for `sweep.sh approve`,
`sweep.sh verified`, `sweep.sh feedback-gate`, `sweep.sh conflict-ack`, `sweep.sh answer` or
`sweep.sh advanced-main`, for `docs-fetch.sh get`, an interpreter-prefixed
form, an absolute-path form, a wildcard over a directory, or any allow entry naming a command other
than a git-paw helper script; the guard rules are the only entries naming another command, and they
SHALL be ask entries only. Every bundled helper script SHALL appear in the grant table, either with
its granted subcommands or as granted nothing, and every `sweep.sh` subcommand SHALL be either
granted or listed as never granted.

#### Scenario: The allow entries are exactly the grant table

- **WHEN** the allowlist item is planned for `claude`
- **THEN** its `permissions.allow` entries SHALL be exactly the twelve grant-table entries
- **AND** its `permissions.ask` entries SHALL include `Bash(.git-paw/scripts/sweep.sh approve *)`

Test: `cli_setup::facts::tests::allow_entries_are_exactly_the_grant_table, cli_setup::plan::tests::allow_entries_are_exactly_the_grant_table`

#### Scenario: The read-only events subcommand is granted

- **WHEN** the allowlist item is planned for `claude`
- **THEN** its `permissions.allow` entries SHALL include `Bash(.git-paw/scripts/sweep.sh events *)`

#### Scenario: Keystroke and verdict subcommands are never allowed

- **WHEN** the allowlist item is planned for `claude`
- **THEN** no `permissions.allow` entry SHALL name `sweep.sh approve`, `sweep.sh verified`, `sweep.sh feedback-gate` or `docs-fetch.sh get`
- **AND** no `permissions.allow` entry SHALL begin `Bash(.git-paw/scripts/sweep.sh *`

Test: `cli_setup::facts::tests::no_allow_entry_names_a_keystroke_or_verdict_subcommand, cli_setup::facts::tests::the_keystroke_subcommand_is_asked_not_allowed`

#### Scenario: Every planned allow entry names a bundled helper

- **WHEN** the allowlist item is planned
- **THEN** every `permissions.allow` entry SHALL begin `Bash(.git-paw/scripts/` followed by the name of a helper the binary bundles
- **AND** no entry SHALL contain `curl`, `bash `, a leading `/` path, or `Bash(*`

Test: `cli_setup::facts::tests::every_allow_entry_is_a_path_scoped_bundled_helper_invocation`

#### Scenario: The grant table covers every helper and subcommand

- **WHEN** the bundled helper scripts and the subcommands `sweep.sh` dispatches are enumerated
- **THEN** each helper SHALL have a grant-table row
- **AND** each `sweep.sh` subcommand SHALL be either granted or listed as never granted

Test: `cli_setup::facts::tests::every_bundled_helper_has_a_grant_table_row, cli_setup::facts::tests::every_dispatched_sweep_subcommand_is_granted_or_never_granted`

#### Scenario: Hook bypass and tmux injection always prompt

- **WHEN** the allowlist item is planned for `claude`
- **THEN** its `permissions.ask` entries SHALL include `Bash(git -c *)`, `Bash(git * --no-verify*)` and `Bash(git commit -n *)`
- **AND** they SHALL include `Bash(tmux send-keys *)`, `Bash(tmux send *)`, `Bash(tmux run-shell *)`, `Bash(tmux run *)`, `Bash(tmux pipe-pane *)` and `Bash(tmux pipep *)`

#### Scenario: Guard rules are never allowed

- **WHEN** the allowlist item is planned for `claude`
- **THEN** no `permissions.allow` entry SHALL begin `Bash(git` or `Bash(tmux`

#### Scenario: A guard rule the user already has is not claimed

- **GIVEN** `permissions.ask` already contains `Bash(tmux send-keys *)`
- **WHEN** the allowlist item is applied
- **THEN** that entry SHALL appear exactly once
- **AND** the setup record SHALL NOT list it

### Requirement: Undo removes exactly what git-paw added

`git paw setup-cli <CLI> --undo` SHALL plan the removal of the recorded entries for `<CLI>`, limited to
the named items when item flags are given and covering every recorded item otherwise, and SHALL apply
it through the same plan output and confirmation gate as setup. An entry SHALL be removed only when it
is still present, equal to its recorded value, and of a shape its item produces: in the item's declared
target file, an allow or ask value `Bash(.git-paw/scripts/<name>…)` naming a helper-script file name, an
ask value equal to one of the guard rules the grant table declares, or a file-tool rule whose path lies
under `.git-paw/`; in the shared `info/exclude` file, the anchored exclusion line for the item's target
file. A recorded entry that is absent, changed or not of a valid shape SHALL NOT be removed from disk;
it SHALL be reported and dropped from the record. Protection entries SHALL be removed only when, after
the removal, the file holds no allow entry naming a path under `.git-paw/scripts/`. Guard rules are not
protection entries and SHALL be removed whenever they are recorded, present and equal. A file git-paw
created SHALL be deleted only when what remains equals the empty structure git-paw created, and a
directory git-paw created SHALL be removed only when it is empty. Undo SHALL NOT modify any other content
and SHALL NOT touch entries recorded for another CLI.

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

#### Scenario: Another CLI's entries are untouched

- **GIVEN** a setup record that also holds entries recorded for another CLI identity
- **WHEN** `git paw setup-cli claude --undo --yes` is run
- **THEN** every entry recorded for the other CLI SHALL remain on disk and in the record

Test: `cli_setup::undo::tests::another_clis_entries_are_untouched_on_disk_and_in_the_record, cli_setup::shell::tests::another_clis_entries_are_untouched`

#### Scenario: Recorded guard rules are removed

- **GIVEN** the allowlist item applied by this version, so the record lists `Bash(git -c *)` and `Bash(tmux send-keys *)` under `permissions.ask`
- **WHEN** `git paw setup-cli claude --undo --yes` is run
- **THEN** both entries SHALL be removed from `permissions.ask`

#### Scenario: A forged git ask entry that is not a guard rule is not removed

- **GIVEN** a setup record edited to list `Bash(git push *)` as a `claude` ask entry, and that value present in `permissions.ask`
- **WHEN** undo runs
- **THEN** `Bash(git push *)` SHALL NOT be removed
- **AND** the entry SHALL be reported as invalid and dropped from the record
