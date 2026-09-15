## 1. Reproduce the three defects

- [ ] 1.1 Add an E2E test launching `--specs` without supervisor mode against a stub CLI, asserting the agent pane's prompt is SUBMITTED (no residual unsubmitted input line) — SHALL fail before the fix
- [ ] 1.2 Extend it to assert the submitted prompt contains the task prompt naming the assignment and the backend's apply invocation — SHALL fail before the fix
- [ ] 1.3 Add a test asserting the worker pane command carries the resolved approval flags for a config with non-empty `Auto` flags — SHALL fail before the fix
- [ ] 1.4 Add the guard test for bare `git paw start`: the boot block SHALL still be pre-filled WITHOUT an `Enter`, so the human-driven flow is provably untouched

## 2. Submit the spec-driven boot prompt

- [ ] 2.1 Replace the bare `build_boot_inject_args` call in `cmd_start_with_specs` with the existing `submit_prompt_to_pane` seam (literal send, per-CLI settle delay, separate `Enter`)
- [ ] 2.2 Resolve the settle delay per CLI via `resolve_submit_delay_ms`, as the supervisor path does
- [ ] 2.3 Confirm no CLI name is branched on anywhere in the new path (`cli-resolution` requires the mechanism stay CLI-agnostic)

## 3. Inject the task prompt

- [ ] 3.1 Make `build_task_prompt` reachable from `commands/start.rs` with the smallest possible move or re-export; do NOT start the v0.18.0 library-surface refactor here
- [ ] 3.2 Build each agent's prompt as boot block + task prompt, byte-identical to what the supervisor path produces for the same spec
- [ ] 3.3 Inject and submit the task prompt even when `[broker] enabled = false`, where no boot block is injected
- [ ] 3.4 Add a test asserting the spec-driven prompt matches the supervisor path's prompt for the same spec entry

## 4. Apply approval flags on every launch path

- [ ] 4.1 Call `config::resolve_approval_flags` in `cmd_start_with_specs` and use the result when building `PaneSpec.cli_command`
- [ ] 4.2 Do the same in bare `cmd_start`
- [ ] 4.3 Assert an empty resolved flag string yields the bare CLI name with no trailing separator
- [ ] 4.4 Grep for any remaining construction of a worker pane command from a bare CLI name and fix or justify each

## 5. Spec traceability

- [ ] 5.1 Verify every scenario in `specs/boot-block/spec.md` maps to at least one test
- [ ] 5.2 Verify every scenario in `specs/supervisor-config/spec.md` maps to at least one test
- [ ] 5.3 Confirm the modified `Manual mode boot block pre-fill` no longer contradicts `cli-resolution`'s split-send requirement — read both together and state the boundary in the commit body

## 6. Docs and gates

- [ ] 6.1 Update the user-guide section describing the from-specs / manual launch flows to state which paths submit and which pre-fill
- [ ] 6.2 Note in the docs that non-supervisor workers now receive the configured approval flags, since this changes effective permission behaviour for existing configs
- [ ] 6.3 `cargo fmt`, then `just lint`
- [ ] 6.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code rather than piped output
- [ ] 6.5 `mdbook build docs/`
- [ ] 6.6 `openspec validate start-spec-launch-parity --strict`
