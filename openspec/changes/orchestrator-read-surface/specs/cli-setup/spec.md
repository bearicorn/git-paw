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
keystroke-sending `approve` subcommand always prompts even when another rule would allow it. The item
SHALL NOT add an allow entry for `sweep.sh approve`, `sweep.sh verified` or `sweep.sh feedback-gate`,
for `docs-fetch.sh get`, an interpreter-prefixed form, an absolute-path form, a wildcard over a
directory, or any entry naming a command other than a git-paw helper script. Every bundled helper
script SHALL appear in the grant table, either with its granted subcommands or as granted nothing, and
every `sweep.sh` subcommand SHALL be either granted or listed as never granted.

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
