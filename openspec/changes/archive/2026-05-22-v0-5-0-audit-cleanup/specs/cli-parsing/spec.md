## ADDED Requirements

### Requirement: `git paw purge` interactive confirmation SHALL honour `y`+Enter under all conditions

The `cmd_purge` interactive confirmation prompt SHALL be reliably readable by the dialoguer `Confirm` widget regardless of preceding stderr output. When the unmerged-commits warning has been written to stderr immediately before the prompt, the warning writer SHALL flush stderr before the prompt's `interact()` call begins, so the user's `y`+Enter input is not racing the warning's buffered bytes.

#### Scenario: Purge with unmerged commits and `y`+Enter proceeds

- **GIVEN** a session with at least one branch carrying commits not in `main`
- **AND** `git paw purge` is invoked from a TTY
- **WHEN** the prompt "Purge is irreversible. Continue?" appears and the user types `y` followed by Enter
- **THEN** the purge SHALL proceed (kill tmux session + remove worktrees + delete session JSON)
- **AND** the exit code SHALL be 0
- **AND** stdout SHALL contain `Purged session 'paw-...'`

#### Scenario: Purge with unmerged commits and `n`+Enter cancels

- **GIVEN** same setup as above
- **WHEN** the user types `n` followed by Enter
- **THEN** the purge SHALL NOT proceed
- **AND** the exit code SHALL be 0
- **AND** stdout SHALL contain `Purge cancelled.`
- **AND** the session worktrees SHALL still be on disk

#### Scenario: Purge with bare Enter (no y/n) defaults to no

- **GIVEN** same setup
- **WHEN** the user types Enter without first typing `y` or `n`
- **THEN** the prompt SHALL default to false (No)
- **AND** the purge SHALL NOT proceed
- **AND** stdout SHALL contain `Purge cancelled.`

### Requirement: `git paw purge --force` SHALL propagate `--force` to `git worktree remove` and emit per-worktree progress

When `git paw purge` is invoked with `--force`, the underlying `git worktree remove` invocations SHALL pass `--force` so the removal succeeds on worktrees with uncommitted changes. The command SHALL also emit per-worktree progress messages to stderr (e.g. `Removing worktree <path>...` before each removal and `done (<elapsed>s)` after) so the user can distinguish a slow-but-progressing removal from an actual hang.

#### Scenario: `purge --force` removes dirty worktrees

- **GIVEN** a session with one worktree containing uncommitted edits
- **WHEN** `git paw purge --force` is invoked
- **THEN** the dirty worktree SHALL be removed successfully
- **AND** the exit code SHALL be 0
- **AND** the underlying `git worktree remove` invocation SHALL include the `--force` flag

#### Scenario: `purge --force` emits per-worktree progress to stderr

- **GIVEN** a session with two or more worktrees
- **WHEN** `git paw purge --force` is invoked
- **THEN** stderr SHALL contain a `Removing worktree <path>...` line for each worktree being removed
- **AND** stderr SHALL contain a `done` or completion marker after each removal
- **AND** the order SHALL match the worktree iteration order

#### Scenario: `purge` without `--force` does NOT pass `--force` to `git worktree remove`

- **GIVEN** a session with one worktree containing uncommitted edits
- **WHEN** `git paw purge` (no `--force`) is invoked and the user confirms with `y`
- **THEN** the underlying `git worktree remove` SHALL NOT include the `--force` flag
- **AND** if `git worktree remove` fails because of the dirty state, the failure SHALL be reported to stderr as `warning: failed to remove worktree '<path>': <git error>` per the existing error-handling path
- **AND** purge SHALL continue with the remaining worktrees

### Requirement: `git paw stop` and `git paw purge` SHALL strip the supervisor boot-block injection from AGENTS.md

`cmd_stop` and `cmd_purge` (`src/main.rs`) SHALL invoke a helper that removes the supervisor-pane boot-block injection block from `<repo>/AGENTS.md`. The block is bounded by HTML comment markers `<!-- git-paw:start -->` ... `<!-- git-paw:end -->` (or similar — the actual marker strings are owned by the injection code path and SHALL match exactly). The helper SHALL be idempotent and SHALL preserve all surrounding content byte-for-byte.

#### Scenario: Stop strips the boot-block injection

- **GIVEN** a session in which `cmd_supervisor` or `cmd_start` injected a `<!-- git-paw:start -->`...`<!-- git-paw:end -->` block into `AGENTS.md`
- **WHEN** `git paw stop` (with or without `--force`) is invoked
- **AND** the teardown completes successfully
- **THEN** the resulting `AGENTS.md` SHALL contain no `<!-- git-paw:start -->` marker
- **AND** no `<!-- git-paw:end -->` marker

#### Scenario: Purge strips the boot-block injection

- **GIVEN** the same setup
- **WHEN** `git paw purge` (with or without `--force`) is invoked
- **THEN** `AGENTS.md` SHALL contain neither marker after the purge completes

#### Scenario: Stop/purge on AGENTS.md without markers is a no-op

- **GIVEN** an `AGENTS.md` with no `<!-- git-paw:start -->` marker
- **WHEN** `git paw stop` or `git paw purge` runs the cleanup helper
- **THEN** `AGENTS.md` SHALL be byte-identical to its pre-cleanup state
- **AND** the helper SHALL return success

### Requirement: `git paw init` SHALL be idempotent and additive on existing config files

`src/init.rs::run_init` SHALL parse the existing `.git-paw/config.toml` (if any) and compare its top-level keys/tables against the bundled-default schema. The init flow SHALL append commented stanzas ONLY for keys/tables missing from the user's config. It SHALL NEVER:

1. Modify the value of an existing key.
2. Add a second occurrence of any top-level table (e.g. a second `[supervisor]`) when the user already has that section commented OR uncommented.
3. Re-order or reformat existing keys/sections.
4. Strip existing user comments or blank lines.

When every bundled-default key is already present in the user's config, init SHALL print `config.toml already has all default keys; no changes` and return Ok without writing.

Init invocations SHALL be idempotent: running `git paw init` a second time on a config that the first run produced SHALL leave the file byte-identical.

#### Scenario: First init writes a complete commented default config

- **GIVEN** a fresh repo with no `.git-paw/config.toml`
- **WHEN** `git paw init` is invoked
- **THEN** the file SHALL be created
- **AND** SHALL parse as valid TOML
- **AND** SHALL contain commented stanzas for every bundled-default top-level key/section

#### Scenario: Second init on the just-written file is a no-op

- **GIVEN** the same repo after the first init
- **WHEN** `git paw init` is invoked again
- **THEN** the file SHALL be byte-identical to the first-run output
- **AND** the exit SHALL be 0

#### Scenario: Init preserves a user-authored `[supervisor]` block

- **GIVEN** a `.git-paw/config.toml` containing only:
  ```toml
  [supervisor]
  enabled = true
  cli = "claude-oss"
  ```
- **WHEN** `git paw init` is invoked
- **THEN** the resulting file SHALL contain `enabled = true` and `cli = "claude-oss"` byte-identical to the input
- **AND** SHALL NOT contain a second `[supervisor]` section header (commented or uncommented)
- **AND** SHALL parse as valid TOML (no `duplicate key` errors)

#### Scenario: Init appends missing top-level sections

- **GIVEN** a `.git-paw/config.toml` containing only `branch_prefix = "feat/"`
- **WHEN** `git paw init` is invoked
- **THEN** the resulting file SHALL preserve `branch_prefix = "feat/"` byte-identical
- **AND** SHALL gain commented stanzas for every bundled-default section the user is missing (`[broker]`, `[dashboard]`, `[supervisor]`, etc.)
- **AND** SHALL parse as valid TOML

#### Scenario: Init never modifies existing user values

- **GIVEN** a `.git-paw/config.toml` with `[broker] port = 9200` (non-default port)
- **WHEN** `git paw init` is invoked
- **THEN** the resulting file SHALL still have `port = 9200`
- **AND** SHALL NOT introduce a second `port` key or a commented `# port = 9119` stanza inside `[broker]`

