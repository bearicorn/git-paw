# cli-setup Specification

## Purpose
Defines `git paw setup-cli <CLI>`, the one command through which git-paw writes into an agent CLI's
own settings. Running it is the user's consent: no launch, recovery, drive loop, dashboard or MCP
tool ever applies it. A compiled per-CLI fact table declares which setup items a CLI supports, and
an item whose facts are not verified against the installed CLI stays plan-only. The command prints
the exact plan before any write and writes only after confirmation on a terminal or an explicit
`--yes`. The allowlist item targets the main checkout's project-local settings file. It adds only
path-scoped allow rules for git-paw's own helper scripts, together with the rules that guard those
helpers against silent modification. It keeps every unrelated key, value and key order, and it
refuses unsafe targets. Every write is recorded in the setup record, and `--undo` removes exactly
what git-paw added.
## Requirements
### Requirement: The setup-cli command applies named setup items for one agent CLI

The system SHALL provide a `git paw setup-cli <CLI>` subcommand with the item flags `--allowlist`,
`--trust` and `--hooks`, and the options `--dry-run`, `--yes` and `--undo`. At least one item flag
SHALL be required unless `--undo` is given; a run without one SHALL be rejected as a usage error with
exit status `2`. `<CLI>` SHALL be a CLI identity git-paw knows — a name in the built-in CLI roster or
a `[clis.<name>]` key — and an unknown `<CLI>` SHALL fail with an error naming it. The command SHALL
refuse to run from a linked worktree (a checkout whose git directory differs from the repository's
common git directory), with an error telling the user to run it from the repository's main checkout.
A run that is refused or rejected SHALL write nothing.

#### Scenario: An item flag is required

- **GIVEN** a git repository
- **WHEN** `git paw setup-cli claude` is run with no item flag and no `--undo`
- **THEN** it SHALL exit `2` with a usage error naming `--allowlist`, `--trust` and `--hooks`
- **AND** no file SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::an_item_flag_is_required, tests/cli_tests.rs::setup_cli_requires_an_item_flag_or_undo`

#### Scenario: An unknown CLI is rejected

- **WHEN** `git paw setup-cli no-such-cli --allowlist` is run
- **THEN** it SHALL fail with an error naming `no-such-cli`
- **AND** no file SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::an_unknown_cli_is_rejected_naming_it`

#### Scenario: Running from a linked worktree is refused

- **GIVEN** the current directory is inside a linked worktree of the repository
- **WHEN** `git paw setup-cli claude --allowlist --yes` is run there
- **THEN** it SHALL fail with an error telling the user to run it from the main checkout
- **AND** no file SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::running_from_a_linked_worktree_is_refused, cli_setup::shell::tests::running_from_a_linked_worktree_is_refused_naming_the_main_checkout`

#### Scenario: Help documents every item and option

- **WHEN** `git paw setup-cli --help` is run
- **THEN** stdout SHALL name `--allowlist`, `--trust`, `--hooks`, `--dry-run`, `--yes` and `--undo`
- **AND** it SHALL include at least one example invocation

Test: `tests/cli_tests.rs::setup_cli_help_documents_every_item_and_option`

### Requirement: Setup items are declared per CLI by a compiled fact table

A setup item SHALL be available for a CLI only when git-paw's compiled per-CLI setup fact table holds
a row for that CLI identity declaring the item. The table SHALL hold exactly one row, for the `claude`
identity, declaring exactly the `allowlist` item; no row SHALL declare `trust` or `hooks`, and no
other CLI identity (including a `[clis.<name>]` entry whose command launches a variant of a declared
CLI) SHALL have a row. When any requested item is not declared for `<CLI>`, the command SHALL print,
for each such item, that it is not supported for that CLI, SHALL write nothing for any requested item,
and SHALL exit `2`. Every entry an item writes SHALL be derived only from the fact row and the
repository root path, and never from a command argument, from configuration, or from repository
content.

#### Scenario: Trust is not supported for claude

- **WHEN** `git paw setup-cli claude --trust --yes` is run
- **THEN** it SHALL print that trust pre-acceptance is not supported for `claude`
- **AND** it SHALL exit `2`
- **AND** no file SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::trust_is_not_supported_for_claude, cli_setup::plan::tests::a_single_unsupported_item_plans_nothing`

#### Scenario: A mixed request writes nothing

- **WHEN** `git paw setup-cli claude --allowlist --hooks --yes` is run
- **THEN** it SHALL report that hooks are not supported for `claude`
- **AND** it SHALL exit `2`
- **AND** no allowlist entry SHALL be written

Test: `tests/cli_setup_e2e.rs::a_mixed_request_writes_nothing, cli_setup::plan::tests::a_mixed_request_plans_nothing_at_all`

#### Scenario: A CLI without a fact row supports no item

- **GIVEN** `[clis.my-agent] command = "my-agent"` in the configuration
- **WHEN** `git paw setup-cli my-agent --allowlist --yes` is run
- **THEN** it SHALL print that the allowlist item is not supported for `my-agent`
- **AND** it SHALL exit `2`
- **AND** no file SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::a_cli_without_a_fact_row_supports_no_item, cli_setup::plan::tests::a_row_declaring_nothing_supports_no_item`

#### Scenario: Fact support is data

- **WHEN** the setup fact table is read
- **THEN** it SHALL contain exactly one row, for `claude`, declaring exactly `allowlist`

Test: `cli_setup::facts::tests::fact_table_holds_exactly_the_claude_allowlist_row, cli_setup::facts::tests::no_row_declares_trust_or_hooks`

#### Scenario: Entries do not depend on configuration

- **GIVEN** two repositories whose `.git-paw/config.toml` files differ in every field
- **WHEN** `git paw setup-cli claude --allowlist --dry-run` is run in each
- **THEN** the planned entries SHALL be identical

Test: `cli_setup::plan::tests::two_differently_configured_repositories_plan_identical_entries, cli_setup::facts::tests::planned_entries_do_not_depend_on_anything_but_the_tables`

### Requirement: An item whose facts are unverified is plan-only

Each declared item in the fact table SHALL carry a verification status recording whether its facts
(target file, key paths and entry forms, and the protection the entries rely on) were confirmed
against an installed copy of the CLI. For an item whose status is unverified, `--dry-run` SHALL print
the plan with a notice that writing is disabled because the facts are unverified, and a run without
`--dry-run` SHALL print the same notice, write nothing, and exit `2`. `--undo` SHALL remain available
for an unverified item so entries written by an earlier version can still be removed.

#### Scenario: An unverified item can be planned

- **GIVEN** a fact row whose allowlist item is unverified
- **WHEN** `git paw setup-cli claude --allowlist --dry-run` is run
- **THEN** it SHALL print the plan and a notice that writing is disabled because the facts are unverified
- **AND** it SHALL exit `0`

Test: `cli_setup::plan::tests::an_unverified_row_still_plans_but_is_marked, cli_setup::render::tests::an_unverified_plan_carries_the_plan_only_notice_and_still_lists_entries`

#### Scenario: An unverified item is not written

- **GIVEN** a fact row whose allowlist item is unverified
- **WHEN** `git paw setup-cli claude --allowlist --yes` is run
- **THEN** it SHALL print that writing is disabled because the facts are unverified
- **AND** it SHALL exit `2`
- **AND** no target file and no setup record SHALL be created or modified

Test: `cli_setup::render::tests::an_unverified_plan_carries_the_plan_only_notice_and_still_lists_entries`

#### Scenario: Undo works for an unverified item

- **GIVEN** recorded allowlist entries for `claude` and a fact row whose allowlist item is unverified
- **WHEN** `git paw setup-cli claude --undo --yes` is run
- **THEN** the recorded entries SHALL be removed under the undo rules

Test: `cli_setup::undo::tests::undo_works_for_an_unverified_row`

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
- **AND** it SHALL list the entry `Bash(.git-paw/scripts/broker.sh *)` under `permissions.allow`
- **AND** it SHALL list the entry `Edit(**/.git-paw/scripts/**)` under `permissions.deny`
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

### Requirement: Writing requires confirmation on a terminal or an explicit --yes

When standard input is a terminal and `--yes` is not given, the command SHALL ask the user to
confirm the printed plan, with No as the default. If the user declines, it SHALL write nothing and
exit `2`. When standard input is not a terminal and `--yes` is not given, it SHALL write nothing,
SHALL print that confirmation could not be shown and that `--yes` (or `--dry-run`) is required, and
SHALL exit `2`. With `--yes`, it SHALL apply the plan without prompting. This gate SHALL apply to
`--undo` in the same way.

#### Scenario: Declining writes nothing

- **GIVEN** a terminal session and a non-empty allowlist plan
- **WHEN** the user answers No at the confirmation
- **THEN** the command SHALL exit `2`
- **AND** no target file and no setup record SHALL be created or modified

Test: `error::tests::test_not_attempted_exit_code_and_message`

#### Scenario: No terminal and no --yes

- **GIVEN** standard input is not a terminal
- **WHEN** `git paw setup-cli claude --allowlist` is run
- **THEN** it SHALL exit `2`
- **AND** stderr SHALL name `--yes`
- **AND** stderr SHALL NOT read only `Cancelled.`
- **AND** no target file and no setup record SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::no_terminal_and_no_yes_writes_nothing_and_says_what_to_do`

#### Scenario: --yes applies without prompting

- **GIVEN** standard input is not a terminal and a verified allowlist item
- **WHEN** `git paw setup-cli claude --allowlist --yes` is run
- **THEN** it SHALL write the planned entries and exit `0`

Test: `tests/cli_setup_e2e.rs::yes_applies_the_plan_records_it_and_leaves_git_status_clean`

#### Scenario: Undo is gated the same way

- **GIVEN** recorded allowlist entries and standard input not a terminal
- **WHEN** `git paw setup-cli claude --undo` is run without `--yes`
- **THEN** it SHALL exit `2` and leave every recorded entry in place

Test: `tests/cli_setup_e2e.rs::undo_is_gated_the_same_way`

### Requirement: The allowlist target is the main checkout's project-local settings file

For the `claude` row, the allowlist item SHALL target exactly one settings file: the project-local
settings file `.claude/settings.local.json` at the root of the repository's main checkout, which the
fact table records as the file the CLI reads for a session started anywhere in the repository,
including inside a linked worktree nested under the main checkout. The item SHALL NOT write a
settings file inside any linked worktree, a user-level settings file, or the path configured as
`[clis.<name>].settings_path`. When the target file is not ignored by git in the main checkout, the
plan SHALL also add one anchored exclusion line for it (`/.claude/settings.local.json`) to the
repository's shared `info/exclude` file.

#### Scenario: The target is the main checkout's file

- **GIVEN** a repository with a live session whose worktrees are nested under `.git-paw/worktrees/`
- **WHEN** `git paw setup-cli claude --allowlist --dry-run` is run at the repository root
- **THEN** the plan SHALL name `<repo>/.claude/settings.local.json` as its only settings target
- **AND** it SHALL name no path inside `.git-paw/worktrees/`

Test: `tests/cli_setup_e2e.rs::the_plan_names_no_worktree_path, cli_setup::plan::tests::the_only_settings_target_is_the_main_checkouts_file`

#### Scenario: The settings_path and user-level files are never targets

- **GIVEN** `[clis.claude] settings_path = "~/.claude/settings.json"` in the configuration
- **WHEN** `git paw setup-cli claude --allowlist --dry-run` is run
- **THEN** the plan SHALL NOT name `~/.claude/settings.json` or any path under the home directory outside the repository

Test: `tests/cli_setup_e2e.rs::the_configured_settings_path_is_never_a_target`

#### Scenario: A new file is kept out of version control

- **GIVEN** a repository whose ignore rules do not cover `.claude/settings.local.json`
- **WHEN** the allowlist item creates that file
- **THEN** the plan SHALL include the line `/.claude/settings.local.json` for the shared `info/exclude` file
- **AND** after the write, `git status` at the repository root SHALL NOT list the created file

Test: `tests/cli_setup_e2e.rs::yes_applies_the_plan_records_it_and_leaves_git_status_clean, cli_setup::shell::tests::the_exclusion_line_is_appended_and_hides_the_created_file`

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
keystroke-sending `approve` subcommand always prompts even when another rule would allow it. The item
SHALL NOT add an allow entry for `sweep.sh approve`, `sweep.sh verified`, `sweep.sh feedback-gate`,
`sweep.sh conflict-ack`, `sweep.sh answer` or `sweep.sh advanced-main`, for `docs-fetch.sh get`, an
interpreter-prefixed form, an absolute-path form, a wildcard over a directory, or any entry naming a
command other than a git-paw helper script. Every bundled helper script SHALL appear in the grant
table, either with its granted subcommands or as granted nothing, and every `sweep.sh` subcommand
SHALL be either granted or listed as never granted.

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

### Requirement: The allowlist item guards the granted helpers against silent modification

Because an allowed helper runs without a prompt, the allowlist item SHALL, in the same file and the
same write, add the protection rules the fact row declares, so that an agent session anywhere in the
repository cannot change what a granted entry runs without a prompt:

- `Edit(**/.git-paw/scripts/**)` in `permissions.deny`, refusing the CLI's file-editing tools on any
  helper-script directory at any depth under the session's working directory;
- `Edit(**/.git-paw/config.toml)` in `permissions.ask`, so a change to the configuration the helpers
  read (for example the broker address) always prompts.

The allowlist entries SHALL NOT be planned without these protection rules, and the fact row SHALL be
marked verified only after the protection has been confirmed against the CLI for the file-editing
tools and for the filesystem commands the CLI auto-approves in its edit-accepting mode.

#### Scenario: Protection is planned with the grants

- **WHEN** the allowlist item is planned for `claude`
- **THEN** the plan SHALL list `Edit(**/.git-paw/scripts/**)` under `permissions.deny`
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

- **GIVEN** `permissions.allow` already contains `Bash(.git-paw/scripts/broker.sh *)`
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

### Requirement: Unsafe targets are refused

The command SHALL refuse to plan, and SHALL write nothing for any item, when a file the plan would
write is tracked by git; when that file, or any path component between the repository root (or, for
the exclusion file, the common git directory) and the file, is a symbolic link; when the resolved
file lies outside the repository root or the common git directory; or when an existing JSON target is
not valid JSON, has a root that is not an object, or holds a value of the wrong type on the item's
key path. The error SHALL name the file and, for a type mismatch, the key.

#### Scenario: A tracked file is refused

- **GIVEN** `.claude/settings.local.json` is tracked in the repository
- **WHEN** `git paw setup-cli claude --allowlist --yes` is run
- **THEN** it SHALL fail naming that file
- **AND** no file SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::a_tracked_target_is_refused, cli_setup::plan::tests::a_tracked_target_is_refused_naming_the_file`

#### Scenario: A symlinked settings directory is refused

- **GIVEN** a repository whose `.claude` is a symbolic link to another directory
- **WHEN** `git paw setup-cli claude --allowlist --yes` is run
- **THEN** it SHALL fail naming the link
- **AND** nothing SHALL be written through the link

Test: `tests/cli_setup_e2e.rs::a_symlinked_settings_directory_is_refused, cli_setup::shell::tests::a_symlinked_settings_directory_is_reported_as_a_symlink`

#### Scenario: Malformed JSON is refused

- **GIVEN** a target file that is not valid JSON
- **WHEN** `git paw setup-cli claude --allowlist --yes` is run
- **THEN** it SHALL fail naming the file
- **AND** no file SHALL be created or modified

Test: `tests/cli_setup_e2e.rs::malformed_json_is_refused_naming_the_file, cli_setup::plan::tests::malformed_json_is_refused_naming_the_file`

#### Scenario: A wrong-typed key is refused

- **GIVEN** a target file whose `permissions.allow` is a string
- **WHEN** `git paw setup-cli claude --allowlist --yes` is run
- **THEN** it SHALL fail naming the file and `permissions.allow`

Test: `tests/cli_setup_e2e.rs::a_wrong_typed_key_is_refused_naming_file_and_key, cli_setup::plan::tests::a_wrong_typed_key_is_refused_naming_file_and_key`

### Requirement: Every write is recorded in the setup record

The command SHALL record every entry it writes in a per-repository setup record at
`<git-common-dir>/git-paw/cli-setup.json`, and SHALL record each entry before writing it. Each record
entry SHALL hold the CLI identity, the item, the absolute file path, the entry kind, the key path or
line, the value, whether git-paw created the file, which directories it created, the time and the
git-paw version. The record SHALL carry a schema version, SHALL be written atomically and SHALL be
readable and writable by its owner only. No command other than `git paw setup-cli` SHALL write the
record.

#### Scenario: Applied entries are recorded

- **WHEN** `git paw setup-cli claude --allowlist --yes` writes entries
- **THEN** the setup record SHALL list each written entry with CLI `claude`, item `allowlist`, its file, its key path and its value

Test: `cli_setup::shell::tests::applying_writes_the_planned_entries_and_records_them, cli_setup::shell::tests::every_entry_is_recorded_before_the_file_is_written`

#### Scenario: A created file is marked

- **GIVEN** a target file that did not exist before the run
- **WHEN** the allowlist item creates it
- **THEN** its record entries SHALL mark the file as created by git-paw

Test: `cli_setup::shell::tests::a_created_file_is_marked_as_created_in_the_record`

#### Scenario: The record is invisible to the working trees

- **WHEN** the setup record has been written
- **THEN** `git status` at the repository root and in every linked worktree SHALL NOT list it

Test: `cli_setup::shell::tests::writing_the_record_leaves_git_status_clean_at_the_root_and_in_a_worktree`

### Requirement: Undo removes exactly what git-paw added

`git paw setup-cli <CLI> --undo` SHALL plan the removal of the recorded entries for `<CLI>`, limited to
the named items when item flags are given and covering every recorded item otherwise, and SHALL apply
it through the same plan output and confirmation gate as setup. An entry SHALL be removed only when it
is still present, equal to its recorded value, and of a shape its item produces: in the item's declared
target file, an allow or ask value `Bash(.git-paw/scripts/<name>…)` naming a helper-script file name, or
a file-tool rule whose path lies under `.git-paw/`; in the shared `info/exclude` file, the anchored
exclusion line for the item's target file. A recorded entry that is absent, changed or not of a valid
shape SHALL NOT be removed from disk; it SHALL be reported and dropped from the record. Protection entries SHALL
be removed only when, after the removal, the file holds no allow entry naming a path under
`.git-paw/scripts/`. A file git-paw created SHALL be deleted only when what remains equals the empty
structure git-paw created, and a directory git-paw created SHALL be removed only when it is empty. Undo
SHALL NOT modify any other content and SHALL NOT touch entries recorded for another CLI.

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

