## Context

`src/lib.rs` exposes the domain modules; `src/commands/` is a module of the binary (`main.rs`), its
functions `pub(crate)`, each resolving `current_dir()`, loading config, prompting with `dialoguer`
(purge, supervisor-mode resolution, the spec and branch pickers), printing its own output (print
macros, and `std::io::stderr()` handed to helpers such as `detach_worktree`), and some ending in
`attach` or the drive loop. Purge, the branch-retirement helpers and several launch helpers never
left `main.rs` (4,182 lines in v0.17.0, about 1,940 production). `setup_cli` is the exception that
already has the target shape: its use case (`prepare` / `apply_setup` / `apply_undo`) never prompts,
and its entry layer (`cmd_setup_cli`, `confirm_setup`) lives in `main.rs`. The MCP server lives in the
library, holds no state, and so cannot call any command. `docs/src/architecture-direction.md` places
the commands in the `app` layer and the CLI / MCP in the `entry` layer, which must stay thin.

## Goals / Non-Goals

**Goals:**
- `git_paw::commands::*` callable in-process with explicit inputs, an injected output sink, and typed
  outcomes; `main.rs` is the entry layer; CLI behavior byte-identical.
- Finish moving command bodies out of `main.rs`.

**Non-Goals:**
- MCP tools (`mcp-orchestrator-write-tools`).
- A stable, semver'd library API.
- Changing any prompt, message, exit code or option.

## Decisions

### D1 — Explicit inputs, injected output, typed outcomes

Each family becomes `pub fn <verb>(ctx: &CommandContext, opts: <Verb>Options, out: &mut dyn Output)
-> Result<<Verb>Outcome, PawError>` where `CommandContext` carries the repository root and loaded
config. `Output` is a port with `line` / `warn` (the CLI adapter writes stdout/stderr with the exact
current strings; a capturing adapter collects lines for MCP and tests). Outcomes carry what callers
branch on (session name, attach command, paused agent count, purge results with retained branches).
*Alternative rejected:* "never print, return everything" — commands emit progress mid-flow (worktree
creation, provisioning hooks, warnings); a sink keeps ordering and byte-identical output without
buffering a whole launch. *Alternative rejected:* MCP tools spawning `git-paw <cmd>` — launders errors
through text and reintroduces "which binary is on PATH".

### D2 — Prompts and blocking stay in the entry layer

`dialoguer` prompts (purge confirmation, "start in supervisor mode?", the spec and branch pickers,
the `setup-cli` write confirmation), TTY detection, `attach`, and the `--unattended` drive loop are
entry concerns: the CLI decides, then calls the use case with the decision in the options (e.g.
`PurgeOptions { confirmed: true }`), then attaches or runs the loop from the outcome. The attach/TTY
helpers (`attach_or_print_hint`, and whatever reattach logic exists when this lands —
`reattach_active_session_or_refuse` in v0.17, possibly reshaped by `drive-loop-reattach`) therefore
leave the command bodies: a launch outcome says "already running" or "launched", and the entry layer
attaches, prints the hint, or refuses non-interactively with the current message — or, with
`--unattended`, takes the drive-loop binding and runs the loop on the live session
(`drive-loop-reattach`). `setup_cli` keeps
its released split rather than gaining a confirmation option: `prepare` plans and writes nothing,
and only an explicit `apply_setup` / `apply_undo` call writes, which the entry layer makes after the
confirmation or `--yes` — so an in-process caller cannot grant helper permissions by accident.

A source audit keeps `src/commands/**` free of `current_dir`, stdin, `dialoguer`, `print!`-family
macros, direct `std::io::stdout()` / `std::io::stderr()` handles (the `remove.rs` →
`detach_worktree(…, &mut std::io::stderr())` shape: helpers that take a `Write` get the sink's
adapter instead) and `attach`. Entry-layer code is allowed **by name**, not by pattern:
`commands/completions.rs` (it exists to write a clap-generated script to stdout) and
`broker-host-separation`'s `commands/broker_host.rs` (the hidden `__broker` host, a long-running
process with its own signal handling) in the source audit; `cmd_prompt_profile`
(hidden `__prompt-profile`, with its private regex helpers), `cmd_setup_cli`, `confirm_setup` and the
thin `__dashboard` client `cmd_dashboard` in the `main.rs` guard.

### D3 — Options mirror the CLI as it is after v0.17

`StopOptions` has no force field (`stop-without-confirmation` removed `--force`); `PurgeOptions`
carries `force` and the purge outcome reports retained unmerged branches with tip SHAs
(`purge-unmerged-branch-safety`). The library offers nothing the CLI cannot do.

### D4 — Finish the move in the same change

The move list is every item the command bodies import back from `main.rs` — compile-enforced, since
a library module cannot name the binary root — plus what only those items use: purge (`cmd_purge`,
`cmd_purge_stale`, `purge_with_prompt`'s non-interactive body, `PurgeOutcome`, `RetainedBranch`,
`detach_worktree`, `collect_unmerged_branches`) to `commands/purge.rs`; the branch-retirement helpers
(`created_branch_base`, `MergeState`, `branch_merge_state`, `discard_flag`,
`retire_created_branch`); the attach primitives (`attach_agent`, `AttachContext`, `AttachedAgent`,
with their private helpers `agent_cli_command`, `dialog_stuck_result`); `write_repo_discovery_file`
and `write_repo_discovery_file_with_agents`; `submit_prompt_to_pane`, `gate_pane_or_fail_on_dialog`;
`build_task_prompt` and `combine_boot_and_task_prompt`; `SpecMode` and `apply_spec_mode` (the picker
call stays in the entry layer); `invalidate_if_stale`. The attach/TTY helpers follow D2. What stays in
`main.rs` is dispatch (`run`, `DispatchTarget`, `resolve_dispatch_target`), `is_interactive_stdin`,
the attach steps, and the by-name entry-layer functions. A guard keeps `main.rs` free of command
bodies. `build_task_prompt`'s
exhaustive `SpecBackendKind` match moves with it; the AGENTS.md §7 ripple list is updated in the same
change.

## Risks / Trade-offs

- [Output drift while threading the sink] → golden-output checks on `status`, `pause`, `stop`,
  `purge --force`; CLI e2e suites are the gate.
- [Library surface misread as public API] → the `lib.rs` notice stays; `commands` repeats it.
- [Rebase friction with v0.17 purge/stop changes] → released; the move list above is taken from the
  v0.17.0 `main.rs`. Earlier v0.18 changes that edit the moved helpers (`drive-loop-reattach`,
  `launch-prompt-entry-point`) land first, so the move is a pure relocation of their result.

## Migration Plan

1. `Output` port + CLI/capturing adapters; `CommandContext`.
2. Move remaining `main.rs` bodies under `src/commands/`; `pub mod commands`.
3. Convert families one at a time (status, pause, stop, purge, add, remove, start, supervisor,
   recover, attach, approvals, clis, replay; setup_cli only changes visibility), each behind the e2e
   gate. `cmd_classify`
   is not converted: v0.17 `classifier-retirement` deletes it.
4. Source audit, `main.rs` guard, golden outputs; docs.

Rollback: revert; no persisted state, wire or CLI change.

## Open Questions

- None blocking.
