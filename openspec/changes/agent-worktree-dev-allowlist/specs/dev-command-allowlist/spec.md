## ADDED Requirements

### Requirement: Per-worktree placement for agent panes

When `[supervisor.common_dev_allowlist]` is enabled, the system SHALL merge the resolved dev-command patterns (universal preset + named stacks + `extra`) into `<worktree>/.claude/settings.json` for EVERY agent worktree, using the same merge semantics as the repo-root target (preserve existing entries, dedup, non-fatal per-target errors reported as warnings). Seeding SHALL run:

- for each worktree attached by `git paw start`;
- for a worktree attached by `git paw add`;
- for every restored worktree during session recovery.

The seeder SHALL create `<worktree>/.claude/` when absent (it lies inside a worktree git-paw created). It SHALL ensure the seeded path is excluded from version control via the WORKTREE-LOCAL ignore mechanism (`info/exclude` for that worktree) — never by editing any tracked `.gitignore`. When the feature is disabled, no worktree settings file SHALL be written by this seeder.

#### Scenario: Start seeds every agent worktree

- **GIVEN** the feature is enabled with `stacks = ["rust"]` and a supervisor session starting with two branches
- **WHEN** the session is started
- **THEN** each agent worktree SHALL contain `.claude/settings.json` whose `allowed_bash_prefixes` include the universal preset and the rust-stack patterns

#### Scenario: Add seeds the new worktree

- **GIVEN** a running session and `git paw add feat-new`
- **WHEN** the new agent attaches
- **THEN** `<new-worktree>/.claude/settings.json` SHALL contain the merged patterns

#### Scenario: Recovery re-seeds restored worktrees

- **GIVEN** a recoverable session with the feature enabled
- **WHEN** the session is recovered
- **THEN** every restored agent worktree SHALL carry the merged patterns (picking up preset updates)

#### Scenario: Existing worktree settings entries are preserved

- **GIVEN** an agent worktree whose `.claude/settings.json` already contains a custom `allowed_bash_prefixes` entry
- **WHEN** the seeder runs
- **THEN** the custom entry SHALL remain and the merged patterns SHALL be appended without duplicates

#### Scenario: Seeded file cannot be committed by the agent

- **GIVEN** a seeded agent worktree
- **WHEN** the agent runs `git status` / `git add .` inside the worktree
- **THEN** `.claude/` SHALL be excluded via the worktree-local ignore
- **AND** no tracked `.gitignore` SHALL have been modified

#### Scenario: Disabled feature writes nothing

- **GIVEN** `[supervisor.common_dev_allowlist] enabled = false`
- **WHEN** a session starts
- **THEN** no agent worktree `.claude/settings.json` SHALL be written by this seeder
