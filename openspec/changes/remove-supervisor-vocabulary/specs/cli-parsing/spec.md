## ADDED Requirements

### Requirement: Removed --supervisor and --no-supervisor flags are rejected with a migration message

The `start` subcommand SHALL reject `--supervisor` and `--no-supervisor` as removed flags. When an invocation contains either — alone or combined with any other flag — the command SHALL fail before any side effect (no worktree, session file, tmux session or config write) with exit code `2`, the code for an argument rejected by parsing, and stderr SHALL name the removed flag, state that it was renamed, and name its replacement (`--coordinated` for `--supervisor`, `--bare` for `--no-supervisor`). Neither flag SHALL appear in `git paw start --help` output or in any generated shell completion script.

#### Scenario: --supervisor fails with a migration message

- **WHEN** `git paw start --supervisor --branches feat/a` runs in a git repository
- **THEN** the process SHALL exit `2`
- **AND** stderr SHALL contain `--supervisor`, the word `renamed`, and `--coordinated`
- **AND** no worktree, session file or tmux session SHALL have been created

#### Scenario: --no-supervisor fails with a migration message

- **WHEN** `git paw start --no-supervisor --from-all-specs` runs
- **THEN** the process SHALL exit `2`
- **AND** stderr SHALL contain `--no-supervisor`, `renamed`, and `--bare`

#### Scenario: The removed flags are not advertised

- **WHEN** `git paw start --help` is run, and `git paw completions bash`, `zsh` and `fish` are generated
- **THEN** none of the outputs SHALL contain `--supervisor` or `--no-supervisor`

## REMOVED Requirements

### Requirement: --supervisor and --no-supervisor are deprecated aliases

**Reason**: The aliases were the v0.21.0 compatibility window for the rename to `--coordinated` and `--bare`, scheduled for removal in v1.0.0 by the deprecation registry. Freezing them would keep two spellings of each flag for the whole 1.x line.

**Migration**: Replace `--supervisor` with `--coordinated` and `--no-supervisor` with `--bare`; behavior is identical. The old flags now fail with exit code `2` and a message naming the replacement (*Removed --supervisor and --no-supervisor flags are rejected with a migration message*).
