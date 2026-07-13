## ADDED Requirements

### Requirement: Protected-path set is config-driven

The system SHALL derive a protected-path set covering operator configuration and memory territory from configuration and well-known defaults — never from hardcoded CLI product names. The set SHALL contain:

- the default home-level Claude-format config directory (`~/.claude`);
- the directory named by the `CLAUDE_CONFIG_DIR` environment variable when set;
- the parent directory of every configured `[clis.<name>].settings_path`;
- all `projects/**/memory` subtrees beneath the directories above;
- the host repository root's `.claude/` and `.git-paw/` directories, whenever the agent's worktree root differs from the repository root.

All entries SHALL be canonicalized at derivation time; entries whose paths do not exist SHALL still be matched syntactically (fail-closed).

#### Scenario: Configured settings_path parent joins the set

- **GIVEN** a config with `[clis.myvariant] settings_path = "~/.myvariant/settings.json"`
- **WHEN** the protected-path set is derived
- **THEN** it SHALL contain the canonicalized `~/.myvariant` directory

#### Scenario: Repo-root control dirs are protected for embedded worktrees

- **GIVEN** an agent whose worktree root is `<repo>/.git-paw/worktrees/feat-x`
- **WHEN** the protected-path set is derived for that agent
- **THEN** it SHALL contain `<repo>/.claude` and `<repo>/.git-paw` (excluding the agent's own worktree subtree)

#### Scenario: No CLI product names are hardcoded beyond the claude-format default

- **WHEN** the derivation logic is inspected against the export-agnosticism policy
- **THEN** every entry SHALL trace to a config field, an environment variable, or the documented claude-format default directory

### Requirement: Coding-agent memory guidance is worktree-scoped

The bundled coordination skill SHALL include a memory-isolation section instructing coding agents that: all persistent artifacts they create (memory files, notes, scratch state, configuration) SHALL live inside their own worktree; operator configuration directories (home-level CLI config dirs, the host repository's `.claude/` and `.git-paw/`) are off-limits for writes; and when a task appears to require writing outside the worktree, the agent SHALL publish `agent.question` and wait instead of writing. The section SHALL be spec-engine-agnostic and CLI-agnostic (export policy).

#### Scenario: Rendered coordination skill carries the section

- **WHEN** the coordination skill template is rendered for any consumer project
- **THEN** the output SHALL contain the memory-isolation guidance (worktree-scoped writes, off-limits operator dirs, question-instead-of-write)

#### Scenario: Guidance names no spec engine or CLI product

- **WHEN** the memory-isolation section is inspected
- **THEN** it SHALL NOT reference OpenSpec, Spec Kit, or any specific CLI product name

### Requirement: Supervisor treats out-of-worktree writes as violations

The bundled supervisor skill SHALL instruct the supervisor to treat an observed out-of-worktree write attempt by a coding agent (a danger-class escalation on the protected-path rule, or any other observation of such a write) as a boundary violation: send the agent scoped `agent.feedback` naming the worktree boundary and the attempted path, and escalate to the operator when the same agent repeats the attempt.

#### Scenario: Supervisor skill carries the violation procedure

- **WHEN** the supervisor skill template is rendered
- **THEN** the output SHALL contain the out-of-worktree write violation procedure (scoped feedback, escalate on repeat)
