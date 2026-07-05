## ADDED Requirements

### Requirement: Robust uncommitted-work detection

The uncommitted-work check SHALL parse `git status` output in NUL-delimited porcelain form (`--porcelain -z`) rather than splitting on newlines, so that a status entry whose path or content contains whitespace or a newline — including git-paw's own multi-line injected coordination block — can never be misparsed into a phantom changed-file entry. Rename and copy entries, which carry a second NUL-delimited path, SHALL be parsed correctly. The path classification that identifies git-paw-managed files SHALL treat the entire `.git-paw/` subtree as git-paw-managed, in addition to the injected sidecar and the managed `AGENTS.md` block.

#### Scenario: A path containing a newline is not misparsed

- **WHEN** the uncommitted-work check reads `git status` output containing an entry whose path or content includes a newline
- **THEN** that entry is parsed as a single record and is never split into a phantom changed-file entry (such as a `**WARNING:` fragment)

#### Scenario: Clean just-started worktree is not flagged by parse bleed

- **GIVEN** a just-started, otherwise-clean agent worktree carrying only git-paw's injected files
- **WHEN** `git paw remove` runs its uncommitted-work check under load (for example, concurrent test execution)
- **THEN** the check reports no uncommitted user changes and surfaces no git-paw-injected content as a changed path, so removal proceeds without requiring `--force`
