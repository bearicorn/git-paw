# core-memory-isolation Specification

## Purpose
Keeps coding agents from writing outside their worktree into the operator's
configuration and memory territory. Defines a config-driven protected-path set
(the home-level Claude-format config dir, `CLAUDE_CONFIG_DIR`, every configured
`[clis.<name>].settings_path` parent, their `projects/**/memory` subtrees, and
the host repository's `.claude/` and `.git-paw/` when the agent runs in a
separate worktree), requires the safe-command classifier to escalate writes into
that set as a terminal danger-class refusal (never auto-approved), and scopes
agent memory guidance to the worktree in the bundled coordination and supervisor
skills.
## Requirements
### Requirement: Coding-agent memory guidance is worktree-scoped

The bundled coordination skill SHALL include a memory-isolation section instructing coding agents that: all persistent artifacts they create (memory files, notes, scratch state, configuration) SHALL live inside their own worktree; operator configuration directories (home-level CLI config dirs, the host repository's `.claude/` and `.git-paw/`) are off-limits for writes; and when a task appears to require writing outside the worktree, the agent SHALL publish `agent.question` and wait instead of writing. The section SHALL be spec-engine-agnostic and CLI-agnostic (export policy).

#### Scenario: Rendered coordination skill carries the section

- **WHEN** the coordination skill template is rendered for any consumer project
- **THEN** the output SHALL contain the memory-isolation guidance (worktree-scoped writes, off-limits operator dirs, question-instead-of-write)

#### Scenario: Guidance names no spec engine or CLI product

- **WHEN** the memory-isolation section is inspected
- **THEN** it SHALL NOT reference OpenSpec, Spec Kit, or any specific CLI product name

### Requirement: Supervisor treats out-of-worktree writes as violations

The bundled supervisor skill SHALL instruct the supervisor to treat an observed out-of-worktree write attempt by a coding agent as a boundary violation. Such an attempt is a permission prompt or escalation that targets a path outside the agent's worktree, or any other observation of such a write. The supervisor SHALL send the agent scoped `agent.feedback` naming the worktree boundary and the attempted path, SHALL relay any pending prompt for it to the operator instead of approving it, and SHALL escalate to the operator when the same agent repeats the attempt.

#### Scenario: Supervisor skill carries the violation procedure

- **WHEN** the supervisor skill template is rendered
- **THEN** the output SHALL contain the out-of-worktree write violation procedure (scoped feedback, escalate on repeat)
- **AND** it SHALL NOT describe the violation as detected by a git-paw classifier rule

