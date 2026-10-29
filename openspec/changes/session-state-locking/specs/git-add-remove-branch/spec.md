## ADDED Requirements

### Requirement: The add/remove lock is released when its holder exits

`git paw add` and `git paw remove` SHALL serialize against each other with a non-blocking exclusive advisory lock on `.git-paw/.add-remove.lock`. When the lock is held by a running `add` or `remove`, the second invocation SHALL fail immediately with an "operation in progress" error naming the lock file, as before. The lock SHALL be released by the operating system when its holder exits, including on a crash, so a later `add` or `remove` SHALL NOT require manual removal of the lock file. The lock file SHALL persist between invocations.

#### Scenario: A concurrent add is refused

- **GIVEN** a `git paw add` that holds the add/remove lock
- **WHEN** a second `git paw add` or `git paw remove` starts in the same repository
- **THEN** it SHALL fail immediately with an error that says an operation is in progress and names `.add-remove.lock`

#### Scenario: A crashed add does not block the next one

- **GIVEN** a process that acquired the add/remove lock and was killed with SIGKILL
- **WHEN** `git paw add feat/y` is run
- **THEN** it SHALL acquire the lock and proceed without the user removing any file

### Requirement: add and remove read session state only under their lock

`git paw add` and `git paw remove` SHALL acquire the add/remove lock before reading the session state they modify, and SHALL apply their session mutation as a read-modify-write under the session lock, so a session change made by another process between invocation and lock acquisition is never overwritten.

#### Scenario: A pause made before remove takes its lock is preserved

- **GIVEN** an active session, and a `git paw pause` that completes after `git paw remove feat/x` started but before it acquired its locks
- **WHEN** `git paw remove feat/x` completes
- **THEN** the session file SHALL record status `paused`
- **AND** SHALL no longer contain `feat/x`
