# session-receipt-hygiene Specification

## Purpose
TBD - created by archiving change session-bugfixes-v0-6-x. Update Purpose after archive.
## Requirements
### Requirement: Status distinguishes stale receipts from active sessions

`git paw status` SHALL probe `tmux has-session -t paw-<project>`
when the receipt claims `active`. When the probe shows the tmux
session is absent, status SHALL report `🔴 stale` instead of
`🟢 active`. The system SHALL preserve the existing `🟢 active`
and `🟡 stopped` displays unchanged.

#### Scenario: Active session reports active

- **GIVEN** a session whose receipt says `active` AND
  whose tmux session exists
- **WHEN** the user runs `git paw status`
- **THEN** the output SHALL display `🟢 active`

#### Scenario: Stale receipt reports stale

- **GIVEN** a session whose receipt says `active` AND whose
  tmux session does NOT exist (crash or release-boundary
  carry-over)
- **WHEN** the user runs `git paw status`
- **THEN** the output SHALL display `🔴 stale` (not
  `🟢 active`)

#### Scenario: Stopped session reports stopped

- **GIVEN** a session whose receipt says `stopped`
- **WHEN** the user runs `git paw status`
- **THEN** the output SHALL display `🟡 stopped` regardless
  of tmux liveness

### Requirement: JSON status output adds stale value

The system SHALL extend the JSON output of `git paw status`
with a new `"stale"` value for the `status` field whenever the
liveness probe identifies a stale receipt. The system SHALL
preserve the existing `"active"` and `"stopped"` values from
v0.5.0 without semantic change.

#### Scenario: JSON output reports stale

- **GIVEN** a stale-receipt session
- **WHEN** the user runs `git paw status --json`
- **THEN** the response object SHALL contain
  `"status": "stale"`

### Requirement: start invalidates stale receipts automatically

`git paw start` SHALL run the same `tmux has-session` probe
before deciding whether to recover or launch fresh. When the
receipt claims `active` but the tmux session is absent, the
system SHALL invalidate the receipt (purging the recorded
worktrees + branches equivalent to `git paw purge --force`)
and SHALL emit a stderr notice naming the invalidated entry
before proceeding with the requested launch.

#### Scenario: Stale receipt is invalidated before launch

- **GIVEN** a stale receipt for `paw-myproj`
- **WHEN** the user runs `git paw start` (any flags)
- **THEN** the system SHALL purge the stale receipt's
  worktrees + branches, SHALL emit a stderr notice
  identifying the purged entry, and SHALL proceed with the
  requested launch as if no prior session existed

#### Scenario: Active receipt is NOT invalidated

- **GIVEN** a live session whose receipt says `active` AND
  whose tmux session exists
- **WHEN** the user runs `git paw start` (no recovery flags)
- **THEN** the system SHALL behave per its existing
  reattach-or-error semantics; it SHALL NOT purge anything

#### Scenario: Notice text identifies the purged entry

- **WHEN** the auto-invalidation fires
- **THEN** the stderr notice SHALL include the session name,
  the receipt's `last_seen` timestamp (if present), and a
  one-line explanation that the tmux session no longer
  exists

### Requirement: purge --stale flag

`git paw purge` SHALL accept a `--stale` flag. When passed,
the system SHALL purge only sessions whose receipt is stale
per the probe. Live sessions SHALL be untouched. The flag is
additive to existing `--force`.

#### Scenario: --stale purges only stale entries

- **GIVEN** two sessions on the machine — one active, one
  stale
- **WHEN** the user runs `git paw purge --stale`
- **THEN** the stale session's worktrees + branches +
  receipt SHALL be purged, and the active session SHALL
  remain intact

#### Scenario: --stale with nothing stale exits cleanly

- **GIVEN** no stale receipts on the machine
- **WHEN** the user runs `git paw purge --stale`
- **THEN** the command SHALL exit 0 with a "nothing to
  purge" message, and SHALL NOT touch any active session

#### Scenario: --stale + --force is well-defined

- **GIVEN** a stale receipt
- **WHEN** the user runs `git paw purge --stale --force`
- **THEN** the system SHALL behave equivalently to
  `--stale` alone (the `--force` flag is redundant in this
  combination; explicitly documented as a no-op pairing)

### Requirement: Liveness probe is cheap

The staleness check SHALL be a single `tmux has-session -t
paw-<project>` invocation. The system SHALL NOT probe the
broker, agent processes, or any other liveness signal as part
of the receipt-staleness check.

#### Scenario: Probe runs only one tmux call

- **WHEN** any of `status`, `start`, or `purge --stale`
  performs the staleness check
- **THEN** the resulting process tree SHALL contain exactly
  one `tmux has-session` invocation per session being
  probed

#### Scenario: Probe failure modes are tolerated

- **GIVEN** a system where the `tmux` binary is absent or
  unreachable
- **WHEN** the staleness check runs
- **THEN** the system SHALL treat the probe failure as
  inconclusive (preserve the receipt's current state) and
  SHALL NOT report `🔴 stale` based on a tmux-missing
  failure

