## ADDED Requirements

### Requirement: Command entry points are a callable library surface

The command bodies SHALL live in the library crate as `git_paw::commands`, exposing one `pub fn` per command family (`start`, `start_with_specs`, `supervisor`, `add`, `remove`, `pause`, `stop`, `purge`, `purge_stale`, `status`, `attach`, `recover`, `approvals`, `replay`, `clis`) that takes explicit inputs — the repository root, the loaded `PawConfig`, and an options struct — and an injected output sink, and returns a typed outcome or `PawError`. `setup_cli` SHALL keep its released use-case shape in the library: `prepare` resolves and plans without writing anything, and only `apply_setup` / `apply_undo` write, so writing requires the caller to call apply after obtaining the confirmation the CLI requires (a terminal confirmation or `--yes`). These functions SHALL NOT read `std::env::current_dir()`, SHALL NOT read stdin or prompt interactively, SHALL NOT write to stdout or stderr except through the injected sink (neither through print macros nor through a `std::io::stdout()` / `std::io::stderr()` handle passed to a helper), and SHALL NOT attach a terminal client or run the unattended drive loop; input resolution, prompting, rendering, attaching (including reattaching to, or refusing to fork, an already-running session) and running the drive loop SHALL happen in the caller (the binary's entry layer, or a caller such as the MCP server). Entry-layer code that exists to talk to the terminal — `commands/completions.rs`, which writes a generated completion script to stdout — and host code that runs as its own long-lived process — `commands/broker_host.rs`, the hidden `__broker` host — SHALL be exempted by name, not by pattern. The options SHALL express exactly what the CLI can express — `stop` takes no force or confirmation option. The library crate's "internal API, not covered by semver" notice SHALL remain.

#### Scenario: A command runs in-process with a capturing sink

- **GIVEN** an active session and a caller resolved to the repository root
- **WHEN** the caller invokes `git_paw::commands::pause` with default options and a capturing sink
- **THEN** the session SHALL be paused exactly as `git paw pause` pauses it
- **AND** the call SHALL return an outcome describing the result
- **AND** nothing SHALL be written to the process's stdout

#### Scenario: Command bodies perform no ambient I/O

- **WHEN** a source audit scans every non-test function under `src/commands/`
- **THEN** none SHALL call `std::env::current_dir`, read from stdin, use `dialoguer`, invoke `println!` / `print!` / `eprintln!` / `eprint!`, obtain a `std::io::stdout()` or `std::io::stderr()` handle (including one passed directly to a helper), or attach a tmux client
- **AND** the only files exempted SHALL be `commands/completions.rs` and `commands/broker_host.rs`, each named explicitly

#### Scenario: CLI behavior is byte-identical

- **WHEN** the CLI e2e suites and the golden-output checks for `status`, `pause`, `stop` and `purge --force` run after the relocation
- **THEN** exit codes, stdout, stderr, session files and created panes SHALL match the pre-change behavior

#### Scenario: Stop has no force option in the library

- **WHEN** the `stop` options type is inspected
- **THEN** it SHALL carry no force or confirmation field

#### Scenario: setup_cli writes nothing without the caller's confirmation

- **GIVEN** a repository with no `.claude/settings.local.json` and a verified `claude` allowlist item
- **WHEN** a caller invokes `git_paw::commands::setup_cli::prepare` for `claude` with the allowlist item and does not call `apply_setup`
- **THEN** no target file and no setup record SHALL be created or modified

### Requirement: Remaining command bodies leave main.rs

`src/main.rs` SHALL contain only argument parsing, input resolution (cwd, config, interactive prompts), dispatch into `git_paw::commands`, rendering of outcomes, the attach / drive-loop steps the CLI performs after a command returns, and the entry-layer functions allowed by name: `cmd_prompt_profile` (the hidden `__prompt-profile` subcommand, with its private helpers), `cmd_setup_cli` / `confirm_setup` (the `setup-cli` plan print, terminal check and confirmation) and `cmd_dashboard` (the hidden `__dashboard` subcommand, a thin client of the broker host). `cmd_purge`, `cmd_purge_stale`, `purge_with_prompt`'s non-interactive body, `PurgeOutcome`, `RetainedBranch`, `detach_worktree`, `collect_unmerged_branches`, the branch-retirement helpers (`created_branch_base`, `MergeState`, `branch_merge_state`, `discard_flag`, `retire_created_branch`), `attach_agent`, `AttachContext`, `AttachedAgent`, `write_repo_discovery_file`, `write_repo_discovery_file_with_agents`, `submit_prompt_to_pane`, `gate_pane_or_fail_on_dialog`, `build_task_prompt`, `combine_boot_and_task_prompt`, `SpecMode`, `apply_spec_mode`, `invalidate_if_stale` and any other command body SHALL move under `src/commands/` (or the library module that owns the concern). Reattach logic for an already-running session (whatever form it has when this lands) SHALL be an entry-layer step driven by the command's outcome, not a call made from inside a command body: the entry layer attaches, prints the hint or refuses, or, with `--unattended`, takes the drive-loop binding and runs the loop on the live session.

#### Scenario: main.rs holds no command body

- **WHEN** `src/main.rs` is scanned
- **THEN** it SHALL contain no function whose name begins `cmd_` other than dispatch helpers and the by-name entry-layer functions `cmd_prompt_profile`, `cmd_setup_cli` and `cmd_dashboard`
- **AND** SHALL contain no definition of `attach_agent`, `AttachedAgent`, `write_repo_discovery_file`, `write_repo_discovery_file_with_agents`, `submit_prompt_to_pane`, `detach_worktree`, `build_task_prompt`, `combine_boot_and_task_prompt`, `apply_spec_mode`, `retire_created_branch`, `PurgeOutcome` or `RetainedBranch`

#### Scenario: Purge behavior is unchanged after relocation

- **WHEN** the purge scenarios in `tests/e2e_tests.rs`, the purge unit tests and `tests/session_stale_hygiene.rs` run
- **THEN** they SHALL pass unchanged
