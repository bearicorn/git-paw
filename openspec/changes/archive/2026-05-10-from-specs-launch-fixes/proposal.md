## Why

A v0.4.0 dogfood pass attempting to drive v0.5.0 spec implementation via `git paw start --from-specs --supervisor` revealed two independent bugs that block end-to-end supervisor-mode use:

1. **Dispatcher collision (D5):** `--from-specs` and `--supervisor` are mutually exclusive in v0.4 because of dispatch ordering in `src/main.rs:55-79`. When `--from-specs` is true, the dispatcher returns `cmd_start_from_specs` immediately, never consulting the `supervisor` flag. The user's combination silently degrades to spec-mode-without-supervisor.
2. **Missing boot injection (D4):** `cmd_start_from_specs` does no boot-prompt injection at all — neither the broker boot block (curl/publish-status instructions) that bare `cmd_start` injects at `src/main.rs:389-398`, nor the spec content / task prompt that `cmd_supervisor` injects at `src/main.rs:781-787`. Agents launched via `--from-specs` start at the Claude welcome screen with no instructions, no broker URL, no agent_id awareness.

Both are blockers. The existing `supervisor-launch` spec at line 11 already says cmd_supervisor handles `--from-specs` via spec scanning; the implementation just doesn't get there because of the dispatcher bug. The boot-injection gap in `cmd_start_from_specs` is unspec'd behaviour drift.

This change fixes both before any v0.5.0 supervisor-touching change is implemented (forward-coordination, conflict-detection, governance-context, learnings-mode all depend on `--from-specs --supervisor` actually engaging supervisor mode).

## What Changes

**D5 — Dispatcher reorder.** The dispatch in `src/main.rs:55-79` SHALL be reordered so that `--from-specs --supervisor` (or `--from-specs` with `[supervisor] enabled = true` per the existing supervisor-cli resolution chain) routes to `cmd_supervisor` instead of `cmd_start_from_specs`. Spec-without-supervisor (`--from-specs` alone) continues to route to `cmd_start_from_specs`.

The plumbing for spec scanning inside `cmd_supervisor` already exists at `src/main.rs:586-604`: when `branches_flag` is `None`, the function falls through to `scan_specs(config, repo_root)` and uses the discovered specs as branches. So the dispatcher fix is purely a routing decision — no new spec-scanning code in `cmd_supervisor`.

**D4 — Boot-block injection in `cmd_start_from_specs`.** When `--from-specs` runs WITHOUT supervisor mode (the bare path), `cmd_start_from_specs` SHALL mirror the boot-block injection that `cmd_start:389-398` already performs. Specifically: after `tmux_session.execute()` and before `tmux::attach()`, when `[broker] enabled = true`, iterate panes and inject the boot block via `build_boot_inject_args` so each agent learns its `BRANCH_ID`, broker URL, and curl-publish patterns.

Beyond the boot block, the bare from-specs path SHOULD also inject the spec content / task prompt so agents start working immediately rather than sitting at the Claude welcome screen. The shape of *what* gets injected for openspec / speckit / markdown backends is settled by the parallel D1 finding (format-native apply skill) — which lands as a separate v0.5.0-or-later change. v0.4 hardening just needs the *broker boot block* injection at minimum, matching `cmd_start`'s parity.

**Non-TTY handling (D2 partial fix).** While we're touching the launch path, also fix the non-TTY attach-error misframing: if `stdin().is_terminal() == false`, skip the `tmux::attach` call and print "Session 'paw-<project>' started. Attach with: `tmux attach -t paw-<project>`." This is small and fits naturally into both the dispatcher fix and the new injection step.

Not in scope:
- The format-native apply skill (`/opsx:apply`, Spec Kit equivalent) per dogfood D1 — separate v0.5.0 change.
- Tmux layout improvements for n>4 panes per dogfood D3 — separate v0.4 hardening item.
- Man-page generation for `git paw --help` per dogfood D6 — v1.0.0 polish.

## Capabilities

### New Capabilities
*(none — both fixes extend existing capabilities)*

### Modified Capabilities
- `cli-parsing`: add a requirement specifying the dispatch ordering for `--from-specs` × `--supervisor` so the supervisor flag is honoured when combined with from-specs.
- `supervisor-launch`: clarify that `cmd_supervisor`'s spec-scanning fallback (already at step 2) is reached when `--from-specs --supervisor` is invoked. The existing requirement implies it; the dispatcher fix makes it actually happen.
- `start-force-flag` (or a neighbour): add a requirement that the bare `cmd_start_from_specs` path injects a boot block per pane when broker is enabled, matching `cmd_start`'s behaviour. No new requirement on the spec-content prompt — that's deferred to D1's separate change.
- *(optionally)* `cli-parsing` again: add a requirement that non-TTY launches skip `tmux::attach` and print an attach hint instead of erroring.

## Impact

**Code**:
- `src/main.rs:55-79` — reorder dispatch so `from_specs && supervisor_enabled` routes to `cmd_supervisor`. The existing `cmd_supervisor` signature accepts `branches_flag.as_deref()`; pass `None` when invoked from from-specs so the spec-scanning fallback runs.
- `src/main.rs:1100-1179` (`cmd_start_from_specs`) — after `tmux_session.execute()`, when `broker_config.enabled`, iterate the spec mappings and call `build_boot_inject_args` + `tmux send-keys` per pane. Mirrors the `cmd_start:389-398` block.
- `src/main.rs` — wherever the launch path calls `tmux::attach`, gate the attach on `IsTerminal::is_terminal(&std::io::stdin())`. On non-TTY, print the manual-attach instruction and return `Ok(())` without erroring.

**Tests**:
- Unit/integration test that parses `start --from-specs --supervisor` and asserts the dispatcher target is `cmd_supervisor`, not `cmd_start_from_specs`.
- Integration test (using `assert_cmd` in `--dry-run` mode) that `--from-specs --supervisor --dry-run` produces the supervisor-mode dry-run header (`Supervisor: ...`, `Agent CLI: ...`, etc.) rather than the from-specs dry-run header (`Session: ...`, plain branch list).
- Integration test that bare `--from-specs` (no supervisor) launches a session and verifies pane 1 (or the broker-aware pane offset) receives a boot-block-shaped string via send-keys. The `tmux::build_boot_inject_args` function is already pure and testable; test via the args it produces.
- Non-TTY test: invoke `start --from-specs` with stdin redirected from `/dev/null`; assert exit status 0 and stdout containing "Attach with: `tmux attach`". No "failed to attach" error.

**Backward compatibility**: fully additive at the user surface. Existing `--from-specs` users without `--supervisor` see the additional boot block in their panes (which is what they'd want — they were missing it). Existing `--supervisor` users without `--from-specs` are unaffected. Existing `--from-specs --supervisor` users who were getting the silently-degraded spec-mode now actually engage supervisor mode (which is what they asked for).

**Mismatches resolved**:
- Dogfood D4 — `cmd_start_from_specs` boot-injection gap closed.
- Dogfood D5 — dispatcher collision fixed; `--from-specs --supervisor` engages supervisor mode.
- Dogfood D2 (partial) — non-TTY attach error replaced with informational hint.
