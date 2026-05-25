## MODIFIED Requirements

### Requirement: Embedded supervisor skill

The embedded supervisor skill SHALL include a "Spec Audit Procedure" section that instructs the supervisor to verify implementation matches spec before publishing `agent.verified`. The procedure SHALL include:

- How to locate spec files for a given change
- How to extract WHEN/THEN scenarios from spec files
- How to search the codebase for matching tests
- How to verify struct fields, function signatures, and types match SHALL/MUST requirements
- How to compile gaps into an `agent.feedback` error list
- When to publish `agent.verified` (no gaps) vs `agent.feedback` (gaps found)

The spec audit SHALL run after the test command passes and before `agent.verified` is published.

The embedded supervisor skill SHALL ALSO include a "Watch peer intents" pointer that informs the supervisor that `agent.intent` messages arrive in its inbox alongside other peer events. With v0.5.0 conflict detection in the broker, the pointer SHALL state that the broker now auto-emits `agent.feedback` (tagged `[conflict-detector]`) for forward, in-flight, and ownership conflicts; the supervisor agent SHALL NOT duplicate this work by manually comparing `modified_files` arrays. The supervisor agent's role with respect to conflict events SHALL be limited to: (a) reading `agent.question` escalations from the conflict detector and applying human judgment when an in-flight conflict has not resolved within the configured window, and (b) following up with agents who repeatedly trigger ownership violations.

The embedded supervisor skill SHALL NOT contain instructions to perform manual `modified_files` overlap comparison across `agent.artifact` events as a primary conflict-detection mechanism. The v0.4 manual section SHALL be removed.

#### Scenario: Supervisor skill contains spec audit procedure

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `Spec Audit`
- **AND** it contains instructions to read `openspec/changes/` spec files
- **AND** it contains instructions to grep for matching tests
- **AND** it contains instructions to verify field names match spec

#### Scenario: Spec audit runs after tests, before verified

- **WHEN** the embedded supervisor skill workflow is inspected
- **THEN** the spec audit step appears after the test command step
- **AND** the spec audit step appears before the `agent.verified` publish step

#### Scenario: Supervisor skill mentions agent.intent

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains the substring `agent.intent`
- **AND** it contains a heading or section titled `Watch peer intents` (or equivalent)

#### Scenario: Supervisor skill documents broker-side conflict detection

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it contains text indicating that the broker auto-emits `agent.feedback` for forward, in-flight, and ownership conflicts
- **AND** it contains the substring `[conflict-detector]`
- **AND** it instructs the supervisor agent to focus on `agent.question` escalations from the detector rather than performing manual `modified_files` comparison

#### Scenario: Supervisor skill removes v0.4 manual conflict-detection section

- **WHEN** the embedded supervisor skill is inspected
- **THEN** it does NOT contain the v0.4 substring "Compare the `modified_files` arrays from every `agent.artifact` event" (or any equivalent instruction to perform manual cross-agent file overlap comparison as the primary detection path)
