# from-specs-launch Specification

## Purpose
Gives the bare `git paw start --from-specs` path broker boot-block parity with `cmd_start`: when the broker is enabled it injects a per-agent boot block (carrying `BRANCH_ID`, broker URL, and publish patterns) into each spec pane via `tmux send-keys`, accounting for the dashboard pane offset, best-effort and skipped entirely when the broker is disabled.
## Requirements
### Requirement: Boot-block injection in cmd_start_from_specs

When `git paw start --from-specs` is invoked WITHOUT supervisor mode (the bare from-specs path, routed by the dispatcher per `cli-parsing` to `cmd_start_from_specs`) AND `[broker] enabled = true` is set in config, the system SHALL inject a broker boot block into each coding agent pane via `tmux send-keys` after the tmux session is executed.

The injection SHALL mirror the existing behaviour of bare `cmd_start` for consistency:
- After `tmux_session.execute()` succeeds.
- For each spec mapping (each branch + worktree), compute `pane_idx = idx + pane_offset` where `pane_offset = 1` when broker is enabled (account for the dashboard pane at index 0).
- Build the boot block via `git_paw::skills::build_boot_block(branch, &broker_config.url())`.
- Build the send-keys argv via `git_paw::tmux::build_boot_inject_args(&tmux_session.name, pane_idx, &boot_block)`.
- Invoke `std::process::Command::new("tmux").args(&args).status()` (best-effort; failures are non-fatal, matching the existing pattern).

The boot block carries the agent's `BRANCH_ID`, broker URL, and curl-publish-status patterns. Without it, agents launched via from-specs sit at the Claude welcome screen with no broker context — which they need in order to participate in any broker-driven coordination (status publishing, conflict detection in v0.5.0+, etc.).

When `[broker] enabled = false`, no boot-block injection occurs (matching the existing `cmd_start` behaviour — the boot block is broker-specific content).

This requirement does NOT cover spec-content / task-prompt injection. The full prompt that tells the agent what work to do is delivered via the per-worktree `AGENTS.md` (per `worktree-agents-md` capability) and, in a future change, may be augmented by a format-native apply skill invocation (per `dogfood-v040-slot` D1 finding). v0.4 hardening only requires boot-block parity here.

#### Scenario: Boot block is injected per agent pane in spec-mode-with-broker

- **GIVEN** `[broker] enabled = true` and `[supervisor]` is not configured (spec-mode-only)
- **AND** three pending spec changes are discovered
- **WHEN** `git paw start --from-specs` is invoked
- **THEN** after `tmux_session.execute()` succeeds, the system SHALL invoke `tmux send-keys` once per spec pane (panes 1, 2, 3 with broker enabled and dashboard at pane 0)
- **AND** each invocation SHALL pass the boot block produced by `build_boot_block(branch, broker_url)` for that pane's branch
- **AND** the per-pane argv SHALL match what `build_boot_inject_args(session_name, pane_idx, boot_block)` produces

#### Scenario: No boot-block injection when broker is disabled

- **GIVEN** `[broker] enabled = false`
- **AND** spec changes are discovered
- **WHEN** `git paw start --from-specs` is invoked
- **THEN** no `tmux send-keys` calls SHALL be made for boot-block injection
- **AND** the launch SHALL still proceed (panes are created, just without broker boot blocks)

#### Scenario: Boot-block injection failure is non-fatal

- **GIVEN** `[broker] enabled = true` and a pending spec change
- **AND** the underlying `tmux send-keys` invocation returns a non-zero exit (simulating a transient tmux issue)
- **WHEN** `git paw start --from-specs` is invoked
- **THEN** the launch SHALL proceed without erroring out
- **AND** the session SHALL still be saved
- **AND** the user SHALL still be guided to attach (per the non-TTY handling requirement in `cli-parsing`, or the actual attach when TTY is present)

#### Scenario: Pane offset accounts for dashboard

- **GIVEN** `[broker] enabled = true` and N pending spec changes
- **WHEN** the launch flow injects boot blocks
- **THEN** the first spec's boot block SHALL target pane index `1` (dashboard is at index `0`)
- **AND** the Nth spec's boot block SHALL target pane index `N`

