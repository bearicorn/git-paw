## ADDED Requirements

### Requirement: Security posture documents broker verdict authority

`docs/src/user-guide/security-posture.md` SHALL describe the broker's verdict-authority rule: `agent.verified`, and `agent.feedback` claiming the root identity `supervisor`, require the per-session credential that only the root session holds; where the credential lives (the user-data session file, never the repository); which surfaces carry it (the drive loop, `sweep.sh` root verbs via `git paw __authority`, root-session MCP tools); its honest threat model — it stops a mistaken or skill-misled worker and a sandboxed rogue worker, and relies on the FS-scoped sandbox's home-directory denial for an unsandboxed rogue worker; and, plainly, what it leaves ungated: `agent.status` and every other non-verdict message, so a status that claims the `supervisor` identity (which can wind down the drive loop's wave) or one agent publishing as another is still accepted.

#### Scenario: Posture chapter names the rule and its limits

- **WHEN** `docs/src/user-guide/security-posture.md` is inspected
- **THEN** it SHALL state that `agent.verified` and `agent.feedback` from `supervisor` require the root-session credential
- **AND** SHALL state that the credential is stored outside the repository
- **AND** SHALL state that an unsandboxed worker with home-directory read access is out of the rule's threat model and is addressed by the sandbox
- **AND** SHALL state that `agent.status` — including one claiming the `supervisor` identity — is not gated
