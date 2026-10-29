## ADDED Requirements

### Requirement: Session writes run under an exclusive per-session lock

Every creation, read-modify-write, and deletion of a session's global session file SHALL run while holding an exclusive advisory lock specific to that session, taken on a lock file beside the session files. A read-modify-write SHALL load the session file after acquiring the lock and SHALL NOT save a session value that was loaded before the lock was acquired. The lock SHALL be released when the operation completes, when it fails, and — by the operating system — when the holding process exits for any reason, so a crashed holder never blocks later writers. Lock files SHALL NOT be deleted. Reading a session without modifying it SHALL NOT require the lock.

#### Scenario: Concurrent read-modify-writes lose no update

- **GIVEN** an active session
- **WHEN** two git-paw processes concurrently apply different read-modify-write mutations to it (for example one adds a worktree entry while the other changes the recorded status)
- **THEN** the session file afterwards SHALL reflect both mutations

#### Scenario: A killed lock holder does not block the next writer

- **GIVEN** a process that acquired a session's lock and was then killed with SIGKILL
- **WHEN** another git-paw command mutates the same session
- **THEN** it SHALL acquire the lock and succeed without manual cleanup

#### Scenario: Readers are not blocked by a writer

- **GIVEN** a process holding a session's lock
- **WHEN** `git paw status` reads that session
- **THEN** it SHALL complete without waiting for the lock and SHALL see a complete session file

### Requirement: Lock acquisition is bounded and reports the holder's lock file

A writer that cannot acquire a session's lock SHALL retry for a bounded time of ten seconds and SHALL then fail with a session error that names the session and the lock file and says another git-paw process holds it. No session-file lock SHALL be held across terminal operations, readiness waits, sleeps, or a drive-loop sweep, and when an operation needs both the repository's add/remove lock and a session lock it SHALL acquire the add/remove lock first.

#### Scenario: A held lock times out with an actionable error

- **GIVEN** a test process holding a session's lock for longer than ten seconds
- **WHEN** `git paw pause` is run for that session
- **THEN** the command SHALL fail after about ten seconds with an error naming the session and the lock file
- **AND** the session file SHALL be unchanged

### Requirement: Session file writes use a private temporary file and flush before rename

Each session-file write SHALL serialize to a temporary file whose name is unique to the writing process and call, SHALL flush the temporary file's data to stable storage before renaming it over the session file, and SHALL remove its temporary file if any step fails. Temporary files SHALL never be read as sessions.

#### Scenario: Concurrent writers never share a temporary file

- **GIVEN** two writers saving the same session at the same moment
- **WHEN** both writes complete
- **THEN** the session file SHALL parse as a complete session written by one of them
- **AND** no temporary file SHALL remain

#### Scenario: A failed write leaves no temporary file

- **GIVEN** a session directory in which the final rename fails
- **WHEN** a save is attempted
- **THEN** the save SHALL return an error and no temporary file SHALL remain

### Requirement: The per-repo discovery file is written under the session lock

When an operation rewrites the per-repo discovery file for a session, it SHALL do so while holding that session's lock, after writing the session file and before releasing the lock, so the discovery file never reflects a different state than the session file it was derived from.

#### Scenario: Discovery file and session file agree after concurrent mutations

- **GIVEN** an active session
- **WHEN** two concurrent operations each mutate the session's worktree set and rewrite the discovery file
- **THEN** afterwards the discovery file's agent list SHALL correspond exactly to the session file's worktree entries
