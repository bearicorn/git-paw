## Why

An external orchestrator drives a session through the same use cases the CLI does — start, pause,
stop, purge, add, remove, status — and the MCP write tools are how it reaches them without a
terminal. In released v0.17.0 none of them is callable that way: `src/commands/` is a module of the
**binary** crate (`main.rs`), its handlers are `pub(crate)` and import helpers back from `main.rs`
(`attach_agent`, `AttachContext`, `AttachedAgent`, `submit_prompt_to_pane`,
`write_repo_discovery_file`, `write_repo_discovery_file_with_agents`, `gate_pane_or_fail_on_dialog`,
`build_task_prompt`, `combine_boot_and_task_prompt`, `SpecMode`, `apply_spec_mode`,
`attach_or_print_hint`, `reattach_active_session_or_refuse`, `invalidate_if_stale`,
`detach_worktree`, `created_branch_base`); purge (`cmd_purge`, `cmd_purge_stale`, `purge_with_prompt`,
`PurgeOutcome`, `RetainedBranch`, `collect_unmerged_branches`) and the branch-retirement helpers
still live in `main.rs` (4,182 lines, about 1,940 of them production code). The handlers read
`std::env::current_dir()` (13 call sites), print directly (up to 25 `println!`/`eprintln!` per
file) or hand `std::io::stderr()` to a helper (`remove.rs:144`), prompt through `dialoguer`, block on
`attach` or the drive loop, and return `Result<()>` — so a caller gets text, not a result.

This is where the *driving* half of the target architecture starts to pay: from this change the CLI
and MCP drive the same use cases. Archive position 15 (first of three: library, then
`broker-verdict-authority` (16), then `mcp-orchestrator-write-tools` (17)).

## What Changes

- **`commands/` moves into the library crate** as `git_paw::commands`: one `pub fn` per command family
  taking explicit inputs (repository root, the loaded `PawConfig`, an options struct) and an injected
  **output sink** (a small `Output` port: progress lines, warnings), returning a typed outcome. No
  library command reads the cwd, reads stdin, prompts, attaches a client, or runs the drive loop.
- **The binary's `main.rs` becomes the entry layer:** parse → resolve cwd and config → prompt where the
  CLI prompts → call the use case with a stdout/stderr sink → render the outcome → attach or run the
  drive loop when the CLI does. Observable CLI behavior is byte-identical.
- **Remaining bodies leave `main.rs`:** every item the command bodies import back from `main.rs`
  moves — compile-enforced once `commands` is a library module, since the library cannot see the
  binary root. That is purge (`cmd_purge`, `cmd_purge_stale`, `purge_with_prompt`'s non-interactive
  body, `PurgeOutcome`, `RetainedBranch`, `detach_worktree`, `collect_unmerged_branches`), the
  branch-retirement helpers (`created_branch_base`, `MergeState`, `branch_merge_state`,
  `discard_flag`, `retire_created_branch`), the attach primitives (`attach_agent`, `AttachContext`,
  `AttachedAgent`), discovery-file writing (`write_repo_discovery_file`,
  `write_repo_discovery_file_with_agents`), the prompt-submit and readiness wrappers, task-prompt
  building (`build_task_prompt`, `combine_boot_and_task_prompt`), spec selection (`SpecMode`,
  `apply_spec_mode` — the picker prompt itself stays in the entry layer), and stale-receipt
  invalidation; they move under `src/commands/` (or the module that owns their concern). The
  attach/TTY steps (`attach_or_print_hint`, and whatever reattach logic exists when this change
  lands — `reattach_active_session_or_refuse` in v0.17, possibly reshaped by `drive-loop-reattach`)
  leave the command bodies: the library reports "already running" / "launched" in its outcome, and
  the entry layer attaches, prints the hint or refuses — or, with `--unattended`, takes the drive-loop
  binding and runs the loop on the live session (`drive-loop-reattach`).
- **Entry layer by name:** `cmd_prompt_profile` (hidden `__prompt-profile`, prints one profile field
  for the shell helpers), `cmd_setup_cli` / `confirm_setup` (plan print, TTY check, `Confirm`),
  `commands/completions.rs` (writes a clap-generated script to stdout by design), and
  `broker-host-separation`'s `commands/broker_host.rs` (hidden `__broker`, a long-running host
  process) and thin `__dashboard` client (`cmd_dashboard`) are entry-layer or host code; the guards
  allow them by name rather than converting them.
- **Option sets follow the v0.17 CLI:** `stop` takes no force/confirmation option
  (`stop-without-confirmation`); `purge`'s force option keeps branches with unmerged commits
  (`purge-unmerged-branch-safety`) — the library cannot express what the CLI no longer can.
  `setup_cli` (`cli-setup-consent`'s `src/commands/setup_cli.rs`) joins the library surface in its
  released shape — `prepare` (plans, writes nothing), then `apply_setup` / `apply_undo` (write) —
  which the entry layer calls only after the confirmation or `--yes`. The hidden `__classify` entry
  point is gone (v0.17 `classifier-retirement` deleted it).
- `lib.rs`'s "internal API, not semver" notice stands; `commands` carries the same notice.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `code-architecture`: command entry points are a callable library surface with an injected output
  sink; the remaining command bodies leave `main.rs`.

## Impact

- **Predecessors (implementation order):** archive position 15, after `drive-loop-functional-core`
  (14) and `broker-host-separation` (13, which already extracts `cmd_dashboard` into the broker host
  and thin client); it also lands after `drive-loop-reattach` (10, may reshape the reattach logic the
  entry layer takes over) and `launch-prompt-entry-point` (11, edits `build_task_prompt` and
  `submit_prompt_to_pane` before they move). Released v0.17 code it rebases over:
  `stop-without-confirmation` (`cmd_stop` signature), `purge-unmerged-branch-safety`
  (`detach_worktree`, `purge_with_prompt`, `collect_unmerged_branches`, `RetainedBranch`, the
  branch-retirement helpers; purge already uses the single `git::default_branch` resolver),
  `start-spec-launch-parity` (`build_task_prompt` reachable from `commands/start.rs`),
  `cli-setup-consent` (adds `src/commands/setup_cli.rs` — already split into `prepare` /
  `apply_setup` / `apply_undo`, which never prompt — and its `main.rs` entry layer `cmd_setup_cli` /
  `confirm_setup`: plan printing, TTY check, `Confirm`; the shape is kept, only visibility changes),
  `classifier-retirement` (removed `Command::Classify` / `cmd_classify`, so neither is relocated).
- **Successors:** `broker-verdict-authority` (16), `mcp-orchestrator-write-tools` (17, calls
  `git_paw::commands`), `orchestrator-skill-export` (18, routes its start announcement through the
  `Output` sink), v0.19 per-CLI provider work (consumes the library surface).
- **Code:** `src/lib.rs` (`pub mod commands`), `src/commands/*` (signatures, options/outcome structs,
  `Output` sink), `src/main.rs` (entry layer only), new `src/commands/purge.rs`.
- **CLI / config / wire / persisted state:** unchanged.
- **Tests:** CLI e2e suites unchanged (`e2e_tests.rs`, `cli_prompt_matrix.rs`, `pause_e2e.rs`,
  `add_remove_e2e.rs`, `session_stale_hygiene.rs`, …); golden-output checks for `status`, `pause`,
  `stop`, `purge --force`; source audit over `src/commands/**` (print macros **and** direct
  `std::io::stdout()` / `std::io::stderr()` handles; `completions.rs` and `broker_host.rs` allowed by
  name); a `main.rs` guard (entry-layer `cmd_prompt_profile`, `cmd_setup_cli`, `confirm_setup`,
  `cmd_dashboard` allowed by name);
  library-level tests calling `pause` / `status` in-process with a capturing sink, and
  `setup_cli::prepare` writing nothing; the `setup-cli` CLI suites unchanged.
- **Docs:** `docs/src/architecture.md` (entry → app layering; `commands/` in the library),
  `docs/src/contributing.md` (library boundary), `docs/src/architecture-direction.md` "Where the code is
  today" Commands row.
- **Enum-variant ripple:** `SpecBackendKind` — `build_task_prompt` moves with its exhaustive match
  (AGENTS.md §7 lists it under `src/main.rs`; update the checklist path, and add the GP-40 note that
  a struct-field addition ripples like a variant addition). `BrokerMessage` untouched.
- **Security / safety:** unchanged behavior; the prompts that guard destructive actions (purge, and
  `setup-cli`'s write confirmation, which is the user's consent to grant helper permissions) stay in
  the CLI entry layer and are not bypassable through the CLI; the library `setup_cli::prepare` writes
  nothing, and only an explicit `apply_setup` / `apply_undo` call — made by the entry layer after the
  confirmation — writes.
- **Dependencies:** none.

## Rebase note (2026-10-25): audit against released v0.17.0

- `main.rs` is 4,182 lines in v0.17.0 (not 3,264), and v0.17 left more there than the original move
  list named: `combine_boot_and_task_prompt`, `SpecMode`/`apply_spec_mode`, `attach_or_print_hint`,
  `reattach_active_session_or_refuse`, `write_repo_discovery_file_with_agents`, `AttachedAgent`, the
  branch-retirement helpers (`created_branch_base`, `MergeState`, `branch_merge_state`,
  `discard_flag`, `retire_created_branch`), `PurgeOutcome`, `RetainedBranch`. Task 1.2, D4 and the
  `main.rs` requirement now cover them; reattach logic is named generically because
  `drive-loop-reattach` (10) may reshape it.
- `cmd_prompt_profile`, `cmd_setup_cli`/`confirm_setup` and `commands/completions.rs` are classified
  as entry layer and allowed by name in the guards.
- The stdout audit also catches `std::io::stdout()` / `std::io::stderr()` handles passed directly
  (`remove.rs:144` hands `&mut std::io::stderr()` to `detach_worktree`), not only print macros.
- Dropped the nonexistent "purge-local default-branch resolver" (purge already calls
  `git::default_branch`).
- `setup_cli` keeps its released `prepare` / `apply_setup` / `apply_undo` shape; the "writes nothing
  without confirmation" scenario now targets `prepare`.
- Ordering prose updated to archive position 15.
- Entry layer: for an already-running session it also covers `drive-loop-reattach` (10) — with
  `--unattended` it takes the drive-loop binding and runs the loop on the live session; and
  `broker-host-separation` (13)'s `commands/broker_host.rs` (hidden `__broker`, a long-running host
  process) and thin `__dashboard` client (`cmd_dashboard`) are allowed by name in the library guards,
  like `completions.rs` and `cmd_prompt_profile` / `cmd_setup_cli`.
- Task 1.2's AGENTS.md §7 edit also carries the GP-40 process note: a field added to a widely
  constructed struct ripples through every struct-literal site like a variant through every `match`,
  and is scoped in the change's Impact section up front.
