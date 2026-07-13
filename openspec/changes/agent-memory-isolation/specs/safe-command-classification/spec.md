## ADDED Requirements

### Requirement: Operator config-path writes escalate as danger

When a filesystem prompt (write / edit / create / delete) or a shell command slice targets a path that resolves inside the protected-path set (per `agent-memory-isolation`), the classifier SHALL classify it as a danger-class escalation — terminal, never auto-approved — evaluated with the same precedence as the curated danger-list (before any allowlist or safe-by-pattern rule). Read-only operations SHALL NOT match this rule.

Target paths SHALL be canonicalized before matching, with the same fail-closed posture as the worktree boundary check: a path that cannot be canonicalized but syntactically contains `..` or `~` components reaching into the protected set SHALL be treated as matching.

#### Scenario: Write to operator memory escalates as danger

- **GIVEN** an agent pane prompt "Do you want to allow this write to /Users/op/.claude/projects/-x-repo/memory/MEMORY.md?"
- **WHEN** the classifier runs
- **THEN** the verdict SHALL be a danger-class escalation
- **AND** no auto-approval keystrokes SHALL ever be dispatched for it

#### Scenario: Shell append to a configured settings file escalates

- **GIVEN** a config with `[clis.claude-oss] settings_path = "~/.claude-oss/settings.json"`
- **WHEN** a prompt's command slice is `echo '{}' >> ~/.claude-oss/settings.json`
- **THEN** the verdict SHALL be a danger-class escalation

#### Scenario: In-worktree writes are unaffected

- **GIVEN** an agent whose worktree root is `/repo/.git-paw/worktrees/feat-x`
- **WHEN** a prompt targets a write to `notes/memory.md` inside that worktree
- **THEN** this rule SHALL NOT match
- **AND** the existing worktree-confined safe-by-pattern classification SHALL apply

#### Scenario: Reads of operator config are not matched by this rule

- **WHEN** a prompt's command slice is `cat ~/.claude/settings.json`
- **THEN** this rule SHALL NOT match (other classification rules decide the verdict)

#### Scenario: Path-escape into the protected set is caught

- **GIVEN** a prompt targeting `<worktree>/../../../.claude/settings.json`
- **WHEN** the classifier canonicalizes the path
- **THEN** the resolved target SHALL match the protected set and escalate as danger
