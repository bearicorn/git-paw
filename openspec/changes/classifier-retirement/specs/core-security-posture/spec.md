## MODIFIED Requirements

### Requirement: Consolidated security disclaimer and trust-model posture

git-paw SHALL present a single consolidated security disclaimer, reachable from
the README, the mdBook user guide, and a `git paw doctor` pointer. The disclaimer
SHALL state plainly that git-paw runs AI agents which can execute arbitrary code
(including in full-auto with skipped permissions), and SHALL describe the blast
radius of that execution. It SHALL say what each control does and does **not**
guarantee: the agent CLI's own permission system (the approval level git-paw maps
to launch flags, and the CLI's own allow rules), the FS-scoped sandbox, and remote
control. It SHALL state that git-paw itself makes no decision about whether an
agent's command is safe and approves no permission prompt on its own, so every
prompt the CLI raises is answered by a human. The disclaimer MUST be stated once as
the authoritative source rather than scattered per feature, and MUST NOT overstate
any control's guarantee.

**Test:** `tests/security_posture_docs.rs`

#### Scenario: Disclaimer is present and reachable
- **WHEN** a reader opens the README or the mdBook user guide
- **THEN** a security disclaimer is present that describes arbitrary-code execution, the blast radius, and the guarantees and non-guarantees of the agent CLI's permission system, the sandbox, and remote control
- **AND** it states that git-paw approves no permission prompt on its own

#### Scenario: Doctor points at the posture
- **WHEN** `git paw doctor` runs
- **THEN** its output references where to read the consolidated security posture

#### Scenario: Controls are not overstated
- **WHEN** the disclaimer describes the agent CLI's permission system or the FS-scoped sandbox
- **THEN** it states their limits: a CLI allow rule is only as narrow as it is written and a full-auto mode skips the prompts entirely; the sandbox protects the machine around the worktree, not the worktree's own contents, and by default protects write-integrity only — not the confidentiality of host secrets — and does not cover network, resource-exhaustion, or exposed-socket vectors unless explicitly added

#### Scenario: No classifier is presented as a control
- **WHEN** the disclaimer's control list is inspected
- **THEN** it SHALL NOT present a git-paw command classifier, a git-paw command allowlist, or a git-paw protected-paths rule as a control
