## REMOVED Requirements

### Requirement: README Supervisor Mode quick start documents the flags

**Reason**: The README keeps a single minimal quick start (see `core-project-conventions` "README is a landing page that forwards detail to the documentation"); the supervised-session quick start lives in the mdBook, where the relocation to an external orchestrator (v0.18) is also explained.
**Migration**: `--no-supervisor` and `start --force` are documented in `docs/src/cli-reference.md` and in the supervised quick-start chapter (`docs/src/quick-start-supervisor.md` or its v0.18 replacement), which the README links to.
