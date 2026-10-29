## 1. Seam: module layout (no behavior change)

- [ ] 1.1 Rebase onto the v0.21.0 wire-DTO split. Locate `broker::wire`, or create `src/broker/wire/mod.rs` if it has not landed. Add an empty `wire::a2a` module and an empty `src/broker/tasks.rs`, each with a `//!` doc comment. Confirm that `just check` is green with no behavior change.
- [ ] 1.2 Add `src/broker/tasks.rs` to the AGENTS.md §7 compile-forcing list. `task_events_from` adds an exhaustive `match` over `BrokerMessage`.

## 2. Core: the task projection (pure)

- [ ] 2.1 Define `TaskState` with the eight A2A states, and `TaskEvent`. `TaskEvent` covers worker status, artifact, blocked, question and intent; root guidance, feedback and verdict; registration; and cancel. Also define `TaskRecord`, with fields for agent id, generation, state, status message, history, artifacts, intents and timestamps. Do not use wire or serde types.
- [ ] 2.2 Implement `task_events_from(&BrokerMessage) -> Vec<TaskEvent>` with an exhaustive match. The root identity (`orchestrator`), `agent.advanced-main`, `agent.learning`, `orchestrator.verify-now` and root-identity questions all map to no event.
- [ ] 2.3 Implement `apply(record, event, at)` per design D4. This includes `INPUT_REQUIRED` stickiness against heartbeats, terminal finality, generation n+1 on feedback or answer after a terminal state, and completion only on the verdict variant.
- [ ] 2.4 Implement deterministic ids. Derive the `contextId` from the session name and the host start instant. Derive the task id from context, agent and generation, and the `messageId` from the broker sequence number. Add a uniqueness test.
- [ ] 2.5 Tests: write a table-driven unit test for the "Projection table" scenario, covering every row. Add tests for session-level events changing nothing, for determinism (fold the same list twice), for the root identity never opening a task, for two workers in one context, and for watch-target registration opening `SUBMITTED`.
- [ ] 2.6 Tests: cover the completion-authority invariant. A status labelled `verified` and an artifact labelled `verified` must not complete. Cover terminal finality: a heartbeat after completion is ignored. Cover generation two after feedback, including `referenceTaskIds` and `previousTaskId`.

## 3. Wire: A2A 1.0 DTOs and the mapper

- [ ] 3.1 DTOs, in ProtoJSON form:
  - `Task`, `TaskStatus`, `TaskState` (screaming-snake strings), `Message`, `Role`, `Part`, `Artifact`.
  - `AgentCard`, `AgentInterface`, `AgentCapabilities`, `AgentExtension`, `AgentSkill`, `SecurityScheme` (the HTTP-auth member), `SecurityRequirement` and `StringList`.
  - The JSON-RPC request, response and error objects, and `google.rpc.ErrorInfo`.

  Apply camelCase, `skip_serializing_if` for unset optionals, and ISO 8601 millisecond `Z` timestamps through `chrono`. `Part` uses member-as-discriminator with no `kind` field.
- [ ] 3.2 Implement the mapper `Task::from(&TaskRecord)`, plus status-message, history and artifact mapping. Write coordination-extension metadata under the v1 URI constant.
- [ ] 3.3 Golden fixtures: one JSON fixture per emitted object, checked by hand against the v1.0.1 proto field list (required fields, names, enum strings). Write the serialization tests for "Task serializes with camelCase fields and enum names", "Text part carries no kind discriminator" and "Unknown input fields are ignored".
- [ ] 3.4 Write the metadata tests for "Task metadata carries the agent id and files", "Intent appears in metadata without changing state" and "Blocked message carries needs and from". Write the history and artifact tests for "Heartbeats update the status message without growing history", "Committed artifact appended as a data part" and "Feedback recorded as a user message with one part per error".

## 4. Docs and gates

- [ ] 4.1 `docs/src/architecture.md`: in the broker section, note the `broker::tasks` core, the `broker::wire::a2a` wire module, and that A2A is a face over the bus. Write `///` docs for every public item. Run `mdbook build docs/`.
- [ ] 4.2 Map every scenario to a named test. Run `just check` and `just deny`. No new dependency.
