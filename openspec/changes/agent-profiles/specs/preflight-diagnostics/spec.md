## ADDED Requirements

### Requirement: Doctor evaluates worker profiles at their own approval level

`git paw doctor`'s sandbox-mode check SHALL also report the CLI of every configured worker profile, evaluated at that profile's effective approval level (its `approval`, else the worker default) rather than only the session-wide level, and its spec-CLI check SHALL resolve a spec's `paw_profile` to that profile's CLI. Both SHALL stay read-only.

#### Scenario: A full-auto worker profile on an unsandboxed CLI warns

- **GIVEN** `agent_approval = "manual"`, no `[clis.claude].sandbox`, and `[profiles.trusted]` with `kind = "worker"`, `cli = "claude"`, `approval = "full-auto"`
- **WHEN** `git paw doctor` runs
- **THEN** the sandbox-mode check SHALL report ⚠ naming `claude` with a remedy pointing at the sandbox documentation
- **AND** it SHALL NOT report ✗

#### Scenario: A spec's profile resolves to its CLI

- **GIVEN** a discovered spec with `paw_profile: routine` and `[profiles.routine]` with `kind = "worker"`, `cli = "beta"`
- **WHEN** `git paw doctor` runs
- **THEN** the spec-CLI check SHALL report `beta` for that spec
