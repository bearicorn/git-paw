## MODIFIED Requirements

### Requirement: Supervisor skill — corrected agent.verified curl example

The embedded orchestrator skill SHALL show `agent.verified` being published through the bundled helper — `.git-paw/scripts/sweep.sh verified <agent-id> <message>` — and SHALL NOT show a raw `curl … /publish` for `agent.verified`, because the broker refuses a verdict that lacks the root-session authority credential the helper attaches. The prose SHALL state that the top-level `agent_id` (the helper's first argument) is the *recipient* — the agent being verified — not the sender, and that the payload the helper builds carries `verified_by` and `message`. It SHALL NOT describe payload fields named `target`, `result`, or `notes`.

#### Scenario: Verified example uses the helper

- **WHEN** the embedded skill is inspected
- **THEN** its `agent.verified` example SHALL contain `sweep.sh verified`
- **AND** SHALL NOT contain a `curl` invocation that posts an `agent.verified` body

#### Scenario: Verified prose names the wire fields and the recipient semantics

- **WHEN** the embedded skill's verification section is inspected
- **THEN** it SHALL mention `verified_by` and `message` as the payload fields the helper sends
- **AND** SHALL state that the agent id argument is the recipient being verified
- **AND** SHALL NOT name `target`, `result`, or `notes` as payload fields

### Requirement: Supervisor skill — corrected agent.feedback curl example

The embedded orchestrator skill SHALL show `agent.feedback` being published through the bundled helper — `.git-paw/scripts/sweep.sh feedback-gate <agent-id> <gate> <message>` — and SHALL NOT show a raw `curl … /publish` for `agent.feedback`, because the broker refuses feedback that lacks the root-session authority credential the helper attaches. The prose SHALL state that the agent id argument is the *recipient* receiving feedback, and that the payload the helper builds carries `from` and an `errors` array. It SHALL NOT describe payload fields named `target` or `message` for feedback.

#### Scenario: Feedback example uses the helper

- **WHEN** the embedded skill is inspected
- **THEN** its `agent.feedback` example SHALL contain `sweep.sh feedback-gate`
- **AND** SHALL NOT contain a `curl` invocation that posts an `agent.feedback` body

#### Scenario: Feedback prose names the wire fields and the recipient semantics

- **WHEN** the embedded skill's feedback section is inspected
- **THEN** it SHALL mention `from` and `errors` as the payload fields the helper sends
- **AND** SHALL state that the agent id argument is the recipient receiving feedback
- **AND** SHALL NOT name `target` or `message` as feedback payload fields
