## ADDED Requirements

### Requirement: The posture documents headless workers

The security posture chapter SHALL state, for the headless backend: that git-paw approves nothing for any worker on any backend, and that a headless worker shows no permission prompt for git-paw to escalate, so an action outside its CLI's own policy is refused rather than asked about; that what a headless worker may do is decided only by its CLI's own permission policy, which the eligibility gate requires to be declared; that `full-auto` headless workers must declare a sandbox and that such a declaration is not verified by git-paw; the residual risks of sandboxed `full-auto` workers running with nobody watching and after the launching terminal closes (network egress, shared repository refs, pushes with the user's credentials, secret reads without read-hardening); that a process a worker detaches into its own session can outlive `git paw stop`; and which environment a turn inherits. It SHALL recommend the read-hardening sandbox tier for headless `full-auto` workers.

#### Scenario: The posture states that nothing is approved or escalated for a headless worker

- **WHEN** the security posture chapter is read
- **THEN** it SHALL state that git-paw approves nothing for any worker
- **AND** it SHALL state that a headless worker raises no prompt to escalate, so an action outside its CLI's policy is refused
- **AND** it SHALL state that a declared sandbox is not verified by git-paw

#### Scenario: The posture names the headless residual risks

- **WHEN** the security posture chapter's headless section is read
- **THEN** it SHALL name network egress, shared repository refs, pushes with the user's credentials and secret reads as residual risks
- **AND** it SHALL recommend the read-hardening tier
