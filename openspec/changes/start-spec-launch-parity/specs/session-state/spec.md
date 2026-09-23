## MODIFIED Requirements

### Requirement: start writes a per-repo session JSON

`git paw start` SHALL write a per-repo session JSON to
`.git-paw/sessions/paw-<project>.json` describing the launched
session. This is the discovery surface the bundled
`sweep.sh` helper reads. The file SHALL be written on launch
and removed on `purge`. "Any flags" includes the spec-driven
launch without supervisor mode (`--specs` / `--from-all-specs`
with no supervisor): every launch path SHALL write the file, so
the helper can discover every session regardless of how it was
started.

#### Scenario: start writes the per-repo JSON

- **WHEN** `git paw start` (any flags) succeeds
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist
  describing the session (name + agent list)

#### Scenario: purge removes the per-repo JSON

- **GIVEN** an active session with the per-repo JSON present
- **WHEN** the user runs `git paw purge --force`
- **THEN** the per-repo JSON SHALL be removed

#### Scenario: A spec-driven launch without supervisor mode writes the per-repo JSON

- **WHEN** `git paw start --specs <names>` succeeds without supervisor mode
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist describing the session (name + agent list)
- **AND** the bundled helper SHALL discover the session from it without relying on running inside the session
