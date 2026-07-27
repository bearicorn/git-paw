# boot-block-injection Specification

## Purpose

This capability defines every path by which the rendered boot block reaches an agent pane: supervisor-mode prepending of the boot block (and, when configured, governance-documents and drive-loop coordination sections) to each pane-bound agent's prompt, manual (non-supervisor) mode pre-fill of the boot block into each pane's input line without sending Enter, and boot-block parity for the bare `git paw start --from-specs` launch path. `--from-all-specs` is the canonical flag and `--from-specs` is a deprecated alias retained for backward compatibility, scheduled for removal at v1.0.0.

## Requirements

### Requirement: Supervisor mode boot block prepending

In supervisor auto-start mode, the system SHALL prepend the boot instruction block to each agent's task prompt before injecting it into the tmux pane. This SHALL apply to ALL pane-bound agents — the supervisor pane (pane 0), the dashboard pane (pane 1, where applicable; the dashboard is a TUI process and does not receive a `send-keys` boot block, but the requirement is unchanged for clarity), and the coding agent panes (panes 2..N+1).

#### Scenario: Boot block prepended to agent prompts

- **GIVEN** agent task prompt "Implement error handling"
- **WHEN** `cmd_supervisor()` constructs the full prompt for the coding agent pane
- **THEN** the injected text SHALL be:
  ```
  <boot_block>\n\nImplement error handling
  ```

#### Scenario: Boot block prepended to supervisor pane prompt

- **GIVEN** the supervisor pane (index 0) is being initialised with a "Begin observing" framing message
- **WHEN** `cmd_supervisor()` constructs the supervisor pane's prompt
- **THEN** the injected text SHALL be:
  ```
  <boot_block (with BRANCH_ID = supervisor)>\n\nBegin observing ...
  ```

#### Scenario: Boot block comes before task content

- **GIVEN** any agent or supervisor pane receiving its initial prompt
- **WHEN** the prompt is injected via `tmux send-keys`
- **THEN** the boot block SHALL appear first
- **AND** the actual task content SHALL appear after two newlines

### Requirement: Supervisor boot block timing

The system SHALL inject boot blocks during the supervisor launch sequence, specifically after tmux session creation but before `cmd_supervisor()` returns. The 2-second sleep between session creation and `tmux send-keys` invocations is preserved (panes need to reach an interactive state before key injection).

#### Scenario: Boot blocks injected before cmd_supervisor returns

- **GIVEN** `cmd_supervisor()` is executing
- **WHEN** agent panes are created and initialized
- **THEN** boot blocks SHALL be injected for all pane-bound agents
- **AND** the 2-second boot delay SHALL elapse between session creation and the first `send-keys` call
- **AND** all `send-keys` calls SHALL complete before `cmd_supervisor()` returns

### Requirement: All agents receive boot blocks

In supervisor mode, the system SHALL ensure every coding agent pane AND the supervisor pane receive the boot instruction block, regardless of whether the agent has a spec file or uses a default prompt. The dashboard pane is excluded (it runs a TUI process, not a chat-style agent).

#### Scenario: Coding agents with specs receive boot blocks

- **GIVEN** a coding agent pane with spec file content
- **WHEN** the prompt is constructed
- **THEN** the boot block SHALL be prepended to the spec content

#### Scenario: Coding agents without specs receive boot blocks

- **GIVEN** a coding agent pane with no spec file (default prompt)
- **WHEN** the prompt is constructed
- **THEN** the boot block SHALL be prepended to the default prompt

#### Scenario: Supervisor pane receives a boot block

- **GIVEN** the supervisor pane (index 0)
- **WHEN** the prompt is constructed
- **THEN** the boot block (with `BRANCH_ID = supervisor`) SHALL be prepended to the "Begin observing" framing message

### Requirement: Boot prompt includes governance documents section

When the supervisor agent's boot prompt is constructed AND `config.governance` has at least one path field set to `Some(_)`, the system SHALL append a "Governance documents" section to the boot prompt. The section SHALL list one bullet per configured path with the doc's canonical name and the configured path. Path fields whose value is `None` SHALL NOT appear in the bullet list.

When ALL `config.governance` path fields are `None`, the system SHALL omit the entire "Governance documents" section from the boot prompt (no header, no empty bullet list, no placeholder text).

The section SHALL be a plain-text block separated from preceding boot-prompt content by a blank line. The section heading SHALL be the literal string `## Governance documents`.

The section SHALL NOT contain a "gates" sub-line, gate-flag summaries, or any per-doc enforcement metadata. `governance-config` no longer ships a `[governance.gates]` table; the boot prompt has nothing to convey about enforcement beyond the path list.

#### Scenario: Section omitted when no paths configured

- **GIVEN** `config.governance` with all five path fields `None`
- **WHEN** the supervisor's boot prompt is constructed
- **THEN** the boot prompt SHALL NOT contain the substring `Governance documents`

#### Scenario: Section present with one path

- **GIVEN** `config.governance.dod = Some("docs/dod.md")` and the other path fields `None`
- **WHEN** the boot prompt is constructed
- **THEN** the boot prompt SHALL contain the heading `## Governance documents`
- **AND** the section SHALL contain a bullet referencing `dod` and `docs/dod.md`
- **AND** the section SHALL NOT contain bullets for `adr`, `test_strategy`, `security`, or `constitution`

#### Scenario: Section lists all configured paths in canonical order

- **GIVEN** `config.governance` with all five paths populated
- **WHEN** the boot prompt is constructed
- **THEN** the section SHALL list five bullets in canonical order: `adr`, `test_strategy`, `security`, `dod`, `constitution`

#### Scenario: Section contains no gates summary

- **GIVEN** any `config.governance` configuration with at least one path set
- **WHEN** the boot prompt is constructed
- **THEN** the "Governance documents" section SHALL NOT contain a "Gated docs" line, a "Governance gates" sub-section, or any text referencing per-doc gate flags

### Requirement: Governance section follows the supervisor skill content

The "Governance documents" section SHALL appear in the boot prompt *after* the supervisor skill content (rendered from `assets/agent-skills/supervisor.md` per the existing supervisor-launch capability) and BEFORE any per-agent task content. This positioning ensures the supervisor agent reads governance configuration in the same context where it reads its own skill instructions.

#### Scenario: Section position is between skill and task content

- **GIVEN** a configured `config.governance` and a supervisor session being launched
- **WHEN** the boot prompt is constructed
- **THEN** the position of `## Governance documents` SHALL come after the substring `## Supervisor Skills` (or whatever the skill heading is)
- **AND** SHALL come before any task-specific content

### Requirement: Drive-loop coordination in the supervisor boot context

When a session runs `--unattended` (an in-process drive loop is auto-approving classifier-safe prompts), git-paw SHALL inject into the supervisor's boot context a directive stating that:

- a drive loop is running and owns mechanical approval of classifier-safe prompts;
- the supervisor SHALL consume the loop's escalations rather than blanket-approving prompts by sweeping panes;
- the supervisor handles the reasoning-level work the loop cannot — escalated non-safe prompts, verification, merge orchestration, and conflict handling.

When the session is NOT unattended (no drive loop), the boot context SHALL NOT contain this directive, and the supervisor operates as the sole approver (full sweep + approve).

#### Scenario: Unattended supervisor boot context announces the drive loop

- **GIVEN** a supervisor session started with `--unattended`
- **WHEN** the supervisor's boot context is assembled
- **THEN** it SHALL contain the directive that a drive loop owns safe-prompt approval and the supervisor consumes escalations

#### Scenario: Attended supervisor boot context omits the drive-loop directive

- **GIVEN** a supervisor session started WITHOUT `--unattended`
- **WHEN** the supervisor's boot context is assembled
- **THEN** it SHALL NOT contain the drive-loop coordination directive

### Requirement: Manual mode boot block pre-fill

In manual broker mode (without supervisor), the system SHALL pre-fill the boot instruction block into each agent pane's input line without sending an Enter key. This allows users to paste their actual task after the boot instructions.

#### Scenario: Boot block pre-filled without Enter

- **GIVEN** broker-enabled session in manual mode
- **WHEN** agent panes are created
- **THEN** boot block SHALL be sent to input line
- **AND** no Enter key SHALL be sent
- **AND** cursor SHALL remain at end of boot block

#### Scenario: User can append task after boot block

- **GIVEN** boot block pre-filled in agent pane
- **WHEN** user pastes task instructions
- **THEN** task appears after boot block
- **AND** user can press Enter to submit combined content

### Requirement: Manual mode injection timing

The system SHALL inject boot blocks in manual mode immediately after tmux session creation, before returning control to the user.

#### Scenario: Boot blocks injected during session setup

- **GIVEN** `git paw start --from-specs --cli claude` (manual mode)
- **WHEN** tmux session is created
- **THEN** boot blocks SHALL be pre-filled before command returns

### Requirement: Manual mode configuration

The system SHALL respect the same boot block configuration in manual mode as in supervisor mode, ensuring consistent behavior across all usage patterns.

#### Scenario: Configuration applies to manual mode

- **GIVEN** boot block configuration enabled
- **WHEN** manual mode session starts
- **THEN** boot blocks SHALL be pre-filled using same template

#### Scenario: Disabled configuration affects both modes

- **GIVEN** boot block configuration disabled (if implemented)
- **WHEN** manual mode session starts
- **THEN** no boot blocks SHALL be pre-filled

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
