## MODIFIED Requirements

### Requirement: `git paw doctor` runs read-only preflight checks

The system SHALL provide a `git paw doctor` command that inspects the environment,
configuration, repository, and session state and prints a grouped diagnostic report. Each check
SHALL be reported with one of three statuses — ✓ (pass), ⚠ (warning), or ✗ (failure) —
and every non-✓ check SHALL print an actionable remedy. Without `--fix`, the command SHALL NOT
mutate any file, config, session, worktree registration, or other persistent state. With
`--fix` it SHALL mutate only what the "`git paw doctor --fix` repairs only git-paw's own session
records" requirement allows.

#### Scenario: Doctor prints a grouped report

- **GIVEN** a git repository
- **WHEN** `git paw doctor` is run
- **THEN** it SHALL print checks grouped under Environment, CLIs, Config, Spec system, Bundled scripts, Broker, Supervisor, Session, and Hygiene
- **AND** each check SHALL carry a ✓, ⚠, or ✗ status
- **AND** every ⚠ or ✗ check SHALL include a remedy line

#### Scenario: Doctor does not mutate state without --fix

- **GIVEN** a repository with a `.git-paw/` directory and a drifted session record
- **WHEN** `git paw doctor` is run without `--fix`
- **THEN** no file under `.git-paw/`, no session file, and no git worktree registration SHALL be created, modified, or deleted

### Requirement: Hygiene check

The command SHALL verify that the required `.gitignore` entries are present, including `.git-paw/worktrees/` being ignored under child placement, and SHALL report ⚠ with a remedy to run `git paw init` when any is missing. Session-state and worktree-registration checks SHALL be reported by the Session group, not by Hygiene.

#### Scenario: Missing gitignore entry

- **GIVEN** a required `.gitignore` entry is absent
- **WHEN** `git paw doctor` is run
- **THEN** the Hygiene group SHALL report ⚠ with a remedy to run `git paw init`

#### Scenario: Hygiene carries no session checks

- **GIVEN** a session record that claims active but whose terminal session is gone
- **WHEN** `git paw doctor` is run
- **THEN** the drift SHALL be reported in the Session group and not in the Hygiene group

## ADDED Requirements

### Requirement: Doctor reports session-state drift in a Session group

`git paw doctor` SHALL run session-state reconciliation for the current repository's session and SHALL report it in a Session group with one check per drift kind: ✓ when that kind is absent, otherwise ⚠ — or ✗ for a broker serving another repository and for a session file from a newer schema — naming the affected session, branch, or path. Each non-✓ check SHALL carry a remedy that actually resolves it: a session gone while recorded active or paused SHALL offer `git paw doctor --fix` (then `git paw start` to recover) or `git paw purge --stale` (to discard); a recorded agent without a live surface SHALL offer `git paw remove <branch>` then `git paw add <branch>`; a terminal session whose agent CLIs have all exited SHALL offer `git paw start` (to relaunch them) or `git paw stop`; an unreachable broker host SHALL offer `git paw pause` then `git paw start`; an untracked worktree directory SHALL offer inspecting it then `git worktree remove <path>`. When the repository has no session, the Session group SHALL report a single informational ✓.

#### Scenario: A killed terminal session is reported with recovery remedies

- **GIVEN** a session recorded `active` whose terminal session was killed externally
- **WHEN** `git paw doctor` is run
- **THEN** the Session group SHALL report ⚠ for the gone session
- **AND** the remedy SHALL name `git paw doctor --fix` followed by `git paw start`, and `git paw purge --stale` as the discard alternative

#### Scenario: Exited agent CLIs in a live terminal session are reported

- **GIVEN** a session whose terminal session is up but whose agent CLIs have all exited past the launch grace window
- **WHEN** `git paw doctor` is run
- **THEN** the Session group SHALL report ⚠ with a remedy naming `git paw start` and `git paw stop`
- **AND** it SHALL NOT report the session as gone and SHALL NOT recommend `git paw purge --stale`

#### Scenario: A hand-deleted worktree is reported

- **GIVEN** a session whose recorded worktree directory for `feat/x` was removed by hand
- **WHEN** `git paw doctor` is run
- **THEN** the Session group SHALL report ⚠ naming `feat/x` and its path

#### Scenario: A broker serving another repository fails the check

- **GIVEN** a live session whose recorded broker port answers for a different repository
- **WHEN** `git paw doctor` is run
- **THEN** the Session group SHALL report ✗ with a remedy to set a distinct `[broker] port`
- **AND** the process SHALL exit non-zero

#### Scenario: No session is informational

- **GIVEN** a repository with no session
- **WHEN** `git paw doctor` is run
- **THEN** the Session group SHALL contain one ✓ stating that there is no session to reconcile

### Requirement: `git paw doctor --fix` repairs only git-paw's own session records

`git paw doctor` SHALL accept `--fix`. With it, after running every check, the command SHALL apply exactly these repairs and no others: mark a session recorded `active` or `paused` whose terminal session is determinately gone as `stopped`; when the terminal session is gone, drop the record entries (and discovery-file rows) of worktrees whose directories no longer exist; run `git worktree prune` only when every registration its dry run would remove is git-paw-owned (recorded by the session or located under `.git-paw/worktrees/`); rewrite the per-repo discovery file of a live session from the session record and the terminal backend's surface listing; and delete the session's own leftover temp files older than sixty seconds. Every session-file and discovery-file change SHALL be made under the session lock against a freshly read record. `--fix` SHALL NOT delete or move any worktree directory or branch, SHALL NOT start, stop, or signal any process, SHALL NOT modify configuration, SHALL NOT prune a registration git-paw does not own, and SHALL NOT act on another repository's session; a drift outside the repair set SHALL remain reported with its remedy.

#### Scenario: A dead active record becomes stopped

- **GIVEN** a session recorded `active` whose terminal session is gone
- **WHEN** `git paw doctor --fix` is run
- **THEN** the session record SHALL have status `stopped`
- **AND** every recorded worktree directory and branch SHALL still exist

#### Scenario: A missing worktree entry is dropped when the session is down

- **GIVEN** a session whose terminal session is gone and whose record lists `feat/x` with a directory removed by hand
- **WHEN** `git paw doctor --fix` is run
- **THEN** `feat/x` SHALL be removed from the session record and from the discovery file

#### Scenario: A missing worktree entry of a live session is kept and reported

- **GIVEN** a live session whose record lists `feat/y` with a directory removed by hand
- **WHEN** `git paw doctor --fix` is run
- **THEN** `feat/y` SHALL remain in the session record
- **AND** it SHALL stay reported with a remedy to stop the session and re-run `git paw doctor --fix`

#### Scenario: A foreign stale registration blocks the prune

- **GIVEN** a stale git-paw worktree registration and also a stale registration for a worktree the user created outside git-paw
- **WHEN** `git paw doctor --fix` is run
- **THEN** `git worktree prune` SHALL NOT be run
- **AND** the user's registration SHALL still be listed by `git worktree list`
- **AND** the git-paw registration SHALL stay reported with a remedy to run `git worktree prune`

#### Scenario: An untracked dirty worktree directory is never touched

- **GIVEN** an untracked directory under `.git-paw/worktrees/` containing uncommitted files
- **WHEN** `git paw doctor --fix` is run
- **THEN** the directory and its files SHALL be unchanged
- **AND** it SHALL stay reported

#### Scenario: A stale discovery file is rewritten for a live session

- **GIVEN** a live session whose discovery file is missing
- **WHEN** `git paw doctor --fix` is run
- **THEN** the discovery file SHALL exist and list one agent per recorded worktree

#### Scenario: No process is started or stopped

- **GIVEN** a live session whose broker host is unreachable
- **WHEN** `git paw doctor --fix` is run
- **THEN** no process SHALL be started or signalled
- **AND** the broker drift SHALL stay reported with its remedy

### Requirement: `git paw doctor --fix` re-checks and reports its repairs

After applying repairs, `git paw doctor --fix` SHALL print each repair with its outcome (applied, skipped, or failed, with a reason), SHALL re-run the Session group, and SHALL report and exit according to the re-checked results: non-zero when any check, including any re-checked Session check, is ✗. A failed repair SHALL NOT stop the remaining repairs. With `--json`, the doctor document SHALL additionally contain a `repairs` array whose entries carry `id`, `action`, `target`, `outcome`, and `detail`, and its `checks` SHALL reflect the re-check.

#### Scenario: Repairs are listed and the re-check is clean

- **GIVEN** a session recorded `active` whose terminal session is gone and whose state has no other drift
- **WHEN** `git paw doctor --fix` is run
- **THEN** the output SHALL list the status repair as applied
- **AND** the re-checked Session group SHALL report ✓ for the gone-session check

#### Scenario: JSON lists repairs

- **WHEN** `git paw doctor --fix --json` is run against a drifted session
- **THEN** the document SHALL contain a `repairs` array with one entry per attempted repair carrying `id`, `action`, `target`, `outcome`, and `detail`

## REMOVED Requirements

### Requirement: Diagnose-only (no `--fix` in v0.13.0)
**Reason**: Superseded. v0.13.0 deferred repair to a later cycle, and this change adds `--fix` with a closed, records-only repair set (see "`git paw doctor --fix` repairs only git-paw's own session records"). Without `--fix`, doctor remains strictly read-only.
**Migration**: None. `git paw doctor` without flags behaves as before. Pass `--fix` to apply session-state repairs.
