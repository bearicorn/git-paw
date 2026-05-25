## MODIFIED Requirements

### Requirement: Initial prompt injection via tmux send-keys

After the tmux session is created in detached mode, the system SHALL wait approximately 2 seconds for all panes to reach an interactive state, then inject the initial task prompt for each coding agent pane via a single `tmux send-keys` invocation.

The initial task prompt SHALL be constructed by appending a per-agent **task prompt** to the standardized boot block (separated by a blank line). The task prompt SHALL be derived from the agent's associated `SpecEntry` (if any) via the pure helper `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String`, which SHALL dispatch on `SpecEntry.backend`:

1. When a spec is associated with the agent's branch (the `--from-specs` path) and `spec_entry.backend == SpecBackendKind::OpenSpec`, the task prompt SHALL be exactly the slash-command invocation `format!("/opsx:apply {id}", id = spec_entry.id)`. The task prompt SHALL NOT contain any prose surrounding the slash command, SHALL NOT contain `AGENTS.md`, and SHALL NOT contain `openspec/changes/`. The slash command SHALL be the entire returned string so that paste-aware CLIs parse it as a slash-command invocation at the start of the agent's first turn.
2. When a spec is associated with the agent's branch and `spec_entry.backend == SpecBackendKind::Markdown` (or any other non-OpenSpec backend that lacks a slash-command apply workflow), the task prompt SHALL point the agent at the worktree's `AGENTS.md` for the full spec body AND include the spec's identifier so the agent can locate sibling artifacts (proposal, design, specs, tasks) under `openspec/changes/<id>/`. The task prompt SHALL NOT contain the spec body itself, nor a truncated heading from the spec body.
3. When no spec is associated with the agent's branch (the `--branches` path), use the default fallback `"Begin your assigned task as described in AGENTS.md."` verbatim.

The full spec body remains the source of truth for `AGENTS.md` generation (`WorktreeAssignment.spec_content` is unchanged); only the injected boot prompt's task-prompt portion changes per backend.

The single `tmux send-keys` invocation SHALL pass the constructed prompt followed by the `Enter` keystroke. On paste-aware CLIs the slash-command form (OpenSpec branch) is short enough that paste-buffer capture is unlikely; the longer Markdown-branch pointer may still trip paste-buffer behaviour, which the supervisor agent recovers from via the paste-buffer-recovery skill (see the `agent-skills` capability).

#### Scenario: Initial prompt is injected after boot delay

- **GIVEN** two coding agent panes have been created
- **WHEN** `cmd_supervisor()` injects initial prompts
- **THEN** `tmux send-keys` SHALL be called for each agent pane with the task prompt followed by `Enter`

#### Scenario: Default prompt when no spec content

- **GIVEN** an agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected task-prompt portion SHALL be the default fallback string `"Begin your assigned task as described in AGENTS.md."`

#### Scenario: Launch flow sends exactly one Enter per pane

- **GIVEN** N coding agent panes
- **WHEN** the supervisor launch flow runs through the prompt-injection loop
- **THEN** the system SHALL invoke `tmux send-keys` exactly once per pane
- **AND** the invocation SHALL include the prompt text and the `Enter` keystroke
- **AND** the system SHALL NOT emit any additional standalone `Enter` keystrokes to the pane during the launch flow

#### Scenario: Paste-buffer recovery is delegated to the supervisor skill

- **GIVEN** a coding agent pane on a paste-aware CLI (e.g. Claude Code v2.1.x) whose injected long prompt has been captured as a paste-buffer placeholder rather than submitted
- **WHEN** the supervisor agent's monitoring loop next inspects the pane via `tmux capture-pane`
- **THEN** the supervisor SHALL apply the paste-buffer-recovery sub-case from the embedded skill (`agent-skills` capability)
- **AND** the launch flow itself SHALL have already exited; the launch flow is NOT responsible for retrying the keystroke

#### Scenario: OpenSpec-backed task prompt invokes the opsx:apply slash command

- **GIVEN** a coding agent on branch `feat/governance-config` whose associated spec entry has `id = "governance-config"` and `backend = SpecBackendKind::OpenSpec`
- **WHEN** the supervisor launch flow builds the task prompt for that agent
- **THEN** `build_task_prompt(Some(&entry))` SHALL return exactly the string `"/opsx:apply governance-config"`
- **AND** the returned string SHALL NOT contain the substring `AGENTS.md`
- **AND** the returned string SHALL NOT contain the substring `openspec/changes/`
- **AND** the returned string SHALL NOT contain any portion of the spec's prompt body

#### Scenario: Markdown-backed task prompt uses the generic AGENTS.md pointer

- **GIVEN** a coding agent on branch `feat/my-feature` whose associated spec entry has `id = "my-feature"` and `backend = SpecBackendKind::Markdown`
- **WHEN** the supervisor launch flow builds the task prompt for that agent
- **THEN** the returned string SHALL contain the substring `AGENTS.md`
- **AND** the returned string SHALL contain the substring `openspec/changes/my-feature`
- **AND** the returned string SHALL NOT begin with `/opsx:apply`
- **AND** the returned string SHALL instruct the agent to read AGENTS.md and the sibling artifacts before starting

#### Scenario: Backend dispatch is exhaustive over SpecBackendKind

- **GIVEN** `SpecBackendKind` enumerates the backends supported in the current build (initially `OpenSpec` and `Markdown`)
- **WHEN** the supervisor launch flow's task-prompt construction is inspected
- **THEN** `build_task_prompt` SHALL match every variant of `SpecBackendKind` exhaustively
- **AND** the compiler SHALL reject `build_task_prompt` if a future variant (e.g. `SpecKit`) is added to `SpecBackendKind` without a corresponding match arm

#### Scenario: build_task_prompt remains a pure function

- **WHEN** the supervisor launch flow's task-prompt construction is inspected
- **THEN** it SHALL be implemented as a pure function `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String`
- **AND** the function SHALL have no I/O side effects (no filesystem reads, no process spawns, no config lookups)
- **AND** the function SHALL be callable from `cfg(test)` without launching tmux
