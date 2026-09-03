## ADDED Requirements

### Requirement: git-paw managed bookkeeping is excluded from conflict overlap

The detector SHALL exclude git-paw's own managed bookkeeping paths (files under a
worktree's `.git-paw/` directory, as identified by `is_managed_path`) from each
agent's modified-file set before computing the modified-file overlap between any two
agents, for both forward-conflict and in-flight conflict detection.
These paths are shared, untracked scaffolding that git-paw itself writes into every
worktree (config, scripts, session state), so their presence in `git status` is not
evidence of a code conflict. Excluding them SHALL NOT affect overlap detection on
genuine source paths.

#### Scenario: A shared untracked .git-paw/ path does not fabricate a conflict

- **GIVEN** a running detector and `feat-x` reports `modified_files = [".git-paw/config.toml"]`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = [".git-paw/config.toml"]`
- **THEN** no `agent.feedback` whose error text contains `in-flight conflict` SHALL be emitted for `.git-paw/config.toml`
- **AND** no conflict SHALL be tracked for that path

#### Scenario: A real source overlap still conflicts when .git-paw/ is also present

- **GIVEN** `feat-x` reports `modified_files = ["src/a.rs", ".git-paw/config.toml"]`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = ["src/a.rs", ".git-paw/config.toml"]`
- **THEN** an `agent.feedback` whose error text contains `in-flight conflict` and `src/a.rs` SHALL be emitted
- **AND** no conflict SHALL be tracked for `.git-paw/config.toml`
