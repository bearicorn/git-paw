# boot-block Specification

## Purpose

This capability defines the standardized boot-instruction block injected into each agent and the shared machinery that renders it. It covers the block's format and content (exactly four runtime events — register, done, blocked, question — expressed as `broker.sh` helper invocations, commit-first task completion with a code-less manual-done fallback, and paste-handling guidance), `{{VARIABLE_NAME}}` template substitution with branch-ID slugification and full pre-expansion at render time, and the shared pure `build_boot_block(branch_id, broker_url) -> String` helper in `src/skills.rs` that every launch path uses to assemble the identical block. It also covers every injection path by which the rendered boot block reaches an agent pane: supervisor-mode prepending of the boot block (and, when configured, governance-documents and drive-loop coordination sections) to each pane-bound agent's prompt, manual (non-supervisor) mode pre-fill of the boot block into each pane's input line without sending Enter, and boot-block parity for the bare `git paw start --from-specs` launch path (`--from-all-specs` is the canonical flag and `--from-specs` a deprecated alias retained for backward compatibility, scheduled for removal at v1.0.0).

## Requirements

### Requirement: Standard boot block format

The system SHALL provide a standardized boot instruction block that contains exactly four essential runtime events: register, done, blocked, and question. The boot block SHALL use a consistent format with clear section headers and pre-expanded curl commands.

#### Scenario: Boot block contains all four essential events

- **WHEN** the boot block is generated
- **THEN** it SHALL contain sections for:
  1. REGISTER - Initial status publication
  2. DONE - Task completion reporting
  3. BLOCKED - Dependency waiting notification
  4. QUESTION - Uncertainty escalation

#### Scenario: Boot block uses consistent formatting

- **WHEN** the boot block is generated
- **THEN** it SHALL use the format:
  ```
  ## BOOT INSTRUCTIONS - DO NOT REMOVE
  
  1. REGISTER: <instructions>
     <pre-expanded curl command>
  
  2. DONE: <instructions>
     <pre-expanded curl command>
  
  3. BLOCKED: <instructions>
     <pre-expanded curl command>
  
  4. QUESTION: <instructions>
     <pre-expanded curl command>
  ```

### Requirement: Boot block content requirements

The boot block SHALL include specific instructions for each event type:

1. **REGISTER**: Instruct agent to immediately publish working status with "booting" message.
2. **DONE**: Instruct agent that the primary task-completion path is `git commit` — the post-commit hook installed by git-paw auto-publishes `agent.artifact { status: "committed" }` with the committed files attached, and the agent SHALL NOT publish anything manually for tasks that produce code changes. The section SHALL retain a manual `agent.artifact { status: "done" }` fallback for code-less tasks (docs-only updates handled outside the worktree, planning notes, exploration tasks where the artifact is information reported to the broker), and SHALL include a clear warning against publishing manual `done` when the worktree has uncommitted changes.
3. **BLOCKED**: Instruct agent to publish agent.blocked with dependency information.
4. **QUESTION**: Instruct agent to publish agent.question and WAIT for answer.

Each event's broker interaction SHALL be expressed as an invocation of
the bundled agent helper `.git-paw/scripts/broker.sh` (the
`agent-broker-helper` capability) rather than a raw `curl` command. The
boot block SHALL NOT inline a raw `curl` to the broker URL for any of
the four events; the broker URL and JSON shaping live inside the helper.

#### Scenario: Each event invokes the broker helper

- **WHEN** the boot block is examined
- **THEN** each of the four events SHALL have:
  - Clear one-line description of when to use it
  - A `.git-paw/scripts/broker.sh` invocation (REGISTER, BLOCKED,
    QUESTION sections; the DONE section's manual fallback) with the
    arguments appropriate to the event
  - Appropriate message content for the event type

#### Scenario: Boot block contains no raw broker curl

- **WHEN** the rendered boot block is examined
- **THEN** it SHALL NOT contain a raw `curl` command targeting the
  broker URL for the REGISTER, DONE-fallback, BLOCKED, or QUESTION
  events
- **AND** those events SHALL be expressed as `.git-paw/scripts/broker.sh`
  invocations instead

#### Scenario: QUESTION event emphasizes waiting

- **WHEN** the QUESTION section is examined
- **THEN** it SHALL contain the phrase "DO NOT continue until you receive an answer!"
- **AND** the instruction SHALL be in bold or uppercase for emphasis

#### Scenario: DONE section leads with commit-first instruction

- **GIVEN** the rendered boot block produced by the boot-block builder for any branch
- **WHEN** the DONE section body is examined
- **THEN** it SHALL contain an instruction directing the agent to commit its work via `git commit` as the primary task-completion path
- **AND** the commit-first instruction SHALL appear before the manual `agent.artifact { status: "done" }` helper invocation in the section body
- **AND** the section SHALL state that the post-commit hook auto-publishes `agent.artifact { status: "committed" }` on each commit, so the agent does not need to publish manually for tasks that produce code changes

#### Scenario: DONE section scopes manual done to code-less tasks

- **GIVEN** the rendered boot block produced by the boot-block builder for any branch
- **WHEN** the DONE section body is examined
- **THEN** it SHALL describe the manual `agent.artifact { status: "done" }` fallback as intended for tasks that produce no code changes
- **AND** it SHALL enumerate representative code-less task types (for example: docs-only updates handled outside this worktree, planning notes, exploration tasks)
- **AND** it SHALL contain an emphasised (bold or uppercase) warning that the agent SHALL NOT publish manual `done` when the worktree has uncommitted changes, and SHALL commit instead

#### Scenario: DONE section retains the manual done fallback for code-less tasks

- **GIVEN** the rendered boot block produced by the boot-block builder for any branch
- **WHEN** the DONE section body is examined
- **THEN** it SHALL include a complete, copy-pasteable `.git-paw/scripts/broker.sh artifact` invocation publishing `agent.artifact` with `status: "done"`
- **AND** the published message SHALL use the same JSON shape as in prior boot-block versions (type `agent.artifact`, payload fields `status`, `exports`, `modified_files`) so code-less agents have an unchanged fallback path

### Requirement: Paste handling instructions

The boot block SHALL include specific instructions for handling paste operations, particularly the requirement to send a second Enter key after pasted content.

#### Scenario: Paste handling instruction included

- **WHEN** the boot block is examined
- **THEN** it SHALL contain instructions about paste detection
- **AND** it SHALL mention the need for a second Enter key

#### Scenario: Paste instruction format

- **WHEN** the paste handling section is examined
- **THEN** it SHALL explain that Claude collapses pasted text into `[Pasted text #N]`
- **AND** it SHALL instruct agents to send an additional Enter after paste operations

### Requirement: Template variable substitution

The system SHALL support template variable substitution in boot blocks using the syntax `{{VARIABLE_NAME}}`. The system SHALL replace these variables with actual values at render time.

#### Scenario: Branch ID substitution

- **GIVEN** boot block template containing `{{BRANCH_ID}}`
- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** all occurrences of `{{BRANCH_ID}}` SHALL be replaced with `"feat-errors"`

#### Scenario: Broker URL substitution

- **GIVEN** boot block template containing `{{GIT_PAW_BROKER_URL}}`
- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** all occurrences of `{{GIT_PAW_BROKER_URL}}` SHALL be replaced with `"http://localhost:9119"`

### Requirement: Branch ID slugification

The system SHALL apply slugification to branch IDs during substitution to ensure valid agent IDs. Slugification SHALL replace `/` with `-` and remove any special characters.

#### Scenario: Branch slugification

- **GIVEN** branch name `"feat/errors"`
- **WHEN** substituted into boot block
- **THEN** it SHALL become `"feat-errors"`

#### Scenario: Complex branch name slugification

- **GIVEN** branch name `"fix/topological-cycle-fallback"`
- **WHEN** substituted into boot block
- **THEN** it SHALL become `"fix-topological-cycle-fallback"`

### Requirement: Pre-expansion at render time

The system SHALL expand all template variables before the boot block is injected into agent panes. This SHALL prevent shell expansion permission prompts in agent CLIs.

#### Scenario: All templates expanded before injection

- **GIVEN** boot block template with multiple `{{VARIABLE}}` placeholders
- **WHEN** `build_boot_block()` returns
- **THEN** the returned string SHALL contain no `{{` or `}}` characters
- **AND** all variables SHALL be replaced with actual values

#### Scenario: Invalid template variables handled gracefully

- **GIVEN** boot block template with unknown variable `{{UNKNOWN_VAR}}`
- **WHEN** `build_boot_block()` is called
- **THEN** the unknown variable SHALL be left as-is (no crash)
- **AND** a warning SHALL be logged

### Requirement: Shared boot block helper function

The system SHALL provide a shared `build_boot_block()` function in `src/skills.rs` that can be called from both supervisor and manual mode code paths.

#### Scenario: Function is accessible from multiple modules

- **GIVEN** `build_boot_block()` defined in `src/skills.rs`
- **WHEN** called from `src/main.rs` (supervisor mode)
- **THEN** it SHALL return the boot block string

#### Scenario: Same function used in manual mode

- **GIVEN** `build_boot_block()` defined in `src/skills.rs`
- **WHEN** called from `src/tmux.rs` (manual mode)
- **THEN** it SHALL return the same boot block string

### Requirement: Helper function signature

The `build_boot_block()` function SHALL have the following signature:
```rust
pub fn build_boot_block(branch_id: &str, broker_url: &str) -> String
```

#### Scenario: Function accepts required parameters

- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** it SHALL accept both parameters without error

#### Scenario: Function returns boot block string

- **WHEN** `build_boot_block("feat/errors", "http://localhost:9119")` is called
- **THEN** it SHALL return a `String` containing the boot instructions

### Requirement: Helper function reusability

The `build_boot_block()` function SHALL be designed for maximum reusability with no dependencies on calling context or global state.

#### Scenario: Function is pure (no side effects)

- **GIVEN** same input parameters
- **WHEN** `build_boot_block()` is called multiple times
- **THEN** it SHALL return identical output each time

#### Scenario: Function requires no external state

- **WHEN** `build_boot_block()` is called
- **THEN** it SHALL not access any global variables, configuration, or external services
- **AND** it SHALL only use its input parameters

### Requirement: Helper function testing

The `build_boot_block()` function SHALL be fully testable with comprehensive unit test coverage.

#### Scenario: Function can be tested in isolation

- **WHEN** unit tests call `build_boot_block()` with various inputs
- **THEN** the function SHALL produce expected output without requiring tmux or broker

#### Scenario: Edge cases are testable

- **WHEN** tests provide edge case inputs (empty strings, special characters)
- **THEN** the function SHALL handle them gracefully

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

### Requirement: AGENTS.md user-guide chapter reflects boot-prompt-full-body

`docs/src/user-guide/agents-md.md` SHALL describe AGENTS.md as
the source of truth for the spec body and SHALL state that the
supervisor-mode boot prompt points the agent at AGENTS.md plus
`openspec/changes/<id>/` rather than embedding the spec body in
the boot prompt. The chapter SHALL NOT describe AGENTS.md as
containing only "Branch + CLI + Spec content + Owned files"
(the v0.4 framing); it SHALL describe AGENTS.md as the full spec
artifact target.

#### Scenario: agents-md chapter describes the boot-prompt-full-body model

- **WHEN** `docs/src/user-guide/agents-md.md` is inspected
- **THEN** it states that AGENTS.md is the source of truth for the spec body
- **AND** it states that the supervisor-mode boot prompt points at AGENTS.md and `openspec/changes/<id>/`
