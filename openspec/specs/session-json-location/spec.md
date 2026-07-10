# session-json-location Specification

## Purpose
Has `git paw start` write (and `purge` remove) a per-repo session JSON at `.git-paw/sessions/paw-<project>.json` — carrying the session name and each agent's branch_id, worktree path, CLI, and pane index — as the discovery surface the bundled `sweep.sh` helper reads, with a live-tmux fallback (`$TMUX` / `tmux display-message`) when the file is absent.

## Requirements
### Requirement: start writes a per-repo session JSON

`git paw start` SHALL write a per-repo session JSON to
`.git-paw/sessions/paw-<project>.json` describing the launched
session. This is the discovery surface the bundled
`sweep.sh` helper reads. The file SHALL be written on launch
and removed on `purge`.

#### Scenario: start writes the per-repo JSON

- **WHEN** `git paw start` (any flags) succeeds
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist
  describing the session (name + agent list)

#### Scenario: purge removes the per-repo JSON

- **GIVEN** an active session with the per-repo JSON present
- **WHEN** the user runs `git paw purge --force`
- **THEN** the per-repo JSON SHALL be removed

### Requirement: Per-repo JSON shape matches sweep.sh expectations

The per-repo `paw-<project>.json` SHALL include the session
name plus an agent list, each entry carrying `branch_id`,
`worktree_path`, `cli`, and `pane_index`. The shape SHALL
match what the bundled `assets/scripts/sweep.sh` helper reads
so the helper works against the file without modification.

#### Scenario: sweep.sh discovers agents via the per-repo JSON

- **GIVEN** a freshly-started session and the bundled
  `sweep.sh` helper invoked from the supervisor pane
- **WHEN** sweep.sh reads
  `.git-paw/sessions/paw-<project>.json`
- **THEN** the helper SHALL enumerate the full agent list
  with branch_id, worktree path, CLI, and pane index — no
  fields missing or renamed relative to its expectations

#### Scenario: Adding a field is backwards-compatible

- **WHEN** a future change adds a new field to the per-repo
  JSON
- **THEN** the current `sweep.sh` SHALL still find every
  documented field; unknown extra fields SHALL be ignored

### Requirement: sweep.sh falls back to live tmux when the file is absent

The bundled `sweep.sh` SHALL fall back to discovering the
session name from the `$TMUX` environment variable, or
`tmux display-message -p '#S'`, when
`.git-paw/sessions/paw-<project>.json` is absent (e.g. the
supervisor attached to a pre-existing `paw-*` session created
outside the normal `git paw start` flow). The helper SHALL
NOT require manual authoring of the session JSON.

#### Scenario: Helper discovers the session with no JSON present

- **GIVEN** a `paw-myproj` tmux session is active but
  `.git-paw/sessions/paw-myproj.json` does not exist
- **WHEN** `sweep.sh` runs inside the session
- **THEN** the helper SHALL resolve the session name via
  `$TMUX` / `tmux display-message -p '#S'` and operate
  without error, instead of failing to discover the session

#### Scenario: Per-repo JSON takes precedence when present

- **GIVEN** both the per-repo JSON and a live tmux session
  exist
- **WHEN** `sweep.sh` runs
- **THEN** the helper SHALL prefer the per-repo JSON's agent
  list (richer — it carries worktree paths + pane indices)
  over the bare session-name fallback

