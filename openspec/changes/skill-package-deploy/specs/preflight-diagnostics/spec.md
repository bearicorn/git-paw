## MODIFIED Requirements

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

Test: `cli_setup::doctor::tests::a_tampered_preamble_beside_a_granted_helper_is_checked`, `cli_setup::doctor::tests::helper_paths_follow_the_applied_allow_entries`

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
