## Purpose

Persist session state to disk for recovery after crashes, reboots, or manual stops. Stores one JSON file per session under the XDG data directory, with atomic writes and tmux liveness checks. It also has `git paw start` write (and `purge` remove) a per-repo session JSON at `.git-paw/sessions/paw-<project>.json` — carrying the session name and each agent's branch_id, worktree path, CLI, and pane index — as the discovery surface the bundled `sweep.sh` helper reads, with a live-tmux fallback when the file is absent; distinguishes stale session receipts from live ones via a single cheap `tmux has-session` probe (`status` reports `🔴 stale`, `start` auto-invalidates stale receipts before launching, and `purge --stale` prunes only stale entries); and guarantees session recovery rebuilds exactly the panes a session defines (`N + 2` in supervisor mode) on a headless canvas sized to tile them without overflow, preserving the persisted session JSON when a `git paw start` errors mid-launch. It also renders a Markdown session summary from `BrokerState` and `PawSession` — metadata, per-agent details (status, modified files, exports, blocked time), merge order, and totals — which the supervisor writes to a timestamped, filesystem-safe file under `.git-paw/sessions/` so multiple runs coexist without overwriting.
## Requirements
### Requirement: Save session state atomically

The system SHALL serialize session data to JSON and write it atomically using a temp file and rename to prevent corruption.

The session data SHALL include optional broker fields: `broker_port` (`Option<u16>`), `broker_bind` (`Option<String>`), and `broker_log_path` (`Option<PathBuf>`). These fields SHALL be omitted from the JSON when `None` and SHALL default to `None` when absent during deserialization.

`broker_log_path` names one log per session — `<session_state_dir>/<session_name>.broker.log` — so two repositories on one machine never write to, or purge, the same file. A session saved by an earlier git-paw records the shared `<session_state_dir>/broker.log` instead; that value SHALL continue to load and remain authoritative for both the broker and `purge` until a resume or recovery re-stamps it.

#### Scenario: Saved session round-trips with all fields intact
- **GIVEN** an active session with 3 worktrees
- **WHEN** `save_session()` is called and the session is loaded back
- **THEN** all fields (session_name, repo_path, project_name, created_at, status, worktrees) SHALL match the original

#### Scenario: Saved session with broker fields round-trips
- **GIVEN** an active session with `broker_port = Some(9119)`, `broker_bind = Some("127.0.0.1")`, `broker_log_path = Some("/path/to/paw-my-project.broker.log")`
- **WHEN** `save_session()` is called and the session is loaded back
- **THEN** all broker fields SHALL match the original

#### Scenario: Session without broker fields loads successfully
- **GIVEN** a session JSON file saved by v0.2.0 (no broker fields)
- **WHEN** the session is loaded
- **THEN** `broker_port`, `broker_bind`, and `broker_log_path` SHALL all be `None`
- **AND** all existing fields SHALL load correctly

#### Scenario: A session recording the old shared log name still loads
- **GIVEN** a session JSON saved by an earlier git-paw with `broker_log_path = Some("<session_state_dir>/broker.log")`
- **WHEN** the session is loaded
- **THEN** `broker_log_path` SHALL be that recorded path
- **AND** it SHALL be the path both the broker writes and `purge` deletes

#### Scenario: Saving again replaces previous state
- **GIVEN** a previously saved session
- **WHEN** `save_session()` is called with updated fields
- **THEN** the new state SHALL overwrite the old state

### Requirement: Load session by name

The system SHALL load a session from disk by name, returning `None` if the file does not exist.

#### Scenario: Loading a nonexistent session returns None
- **GIVEN** no session file exists with the given name
- **WHEN** `load_session()` is called
- **THEN** it SHALL return `Ok(None)`

Test: `session::tests::loading_nonexistent_session_returns_none`

### Requirement: Find session by repository path

The system SHALL scan all session files and return the session matching a given repository path.

The default lookup SHALL match a session's repository path EXACTLY: a path that is the root of — or lies inside — a linked worktree SHALL NOT resolve to the owning repository's session. This exactness is a safety boundary. Every command that MUTATES session state (`stop`, `purge`, `add`, `remove`, `pause`, and `start`'s recover/resume dispatch) resolves its repository to the root of the checkout it is run from, which inside an agent worktree is that WORKTREE's root; were the default lookup to resolve the owner, such a command run by an agent in its own worktree would act on the whole session — deleting sibling worktrees and branches, or killing the session.

The system SHALL additionally provide a SEPARATE, read-only owner-resolving lookup that returns the session of the repository that OWNS a given path, by resolving a linked worktree to that repository. It SHALL try the exact match first and resolve the owner only on a miss. It SHALL be used ONLY by the MCP server's repository context and its read-only session view, so a `git paw mcp` spawned by an agent's CLI from inside the agent's worktree with no `--repo` finds the session and its broker. No mutating command SHALL use it.

#### Scenario: Finds correct session among multiple
- **GIVEN** two sessions for different repositories
- **WHEN** `find_session_for_repo()` is called with one repo path
- **THEN** it SHALL return the matching session

Test: `session::tests::finds_correct_session_among_multiple_by_repo_path`

#### Scenario: No matching session
- **GIVEN** saved sessions for other repositories
- **WHEN** `find_session_for_repo()` is called with a different path
- **THEN** it SHALL return `None`

Test: `session::tests::find_returns_none_when_no_repo_matches`

#### Scenario: No sessions directory
- **GIVEN** no sessions directory exists
- **WHEN** `find_session_for_repo()` is called
- **THEN** it SHALL return `None`

Test: `session::tests::find_returns_none_when_no_sessions_exist`

#### Scenario: A worktree root resolves to its repository's session

- **GIVEN** an active session for a repository and one of that session's agent worktrees
- **WHEN** `find_session_for_repo_or_owner()` is called with the worktree's root
- **THEN** it SHALL return the repository's session

#### Scenario: The owner-resolving lookup prefers an exact match

- **GIVEN** a session recorded for an agent worktree's own root and another for the repository that owns it
- **WHEN** `find_session_for_repo_or_owner()` is called with the worktree's root
- **THEN** it SHALL return the worktree's own session, not the owner's

#### Scenario: The default lookup does not cross the worktree boundary

- **GIVEN** an active session for a repository and one of that session's agent worktrees
- **WHEN** `find_session_for_repo()` is called with the worktree's root
- **THEN** it SHALL return `None`

#### Scenario: A mutating command run inside an agent worktree does not act on the session

- **GIVEN** an active session and one of its agent worktrees
- **WHEN** `git paw stop` or `git paw purge --force` is run with that worktree as the working directory
- **THEN** the session SHALL remain alive, its branches and its sibling agents' worktrees SHALL remain, and the command SHALL report that there is no session for this repository

#### Scenario: The MCP server started in a worktree finds the broker

- **GIVEN** an active broker-enabled session and `git paw mcp` started with its working directory inside an agent worktree and no `--repo`
- **WHEN** the server resolves its repository context
- **THEN** its broker URL SHALL be the session's broker URL, not absent

### Requirement: Delete session by name

The system SHALL delete a session file, succeeding even if the file does not exist (idempotent).

#### Scenario: Deleted session is no longer loadable
- **GIVEN** a saved session
- **WHEN** `delete_session()` is called
- **THEN** `load_session()` SHALL return `None`

Test: `session::tests::deleted_session_is_no_longer_loadable`

#### Scenario: Deleting nonexistent session succeeds
- **GIVEN** no session file with the given name
- **WHEN** `delete_session()` is called
- **THEN** it SHALL return `Ok(())`

Test: `session::tests::deleting_nonexistent_session_succeeds`

### Requirement: Effective status combines file state with tmux liveness

`Session::effective_status(is_tmux_alive)` SHALL combine the persisted `status` field with the result of `is_tmux_alive` to produce the runtime-effective status:

| Recorded status | tmux alive? | Effective status |
|-----------------|-------------|------------------|
| `Active`        | yes         | `Active`         |
| `Active`        | no          | `Stopped`        |
| `Paused`        | yes         | `Paused`         |
| `Paused`        | no          | `Stopped`        |
| `Stopped`       | any         | `Stopped`        |

The rule for `Paused`: tmux must still be alive for the `Paused` state to be valid — pause's whole purpose is to keep tmux + CLI panes running while the client is detached. If tmux died despite a recorded `Paused` state (e.g. tmux server crash), `effective_status` SHALL downgrade to `Stopped`, and `cmd_start` SHALL run the cold-recovery path (fresh CLI spawn) rather than the restart-from-pause path.

#### Scenario: Active + alive remains Active

- **GIVEN** a session with `status = Active`
- **WHEN** `effective_status(|_| true)` is called
- **THEN** it SHALL return `Active`

#### Scenario: Active + dead downgrades to Stopped

- **GIVEN** a session with `status = Active`
- **WHEN** `effective_status(|_| false)` is called
- **THEN** it SHALL return `Stopped`

#### Scenario: Paused + alive remains Paused

- **GIVEN** a session with `status = Paused`
- **WHEN** `effective_status(|_| true)` is called
- **THEN** it SHALL return `Paused`

#### Scenario: Paused + dead downgrades to Stopped

- **GIVEN** a session with `status = Paused`
- **WHEN** `effective_status(|_| false)` is called
- **THEN** it SHALL return `Stopped`

#### Scenario: Stopped remains Stopped regardless of tmux liveness

- **GIVEN** a session with `status = Stopped`
- **WHEN** `effective_status` is called with either liveness result
- **THEN** it SHALL return `Stopped`

### Requirement: SessionStatus display format

The `SessionStatus` enum SHALL display as lowercase strings.

#### Scenario: SessionStatus display strings
- **GIVEN** `SessionStatus::Active` and `SessionStatus::Stopped`
- **WHEN** formatted with `Display`
- **THEN** they SHALL render as `"active"` and `"stopped"`

Test: `session::tests::session_status_displays_as_lowercase_string`

### Requirement: Recovery data survives tmux crashes

After a tmux crash, the persisted session SHALL contain all data needed to reconstruct the session.

#### Scenario: Crashed session has all recovery data including broker fields
- **GIVEN** a saved session with worktrees and broker enabled
- **WHEN** tmux crashes and the session is loaded from disk
- **THEN** it SHALL have the session name, repo path, all worktree details, AND broker_port, broker_bind, broker_log_path

#### Scenario: Session recovery recreates dashboard pane when broker was enabled
- **GIVEN** a saved session with `broker_port = Some(9119)` and `broker_bind = Some("127.0.0.1")`
- **WHEN** `recover_session()` is called
- **THEN** the rebuilt tmux session SHALL have:
  - Dashboard pane in pane 0 running `git-paw __dashboard`
  - `GIT_PAW_BROKER_URL` environment variable set to `http://127.0.0.1:9119`
  - All original worktree panes in subsequent indices

#### Scenario: Session recovery uses original broker config, not current config
- **GIVEN** a saved session with `broker_port = Some(9119)`
- **AND** current repo config has `broker.enabled = false`
- **WHEN** `recover_session()` is called
- **THEN** the dashboard pane SHALL still be created with the original broker URL

#### Scenario: Session recovery without original broker creates no dashboard
- **GIVEN** a saved session with `broker_port = None`
- **WHEN** `recover_session()` is called
- **THEN** no dashboard pane SHALL be created

### Requirement: Session persistence SHALL work through the public API

#### Scenario: Save and load round-trip
- **GIVEN** a session with 2 worktrees
- **WHEN** `save_session_in()` and `load_session_from()` are called
- **THEN** all fields SHALL match

Test: `session_integration::save_and_load_round_trip`

#### Scenario: Find session by repo path
- **GIVEN** a saved session
- **WHEN** `find_session_for_repo_in()` is called with the matching repo path
- **THEN** the correct session SHALL be returned

Test: `session_integration::find_session_by_repo_path`

#### Scenario: Find returns None for unknown repo
- **GIVEN** no matching session
- **WHEN** `find_session_for_repo_in()` is called
- **THEN** it SHALL return `None`

Test: `session_integration::find_session_returns_none_for_unknown_repo`

#### Scenario: Find correct session among multiple
- **GIVEN** two sessions for different repos
- **WHEN** `find_session_for_repo_in()` is called for one
- **THEN** the correct session SHALL be returned

Test: `session_integration::find_correct_session_among_multiple`

#### Scenario: Delete removes session
- **GIVEN** a saved session
- **WHEN** `delete_session_in()` is called
- **THEN** `load_session_from()` SHALL return `None`

Test: `session_integration::delete_removes_session`

#### Scenario: Delete nonexistent is idempotent
- **GIVEN** no session file
- **WHEN** `delete_session_in()` is called
- **THEN** it SHALL succeed

Test: `session_integration::delete_nonexistent_is_idempotent`

#### Scenario: Load nonexistent returns None
- **GIVEN** no session file
- **WHEN** `load_session_from()` is called
- **THEN** it SHALL return `None`

Test: `session_integration::load_nonexistent_returns_none`

#### Scenario: Saving again replaces previous state
- **GIVEN** a saved session
- **WHEN** the status is changed and saved again
- **THEN** the loaded session SHALL have the new status

Test: `session_integration::saving_again_replaces_previous_state`

#### Scenario: Effective status active when tmux alive
- **GIVEN** a session with `Active` status and tmux alive
- **WHEN** `effective_status()` is called
- **THEN** it SHALL return `Active`

Test: `session_integration::effective_status_active_when_tmux_alive`

#### Scenario: Effective status stopped when tmux dead
- **GIVEN** a session with `Active` status and tmux dead
- **WHEN** `effective_status()` is called
- **THEN** it SHALL return `Stopped`

Test: `session_integration::effective_status_stopped_when_tmux_dead`

#### Scenario: Effective status stopped stays stopped
- **GIVEN** a session with `Stopped` status
- **WHEN** `effective_status()` is called
- **THEN** it SHALL return `Stopped` regardless of tmux

Test: `session_integration::effective_status_stopped_stays_stopped`

#### Scenario: Saved session has all recovery fields
- **GIVEN** a saved and reloaded session
- **WHEN** recovery fields are checked
- **THEN** session_name, repo_path, project_name, and all worktree entries SHALL be non-empty

Test: `session_integration::saved_session_has_all_recovery_fields`

### Requirement: Paused session status variant

The `SessionStatus` enum SHALL include a third variant `Paused` (alongside `Active` and `Stopped`). The serde representation SHALL serialize as the lowercase string `"paused"` and SHALL deserialize from the same string. The `Display` implementation SHALL render `Paused` as `"paused"`.

The `Paused` state means: the tmux session is intended to remain alive, all coding-agent CLI panes are intended to remain running, the user's tmux client is detached, and the broker is stopped. Session state files saved by v0.4.0 binaries (which only know `Active` and `Stopped`) SHALL continue to load successfully under v0.5+ binaries — the new variant only appears in files saved by v0.5+.

#### Scenario: Paused status serializes lowercase

- **GIVEN** a `Session` with `status = SessionStatus::Paused`
- **WHEN** `save_session()` is called and the JSON file is inspected
- **THEN** the `"status"` field SHALL be `"paused"`

#### Scenario: Paused status round-trips

- **GIVEN** a `Session` with `status = SessionStatus::Paused` saved to disk
- **WHEN** the session is loaded back via `load_session()`
- **THEN** `status` SHALL be `SessionStatus::Paused`

#### Scenario: v0.4-saved sessions load under v0.5

- **GIVEN** a session JSON file saved by v0.4.0 (only `"active"` or `"stopped"` in the status field)
- **WHEN** the file is loaded by a v0.5+ binary
- **THEN** the load SHALL succeed
- **AND** the `status` field SHALL match the original (`Active` or `Stopped`)

#### Scenario: Paused Display renders lowercase

- **WHEN** `format!("{}", SessionStatus::Paused)` is evaluated
- **THEN** the result SHALL be `"paused"`

### Requirement: Dashboard pane index persisted in session state

The `Session` struct SHALL include an optional field `dashboard_pane: Option<u32>` that records the pane index of the dashboard pane within the tmux session. The field SHALL use `#[serde(default, skip_serializing_if = "Option::is_none")]` so sessions saved without it load with `None`. The field SHALL be populated with `Some(0)` by every supervised start flow (the supervised layout always places the dashboard at pane 0) and by every bare start flow that enables the broker, because the dashboard is pane 0 in both layouts; only a bare start without the broker records `None`.

Consumers that need the first coding-agent pane index or the dashboard index (`status`, `stop`, `pause`, restart from pause, `remove`, liveness, recovery, the drive loop) SHALL derive them from the session's layout record, which reads the RECORDED `dashboard_pane`. When it is `Some(n)`, the dashboard is pane `n` and the first coding agent is pane `n + 1`. A session saved by a pre-relocation binary with `dashboard_pane = Some(1)` (supervisor at 0, dashboard at 1, agents from 2) therefore keeps resolving its agent panes from index 2 while it is still alive after an upgrade. When the field is `None`, the layout record SHALL apply the earlier (pre-relocation) rules, so live sessions saved by older binaries keep resolving: a supervised session's coding agents start at pane 2, a bare session's at pane 1 when it recorded a broker and pane 0 when it did not, and the dashboard index defaults to `0` for pause and restart from pause. `SessionMode` SHALL be consulted only when `dashboard_pane` is `None`.

#### Scenario: Dashboard pane index round-trips

- **GIVEN** a `Session` with `dashboard_pane = Some(0)` saved to disk
- **WHEN** the session is loaded back
- **THEN** `dashboard_pane` SHALL be `Some(0)`

#### Scenario: Session without dashboard_pane defaults to None on load

- **GIVEN** a session JSON file with no `dashboard_pane` field
- **WHEN** the session is loaded
- **THEN** `dashboard_pane` SHALL be `None`

#### Scenario: Dashboard pane field is omitted when None

- **GIVEN** a `Session` with `dashboard_pane = None`
- **WHEN** `save_session()` is called and the JSON file is inspected
- **THEN** the JSON SHALL NOT contain a `dashboard_pane` field

#### Scenario: Supervised start records the dashboard at pane 0

- **GIVEN** `[broker] enabled = true` and a supervised launch
- **WHEN** the session state is saved
- **THEN** `dashboard_pane` SHALL be `Some(0)`
- **AND** the first recorded coding agent's `pane_index` in the per-repo discovery file SHALL be `1`

#### Scenario: A pre-relocation live session keeps its agent offset

- **GIVEN** a saved session with `mode = Supervisor`, `dashboard_pane = Some(1)` and broker enabled
- **WHEN** `git paw status` resolves the session's agent panes
- **THEN** it SHALL treat pane 2 as the first coding-agent pane
- **AND** no keystroke sender (the drive loop, or `sweep.sh approve`) SHALL treat pane 0 or pane 1 of that session as a coding agent

#### Scenario: A session without dashboard_pane follows the earlier rules

- **GIVEN** a live saved session with `mode = Supervisor`, `dashboard_pane = None` and no broker (a broker-less supervised session from a pre-relocation binary)
- **WHEN** `git paw status` resolves the session's agent panes
- **THEN** it SHALL treat pane 2 as the first coding-agent pane
- **AND** a saved bare session with `dashboard_pane = None` SHALL resolve its first coding agent at pane 1 when it recorded a broker and at pane 0 when it did not

#### Scenario: A broker-less supervised start still records the dashboard

- **GIVEN** `[broker] enabled = false` and a supervised launch
- **WHEN** the session state is saved
- **THEN** `dashboard_pane` SHALL be `Some(0)`

### Requirement: start writes a per-repo session JSON

`git paw start` SHALL write a per-repo session JSON to
`.git-paw/sessions/paw-<project>.json` describing the launched
session. This is the discovery surface the bundled
`sweep.sh` helper reads. The file SHALL be written on launch
and removed on `purge`. Every path through which `git paw start`
creates or rebuilds the session's panes SHALL (re)write it — the
bare launch, the spec-driven launch with or without supervisor
mode, the supervisor launch, recovery of a stopped session, and
restart of a paused session — and each agent entry's `pane_index`
SHALL identify the pane that agent occupies in the resulting
session.

#### Scenario: start writes the per-repo JSON

- **WHEN** `git paw start` (any flags) succeeds
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist
  describing the session (name + agent list)

#### Scenario: A spec-driven launch without supervisor mode writes the per-repo JSON

- **WHEN** `git paw start --specs <names>` succeeds without supervisor mode
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist with one
  `agents` entry per launched spec branch
- **AND** the bundled helper SHALL discover the session from it without relying on running inside the session

#### Scenario: Recovery rewrites the per-repo JSON

- **GIVEN** a stopped session whose per-repo JSON is missing
- **WHEN** `git paw start` recovers it
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL exist and list
  every recovered agent with the pane index it occupies in the rebuilt
  session

#### Scenario: Restart from pause rewrites the per-repo JSON

- **GIVEN** a paused session
- **WHEN** `git paw start` resumes it
- **THEN** `.git-paw/sessions/paw-<project>.json` SHALL list every agent
  with the pane index it occupies in the resumed session

#### Scenario: purge removes the per-repo JSON

- **GIVEN** an active session with the per-repo JSON present
- **WHEN** the user runs `git paw purge --force`
- **THEN** the per-repo JSON SHALL be removed

### Requirement: Per-repo JSON shape matches sweep.sh expectations

The per-repo `paw-<project>.json` SHALL include the session name plus an agent list, each entry carrying `branch_id`, `worktree_path`, `cli`, and `pane_index`. The shape SHALL match what the bundled `assets/scripts/sweep.sh` helper reads — invoked from the orchestrator's own session at the repository root, outside the tmux session — so the helper works against the file without modification. The agent list SHALL list coding agents only.

#### Scenario: sweep.sh discovers agents via the per-repo JSON

- **GIVEN** a freshly-started session and the bundled `sweep.sh` helper invoked from a shell at the repository root that is not attached to the tmux session
- **WHEN** sweep.sh reads `.git-paw/sessions/paw-<project>.json`
- **THEN** the helper SHALL enumerate the full agent list with branch_id, worktree path, CLI, and pane index — no fields missing or renamed relative to its expectations

#### Scenario: Adding a field is backwards-compatible

- **WHEN** a future change adds a new field to the per-repo JSON
- **THEN** the current `sweep.sh` SHALL still find every documented field; unknown extra fields SHALL be ignored

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

`git paw purge` SHALL accept a `--stale` flag. When passed, the system SHALL purge only sessions whose receipt is stale per the liveness probe, and by default SHALL scope the sweep to the CURRENT repository — it SHALL NOT purge or touch a session belonging to any other repository. A machine-wide sweep of stale receipts across all repositories SHALL require the explicit `--all-repos` flag. In every case the guarantee is fail-safe: a session that is live per the probe — or whose liveness cannot be positively confirmed — SHALL NEVER be purged by `--stale`, in the current repository or any other. The flag is additive to existing `--force`.

#### Scenario: --stale is scoped to the current repository by default

- **GIVEN** a stale session belonging to the current repository AND a stale session belonging to a different repository
- **WHEN** the user runs `git paw purge --stale` (without `--all-repos`)
- **THEN** only the current repository's stale session's worktrees + branches + receipt SHALL be purged
- **AND** the other repository's session SHALL NOT be touched

#### Scenario: --stale never purges a live or liveness-indeterminate session

- **GIVEN** a live session (in the current or another repository), or a session whose liveness cannot be positively confirmed
- **WHEN** the user runs `git paw purge --stale` (with or without `--all-repos`)
- **THEN** that session SHALL NOT be purged

#### Scenario: --all-repos sweeps stale receipts across repositories

- **GIVEN** stale sessions belonging to multiple repositories
- **WHEN** the user runs `git paw purge --stale --all-repos`
- **THEN** every repository's stale session SHALL be purged
- **AND** every live session SHALL remain intact

#### Scenario: --stale with nothing stale exits cleanly

- **GIVEN** no stale receipts in scope
- **WHEN** the user runs `git paw purge --stale`
- **THEN** the command SHALL exit 0 with a "nothing to purge" message, and SHALL NOT touch any active session

#### Scenario: --stale + --force is well-defined

- **GIVEN** a stale receipt in the current repository
- **WHEN** the user runs `git paw purge --stale --force`
- **THEN** the system SHALL behave equivalently to `--stale` alone (the `--force` flag is redundant in this combination; explicitly documented as a no-op pairing)

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

### Requirement: Recovery rebuilds exactly the session's panes

Recovering a stopped session SHALL rebuild exactly the panes the session defines — in a supervised session, the dashboard pane plus one pane per recorded worktree (`N + 1`). The recovery SHALL NOT create more panes than the session defines and SHALL NOT create an orchestrator pane, including when the stopped session was saved with an earlier layout.

#### Scenario: Recovery of an N-worktree supervised session creates N+1 panes

- **GIVEN** a stopped supervised session with 3 recorded worktrees
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt tmux session SHALL have exactly 4 panes (dashboard + 3 agents)

#### Scenario: Recovery of a pre-relocation session rebuilds the new layout

- **GIVEN** a stopped supervised session saved with `dashboard_pane = Some(1)` and 2 recorded worktrees
- **WHEN** `git paw start` recovers it
- **THEN** the rebuilt session SHALL have exactly 3 panes with the dashboard at pane 0
- **AND** the re-saved session SHALL record `dashboard_pane = Some(0)`

#### Scenario: Recovery that cannot fit fails cleanly

- **GIVEN** a recovery whose panes cannot be tiled in the available canvas
- **WHEN** the rebuild fails
- **THEN** it SHALL NOT leave a half-tiled session and SHALL NOT mutate the persisted session state

### Requirement: A failed start preserves persisted session state

The system SHALL leave the persisted per-repo session JSON unchanged when a
`git paw start` invocation errors before completing its launch. The session
state file SHALL be rewritten only after a launch completes successfully, so
an aborted start cannot destroy a recoverable session (e.g. rewrite it to an
empty worktree list).

#### Scenario: Aborted start does not corrupt the session JSON

- **GIVEN** an existing saved session with N worktrees
- **WHEN** a subsequent `git paw start` errors mid-launch
- **THEN** the saved session JSON SHALL still list those N worktrees
  (unchanged), so a later `git paw start` can still recover it

### Requirement: Headless canvas fits multi-agent supervisor layouts

The system SHALL size the detached/headless `new-session` canvas and the
global `default-size` large enough to tile a supervisor session with the
supported number of agents without tmux returning `no space for new pane`.
An attached client still resizes the session to the real terminal on attach.

#### Scenario: Headless supervisor session tiles without overflow

- **GIVEN** a detached (no attached client) supervisor launch with several
  agents
- **WHEN** the session is built at the headless fallback size
- **THEN** all panes SHALL tile successfully (no `no space for new pane`
  error)

### Requirement: write_session_summary function

The system SHALL provide a public function `pub fn write_session_summary(state: &BrokerState, session: &PawSession, merge_order: &[String], output_path: &Path) -> Result<(), PawError>` in a new `src/summary.rs` module.

The function SHALL extract data from `BrokerState` (agent records and message log) and `PawSession` (project name, start time) to produce the summary. The `merge_order` parameter provides the sequence in which branches were merged.

The function SHALL write the summary to `output_path` as a UTF-8 Markdown file. If the file already exists, it SHALL be overwritten.

#### Scenario: Summary is written to the specified path

- **GIVEN** a valid `BrokerState` with two agents and a `PawSession`
- **WHEN** `write_session_summary(&state, &session, &merge_order, &path)` is called
- **THEN** a file SHALL exist at `path`
- **AND** the function returns `Ok(())`

#### Scenario: Existing summary is overwritten

- **GIVEN** a summary file already exists at the output path
- **WHEN** `write_session_summary` is called again
- **THEN** the file SHALL contain only the new summary content

#### Scenario: Write failure returns PawError

- **GIVEN** the output path is in a read-only directory
- **WHEN** `write_session_summary` is called
- **THEN** the function returns `Err(PawError::...)`

### Requirement: Session metadata section

The summary SHALL begin with a `# Session Summary` heading followed by a metadata block containing:

- Project name (from `PawSession`)
- Session date (formatted as `YYYY-MM-DD`)
- Session duration (from `PawSession` start time to now)
- Number of agents

#### Scenario: Summary contains correct project name

- **GIVEN** a `PawSession` with `project_name = "my-app"`
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL contain `my-app`

#### Scenario: Summary contains agent count

- **GIVEN** a `BrokerState` with 3 known agents
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL contain the number `3` adjacent to "Agents"

### Requirement: Per-agent details section

The summary SHALL include an `## Agents` section with one subsection (`### <branch> (<cli>)`) per known agent, containing:

- `Status:` — the agent's final status from its last message
- `Files modified:` — from the last `agent.artifact` message's `modified_files`
- `Exports:` — from the last `agent.artifact` message's `exports`
- `Blocked time:` — estimated from `agent.blocked` to next `agent.status` timestamps in the message log

When an agent has no `agent.artifact` message, the files and exports fields SHALL show `(none)`.

#### Scenario: Per-agent section includes modified files

- **GIVEN** agent `"feat-config"` published `agent.artifact` with `modified_files = ["src/config.rs"]`
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL contain `src/config.rs` under the `feat-config` agent section

#### Scenario: Per-agent section handles no artifact

- **GIVEN** agent `"feat-config"` only published `agent.status` (never completed)
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL still include a section for `feat-config`
- **AND** files and exports SHALL show `(none)`

### Requirement: Merge order section

The summary SHALL include an `## Overview` section listing the merge order as a comma-separated list or bullet list.

#### Scenario: Merge order is included

- **GIVEN** `merge_order = ["feat-errors", "feat-config", "feat-detect"]`
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL list `feat-errors`, `feat-config`, `feat-detect` in that order under the merge order label

### Requirement: Totals section

The summary SHALL include a `## Totals` section with:

- Total agents: count of all known agents
- Total time: overall session duration

#### Scenario: Totals section is present

- **GIVEN** a session with 3 agents
- **WHEN** `write_session_summary` is called
- **THEN** the output SHALL contain a "Totals" section with at least agent count and duration

### Requirement: Summary module is public

The `summary` module SHALL be declared as `pub mod summary` in `src/lib.rs`. The `write_session_summary` function SHALL be callable from `src/main.rs` supervisor handler.

#### Scenario: Summary module is accessible from main

- **WHEN** `src/lib.rs` is compiled
- **THEN** `git_paw::summary::write_session_summary` SHALL be a callable public function

### Requirement: Summary output path is timestamped under .git-paw/sessions/

The supervisor SHALL call `write_supervisor_summary`, which writes the rendered Markdown to a timestamped file under `<repo_root>/.git-paw/sessions/<UTC-timestamp>.md`. The timestamp SHALL be in `YYYY-MM-DDTHH-MM-SSZ` format (filesystem-safe — colons replaced with hyphens) so multiple supervisor runs against the same repository produce distinct files that can coexist on disk without overwriting each other. `write_supervisor_summary` SHALL create `.git-paw/sessions/` if it does not already exist, and SHALL return the absolute path it wrote so the caller can log it.

#### Scenario: Supervisor writes timestamped summary under sessions dir

- **GIVEN** a completed supervisor session with all agents verified and merged
- **WHEN** the supervisor finishes
- **THEN** a file `<repo_root>/.git-paw/sessions/<UTC-timestamp>.md` SHALL exist
- **AND** the timestamp SHALL match `YYYY-MM-DDTHH-MM-SSZ`
- **AND** the filename SHALL NOT contain a `:` character

#### Scenario: Two sequential supervisor runs produce distinct summary files

- **GIVEN** an empty repository with no `.git-paw/sessions/` directory
- **WHEN** `write_supervisor_summary` is called twice with at least one second between calls
- **THEN** both calls SHALL succeed
- **AND** the returned paths SHALL differ
- **AND** both files SHALL exist on disk

#### Scenario: Sessions directory is created on demand

- **GIVEN** a repository where `.git-paw/sessions/` does not exist
- **WHEN** `write_supervisor_summary` is called
- **THEN** `.git-paw/sessions/` SHALL be created
- **AND** the rendered summary SHALL be written inside it

### Requirement: Session lookup matches on the canonical repository path

`find_session_for_repo` SHALL match a stored session to a repository by comparing
the two paths in canonical form, so a repository referred to by a symlinked or
non-normalized path (for example `/var/…` resolving to `/private/var/…`, or a path
containing a `..` segment) still resolves to its session. When a path cannot be
canonicalized (for example the repository directory no longer exists), the system
SHALL fall back to comparing the raw paths, preserving the prior behavior for that
case.

#### Scenario: A session is found through a symlinked repo path

- **GIVEN** a session registered at a repository directory
- **AND** a symlink that resolves to that same directory
- **WHEN** the session is looked up by the symlink path
- **THEN** the session SHALL be found

#### Scenario: A non-existent path falls back to raw comparison

- **GIVEN** a session registered at a repository path that does not exist on disk
- **WHEN** it is looked up by that same (non-existent) path
- **THEN** the session SHALL still be found via the raw-path fallback

### Requirement: Liveness reflects dead agent panes, not just session existence

The tmux-liveness probe that feeds `effective_status` SHALL report a session as not
alive when the tmux session object still exists but its agent panes' CLI processes
have all exited (the `is_tmux_alive` input becomes false) — so a session whose panes
died shortly after launch is reported `Stopped`, not `Active`. Liveness SHALL NOT be
satisfied by the mere existence of the named tmux session; it SHALL require at
least one live agent pane (a pane whose launched CLI process is still running). This
ensures `git paw status` and `git paw doctor` never report a session healthy after
its panes have all died (the full-auto boot-exit failure mode). The probe SHALL
tolerate the brief launch window in which panes are still starting their CLIs (it
SHALL NOT flap a just-launched session to `Stopped`).

The probe SHALL report "agent panes exited" as a separate liveness outcome from "tmux
session absent". `🔴 stale` and the JSON `"stale"` value SHALL mean only that the tmux
session is absent. `git paw status` SHALL NOT report a session as stale when its tmux
session exists and all its agent panes have exited. For such a session it SHALL:

- display `🟡 stopped`;
- print a hint that the agent panes exited and that `git paw start` recovers them;
- report the tmux session as running: `Tmux: running` in the human output and
  `"tmux_running": true` in JSON;
- never say that the tmux session no longer exists.

`start` auto-invalidation and `purge --stale` SHALL keep deciding staleness with the single
`tmux has-session` probe. As a result, `git paw start` recovers a session whose agent panes
exited, and `purge --stale` never purges it.

The probe SHALL identify coding-agent panes by their pane-scoped `@paw_role` option rather
than by pane position, because a pane index shifts when an earlier pane is killed. It SHALL
classify each pane independently: a pane whose role is `dashboard` or `supervisor` is never a
coding-agent pane, any other role is, and a pane carrying no role falls back to the positional
rule. git-paw SHALL set `@paw_role` on every pane it creates, whether or not
`[layout].border_affordances` is enabled. Consequently a paused session — `git paw pause`
kills the dashboard pane, and tmux renumbers the survivors — SHALL NOT be reported as having
exited agent panes while any of its agents is still running.

#### Scenario: A session whose agent panes all exited reports Stopped

- **GIVEN** a session recorded `Active` whose tmux session object still exists but all agent panes' CLI processes have exited
- **WHEN** its effective status is resolved via the liveness probe
- **THEN** the liveness probe SHALL report the session not alive
- **AND** the effective status SHALL be `Stopped` (not `Active`)

#### Scenario: A session with at least one live agent pane remains Active

- **GIVEN** a session recorded `Active` with at least one agent pane whose CLI process is still running
- **WHEN** its effective status is resolved
- **THEN** the liveness probe SHALL report the session alive
- **AND** the effective status SHALL be `Active`

#### Scenario: Exited agent panes are a distinct liveness outcome from an absent session

- **GIVEN** a tmux session that exists, created past the launch grace period, whose every agent pane runs a bare shell
- **WHEN** the agent-pane liveness probe runs
- **THEN** it SHALL return the agent-panes-exited outcome, not the outcome for an absent tmux session
- **AND** for a tmux session that does not exist the same probe SHALL return the absent-session (stale) outcome

#### Scenario: Status shows exited agent panes as stopped with tmux running

- **GIVEN** a session whose receipt says `active`, whose tmux session exists, and whose agent panes have all exited past the launch grace period
- **WHEN** the user runs `git paw status`
- **THEN** the output SHALL display `🟡 stopped`, not `🔴 stale`
- **AND** it SHALL print a hint that the agent panes exited and that `git paw start` recovers them
- **AND** it SHALL print `Tmux: running`
- **AND** it SHALL NOT state that the tmux session no longer exists

#### Scenario: JSON status for exited agent panes reports stopped with tmux running

- **GIVEN** the same session (tmux session present, all agent panes exited)
- **WHEN** the user runs `git paw status --json`
- **THEN** the response object SHALL contain `"status": "stopped"` and `"tmux_running": true`

#### Scenario: An absent tmux session still reports stale with tmux not running

- **GIVEN** a session whose receipt says `active` and whose tmux session does not exist
- **WHEN** the user runs `git paw status --json`
- **THEN** the response object SHALL contain `"status": "stale"` and `"tmux_running": false`

#### Scenario: purge --stale leaves a session whose agent panes exited

- **GIVEN** a session in the current repository whose receipt says `active`, whose tmux session exists, and whose agent panes have all exited
- **WHEN** the user runs `git paw purge --stale`
- **THEN** that session's receipt SHALL NOT be purged
- **AND** the command SHALL report that there is nothing to purge

#### Scenario: A paused session whose dashboard pane was killed reports paused, not exited

- **GIVEN** a paused session whose dashboard pane `git paw pause` killed, whose remaining agent pane tmux has renumbered into the dashboard's old index, and whose agent process is still running
- **WHEN** the user runs `git paw status` and `git paw doctor`
- **THEN** status SHALL display `🔵 paused` and SHALL NOT print the agent-panes-exited hint
- **AND** doctor's session-state finding SHALL NOT say that its agent panes exited

### Requirement: Worktree entry records the worker CLI session id

The `WorktreeEntry` struct SHALL include an optional field `cli_session_id: Option<String>` recording
the session identifier git-paw assigned to the worker's CLI at launch. The field SHALL be annotated
`#[serde(default, skip_serializing_if = "Option::is_none")]` so session files written before this
change load with `None` and an entry without an id serializes byte-identically to before.

#### Scenario: Assigned id round-trips

- **GIVEN** a `WorktreeEntry` with `cli_session_id = Some("3f2c…")` saved to disk
- **WHEN** the session is loaded back
- **THEN** `cli_session_id` SHALL be `Some("3f2c…")`

#### Scenario: Older session files load with None

- **GIVEN** a session JSON file with no `cli_session_id` field on its worktree entries
- **WHEN** the session is loaded
- **THEN** every entry's `cli_session_id` SHALL be `None`
- **AND** loading SHALL succeed without error

#### Scenario: Entry without an id serializes as before

- **GIVEN** a `WorktreeEntry` with `cli_session_id = None`
- **WHEN** the session is saved and the JSON inspected
- **THEN** the JSON SHALL NOT contain a `cli_session_id` key

### Requirement: Recovery relaunches a worker with its recorded session id

When `git paw start` recovers a stopped session, the system SHALL relaunch each worktree whose entry
carries a `cli_session_id` AND whose CLI resolves a resume template with that template's arguments
appended to the CLI launch command, so the worker's conversation context is restored. A worktree
whose entry has no id, or whose CLI resolves no resume template, SHALL be relaunched exactly as before
this change. Recovery SHALL NOT fail because a resume template is absent.

#### Scenario: Recovered Claude worker is resumed

- **GIVEN** a stopped session whose worktree entry for `feat/x` has `cli = "claude"` and
  `cli_session_id = Some("<uuid>")`, and no `[clis.claude]` override
- **WHEN** the session is recovered
- **THEN** the pane command for `feat/x` SHALL contain `--resume <uuid>`

#### Scenario: Entry without an id launches fresh

- **GIVEN** a stopped session whose worktree entry has `cli_session_id = None`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL be identical to the pre-change recovery command for that entry

#### Scenario: CLI without a resume template launches fresh

- **GIVEN** a worktree entry with `cli = "my-agent"`, `cli_session_id = Some("<id>")`, and no
  `[clis.my-agent].resume_args`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL contain no resume arguments
- **AND** recovery SHALL succeed

### Requirement: Status JSON exposes the worker CLI session id

`git paw status --json` SHALL include each agent's `cli_session_id` (a string, or `null` when
unassigned) so an operator or external orchestrator can resume a worker by hand. The human-readable
`git paw status` output SHALL be unchanged.

#### Scenario: JSON status carries the id

- **GIVEN** an active session whose `feat/x` entry has `cli_session_id = Some("<uuid>")`
- **WHEN** `git paw status --json` is run
- **THEN** the `feat/x` agent object SHALL contain `"cli_session_id": "<uuid>"`

#### Scenario: JSON status shows null when unassigned

- **GIVEN** an agent whose entry has no session id
- **WHEN** `git paw status --json` is run
- **THEN** that agent object SHALL contain `"cli_session_id": null`

#### Scenario: Human-readable status is unchanged

- **WHEN** `git paw status` is run without `--json`
- **THEN** the output SHALL be byte-identical to the pre-change output for the same session

### Requirement: Worktree entry records the spec it was launched from

The `WorktreeEntry` struct SHALL include an optional field `spec_id: Option<String>` recording the id of the `SpecEntry` an agent was launched from. Every launch path that creates an agent from a spec — the spec-driven launch with or without supervisor mode, and `git paw add --from-spec` — SHALL set it to that spec's id; a launch without a spec (bare `git paw start`, `git paw add <branch>`) SHALL leave it `None`. The field SHALL be annotated `#[serde(default, skip_serializing_if = "Option::is_none")]` so session files written before this field existed load with `None` and an entry without a spec serializes byte-identically to before.

#### Scenario: A spec-driven launch records each agent's spec id

- **GIVEN** a spec-driven launch of specs `003-user-list` and `004-error-handling`
- **WHEN** the session is saved and loaded back
- **THEN** each agent's worktree entry SHALL carry the `spec_id` of the spec it was launched from

#### Scenario: A launch without a spec records no spec id

- **GIVEN** a bare `git paw start` with explicit branches
- **WHEN** the session is saved and the JSON inspected
- **THEN** no worktree entry SHALL contain a `spec_id` key

#### Scenario: Older session files load with no spec id

- **GIVEN** a session JSON file whose worktree entries have no `spec_id` field
- **WHEN** the session is loaded
- **THEN** loading SHALL succeed without error
- **AND** every entry's `spec_id` SHALL be `None`

### Requirement: Session records its pane layout

The `Session` struct SHALL include an optional field recording the window's pane layout, written as the agent grid by every start, recovery and restart that builds or rebuilds the window. The field SHALL be annotated `#[serde(default, skip_serializing_if = "Option::is_none")]` so session files written earlier load unchanged. A session without the field SHALL be read as the agent grid when its mode is supervised, and as the earlier tiled layout when its mode is bare; consumers that depend on the geometry (`add`, `remove`) SHALL decide from this field and the mode, never from the mode alone.

#### Scenario: New sessions record the grid

- **GIVEN** a bare `git paw start` with two branches
- **WHEN** the session state is saved
- **THEN** the session file SHALL record the grid layout

#### Scenario: Earlier session files load unchanged

- **GIVEN** a session file written before this field existed
- **WHEN** it is loaded
- **THEN** loading SHALL succeed and the layout field SHALL be absent

#### Scenario: An earlier bare session reads as tiled

- **GIVEN** a loaded bare session without the layout field
- **WHEN** `git paw add` inspects its layout
- **THEN** it SHALL be treated as the tiled layout

#### Scenario: An earlier supervised session reads as the grid

- **GIVEN** a loaded supervised session without the layout field
- **WHEN** `git paw add` inspects its layout
- **THEN** it SHALL be treated as the agent grid

