## ADDED Requirements

### Requirement: Shared spec-task-tracking artifact is exempt from ownership violations

The detector SHALL exempt the shared spec-task-tracking artifact — the tasks file git-paw directs every agent to update with its own checkbox (e.g. a Spec Kit `specs/<feature>/tasks.md` or the OpenSpec `tasks.md`) — from ownership-violation detection, because git-paw's own workflow instructs every agent to write its own line in that single shared file, so a writeback there is expected coordination rather than a collision. The exemption SHALL apply only to the designated task-tracking artifact; ownership violations on genuine source files SHALL still be reported, so a real collision buried among task-file writebacks is not masked.

#### Scenario: A writeback to the shared tasks file is not an ownership violation

- **GIVEN** `feat-x` has an active intent for the shared task-tracking file `specs/001/tasks.md`
- **AND** `feat-y` has an active intent for `["src/b.rs"]`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = ["specs/001/tasks.md"]` (ticking its own checkbox)
- **THEN** no `agent.feedback` containing `ownership violation` SHALL be emitted for `specs/001/tasks.md`

#### Scenario: A real source-file ownership violation is still reported

- **GIVEN** `feat-x` has an active intent for `["src/a.rs"]`
- **AND** `feat-y` has an active intent for `["src/b.rs"]`
- **WHEN** `feat-y` publishes `agent.status` with `modified_files = ["src/a.rs"]`
- **THEN** an `agent.feedback` whose error text contains `[conflict-detector] ownership violation`, `src/a.rs`, and `feat-x` SHALL still be emitted
