# supervisor-injection Specification

## Purpose
Prepends the standardized boot instruction block (and, when configured, a governance-documents section) to every pane-bound agent's initial prompt — coding agents and the supervisor pane — during the supervisor launch sequence, so each agent reads its runtime-event instructions before its task content.
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

