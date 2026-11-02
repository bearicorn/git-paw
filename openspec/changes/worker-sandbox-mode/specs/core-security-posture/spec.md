## ADDED Requirements

### Requirement: The posture documents sandboxed full-auto workers

The consolidated security posture SHALL document running workers at `full-auto` under a declared
sandbox: how to declare `native` and `wrapper` sandboxes, that declaring one does not change any
approval level, and that a `native` sandbox is refused rather than disabled by a full-auto flag that
would turn it off.

**Test:** `tests/security_posture_docs.rs`

#### Scenario: The sandboxed full-auto section is present

- **WHEN** the security posture chapter is inspected
- **THEN** it SHALL describe declaring `native` and `wrapper` sandboxes and running workers at `full-auto` under them

### Requirement: The posture presents sandboxed full-auto as the unattended route

Because git-paw approves no permission prompt on its own, the posture SHALL present sandboxed
`full-auto`, alongside the agent CLI's own allow rules, as the way to run an `--unattended` wave
without its prompts waiting on a human.

**Test:** `tests/security_posture_docs.rs`

#### Scenario: Sandboxed full-auto is presented as the unattended route

- **WHEN** the security posture chapter is inspected
- **THEN** it SHALL present sandboxed `full-auto` as a way to run an `--unattended` wave without its prompts waiting on a human

### Requirement: The posture states the residual risk of sandboxed full-auto

The posture SHALL state plainly that with no permission prompt no command a sandboxed full-auto
worker runs is put to a human, so only the agent CLI's own deny rules and the sandbox stand between
it and the machine; and that the FS-scoped sandbox does not restrict network egress, reads of
secrets without read-hardening, updates to any branch ref in the shared repository, or pushes made
with the user's credentials.

**Test:** `tests/security_posture_docs.rs`

#### Scenario: Residual risks are stated

- **WHEN** the sandboxed full-auto section is inspected
- **THEN** it SHALL state that without a permission prompt no command of that worker is put to a human
- **AND** it SHALL name network egress, secret reads, shared-repository ref updates and credentialed pushes as not covered by the FS sandbox

### Requirement: The posture does not present a declared sandbox as verified

The posture SHALL state that a declared sandbox mode is the user's assertion and is not verified by
git-paw.

**Test:** `tests/security_posture_docs.rs`

#### Scenario: Declarations are not presented as verified

- **WHEN** the sandboxed full-auto section is inspected
- **THEN** it SHALL state that git-paw does not verify a declared sandbox
