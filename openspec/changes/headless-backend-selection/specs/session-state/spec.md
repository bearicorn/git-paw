## ADDED Requirements

### Requirement: The session file records its terminal backend

The session file SHALL record a session's terminal backend in an optional `backend` field, written as `"headless"` for a headless session and omitted for a tmux session, so a tmux session file is byte-identical to one written before this requirement and a file without the field loads as a tmux session. Every command that acts on an existing session SHALL construct its terminal backend from this field.

#### Scenario: A tmux session file is unchanged

- **GIVEN** a tmux session saved by this release and the same session saved by the previous release
- **WHEN** the two files are compared
- **THEN** they SHALL be byte-identical

#### Scenario: A headless session records its backend

- **GIVEN** a headless session
- **WHEN** its session file is saved and inspected
- **THEN** it SHALL contain `"backend": "headless"`

#### Scenario: An older file loads as tmux

- **GIVEN** a session file with no `backend` field
- **WHEN** it is loaded
- **THEN** the session's backend SHALL be tmux

### Requirement: The per-repo JSON describes headless workers additively

For a headless session, the per-repo session JSON SHALL carry a top-level `backend` with the value `"headless"`, an empty `agents` list, and a `headless_agents` list with one entry per worker carrying `branch_id`, `worktree_path` and `cli`. For a tmux session it SHALL carry neither `backend` nor `headless_agents`, and SHALL be byte-identical to the file written before this requirement. No `pane_index` value SHALL be written for a headless worker.

#### Scenario: A headless discovery file

- **GIVEN** a headless session with workers on `feat/a` and `feat/b`
- **WHEN** its per-repo JSON is read
- **THEN** `backend` SHALL be `"headless"`, `agents` SHALL be empty, and `headless_agents` SHALL list `feat-a` and `feat-b` with their worktree paths and CLIs

#### Scenario: A tmux discovery file is unchanged

- **GIVEN** a tmux session
- **WHEN** its per-repo JSON is read
- **THEN** it SHALL contain no `backend` and no `headless_agents` key

### Requirement: Status reports a headless session's backend and worker activity

For a headless session, `git paw status` SHALL derive the session's liveness from the headless backend instead of tmux, SHALL print the backend, and SHALL print each worker's activity (busy, awaiting input, failed with its exit code, or interrupted). For a tmux session its human-readable output SHALL be byte-identical to the output before this requirement. `git paw status --json` SHALL add a top-level `backend` (`"tmux"` or `"headless"`) and, on every `worktrees` entry, an `activity` that is the worker's activity for a headless session and `null` for a tmux session; `terminal_alive` SHALL report the backend's session liveness and `tmux_running` SHALL keep carrying the same value as `terminal_alive`.

#### Scenario: Human status of a headless session

- **GIVEN** a live headless session whose worker `feat-a` is busy and `feat-b` failed with exit code 1
- **WHEN** `git paw status` runs
- **THEN** the output SHALL name the headless backend
- **AND** SHALL show `feat-a` as busy and `feat-b` as failed with code 1

#### Scenario: JSON status of a headless session

- **GIVEN** the same session
- **WHEN** `git paw status --json` runs
- **THEN** `backend` SHALL be `"headless"`, `terminal_alive` SHALL be `true`, and the `feat-b` entry's `activity` SHALL describe a failed turn with code 1

#### Scenario: tmux status is unchanged

- **GIVEN** a live tmux session
- **WHEN** `git paw status` runs
- **THEN** its output SHALL be byte-identical to the output before this requirement
- **AND** `git paw status --json` SHALL report `backend` `"tmux"` and `activity` `null` on every entry
