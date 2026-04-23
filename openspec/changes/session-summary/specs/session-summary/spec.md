## ADDED Requirements

### Requirement: write_session_summary function

The system SHALL provide a public function `pub fn write_session_summary(state: &BrokerState, session: &PawSession, merge_order: &[String], output_path: &Path) -> Result<(), PawError>` in a new `src/summary.rs` module.

The function SHALL extract data from `BrokerState` (agent records and message log) and `PawSession` (project name, start time) to produce the summary. The `merge_order` parameter provides the sequence in which branches were merged.

The function SHALL write the summary to `output_path` as a UTF-8 Markdown file. If the file already exists, it SHALL be overwritten.

#### Scenario: Summary is written to the specified path

- **GIVEN** a valid `BrokerState` with two agents and a `PawSession`
- **WHEN** `write_session_summary(&state, &session, &merge_order, &path)` is called
- **THEN** a file SHALL exist at `path`
- **AND** the function returns `Ok(())`

#### Scenario: Existing summary is overwritten

- **GIVEN** a summary file already exists at the output path
- **WHEN** `write_session_summary` is called again
- **THEN** the file SHALL contain only the new summary content

#### Scenario: Write failure returns PawError

- **GIVEN** the output path is in a read-only directory
- **WHEN** `write_session_summary` is called
- **THEN** the function returns `Err(PawError::...)`

### Requirement: Session metadata section

The summary SHALL begin with a `# Session Summary` heading followed by a metadata block containing:

- Project name (from `PawSession`)
- Session date (formatted as `YYYY-MM-DD`)
- Session duration (from `PawSession` start time to now)
- Number of agents

#### Scenario: Summary contains correct project name

- **GIVEN** a `PawSession` with `project_name = "my-app"`
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL contain `my-app`

#### Scenario: Summary contains agent count

- **GIVEN** a `BrokerState` with 3 known agents
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL contain the number `3` adjacent to "Agents"

### Requirement: Per-agent details section

The summary SHALL include an `## Agents` section with one subsection (`### <branch> (<cli>)`) per known agent, containing:

- `Status:` — the agent's final status from its last message
- `Files modified:` — from the last `agent.artifact` message's `modified_files`
- `Exports:` — from the last `agent.artifact` message's `exports`
- `Blocked time:` — estimated from `agent.blocked` to next `agent.status` timestamps in the message log

When an agent has no `agent.artifact` message, the files and exports fields SHALL show `(none)`.

#### Scenario: Per-agent section includes modified files

- **GIVEN** agent `"feat-config"` published `agent.artifact` with `modified_files = ["src/config.rs"]`
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL contain `src/config.rs` under the `feat-config` agent section

#### Scenario: Per-agent section handles no artifact

- **GIVEN** agent `"feat-config"` only published `agent.status` (never completed)
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL still include a section for `feat-config`
- **AND** files and exports SHALL show `(none)`

### Requirement: Merge order section

The summary SHALL include an `## Overview` section listing the merge order as a comma-separated list or bullet list.

#### Scenario: Merge order is included

- **GIVEN** `merge_order = ["feat-errors", "feat-config", "feat-detect"]`
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL list `feat-errors`, `feat-config`, `feat-detect` in that order under the merge order label

### Requirement: Totals section

The summary SHALL include a `## Totals` section with:

- Total agents: count of all known agents
- Total time: overall session duration

#### Scenario: Totals section is present

- **GIVEN** a session with 3 agents
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL contain a "Totals" section with at least agent count and duration

### Requirement: Summary module is public

The `summary` module SHALL be declared as `pub mod summary` in `src/lib.rs`. The `write_session_summary` function SHALL be callable from `src/main.rs` supervisor handler.

#### Scenario: Summary module is accessible from main

- **WHEN** `src/lib.rs` is compiled
- **THEN** `git_paw::summary::write_session_summary` SHALL be a callable public function

### Requirement: Summary output path is timestamped under .git-paw/sessions/

The supervisor SHALL call `write_supervisor_summary`, which writes the rendered Markdown to a timestamped file under `<repo_root>/.git-paw/sessions/<UTC-timestamp>.md`. The timestamp SHALL be in `YYYY-MM-DDTHH-MM-SSZ` format (filesystem-safe — colons replaced with hyphens) so multiple supervisor runs against the same repository produce distinct files that can coexist on disk without overwriting each other. `write_supervisor_summary` SHALL create `.git-paw/sessions/` if it does not already exist, and SHALL return the absolute path it wrote so the caller can log it.

#### Scenario: Supervisor writes timestamped summary under sessions dir

- **GIVEN** a completed supervisor session with all agents verified and merged
- **WHEN** the supervisor finishes
- **THEN** a file `<repo_root>/.git-paw/sessions/<UTC-timestamp>.md` SHALL exist
- **AND** the timestamp SHALL match `YYYY-MM-DDTHH-MM-SSZ`
- **AND** the filename SHALL NOT contain a `:` character

#### Scenario: Two sequential supervisor runs produce distinct summary files

- **GIVEN** an empty repository with no `.git-paw/sessions/` directory
- **WHEN** `write_supervisor_summary` is called twice with at least one second between calls
- **THEN** both calls SHALL succeed
- **AND** the returned paths SHALL differ
- **AND** both files SHALL exist on disk

#### Scenario: Sessions directory is created on demand

- **GIVEN** a repository where `.git-paw/sessions/` does not exist
- **WHEN** `write_supervisor_summary` is called
- **THEN** `.git-paw/sessions/` SHALL be created
- **AND** the rendered summary SHALL be written inside it
