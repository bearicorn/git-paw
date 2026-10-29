## MODIFIED Requirements

### Requirement: Bundled-scripts check

The command SHALL verify, for every package in the bundled package registry, that the package is installed at the repository root under `.git-paw/skills/<package>/` with its `SKILL.md` present, and that each of the package's helper scripts exists under the package's `scripts/`, is executable, and matches the running binary's embedded version. A missing package, a missing or non-executable helper, or a stale helper SHALL be reported with a remedy to run `git paw init`. The command SHALL also check each helper's compatibility copy under `.git-paw/scripts/`; because nothing bundled invokes that path, a missing or stale compatibility copy SHALL be reported as ⚠ and SHALL NOT be ✗. The command SHALL also verify that a Python 3 interpreter is available on PATH (`python3`, or `python` reporting major version 3), since every bundled helper script requires one to run; when none is found it SHALL report ⚠ with a remedy to install Python 3. Because core git-paw orchestration (start/add/remove) does not require Python, a missing interpreter SHALL be ⚠ and SHALL NOT be ✗.

#### Scenario: A bundled script is missing

- **GIVEN** `.git-paw/skills/orchestrator/scripts/sweep.sh` does not exist
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ✗ for `sweep.sh` with a remedy to run `git paw init`

#### Scenario: A bundled script is stale

- **GIVEN** a package helper exists but its content differs from the running binary's embedded version
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report a non-✓ status with a remedy to run `git paw init`

#### Scenario: A package's SKILL.md is missing

- **GIVEN** `.git-paw/skills/coordination/SKILL.md` does not exist
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ✗ for the `coordination` package with a remedy to run `git paw init`

#### Scenario: A missing compatibility copy is a warning

- **GIVEN** every package is installed and intact but `.git-paw/scripts/broker.sh` is missing
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ⚠ for the compatibility copy
- **AND** SHALL NOT report ✗

#### Scenario: No Python 3 interpreter on PATH

- **GIVEN** neither `python3` nor a version-3 `python` is on PATH
- **WHEN** `git paw doctor` is run
- **THEN** the Bundled-scripts group SHALL report ⚠ for the Python 3 interpreter with a remedy to install Python 3 (the bundled helper scripts require it)

### Requirement: Doctor reports CLI setup state

`git paw doctor` SHALL report, in the CLIs group, one setup check for each CLI identity that has a row in the setup fact table or entries in the setup record. The check SHALL be read-only: it reads the setup record, the files the record names, the item's target settings file, the helper scripts in the main checkout and in the current session's worktrees, and SHALL NOT create, modify or delete any of them. An entry the running binary would plan SHALL count as applied when the item's target settings file holds that value at the entry's key path, whether git-paw wrote it or it was there before the item was applied. The comparison against the running binary's plan SHALL be made whenever the item is applied — that is, whenever the record holds any entry for it — and SHALL NOT depend on the record holding a permission entry, because an item applied to a file that already held every planned entry records none. It SHALL report:

- ✓ when no item is applied, with a detail listing the available items and whether each is verified or plan-only;
- ✓ when every applied item's recorded entries are present and unchanged and every entry the running binary would plan is present in the target file;
- ⚠ when a recorded entry is missing or changed, with a remedy naming `git paw setup-cli <CLI> --undo` or re-applying the item;
- ⚠ when the applied entries differ from the entries the running binary would plan — an entry it would plan is absent from the target file, or the record holds an entry it no longer plans — with a remedy naming `git paw setup-cli <CLI> --undo` followed by re-applying the item;
- ⚠ when an allowlist item is applied and a helper script at a path an applied allow entry names — its package path, or the compatibility path `.git-paw/scripts/` for entries applied before the package move — in the main checkout or in a worktree of the current session differs from the copy the binary bundles, naming that helper's path, because the CLI would run the modified helper without a prompt, with a remedy to restore the helper (`git paw init` for the main checkout, restarting the session for a worktree) or undo the item.

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

#### Scenario: Entries applied before the package move are out of date

- **GIVEN** a setup record, written by an earlier version, whose applied allowlist entries name `.git-paw/scripts/` paths, all present and unchanged
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ with a remedy naming `git paw setup-cli claude --undo` followed by re-applying `--allowlist`
- **AND** the settings file and the setup record SHALL be byte-identical afterwards

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
