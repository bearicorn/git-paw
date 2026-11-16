## ADDED Requirements

### Requirement: A coordinated launch's broker answers before on_create runs

In a coordinated launch with the broker enabled, git-paw SHALL start the broker host and wait for it to answer before running any worktree's `on_create` hook, and SHALL register each agent with it as that agent attaches. When provisioning fails, the launch SHALL still tear down the session and its broker host.

#### Scenario: The broker answers while a slow hook runs

- **GIVEN** a coordinated launch of two worktrees whose `on_create` sleeps for 20 seconds
- **WHEN** a client calls the broker's status endpoint 5 seconds after the launch starts
- **THEN** the broker SHALL answer

#### Scenario: A failing hook takes the broker down with the session

- **GIVEN** a coordinated launch whose second worktree's `on_create` exits non-zero
- **WHEN** the launch fails
- **THEN** no broker host for that session SHALL remain running
