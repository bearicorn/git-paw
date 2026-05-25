## MODIFIED Requirements

### Requirement: Start subcommand with optional flags

The `start` subcommand SHALL be extended to accept a `--supervisor` flag (boolean, defaults to `false`). The flag MAY be combined with any other `start` flags.

When `--supervisor` is passed, the parsed `StartArgs` struct SHALL have `supervisor: bool` set to `true`.

The `start` subcommand SHALL also accept a `--no-rebase` flag (boolean, defaults to `false`). When `--no-rebase` is passed, the parsed `StartArgs` struct SHALL have `no_rebase: bool` set to `true`. The dispatch SHALL invoke `create_worktree` with `rebase_onto_main = !args.no_rebase` for every worktree creation in the launch. When `--no-rebase` is omitted (i.e. `no_rebase == false`), agent branches SHALL be rebased onto the repository's default branch before their worktrees are opened. When `--no-rebase` is present, agent branches SHALL NOT be rebased, matching the post-`worktree-resume-fix` v0.5.0 behaviour.

The `--no-rebase` flag MAY be combined with any other `start` flags including `--supervisor`, `--from-specs`, `--cli`, and `--branches`.

#### Scenario: Start with --supervisor flag

- **GIVEN** `start --supervisor`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `true`

#### Scenario: Start with --supervisor combined with other flags

- **GIVEN** `start --supervisor --cli claude --branches feat/a,feat/b`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `true`
- **AND** `cli` SHALL be `Some("claude")`
- **AND** `branches` SHALL be `["feat/a", "feat/b"]`

#### Scenario: Start without --supervisor defaults to false

- **GIVEN** `start --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `supervisor` SHALL be `false`

#### Scenario: Start with --no-rebase flag

- **GIVEN** `start --no-rebase`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `true`

#### Scenario: Start without --no-rebase defaults to false

- **GIVEN** `start --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `false`

#### Scenario: Start with --no-rebase combined with other flags

- **GIVEN** `start --no-rebase --supervisor --from-specs`
- **WHEN** the CLI is parsed
- **THEN** `no_rebase` SHALL be `true`
- **AND** `supervisor` SHALL be `true`
- **AND** `from_specs` SHALL be `true`

#### Scenario: --no-rebase propagates to create_worktree as rebase_onto_main = false

- **GIVEN** `start --branches feat/a --no-rebase` is invoked
- **WHEN** the dispatch reaches the worktree-creation loop
- **THEN** `create_worktree(repo_root, "feat/a", rebase_onto_main)` SHALL be called with `rebase_onto_main = false`

#### Scenario: Default start propagates rebase_onto_main = true

- **GIVEN** `start --branches feat/a` is invoked without `--no-rebase`
- **WHEN** the dispatch reaches the worktree-creation loop
- **THEN** `create_worktree(repo_root, "feat/a", rebase_onto_main)` SHALL be called with `rebase_onto_main = true`
