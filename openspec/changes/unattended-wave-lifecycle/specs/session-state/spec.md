## ADDED Requirements

### Requirement: Session file records the orchestrator pane

The per-repo session JSON SHALL record the supervisor/orchestrator pane (pane 0) and its resolved CLI as a distinct entry (or a dedicated `supervisor` field), in addition to the coding-agent entries, so tooling and the operator can see the full roster and the tiered-model split (e.g. a `fable`-tier orchestrator driving `sonnet` workers) from the state file. The field SHALL be additive and back-compatible: it SHALL use `#[serde(default)]` so older session files load unchanged, and the bundled `sweep.sh` SHALL continue to work against the file (it ignores the extra field).

#### Scenario: The orchestrator pane appears in the session file

- **GIVEN** a supervisor-mode session started with a `fable` orchestrator and `sonnet` workers
- **WHEN** the per-repo session JSON is written
- **THEN** it SHALL record the orchestrator pane (pane 0) with its resolved CLI, distinct from the coding-agent entries

#### Scenario: Older session files without the orchestrator entry still load

- **GIVEN** a session file written before this change (no orchestrator entry)
- **WHEN** it is loaded
- **THEN** it SHALL load without error (the orchestrator field defaults to absent)
