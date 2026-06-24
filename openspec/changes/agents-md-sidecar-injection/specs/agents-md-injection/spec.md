## MODIFIED Requirements

### Requirement: Inject section into file

The system SHALL read the injection-target file, inject the section, and write the result back. The injection target SHALL be a gitignored sidecar instruction file (e.g. `.git-paw/AGENTS.local.md`), NOT the worktree's tracked `AGENTS.md`. The system SHALL NOT set `git update-index --assume-unchanged` on the tracked `AGENTS.md`.

#### Scenario: File exists without git-paw section
- **WHEN** `inject_section_into_file()` is called on a file without a git-paw section
- **THEN** the section SHALL be appended and the file written

#### Scenario: File exists with git-paw section
- **WHEN** `inject_section_into_file()` is called on a file with an existing git-paw section
- **THEN** the section SHALL be replaced and the file written

#### Scenario: File does not exist
- **WHEN** `inject_section_into_file()` is called with a path that does not exist
- **THEN** the file SHALL be created containing only the section

#### Scenario: File is not writable
- **WHEN** `inject_section_into_file()` is called on a read-only file
- **THEN** it SHALL return `PawError::AgentsMdError` with a message mentioning the file path

#### Scenario: Injection target is the sidecar, not the tracked AGENTS.md
- **WHEN** the managed git-paw block is injected during worktree setup
- **THEN** the block SHALL be written to the gitignored sidecar instruction file
- **AND** the worktree's tracked `AGENTS.md` SHALL NOT contain the managed git-paw block written by git-paw

#### Scenario: Tracked AGENTS.md is not marked assume-unchanged
- **WHEN** worktree setup completes
- **THEN** the system SHALL NOT have run `git update-index --assume-unchanged AGENTS.md`
- **AND** a hand edit to the tracked `AGENTS.md` SHALL appear in `git status`
