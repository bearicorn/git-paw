## ADDED Requirements

### Requirement: Supervisor skill — corrected agent.verified curl example

The embedded `supervisor.md` skill's `curl` example for publishing `agent.verified` SHALL use payload field names that match the wire format defined in `broker-messages`. Specifically, the example SHALL include the substrings `verified_by` and `message` in the payload, and SHALL NOT include the substrings `target`, `result`, or `notes` (the v0.4.0 mistakes that did not match the validated wire format).

The `agent_id` field at the top level of the example SHALL be the *recipient* (the agent being verified), per the existing v0.4 convention for supervisor-originated messages. The skill text SHALL clarify this so users do not put `"supervisor"` in `agent_id` (which would route the verification to the supervisor's own inbox).

#### Scenario: Supervisor skill verified example contains correct payload fields

- **WHEN** the embedded `supervisor.md` skill is inspected
- **THEN** the `agent.verified` `curl` example contains the substring `verified_by`
- **AND** the example contains the substring `message`
- **AND** the example does NOT contain the substrings `target`, `result`, or `notes` as payload field names

#### Scenario: Supervisor skill verified example clarifies agent_id semantics

- **WHEN** the embedded `supervisor.md` skill is inspected
- **THEN** the surrounding text or comment indicates that the top-level `agent_id` is the recipient (the agent being verified), not the sender

### Requirement: Supervisor skill — corrected agent.feedback curl example

The embedded `supervisor.md` skill's `curl` example for publishing `agent.feedback` SHALL use payload field names that match the wire format defined in `broker-messages`. Specifically, the example SHALL include the substrings `from` and `errors` in the payload, with `errors` shown as a JSON array of strings, and SHALL NOT include the substrings `target` or `message` as payload field names (the v0.4.0 mistakes).

The `agent_id` field at the top level of the example SHALL be the *recipient* (the agent receiving feedback), per the existing v0.4 convention for `agent.feedback` delivery. The skill text SHALL clarify this so users do not put `"supervisor"` in `agent_id`.

#### Scenario: Supervisor skill feedback example contains correct payload fields

- **WHEN** the embedded `supervisor.md` skill is inspected
- **THEN** the `agent.feedback` `curl` example contains the substring `from`
- **AND** the example contains the substring `errors`
- **AND** the example shows `errors` as a JSON array (contains `[` and `]` brackets within the example body)
- **AND** the example does NOT contain the substring `target` as a payload field name
- **AND** the `agent.feedback` example does NOT contain `message` as a payload field name (it's a Verified-payload field, not Feedback)

#### Scenario: Supervisor skill feedback example clarifies agent_id semantics

- **WHEN** the embedded `supervisor.md` skill is inspected
- **THEN** the surrounding text or comment indicates that the top-level `agent_id` for `agent.feedback` is the recipient (the agent receiving feedback), not the sender

### Requirement: Supervisor skill prose references correct field names

The embedded `supervisor.md` skill's prose surrounding the curl examples (workflow steps, audit notes, etc.) SHALL reference payload field names that match the wire format. Specifically:

- References to publishing `agent.verified` SHALL describe the payload as containing `verified_by` (the sender, typically `"supervisor"`) and `message` (the optional summary). References SHALL NOT use `result` or `notes` as field names.
- References to publishing `agent.feedback` SHALL describe the payload as containing `from` and `errors`. References SHALL NOT use `target` or `message` (singular) as Feedback payload field names.

#### Scenario: Workflow prose references verified_by, not result/notes

- **WHEN** the embedded `supervisor.md` skill's workflow prose is inspected
- **THEN** references to the `agent.verified` payload structure use `verified_by` and/or `message`
- **AND** the workflow prose does NOT instruct setting `result:"pass"` or `notes:""` as part of the verified payload

#### Scenario: Workflow prose references errors, not message, for feedback

- **WHEN** the embedded `supervisor.md` skill's workflow prose is inspected
- **THEN** references to `agent.feedback` payload describe the `errors` field (a list of strings)
- **AND** the workflow prose does NOT instruct setting `message:"..."` as the feedback payload (the `message` field belongs to Verified, not Feedback)
