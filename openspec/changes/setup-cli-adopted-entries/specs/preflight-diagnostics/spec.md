## MODIFIED Requirements

### Requirement: Doctor reports CLI setup state

`git paw doctor` SHALL report, in the CLIs group, one setup check for each CLI identity that has a row in the setup fact table or entries in the setup record. The check SHALL be read-only: it reads the setup record, the files the record names, the item's target settings file, the helper scripts in the main checkout and in the current session's worktrees, and SHALL NOT create, modify or delete any of them. An entry the running binary would plan SHALL count as applied when the item's target settings file holds that value at the entry's key path, whether git-paw wrote it or it was there before the item was applied. It SHALL report:

- ✓ when no item is applied, with a detail listing the available items and whether each is verified or plan-only;
- ✓ when every applied item's recorded entries are present and unchanged and every entry the running binary would plan is present in the target file;
- ⚠ when a recorded entry is missing or changed, with a remedy naming `git paw setup-cli <CLI> --undo` or re-applying the item;
- ⚠ when the applied entries differ from the entries the running binary would plan — an entry it would plan is absent from the target file, or the record holds an entry it no longer plans — with a remedy naming `git paw setup-cli <CLI> --undo` followed by re-applying the item;
- ⚠ when an allowlist item is applied and a helper script in the main checkout or in a worktree of the current session differs from the copy the binary bundles, naming that helper's path, because the CLI would run the modified helper without a prompt, with a remedy to restore the helper (`git paw init` for the main checkout, restarting the session for a worktree) or undo the item.

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

- **GIVEN** a `.claude/settings.local.json` that already held `Bash(.git-paw/scripts/broker.sh *)` before `git paw setup-cli claude --allowlist --yes` was run
- **AND** the setup record therefore does not list that entry
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ✓

#### Scenario: Undo and re-apply converge when entries pre-existed

- **GIVEN** the repository from "An entry the user already had counts as applied"
- **WHEN** `git paw setup-cli claude --undo --yes` and then `git paw setup-cli claude --allowlist --yes` are run
- **THEN** `Bash(.git-paw/scripts/broker.sh *)` SHALL still be present in `permissions.allow`
- **AND** `git paw doctor` SHALL report the setup check ✓

#### Scenario: A recorded entry the binary no longer plans is reported

- **GIVEN** a setup record listing an allowlist entry, present in the target file, that the running binary would not plan
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ with a remedy naming `git paw setup-cli claude --undo` and re-applying `--allowlist`

#### Scenario: A granted helper was modified

- **GIVEN** the allowlist item applied and a session worktree whose `.git-paw/scripts/broker.sh` differs from the bundled copy
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ naming that helper's path

Test: `cli_setup::doctor::tests::a_modified_granted_helper_warns_naming_its_path`

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

## ADDED Requirements

### Requirement: Doctor warns about dangerous grant residue in the project's CLI settings

For each CLI identity whose fact row declares an allowlist item, `git paw doctor` SHALL report in the CLIs group one grant-residue check over the main checkout's project-shared and project-local settings files of that CLI as the fact row records them (for `claude`: `.claude/settings.json` and `.claude/settings.local.json`), when at least one of them exists. The check SHALL be ⚠ when a file holds any of:

1. a `permissions.allow` entry that grants `curl` with any arguments — a `Bash` rule whose specifier is `curl` followed only by a wildcard (for example `Bash(curl *)`, `Bash(curl:*)`, `Bash(curl*)`);
2. a `permissions.allow` entry that names a git-paw helper script followed by a subcommand the grant table lists as never granted, or by a wildcard in the subcommand position — whatever path form names the script (relative, absolute or interpreter-prefixed), for example `Bash(.git-paw/scripts/sweep.sh approve *)`, `Bash(.git-paw/scripts/sweep.sh verified *)` or `Bash(.git-paw/scripts/sweep.sh *)`;
3. a top-level `allowed_bash_prefixes` key, the inert seed earlier git-paw versions wrote.

The ⚠ detail SHALL name each file and each matching entry, abbreviated as the broad-grant advisory abbreviates entries, and SHALL say why each kind is dangerous: an unrestricted network client; a grant that lets an agent session approve its own prompts or publish a verdict without a prompt; an inert key that grants nothing in the current CLI but is left over from an earlier version. The remedy SHALL state that git-paw never rewrites grants it did not write and that the user should remove these entries by hand. The check SHALL be ✓ when no file holds a match, and ⚠ stating the file was not inspected when an existing file is not valid JSON. The check SHALL be read-only, SHALL NOT report ✗, SHALL NOT print any other content of the files, and SHALL NOT influence how any permission prompt is handled.

#### Scenario: A durable self-approval grant is flagged

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(.git-paw/scripts/sweep.sh approve *)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL be ⚠ naming that file and that entry
- **AND** its remedy SHALL say to remove the entry by hand and that git-paw does not rewrite it

#### Scenario: A blanket curl grant is flagged

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(curl *)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL be ⚠ naming `Bash(curl *)`

#### Scenario: The inert seed in the shared settings file is flagged

- **GIVEN** a main-checkout `.claude/settings.json` holding a top-level `allowed_bash_prefixes` array and no `.claude/settings.local.json`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL be ⚠ naming `.claude/settings.json` and `allowed_bash_prefixes`

#### Scenario: git-paw's own entries are not residue

- **GIVEN** the allowlist item applied and no other entry in either settings file
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL be ✓

#### Scenario: A loopback-scoped curl grant is not residue

- **GIVEN** `permissions.allow` holds `Bash(curl -s http://127.0.0.1:9119/status)`
- **WHEN** `git paw doctor` runs
- **THEN** the grant-residue check SHALL NOT list that entry

#### Scenario: The residue check never edits the files

- **GIVEN** both settings files holding residue
- **WHEN** `git paw doctor` runs
- **THEN** both files SHALL be byte-identical afterwards
- **AND** no check SHALL be reported ✗ because of them
