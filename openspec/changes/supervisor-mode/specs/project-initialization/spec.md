## MODIFIED Requirements

### Requirement: Init generates default config.toml

`git paw init` SHALL be extended to prompt the user about supervisor mode configuration during initialization. After the existing init steps, the system SHALL:

1. Prompt: "Enable supervisor mode by default? (y/n)"
2. If yes, prompt: "Test command to run after agents complete (e.g. 'just check', leave empty to skip):"
3. If the user answers yes: write a `[supervisor]` section with `enabled = true` and the test command (if provided) to the generated config
4. If the user answers no: write `[supervisor]\nenabled = false` to explicitly disable supervisor mode

These prompts SHALL only appear when the `[supervisor]` section is absent from the existing config. If the section already exists (even with non-default values), it SHALL NOT be modified or re-prompted.

#### Scenario: Init yes to supervisor writes enabled section

- **GIVEN** `git paw init` is run in a fresh repo and the user answers "y" to supervisor mode
- **AND** the user enters "just check" as the test command
- **WHEN** init completes
- **THEN** `.git-paw/config.toml` SHALL contain `[supervisor]`, `enabled = true`, and `test_command = "just check"`

#### Scenario: Init no to supervisor writes disabled section

- **GIVEN** `git paw init` is run in a fresh repo and the user answers "n" to supervisor mode
- **WHEN** init completes
- **THEN** `.git-paw/config.toml` SHALL contain `[supervisor]` with `enabled = false`
- **AND** no `test_command` field SHALL be present

#### Scenario: Init yes with empty test command writes no test_command

- **GIVEN** the user answers "y" to supervisor mode and leaves the test command blank
- **WHEN** init completes
- **THEN** `.git-paw/config.toml` SHALL contain `enabled = true` under `[supervisor]`
- **AND** `test_command` SHALL be absent from the config

#### Scenario: Existing config with supervisor section suppresses prompts

- **GIVEN** `git paw init` is run in a repo with `.git-paw/config.toml` that already has a `[supervisor]` section
- **WHEN** init runs
- **THEN** no supervisor prompts SHALL be shown
- **AND** the existing `[supervisor]` section SHALL NOT be modified

#### Scenario: Existing config without supervisor section triggers prompts

- **GIVEN** `git paw init` is run in a repo with `.git-paw/config.toml` that has NO `[supervisor]` section (e.g. a v0.3.0 config)
- **WHEN** init runs
- **THEN** the supervisor prompts SHALL be shown
- **AND** the `[supervisor]` section SHALL be appended to the existing config
- **AND** all other existing config content SHALL be preserved unchanged

### Requirement: Init merges new config fields without mutating existing ones

When `git paw init` runs on a repo with an existing `.git-paw/config.toml`, the system SHALL compare the generated default config against the existing file and append only sections that are absent. The system SHALL NOT modify or remove any existing content.

This makes `init` a safe upgrade path for every version bump — users run `git paw init` after upgrading and new config sections are added without touching their customized settings.

#### Scenario: Init preserves existing broker config while adding supervisor

- **GIVEN** a `.git-paw/config.toml` with `[broker]` section containing `port = 9200` (non-default) but no `[supervisor]` section
- **WHEN** `git paw init` is run
- **THEN** the `[broker]` section SHALL still contain `port = 9200`
- **AND** a `[supervisor]` section SHALL be appended

#### Scenario: Init does not duplicate existing sections

- **GIVEN** a `.git-paw/config.toml` that already has both `[broker]` and `[supervisor]` sections
- **WHEN** `git paw init` is run
- **THEN** no sections SHALL be added or modified
- **AND** the file content SHALL be unchanged

#### Scenario: Init on a completely empty config file adds all sections

- **GIVEN** a `.git-paw/config.toml` that exists but is empty
- **WHEN** `git paw init` is run
- **THEN** all default sections SHALL be appended (commented out)

### Requirement: Init appends logs directory to .gitignore

The system SHALL also ensure `.git-paw/session-summary.md` is listed in the repo's `.gitignore`, in addition to `.git-paw/logs/`.

#### Scenario: Gitignore includes session-summary.md after init

- **GIVEN** `git paw init` is run in a repo without `.git-paw/session-summary.md` in `.gitignore`
- **WHEN** init completes
- **THEN** `.gitignore` SHALL contain `.git-paw/session-summary.md`

#### Scenario: Gitignore not duplicated on repeated init

- **GIVEN** `.gitignore` already contains `.git-paw/session-summary.md`
- **WHEN** `git paw init` is run again
- **THEN** `.git-paw/session-summary.md` SHALL appear exactly once in `.gitignore`
