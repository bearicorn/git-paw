## MODIFIED Requirements

### Requirement: Supervisor treats out-of-worktree writes as violations

The bundled supervisor skill SHALL instruct the supervisor to treat an observed out-of-worktree write attempt by a coding agent as a boundary violation. Such an attempt is a permission prompt or escalation that targets a path outside the agent's worktree, or any other observation of such a write. The supervisor SHALL send the agent scoped `agent.feedback` naming the worktree boundary and the attempted path, SHALL relay any pending prompt for it to the operator instead of approving it, and SHALL escalate to the operator when the same agent repeats the attempt.

#### Scenario: Supervisor skill carries the violation procedure

- **WHEN** the supervisor skill template is rendered
- **THEN** the output SHALL contain the out-of-worktree write violation procedure (scoped feedback, escalate on repeat)
- **AND** it SHALL NOT describe the violation as detected by a git-paw classifier rule

## REMOVED Requirements

### Requirement: Protected-path set is config-driven

**Reason**: The protected-path set's only enforcement was the classifier's danger-class escalation, which existed to stop an automatic approval. With no automatic approval, every write prompt into operator configuration already reaches the human.
**Migration**: The worktree-scoped memory guidance for coding agents and the supervisor's violation procedure stay. To enforce the boundary mechanically, use deny rules in the agent CLI's own settings or the documented FS-scoped sandbox (`core-security-posture`). `ProtectedPaths` is deleted. `[clis.<name>].settings_path` still parses (`approval-command-safety`, *CLI settings path keeps parsing and round-tripping*).
