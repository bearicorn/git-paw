## ADDED Requirements

### Requirement: The dry-run plan names each worker's approval level and flags

For every worker in a `git paw start --dry-run` plan, whether it is a plain, coordinated or spec-driven launch, the plan SHALL print a line directly below that worker's `runs:` line, of the form `approval: <level> → <flags>`. In that line:

- `<level>` SHALL be the worker's own effective level: its profile's `approval` when set, else the configured worker default.
- `<flags>` SHALL be exactly the approval flags its pane command carries for that level.
- When the flags are empty, the line SHALL read `no flags (the CLI's default permission mode)` instead of the flags.
- When the flags come from a `[clis.<id>].approval_args` override, the line SHALL end with `([clis.<id>].approval_args)`.

The line SHALL be built from the same resolution the pane command uses, so the two never disagree. The line SHALL go to stdout as part of the plan. The dry run SHALL stay read-only.

#### Scenario: A claude worker at the default level

- **GIVEN** a config with no worker approval and `default_cli = "claude"`
- **WHEN** `git paw start --dry-run --branches feat/a` runs
- **THEN** the plan SHALL show `approval: accept-edits → --permission-mode acceptEdits` below `feat/a`'s `runs:` line

#### Scenario: A claude worker at auto

- **GIVEN** a version-2 config with `[workers] approval = "auto"` and `default_cli = "claude"`
- **WHEN** `git paw start --dry-run --branches feat/a` runs
- **THEN** the plan SHALL show `approval: auto → --permission-mode auto`

#### Scenario: An override is attributed

- **GIVEN** `[clis.claude] approval_args = { "accept-edits" = "--permission-mode auto" }` and a worker at `accept-edits`
- **WHEN** the dry-run plan is printed
- **THEN** the line SHALL read `approval: accept-edits → --permission-mode auto ([clis.claude].approval_args)`

#### Scenario: A level with no flags says so

- **GIVEN** a `qwen` worker at `auto`
- **WHEN** the dry-run plan is printed
- **THEN** the line SHALL read `approval: auto → no flags (the CLI's default permission mode)`

#### Scenario: Profiles show their own level

- **GIVEN** a worker default of `accept-edits` and `[profiles.deep] kind = "worker"`, `cli = "claude"`, `approval = "auto"`
- **WHEN** `git paw start --dry-run --profile deep --branches feat/a` runs alongside a plain worker on `feat/b`
- **THEN** `feat/a`'s line SHALL show `auto` and `feat/b`'s line SHALL show `accept-edits`
