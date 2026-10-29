## MODIFIED Requirements

### Requirement: Doctor lists broad durable grants in the project's CLI settings

For each CLI identity whose fact row declares an allowlist item, when that item's target settings file exists, `git paw doctor` SHALL report in the CLIs group one advisory check listing every `permissions.allow` entry in that file whose shape grants more than one specific command. The shapes SHALL be a closed list:

1. a tool rule with no specifier or a bare wildcard specifier (for example `Bash`, `Bash(*)`, `Bash(:*)`);
2. a single-word prefix grant (for example `Bash(cargo:*)`, `Bash(git *)`, `Bash(rm*)`);
3. a prefix grant whose first word is a shell, an interpreter or a command wrapper (`sh`, `bash`, `zsh`, `fish`, `dash`, `ksh`, `python`, `python3`, `node`, `deno`, `bun`, `ruby`, `perl`, `php`, `lua`, `env`, `xargs`, `timeout`, `nohup`, `nice`, `sudo`, `su`, `doas`, `eval`, `exec`, `ssh`);
4. a prefix grant for a git subcommand that publishes, rewrites or discards history or working-tree state (`push`, `reset`, `clean`, `rebase`, `branch`, `checkout`, `restore`, `stash`, `worktree`, `update-ref`, `reflog`, `gc`, `filter-branch`, `config`);
5. a prefix grant for git-paw itself (`git paw` or `git-paw`) with no subcommand or with a subcommand that changes state (`start`, `add`, `remove`, `stop`, `pause`, `purge`, `init`, `add-cli`, `remove-cli`, `setup-cli`, `finish`);
6. a prefix grant whose first word is `rm`, `chmod`, `chown`, `dd`, `curl` or `wget`, except a `curl` grant to a loopback address;
7. a file-tool rule (`Read`, `Edit`, `Write`) anchored at the filesystem root or the home directory and ending in a wildcard;
8. a grant naming a git-paw helper script that is broader than the setup item's own entry for that helper (for example `Bash(.git-paw/scripts/sweep.sh *)`).

The check SHALL be ⚠ when at least one entry matches, listing the file and each matching entry — abbreviated to its tool name and the first two words of its specifier, followed by `…` when anything was cut, so a credential embedded later in a rule is not echoed — with a remedy stating that git-paw does not edit these entries and that the user should review and remove the ones they did not intend; it SHALL be ✓ when none matches, and ⚠ stating the file was not inspected when the file is not valid JSON. The check SHALL NOT report ✗, SHALL NOT print any other content of the file, SHALL NOT create, modify or delete the file, and SHALL NOT influence how any permission prompt is handled. An entry equal to one of the setup item's own planned entries SHALL NOT be listed, whoever added it.

#### Scenario: Broad grants left by earlier approvals are listed

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(git-paw purge:*)`, `Bash(git push:*)`, `Bash(cargo:*)` and `Bash(npm run test:*)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ listing `Bash(git-paw purge:*)`, `Bash(git push:*)` and `Bash(cargo:*)`
- **AND** it SHALL NOT list `Bash(npm run test:*)`

Test: `cli_setup::doctor::tests::broad_grants_left_by_earlier_approvals_are_listed, tests/doctor_e2e.rs::doctor_lists_a_broad_grant_the_user_already_had`

#### Scenario: A durable grant to finish a branch is listed

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(git paw finish:*)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ listing `Bash(git paw finish:*)`

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
