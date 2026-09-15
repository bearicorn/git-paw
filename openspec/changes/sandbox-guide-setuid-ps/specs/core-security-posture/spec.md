## ADDED Requirements

### Requirement: The sandbox chapter documents what the FS sandbox breaks

The FS-scoped sandbox chapter SHALL carry a "what breaks under the sandbox" note, adjacent to the heredoc temp-dir explanation, stating that setuid/setgid binaries cannot be executed under `sandbox-exec` — naming macOS `/bin/ps` (and `/usr/bin/top`) — with the observed symptom (`execvp() … Operation not permitted`, surfacing to an agent as a bare `operation not permitted: ps`), that no profile allow-rule fixes it because the refusal is not a file-write denial, that it is expected behaviour rather than a mis-tuned profile, and which non-setuid alternatives work (`pgrep`). The note SHALL add that on Linux `bwrap` runs setuid binaries without their elevated privileges while `ps` itself is not setuid there and works, and SHALL point out that the bundled worker-orientation skill already tells agents the same thing.

#### Scenario: The sandbox chapter documents the setuid exec refusal

- **WHEN** the FS-scoped sandbox chapter is inspected
- **THEN** it SHALL contain a note that setuid binaries such as macOS `/bin/ps` fail under `sandbox-exec` with `Operation not permitted`
- **AND** it SHALL state that no profile rule fixes this and that it is expected
- **AND** it SHALL name `pgrep` as a working non-setuid alternative

#### Scenario: The note sits beside the heredoc explanation and covers Linux

- **WHEN** the FS-scoped sandbox chapter is inspected
- **THEN** the setuid note SHALL appear in the same "what breaks" context as the `/private/tmp` heredoc grant explanation
- **AND** it SHALL state how `bwrap` treats setuid binaries on Linux and that `ps` works there

#### Scenario: The doc test guards the note

- **WHEN** `tests/security_posture_docs.rs` runs against a sandbox chapter lacking the setuid note
- **THEN** the sandbox-chapter test SHALL fail, naming the missing needle
