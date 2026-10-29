## ADDED Requirements

### Requirement: Security posture documents the A2A face

The security-posture chapter SHALL document the A2A face. It SHALL state that the face is off by default and is served only on a loopback bind. It SHALL state that every A2A operation requires the session's root-authority credential, as a bearer token obtained out of band with `git paw __authority`. It SHALL list what an A2A client can do: read every worker task, send guidance, record verdicts, and mark a task cancelled. It SHALL list what an A2A client cannot do: create tasks, start, stop or kill agents, or touch worktrees or branches. It SHALL state that guidance and feedback text is read by agents with execution power, with the same trust as the root's other paths. It SHALL state that a holder of the credential is the root; the credential is not isolation from an unsandboxed process that can read the operator's home directory.

#### Scenario: Posture chapter states the A2A privileges and limits

- **WHEN** `docs/src/user-guide/security-posture.md` is read
- **THEN** it SHALL state that A2A is off by default and loopback-only
- **AND** it SHALL state that every operation requires the root credential
- **AND** it SHALL list the allowed operations and the forbidden ones (task creation, process control, worktree changes)
- **AND** it SHALL state the prompt-injection trust and the credential's threat-model limit
