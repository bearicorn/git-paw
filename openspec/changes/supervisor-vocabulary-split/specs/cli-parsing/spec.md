## RENAMED Requirements

- FROM: `### Requirement: --no-supervisor flag`
- TO: `### Requirement: --bare flag`
- FROM: `### Requirement: --supervisor and --no-supervisor are mutually exclusive`
- TO: `### Requirement: --coordinated and --bare are mutually exclusive`

## MODIFIED Requirements

### Requirement: Start subcommand with optional flags

The `start` subcommand SHALL accept a `--coordinated` flag (boolean, defaults to `false`). The flag MAY be combined with any other `start` flag except `--bare`.

When `--coordinated` is passed, the parsed `StartArgs` struct SHALL have `coordinated: bool` set to `true`.

The `start` subcommand SHALL also accept a `--no-rebase` flag (boolean, defaults to `false`). When `--no-rebase` is passed, the parsed `StartArgs` struct SHALL have `no_rebase: bool` set to `true`. The dispatch SHALL invoke `create_worktree` with `rebase_onto_main = !args.no_rebase` for every worktree creation in the launch. When `--no-rebase` is omitted (i.e. `no_rebase == false`), agent branches SHALL be rebased onto the repository's default branch before their worktrees are opened. When `--no-rebase` is present, agent branches SHALL NOT be rebased, matching the post-`worktree-resume-fix` v0.5.0 behaviour.

The `--no-rebase` flag MAY be combined with any other `start` flags including `--coordinated`, `--from-all-specs`, `--cli`, and `--branches`.

#### Scenario: Start with --coordinated flag

- **GIVEN** `start --coordinated`
- **WHEN** the CLI is parsed
- **THEN** `coordinated` SHALL be `true`

#### Scenario: Start with --coordinated combined with other flags

- **GIVEN** `start --coordinated --cli claude --branches feat/a,feat/b`
- **WHEN** the CLI is parsed
- **THEN** `coordinated` SHALL be `true`
- **AND** `cli` SHALL be `Some("claude")`
- **AND** `branches` SHALL be `["feat/a", "feat/b"]`

#### Scenario: Start without --coordinated defaults to false

- **GIVEN** `start --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `coordinated` SHALL be `false`

#### Scenario: Start with --no-rebase flag

- **GIVEN** `start --no-rebase`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `true`

#### Scenario: Start without --no-rebase defaults to false

- **GIVEN** `start --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `false`

#### Scenario: Start with --no-rebase combined with other flags

- **GIVEN** `start --no-rebase --coordinated --from-all-specs`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `true`
- **AND** `coordinated` SHALL be `true`
- **AND** `from_all_specs` SHALL be `true`

#### Scenario: --no-rebase propagates to create_worktree as rebase_onto_main = false

- **GIVEN** `start --branches feat/a --no-rebase` is invoked
- **WHEN** the dispatch reaches the worktree-creation loop
- **THEN** `create_worktree(repo_root, "feat/a", rebase_onto_main)` SHALL be called with `rebase_onto_main = false`

#### Scenario: Default start propagates rebase_onto_main = true

- **GIVEN** `start --branches feat/a` is invoked without `--no-rebase`
- **WHEN** the dispatch reaches the worktree-creation loop
- **THEN** `create_worktree(repo_root, "feat/a", rebase_onto_main)` SHALL be called with `rebase_onto_main = true`

### Requirement: --bare flag

The `start` subcommand SHALL accept a `--bare` flag (boolean, default `false`). When passed, the parsed `StartArgs` SHALL have `bare: bool` set to `true`, and the session SHALL run in bare mode whatever `default_mode` says. The flag SHALL appear in `git paw start --help` output with a description that names the use case (running one session without coordination, overriding `default_mode = "coordinated"`).

#### Scenario: --bare sets the flag

- **GIVEN** the user invokes `git paw start --bare`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs.bare` SHALL be `true`
- **AND** `StartArgs.coordinated` SHALL be `false`

#### Scenario: --bare absent leaves flag false

- **GIVEN** the user invokes `git paw start` with neither `--coordinated` nor `--bare`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs.bare` SHALL be `false`
- **AND** `StartArgs.coordinated` SHALL be `false`

#### Scenario: --bare appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output contains `--bare` and `--coordinated`
- **AND** the output describes `--bare` as running the session without coordination and overriding `default_mode = "coordinated"`
- **AND** the output SHALL NOT contain the substring `supervisor`

### Requirement: --coordinated and --bare are mutually exclusive

The system SHALL reject any invocation that combines `--coordinated` and `--bare` on the same `start` command, under any spelling of either flag. clap's parse step SHALL produce an error before the command runs. The error message SHALL clearly state that the two flags express opposing intents and SHALL list both.

#### Scenario: Both flags together are rejected

- **GIVEN** `start --coordinated --bare`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--coordinated` and `--bare`

#### Scenario: --bare combines with other flags

- **GIVEN** `start --bare --cli claude --branches feat/a,feat/b`
- **WHEN** the CLI is parsed
- **THEN** `bare` SHALL be `true`
- **AND** `cli` SHALL be `Some("claude")`
- **AND** `branches` SHALL contain `feat/a` and `feat/b`
- **AND** parsing SHALL succeed

### Requirement: --unattended flag

The `start` subcommand SHALL accept an `--unattended` flag (boolean, default `false`). When passed, the parsed `StartArgs` SHALL have `unattended: bool` set to `true`. When omitted, `unattended` SHALL be `false`.

The flag SHALL appear in `git paw start --help` output with a description naming the use case: drive a coordinated wave to completion in-process (escalate every permission prompt for a human decision without blocking the rest of the wave, detect completion, exit with a summary). The help text SHALL NOT claim that the flag approves prompts, and its long help SHALL state that prompt-free unattended runs need the agent CLI's own allow rules or the `full-auto` approval level inside a sandbox.

`--unattended` engages coordinated mode: when `--unattended` is passed, the start-mode resolution chain SHALL resolve coordinated mode (equivalent to `--coordinated`) so the dispatch routes to the coordinated launch path. `--unattended` MAY be combined with `--coordinated`, `--from-all-specs`, `--specs`, `--cli`, and `--branches`.

`--unattended` SHALL NOT be combined with `--bare`; the system SHALL reject any invocation that combines `--unattended` with `--bare`, under any spelling, with a parse error naming both flags, because they express opposing intents.

#### Scenario: --unattended sets the flag

- **GIVEN** the user invokes `git paw start --unattended`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs.unattended` SHALL be `true`

#### Scenario: --unattended absent leaves flag false

- **GIVEN** the user invokes `git paw start` without `--unattended`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs.unattended` SHALL be `false`

#### Scenario: --unattended combines with other flags

- **GIVEN** `start --unattended --from-all-specs --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `unattended` SHALL be `true`
- **AND** the spec-mode launch-all state SHALL be set
- **AND** `cli` SHALL be `Some("claude")`
- **AND** parsing SHALL succeed

#### Scenario: --unattended appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output SHALL contain `--unattended`
- **AND** the output SHALL describe the flag as driving the wave to completion and escalating permission prompts
- **AND** the output SHALL NOT describe the flag as auto-approving prompts

#### Scenario: --unattended engages coordinated mode

- **GIVEN** `git paw start --unattended --branches feat/a,feat/b` is invoked
- **WHEN** the dispatch resolves the start-mode resolution chain
- **THEN** coordinated mode SHALL resolve active
- **AND** the dispatch SHALL route to the coordinated launch path

#### Scenario: --unattended with --bare is rejected

- **GIVEN** `start --unattended --bare`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--unattended` and `--bare`

## ADDED Requirements

### Requirement: --supervisor and --no-supervisor are deprecated aliases

Until v1.0.0 the `start` subcommand SHALL accept `--supervisor` as a hidden alias of `--coordinated` and `--no-supervisor` as a hidden alias of `--bare`. Each alias SHALL parse to exactly the `StartArgs` its canonical flag produces and SHALL be subject to the same conflict rules. Each use SHALL print one deprecation warning through the shared deprecation helper naming the alias, its deprecating release, its removal in v1.0.0 and its canonical flag; the warning SHALL NOT change stdout or the exit code. Neither alias SHALL appear in `git paw start --help` or in any generated shell completion script. Requirements that name `--supervisor`, `--no-supervisor`, "supervisor mode" or "supervised session" SHALL be read as naming `--coordinated`, `--bare` and coordinated mode.

#### Scenario: Aliases parse like their canonical flags

- **GIVEN** the pairs `start --supervisor` / `start --coordinated` and `start --no-supervisor` / `start --bare`
- **WHEN** each is parsed
- **THEN** the two members of each pair SHALL produce equal `StartArgs`

#### Scenario: An alias warns and still works

- **GIVEN** a repository with two agent branches
- **WHEN** `git paw start --no-supervisor --dry-run --branches feat/a,feat/b` runs
- **THEN** stderr SHALL contain one warning naming `--no-supervisor`, v1.0.0 and `--bare`
- **AND** stdout and the exit status SHALL equal those of the same command with `--bare`

#### Scenario: Aliases keep the conflict rules

- **WHEN** `start --supervisor --no-supervisor` and `start --unattended --no-supervisor` are parsed
- **THEN** both SHALL fail to parse

#### Scenario: Aliases are not advertised

- **WHEN** `git paw start --help` is run and `git paw completions bash`, `zsh` and `fish` are generated
- **THEN** none of the outputs SHALL contain `--supervisor` or `--no-supervisor`
