## MODIFIED Requirements

### Requirement: Initial prompt injection via tmux send-keys

After the tmux session is created in detached mode, the system SHALL wait approximately 2 seconds for all panes to reach an interactive state, then inject the initial task prompt for each coding agent pane via a single `tmux send-keys` invocation.

The initial task prompt SHALL be constructed by appending a per-agent **task prompt** to the standardized boot block (separated by a blank line). The task prompt SHALL be derived from the agent's associated `SpecEntry` (if any) via the pure helper `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String`. Because the managed assignment block moved out of the worktree-root `AGENTS.md` into the gitignored sidecar `.git-paw/AGENTS.local.md` (`git_paw::agents::SIDECAR_REL_PATH`) — which coding CLIs do NOT auto-load — every arm of `build_task_prompt` SHALL first point the agent at that sidecar. `build_task_prompt` SHALL dispatch on `SpecEntry.backend`:

1. `SpecBackendKind::OpenSpec` — the task prompt SHALL point the agent at the sidecar and then invoke the slash command `/opsx:apply {id}` (`{id} = spec_entry.id`). It SHALL contain the substring `/opsx:apply {id}` and SHALL NOT contain the path prose `openspec/changes/` (the sidecar already carries the artifact map). The slash command is retained so paste-aware CLIs parse it as a slash-command invocation.
2. `SpecBackendKind::Markdown` or `SpecBackendKind::SpecKit` — the task prompt SHALL point the agent at the sidecar for the project rules and full spec, AND name the sibling-artifact directory `openspec/changes/{id}/`. It SHALL NOT contain `/opsx:apply` (these backends have no slash-command apply workflow).
3. `SpecBackendKind::Superpowers` — the task prompt SHALL point the agent at the sidecar for the project rules and the full superpowers plan (goal, tasks, exact file paths, per-step verification commands), instructing the agent to work the steps in order and flip `- [ ]` to `- [x]` as each lands.
4. When no spec is associated with the agent's branch (the `--branches` path), the task prompt SHALL be the verbatim fallback `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."`.

The task prompt SHALL NOT contain the spec body itself, nor a truncated heading from the spec body. The full spec body remains the source of truth for the sidecar's generation (`WorktreeAssignment.spec_content` is unchanged); only the injected task-prompt portion changes per backend.

The single `tmux send-keys` invocation SHALL pass the constructed prompt followed by the `Enter` keystroke. The longer pointer prompts may still trip paste-aware CLIs' paste-buffer behaviour, which the supervisor agent recovers from via the paste-buffer-recovery skill (see the `agent-skills` capability).

#### Scenario: Initial prompt is injected after boot delay

- **GIVEN** two coding agent panes have been created
- **WHEN** `cmd_supervisor()` injects initial prompts
- **THEN** `tmux send-keys` SHALL be called for each agent pane with the task prompt followed by `Enter`

#### Scenario: Default prompt when no spec content

- **GIVEN** an agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected task-prompt portion SHALL be the verbatim fallback `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."`

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

#### Scenario: OpenSpec-backed task prompt points at the sidecar then invokes opsx:apply

- **GIVEN** a coding agent on branch `feat/my-change` whose associated spec entry has `id = "my-change"` and `backend = SpecBackendKind::OpenSpec`
- **WHEN** the supervisor launch flow builds the task prompt for that agent
- **THEN** `build_task_prompt(Some(&entry))` SHALL return a string containing the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the returned string SHALL contain the substring `/opsx:apply my-change`
- **AND** the returned string SHALL NOT contain the substring `openspec/changes/`
- **AND** the returned string SHALL NOT contain any portion of the spec's prompt body

Test: `main::tests::task_prompt_openspec_backend_points_at_sidecar_then_invokes_opsx_apply`

#### Scenario: Markdown-backed task prompt uses the sidecar pointer

- **GIVEN** a coding agent on branch `feat/my-feature` whose associated spec entry has `id = "my-feature"` and `backend = SpecBackendKind::Markdown`
- **WHEN** the supervisor launch flow builds the task prompt for that agent
- **THEN** the returned string SHALL contain the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the returned string SHALL contain the substring `openspec/changes/my-feature`
- **AND** the returned string SHALL NOT contain `/opsx:apply`

Test: `main::tests::task_prompt_markdown_backend_uses_sidecar_pointer`

#### Scenario: No-spec fallback points at the sidecar verbatim

- **WHEN** `build_task_prompt(None)` is called
- **THEN** the returned string SHALL equal `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."` byte-for-byte

Test: `main::tests::task_prompt_without_spec_points_at_sidecar_verbatim`

#### Scenario: Backend dispatch is exhaustive over SpecBackendKind

- **GIVEN** `SpecBackendKind` enumerates the backends supported in the current build (`OpenSpec`, `Markdown`, `SpecKit`, `Superpowers`)
- **WHEN** the supervisor launch flow's task-prompt construction is inspected
- **THEN** `build_task_prompt` SHALL match every variant of `SpecBackendKind` exhaustively
- **AND** the compiler SHALL reject `build_task_prompt` if a future variant is added to `SpecBackendKind` without a corresponding match arm

#### Scenario: build_task_prompt remains a pure function

- **WHEN** the supervisor launch flow's task-prompt construction is inspected
- **THEN** it SHALL be implemented as a pure function `build_task_prompt(spec_entry: Option<&SpecEntry>) -> String`
- **AND** the function SHALL have no I/O side effects (no filesystem reads, no process spawns, no config lookups)
- **AND** the function SHALL be callable from `cfg(test)` without launching tmux
