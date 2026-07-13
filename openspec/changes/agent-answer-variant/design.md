## Context

`BrokerMessage` (`src/broker/messages.rs:443-550`, `#[serde(tag = "type")]`) has ten variants; a test pins the exact count. Supervisor answers currently ride `agent.feedback`, whose `FeedbackPayload { from, errors: Vec<String> }` validation demands non-empty `errors` (`MessageError::EmptyErrors`) — so answers arrive labeled as errors. Routing for feedback targets the envelope `agent_id`'s inbox (`src/broker/delivery.rs:243-249`); roster hygiene distinguishes payload sender from envelope target (v0.6.0 `broker-roster-hygiene`).

## Goals / Non-Goals

**Goals:** a first-class non-error reply shape; feedback stays semantically corrective; zero impact on existing message flows.
**Non-Goals:** threading/conversation IDs (the optional `re` string is a human-readable hint, not a message reference system); changing `agent.question`; deprecating `/tell` (interactive path stays).

## Decisions

- **D1 — New variant over relaxing FeedbackPayload.** Making `errors` optional would silently change every feedback consumer's semantics and mask real validation bugs; an eleventh variant is additive and self-describing. The tagged-enum wire format makes this a pure addition.
- **D2 — `re: Option<String>` as a plain string.** A structured question-reference (message id) would require id plumbing the broker doesn't have; a short free-text echo of the question is enough for the agent to correlate, and omission costs nothing.
- **D3 — Route like feedback (target inbox), attribute like feedback (payload `from`).** Reuses both existing disciplines: targeted delivery and roster hygiene (publisher-only rows).
- **D4 — Skill docs split the semantics.** supervisor.md: answer questions with `agent.answer`; use `agent.feedback` only for corrective errors. coordination.md ("messages you receive"): act on an answer as an authoritative reply; do not treat it as a failure report.

## Risks / Trade-offs

- Every exhaustive match over `BrokerMessage` must gain an arm (`agent_id()`, `status_label()`, `Display`, `validate()`, `check_placeholder_fields`, `message_sender()`, delivery filter, `route_message()`); the compiler enforces coverage — the variant-count test (10 → 11) is updated deliberately, not mechanically.
- Older binaries polling a newer broker would fail to parse the new variant — not a supported topology (broker and CLI ship in one binary); noted for completeness.

## Migration Plan

None: additive. Supervisors that keep answering via feedback continue to work; the skills steer them to the new shape.

## Open Questions

None.
