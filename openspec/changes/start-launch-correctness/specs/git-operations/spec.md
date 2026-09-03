## ADDED Requirements

### Requirement: Default-branch resolution is single-source and tolerant of a missing origin/HEAD

The system SHALL resolve the repository's default branch through a single resolver used by every launch path (initial `start`, resume, `add`, and the rebase step), so no two paths can disagree. When `refs/remotes/origin/HEAD` is not set — e.g. a repository with local commits but no configured remote HEAD — the resolver SHALL fall back deterministically (a local `main`, else a local `master`, else the checked-out branch via `git symbolic-ref --short HEAD`) rather than aborting. A launch that succeeded initially SHALL therefore also succeed on resume in the same repository state.

#### Scenario: Resolution succeeds without origin/HEAD

- **GIVEN** a repository with local commits and no `refs/remotes/origin/HEAD` set
- **WHEN** the default branch is resolved
- **THEN** it SHALL return a valid local default branch (via the fallback) rather than aborting with an error

#### Scenario: First launch and resume resolve identically

- **GIVEN** a repository whose first `git paw start` resolved a default branch
- **WHEN** the session is later resumed (a second `git paw start`) in the same repository state
- **THEN** the resume SHALL resolve the same default branch through the same resolver and SHALL NOT fail where the first launch succeeded

#### Scenario: origin/HEAD is still honoured when present

- **GIVEN** a repository with `refs/remotes/origin/HEAD` pointing at `origin/main`
- **WHEN** the default branch is resolved
- **THEN** it SHALL return `main` (the configured remote HEAD), unchanged from prior behaviour
