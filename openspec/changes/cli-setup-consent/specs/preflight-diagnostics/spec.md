## ADDED Requirements

### Requirement: Doctor reports CLI setup state

`git paw doctor` SHALL report, in the CLIs group, one setup check for each CLI identity that has a row in the setup fact table or entries in the setup record. The check SHALL be read-only: it reads the setup record, the files the record names, the helper scripts in the main checkout and in the current session's worktrees, and SHALL NOT create, modify or delete any of them. It SHALL report:

- ✓ when no item is applied, with a detail listing the available items and whether each is verified or plan-only;
- ✓ when every applied item's recorded entries are present and unchanged and match the entries the running binary would plan;
- ⚠ when a recorded entry is missing or changed, with a remedy naming `git paw setup-cli <CLI> --undo` or re-applying the item;
- ⚠ when the applied entries differ from the entries the running binary would plan, with a remedy naming `git paw setup-cli <CLI> --undo` followed by re-applying the item;
- ⚠ when an allowlist item is applied and a helper script in the main checkout or in a worktree of the current session differs from the copy the binary bundles, naming that helper's path, because the CLI would run the modified helper without a prompt, with a remedy to restore the helper (`git paw init` for the main checkout, restarting the session for a worktree) or undo the item.

The check SHALL NOT report ✗.

#### Scenario: Available but not applied

- **GIVEN** no setup record
- **WHEN** `git paw doctor` runs
- **THEN** the CLIs group SHALL contain a ✓ setup check for `claude` whose detail names the allowlist item and its verification status

#### Scenario: A recorded entry was removed by hand

- **GIVEN** a recorded allowlist entry that is no longer in its file
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ with a remedy naming `--undo`

#### Scenario: Applied entries are out of date

- **GIVEN** a setup record whose recorded allowlist entries lack one entry the running binary would plan
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ with a remedy naming `git paw setup-cli claude --undo` and re-applying `--allowlist`

#### Scenario: A granted helper was modified

- **GIVEN** the allowlist item applied and a session worktree whose `.git-paw/scripts/broker.sh` differs from the bundled copy
- **WHEN** `git paw doctor` runs
- **THEN** the setup check SHALL be ⚠ naming that helper's path

#### Scenario: The setup check is read-only and never fails doctor

- **GIVEN** a repository with a setup record
- **WHEN** `git paw doctor` runs
- **THEN** the setup record and every file it names SHALL be byte-identical afterwards
- **AND** no setup check SHALL be reported ✗

#### Scenario: A CLI with nothing to report gets no setup check

- **GIVEN** a `[clis.my-agent]` entry with no fact row and no record entries
- **WHEN** `git paw doctor` runs
- **THEN** no setup check SHALL be reported for `my-agent`

### Requirement: Doctor lists broad durable grants in the project's CLI settings

For each CLI identity whose fact row declares an allowlist item, when that item's target settings file exists, `git paw doctor` SHALL report in the CLIs group one advisory check listing every `permissions.allow` entry in that file whose shape grants more than one specific command. The shapes SHALL be a closed list:

1. a tool rule with no specifier or a bare wildcard specifier (for example `Bash`, `Bash(*)`, `Bash(:*)`);
2. a single-word prefix grant (for example `Bash(cargo:*)`, `Bash(git *)`, `Bash(rm*)`);
3. a prefix grant whose first word is a shell, an interpreter or a command wrapper (`sh`, `bash`, `zsh`, `fish`, `dash`, `ksh`, `python`, `python3`, `node`, `deno`, `bun`, `ruby`, `perl`, `php`, `lua`, `env`, `xargs`, `timeout`, `nohup`, `nice`, `sudo`, `su`, `doas`, `eval`, `exec`, `ssh`);
4. a prefix grant for a git subcommand that publishes, rewrites or discards history or working-tree state (`push`, `reset`, `clean`, `rebase`, `branch`, `checkout`, `restore`, `stash`, `worktree`, `update-ref`, `reflog`, `gc`, `filter-branch`, `config`);
5. a prefix grant for git-paw itself (`git paw` or `git-paw`) with no subcommand or with a subcommand that changes state (`start`, `add`, `remove`, `stop`, `pause`, `purge`, `init`, `add-cli`, `remove-cli`, `setup-cli`);
6. a prefix grant whose first word is `rm`, `chmod`, `chown`, `dd`, `curl` or `wget`, except a `curl` grant to a loopback address;
7. a file-tool rule (`Read`, `Edit`, `Write`) anchored at the filesystem root or the home directory and ending in a wildcard;
8. a grant naming a git-paw helper script that is broader than the setup item's own entry for that helper (for example `Bash(.git-paw/scripts/sweep.sh *)`).

The check SHALL be ⚠ when at least one entry matches, listing the file and each matching entry — abbreviated to its tool name and the first two words of its specifier, followed by `…` when anything was cut, so a credential embedded later in a rule is not echoed — with a remedy stating that git-paw does not edit these entries and that the user should review and remove the ones they did not intend; it SHALL be ✓ when none matches, and ⚠ stating the file was not inspected when the file is not valid JSON. The check SHALL NOT report ✗, SHALL NOT print any other content of the file, SHALL NOT create, modify or delete the file, and SHALL NOT influence how any permission prompt is handled. An entry equal to one of the setup item's own planned entries SHALL NOT be listed, whoever added it.

#### Scenario: Broad grants left by earlier approvals are listed

- **GIVEN** a main-checkout `.claude/settings.local.json` whose `permissions.allow` holds `Bash(git-paw purge:*)`, `Bash(git push:*)`, `Bash(cargo:*)` and `Bash(npm run test:*)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ listing `Bash(git-paw purge:*)`, `Bash(git push:*)` and `Bash(cargo:*)`
- **AND** it SHALL NOT list `Bash(npm run test:*)`

#### Scenario: git-paw's own setup entries are not listed

- **GIVEN** the allowlist item applied and no other entry in `permissions.allow`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ✓

#### Scenario: A long rule is abbreviated

- **GIVEN** `permissions.allow` holds `Bash(curl -H "Authorization: Bearer abc123" https://api.example.com/*)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL list the entry abbreviated as `Bash(curl -H …)`
- **AND** the output SHALL NOT contain `abc123`

#### Scenario: A wide helper grant is listed

- **GIVEN** `permissions.allow` holds `Bash(.git-paw/scripts/sweep.sh *)`
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ listing `Bash(.git-paw/scripts/sweep.sh *)`

#### Scenario: The advisory never edits the file

- **GIVEN** a settings file with broad grants
- **WHEN** `git paw doctor` runs
- **THEN** the file SHALL be byte-identical afterwards
- **AND** no check SHALL be reported ✗ because of it

#### Scenario: An unparseable file is reported, not inspected

- **GIVEN** a main-checkout `.claude/settings.local.json` that is not valid JSON
- **WHEN** `git paw doctor` runs
- **THEN** the advisory check SHALL be ⚠ stating the file was not inspected
- **AND** it SHALL NOT print the file's content
