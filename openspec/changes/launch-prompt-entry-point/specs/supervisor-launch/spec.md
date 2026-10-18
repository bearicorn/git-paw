## MODIFIED Requirements

### Requirement: Initial prompt injection via tmux send-keys

After the tmux session is created in detached mode, the system SHALL wait until each coding agent pane passes the launch-readiness gate (`tmux-orchestration`, "Launch-readiness gate before boot-block injection"), then inject the initial task prompt for that pane and submit it with the split-send mechanism (`cli-resolution`, "Boot prompt submitted via split-send + settle delay").

The initial task prompt SHALL be constructed by appending a per-agent **task prompt** to the standardized boot block (separated by a blank line), followed by the worktree rule (`boot-block`, "Launch prompts name the agent's worktree root"). The task prompt SHALL be derived from the agent's associated `SpecEntry` (if any) via the pure helper `build_task_prompt(spec_entry: Option<&SpecEntry>) -> TaskPrompt`, which returns the task prompt's prose and, when the backend's bundled task-prompt templates declare one, a workflow entry point: a single command line taken from the backend's template with its placeholders substituted. The system SHALL NOT hard-code an entry-point command anywhere but in those templates. The prose is what the initial task prompt carries; the entry point SHALL NOT be embedded in it and SHALL be submitted as its own line per `boot-block`, "A declared workflow entry point is submitted as its own composer line". Because the managed assignment block moved out of the worktree-root `AGENTS.md` into the gitignored sidecar `.git-paw/AGENTS.local.md` (`git_paw::agents::SIDECAR_REL_PATH`) — which coding CLIs do NOT auto-load — every arm of `build_task_prompt` SHALL first point the agent at that sidecar. `build_task_prompt` SHALL dispatch on `SpecEntry.backend`:

1. `SpecBackendKind::OpenSpec` — the prose SHALL point the agent at the sidecar and state that its workflow command follows as the next message. The prose SHALL NOT contain `/opsx:apply` and SHALL NOT contain the path prose `openspec/changes/` (the sidecar already carries the artifact map). The entry point SHALL be the slash command `/opsx:apply {id}` (`{id} = spec_entry.id`), submitted as its own line so paste-aware CLIs parse it as a slash-command invocation and a weaker model cannot narrate it instead of running it.
2. `SpecBackendKind::Markdown` — the prose SHALL point the agent at the sidecar for the project rules and full spec, AND name the sibling-artifact directory `openspec/changes/{id}/`. It SHALL NOT contain `/opsx:apply` and SHALL declare no entry point (this backend has no slash-command apply workflow).
3. `SpecBackendKind::SpecKit` — the prose SHALL point the agent at the sidecar, which carries the project rules and the full Spec Kit assignment inline, and SHALL NOT name an `openspec/changes/` directory (per `core-lang-agnostic`). It SHALL NOT contain `/opsx:apply` and SHALL declare no entry point.
4. `SpecBackendKind::Superpowers` — the prose SHALL point the agent at the sidecar for the project rules and the full superpowers plan (goal, tasks, exact file paths, per-step verification commands), instructing the agent to work the steps in order and flip `- [ ]` to `- [x]` as each lands. It SHALL declare no entry point.
5. When no spec is associated with the agent's branch (the `--branches` path), the prose SHALL be the verbatim fallback `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."` and there SHALL be no entry point.

The task prompt SHALL NOT contain the spec body itself, nor a truncated heading from the spec body. The full spec body remains the source of truth for the sidecar's generation (`WorktreeAssignment.spec_content` is unchanged); only the injected task-prompt portion changes per backend.

The initial prompt SHALL be submitted with exactly one `Enter`, sent as a separate keystroke after the settle delay; a declared entry point SHALL be the only other line the launch flow submits to that pane. The longer pointer prompts may still trip paste-aware CLIs' paste-buffer behaviour, which the supervisor agent recovers from via the paste-buffer-recovery skill (see the `agent-skills` capability).

#### Scenario: Initial prompt is injected once the pane is ready

- **GIVEN** two coding agent panes have been created
- **WHEN** the launch flow injects initial prompts
- **THEN** each agent pane SHALL first pass the launch-readiness gate
- **AND** each SHALL receive its composed prompt typed literally, followed by a separate `Enter` after the settle delay

#### Scenario: Default prompt when no spec content

- **GIVEN** an agent pane with no spec file assigned
- **WHEN** the initial prompt is injected
- **THEN** the injected task-prompt portion SHALL be the verbatim fallback `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."`

#### Scenario: Launch flow sends one Enter per submitted line

- **GIVEN** N coding agent panes
- **WHEN** the launch flow runs through the prompt-injection loop
- **THEN** a pane whose task prompt declares no entry point SHALL receive exactly one standalone `Enter`, submitting the composed prompt
- **AND** a pane whose task prompt declares an entry point SHALL receive exactly two: one submitting the composed prompt and one submitting the entry point
- **AND** the system SHALL NOT emit any other standalone `Enter` keystroke to the pane during the launch flow

#### Scenario: Paste-buffer recovery is delegated to the supervisor skill

- **GIVEN** a coding agent pane on a paste-aware CLI (e.g. Claude Code v2.1.x) whose injected long prompt has been captured as a paste-buffer placeholder rather than submitted
- **WHEN** the supervisor agent's monitoring loop next inspects the pane via `tmux capture-pane`
- **THEN** the supervisor SHALL apply the paste-buffer-recovery sub-case from the embedded skill (`agent-skills` capability)
- **AND** the launch flow itself SHALL have already exited; the launch flow is NOT responsible for retrying the keystroke

#### Scenario: OpenSpec task prompt declares opsx:apply as its entry point

- **GIVEN** a coding agent on branch `feat/my-change` whose associated spec entry has `id = "my-change"` and `backend = SpecBackendKind::OpenSpec`
- **WHEN** the launch flow builds the task prompt for that agent
- **THEN** the prose returned by `build_task_prompt(Some(&entry))` SHALL contain the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the prose SHALL NOT contain the substring `/opsx:apply` nor the substring `openspec/changes/`
- **AND** the prose SHALL NOT contain any portion of the spec's prompt body
- **AND** the entry point SHALL equal `/opsx:apply my-change`

#### Scenario: Markdown-backed task prompt uses the sidecar pointer

- **GIVEN** a coding agent on branch `feat/my-feature` whose associated spec entry has `id = "my-feature"` and `backend = SpecBackendKind::Markdown`
- **WHEN** the launch flow builds the task prompt for that agent
- **THEN** the prose SHALL contain the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the prose SHALL contain the substring `openspec/changes/my-feature`
- **AND** the prose SHALL NOT contain `/opsx:apply`
- **AND** there SHALL be no entry point

Test: `main::tests::task_prompt_markdown_backend_uses_sidecar_pointer`

#### Scenario: Spec Kit task prompt names no artifact directory and no entry point

- **GIVEN** a coding agent whose associated spec entry has `backend = SpecBackendKind::SpecKit`
- **WHEN** the launch flow builds the task prompt for that agent
- **THEN** the prose SHALL contain the sidecar path `.git-paw/AGENTS.local.md`
- **AND** the prose SHALL NOT contain `openspec/changes/` nor `/opsx:apply`
- **AND** there SHALL be no entry point

Test: `main::tests::build_task_prompt_speckit_does_not_reference_openspec_changes_path`

#### Scenario: No-spec fallback points at the sidecar verbatim

- **WHEN** `build_task_prompt(None)` is called
- **THEN** its prose SHALL equal `"Read .git-paw/AGENTS.local.md first for your assignment, then begin your assigned task."` byte-for-byte
- **AND** there SHALL be no entry point

#### Scenario: The composed prompt ends with the worktree rule

- **GIVEN** a coding agent whose worktree root is `/repo/.git-paw/worktrees/feat-x`
- **WHEN** the launch flow composes its initial prompt
- **THEN** the prompt SHALL be the boot block (when the broker is enabled), the task prompt's prose, and the worktree rule naming `/repo/.git-paw/worktrees/feat-x`, in that order, separated by blank lines

#### Scenario: Backend dispatch is exhaustive over SpecBackendKind

- **GIVEN** `SpecBackendKind` enumerates the backends supported in the current build (`OpenSpec`, `Markdown`, `SpecKit`, `Superpowers`)
- **WHEN** the launch flow's task-prompt construction is inspected
- **THEN** `build_task_prompt` SHALL match every variant of `SpecBackendKind` exhaustively, for both the prose template and the entry-point template
- **AND** the compiler SHALL reject `build_task_prompt` if a future variant is added to `SpecBackendKind` without a corresponding match arm

#### Scenario: build_task_prompt remains a pure function

- **WHEN** the launch flow's task-prompt construction is inspected
- **THEN** it SHALL be implemented as a pure function `build_task_prompt(spec_entry: Option<&SpecEntry>) -> TaskPrompt`
- **AND** the function SHALL have no I/O side effects (no filesystem reads, no process spawns, no config lookups)
- **AND** the function SHALL be callable from `cfg(test)` without launching tmux
