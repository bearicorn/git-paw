## ADDED Requirements

### Requirement: Session state location is authoritative

Session state files SHALL be stored under the platform data directory resolved by the system's own directory resolution — `~/Library/Application Support/git-paw/sessions/` on macOS, and `$XDG_DATA_HOME/git-paw/sessions/` (falling back to `~/.local/share/git-paw/sessions/`) elsewhere. This requirement is the single authority on that location.

Session state SHALL NOT be stored in a repository-relative directory. A repository may contain a `.git-paw/sessions/` directory for other per-repository artifacts, and its presence SHALL NOT be taken to indicate where session state lives.

Where another capability mentions a session-state path incidentally, while specifying something else, that mention SHALL be treated as non-normative and this requirement SHALL prevail. Any component that needs to locate session state — including bundled helper scripts — SHALL resolve it through the same resolution this requirement names, rather than hardcoding a path that happens to work in one layout.

#### Scenario: Session state resolves to the platform data directory

- **WHEN** a session is saved
- **THEN** its state file SHALL be written under the platform data directory's `git-paw/sessions/` path
- **AND** SHALL NOT be written under the repository's own directory

#### Scenario: A repo-relative sessions directory is not consulted

- **GIVEN** a repository containing an empty `.git-paw/sessions/` directory
- **WHEN** a component locates session state for that repository
- **THEN** it SHALL resolve the platform data directory
- **AND** SHALL NOT report that no session exists merely because the repo-relative directory is empty

#### Scenario: An incidental path mention elsewhere does not override this

- **WHEN** another capability's requirement mentions a session-state path while specifying different behaviour
- **THEN** that mention SHALL be non-normative
- **AND** implementations SHALL follow this requirement
