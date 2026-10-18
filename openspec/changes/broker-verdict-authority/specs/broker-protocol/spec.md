## ADDED Requirements

### Requirement: Verdict messages require the root-session credential at /publish

The `POST /publish` handler SHALL accept a verdict message — any `agent.verified`, and any `agent.feedback` whose `payload.from` is the root identity `supervisor` — only when the request carries an `X-Paw-Authority` header whose value equals the session's authority credential; otherwise it SHALL respond `403 Forbidden` with the JSON body `{"error":"authority required"}` and SHALL mutate no inbox, roster, or log state. The comparison SHALL be constant-time. An `agent.feedback` from any other sender, and every other message type, SHALL be accepted or rejected exactly as before, with or without the header. The `BrokerMessage` wire envelope SHALL be unchanged — the credential travels only in the header. Messages published in-process by the broker's own subsystems do not cross HTTP and are not subject to this check.

#### Scenario: Verified without the header is refused

- **GIVEN** a running broker with a session credential
- **WHEN** a client POSTs a well-formed `agent.verified` message with no `X-Paw-Authority` header
- **THEN** the response status SHALL be 403
- **AND** the body SHALL contain `"authority required"`
- **AND** the target agent's inbox and the message log SHALL be unchanged

#### Scenario: Supervisor feedback with a wrong credential is refused

- **GIVEN** a running broker with a session credential
- **WHEN** a client POSTs a well-formed `agent.feedback` with `payload.from = "supervisor"` and `X-Paw-Authority: not-the-credential`
- **THEN** the response status SHALL be 403
- **AND** no message SHALL be delivered

#### Scenario: Verified with the credential is accepted

- **GIVEN** a running broker with a session credential
- **WHEN** a client POSTs a well-formed `agent.verified` message with `X-Paw-Authority` equal to the credential
- **THEN** the response status SHALL be 202
- **AND** the message SHALL be broadcast exactly as before this change

#### Scenario: Branch-guard feedback from a worktree is still accepted

- **GIVEN** a running broker with a session credential
- **WHEN** a worktree's post-commit hook POSTs `agent.feedback` with `payload.from = "branch-guard"` and no header
- **THEN** the response status SHALL be 202

#### Scenario: Non-verdict verbs are unaffected

- **GIVEN** a running broker with a session credential
- **WHEN** a client POSTs a well-formed `agent.status`, `agent.artifact`, `agent.blocked`, `agent.question`, or `agent.intent` message with no `X-Paw-Authority` header
- **THEN** the response status SHALL be 202

#### Scenario: A worker cannot self-verify

- **GIVEN** a worker whose only publishing surfaces are its worktree (bundled `broker.sh`, worktree MCP session, `curl`)
- **WHEN** it POSTs `agent.verified` for its own `agent_id` with `verified_by = "supervisor"` and no valid header
- **THEN** the response SHALL be 403
- **AND** the branch SHALL NOT be marked verified

#### Scenario: A worker cannot forge a gate failure

- **GIVEN** a running drive loop with the correction loop enabled
- **WHEN** a worker POSTs `agent.feedback` for a peer with `payload.from = "supervisor"` and a `[test]` gate tag and no valid header
- **THEN** the response SHALL be 403
- **AND** no correction cycle SHALL start for the peer

#### Scenario: In-process publishers are not gated

- **GIVEN** the conflict detector emitting an `agent.feedback` through `publish_message` inside the broker process
- **WHEN** the detector publishes
- **THEN** the feedback SHALL be delivered
