## ADDED Requirements

### Requirement: Shared spec-task-tracking artifact is exempt from forward and in-flight overlap

The detector SHALL exclude the shared spec-task-tracking artifact (the same file the ownership exemption names) from each agent's file set before computing forward-conflict and in-flight overlap, because every worker is directed to tick its own checkbox there. Overlap on any other path SHALL still be detected.

#### Scenario: Two intents naming tasks.md raise no forward conflict

- **GIVEN** a running detector with `warn_on_intent_overlap = true`
- **WHEN** `task-t009` publishes an intent for `["specs/003/tasks.md", "src/store.ts"]` and `task-t010` publishes one for `["specs/003/tasks.md", "src/form.ts"]`
- **THEN** no `agent.feedback` containing `forward conflict` SHALL be emitted

#### Scenario: Two writebacks raise no in-flight conflict

- **GIVEN** `task-t009` reports `modified_files = ["specs/003/tasks.md"]`
- **WHEN** `task-t010` reports `modified_files = ["specs/003/tasks.md"]`
- **THEN** no `agent.feedback` containing `in-flight conflict` SHALL be emitted and no triple SHALL be tracked for that path

#### Scenario: A source overlap beside the tasks file still conflicts

- **GIVEN** both agents' intents name `specs/003/tasks.md` and `src/store.ts`
- **WHEN** the second intent is published
- **THEN** a forward-conflict `agent.feedback` naming `src/store.ts` SHALL be emitted and SHALL NOT name `specs/003/tasks.md`
