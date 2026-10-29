## MODIFIED Requirements

### Requirement: The per-repo JSON describes headless workers additively

The per-repo session JSON SHALL list each worker under the list of its own backend: a tmux worker in `agents` with its `branch_id`, `worktree_path`, `cli` and `pane_index`; a headless worker in a `headless_agents` list with `branch_id`, `worktree_path` and `cli`; and a remote worker in a `remote_agents` list with `branch_id`, `worktree_path`, `vendor` and the vendor session's `url`. For a headless session the JSON SHALL carry a top-level `backend` with the value `"headless"` and an empty `agents` list. `headless_agents` SHALL be present exactly when the session is headless or holds a headless worker, and `remote_agents` exactly when the session holds a remote worker. For a tmux session all of whose workers are tmux workers the JSON SHALL carry none of `backend`, `headless_agents` and `remote_agents`, and SHALL be byte-identical to the file written before this requirement. No `pane_index` value SHALL be written for a headless or remote worker.

#### Scenario: A headless discovery file

- **GIVEN** a headless session with workers on `feat/a` and `feat/b`
- **WHEN** its per-repo JSON is read
- **THEN** `backend` SHALL be `"headless"`, `agents` SHALL be empty, and `headless_agents` SHALL list `feat-a` and `feat-b` with their worktree paths and CLIs

#### Scenario: A tmux discovery file is unchanged

- **GIVEN** a tmux session all of whose workers are tmux workers
- **WHEN** its per-repo JSON is read
- **THEN** it SHALL contain no `backend`, no `headless_agents` and no `remote_agents` key

#### Scenario: A mixed discovery file lists each worker once

- **GIVEN** a tmux session with tmux worker `feat/a`, headless worker `feat/h` and remote worker `feat/r`
- **WHEN** its per-repo JSON is read
- **THEN** `agents` SHALL list only `feat-a`, with its pane index
- **AND** `headless_agents` SHALL list only `feat-h` and `remote_agents` only `feat-r`, with its vendor and URL
- **AND** the JSON SHALL contain no top-level `backend`

### Requirement: Status reports a headless session's backend and worker activity

`git paw status` SHALL derive a session's liveness from the session's own backend — the headless backend for a headless session and tmux otherwise — and SHALL print the backend of a headless session. For every worker whose backend is not tmux it SHALL print the worker's backend and state: for a headless worker its activity (busy, awaiting input, failed with its exit code, or interrupted), and for a remote worker its last-known vendor state, whether git-paw still follows it, and its vendor session URL. For a tmux session all of whose workers are tmux workers, its human-readable output SHALL be byte-identical to the output before this requirement. `git paw status --json` SHALL add a top-level `backend` (`"tmux"` or `"headless"`, the session's backend) and, on every `worktrees` entry, a `backend` (`"tmux"`, `"headless"` or `"remote"`, the worker's backend), an `activity` that is the worker's activity for a headless worker and `null` otherwise, and a `remote` object for a remote worker — carrying `vendor`, `session`, `url`, `state`, `following` and `tasks`, the number of vendor sessions git-paw has created for the worker — and `null` otherwise. `terminal_alive` SHALL report the session backend's session liveness, and `tmux_running` SHALL keep carrying the same value as `terminal_alive`.

#### Scenario: Human status of a headless session

- **GIVEN** a live headless session whose worker `feat-a` is busy and `feat-b` failed with exit code 1
- **WHEN** `git paw status` runs
- **THEN** the output SHALL name the headless backend
- **AND** SHALL show `feat-a` as busy and `feat-b` as failed with code 1

#### Scenario: JSON status of a headless session

- **GIVEN** the same session
- **WHEN** `git paw status --json` runs
- **THEN** `backend` SHALL be `"headless"`, `terminal_alive` SHALL be `true`, and the `feat-b` entry's `activity` SHALL describe a failed turn with code 1

#### Scenario: tmux status is unchanged

- **GIVEN** a live tmux session all of whose workers are tmux workers
- **WHEN** `git paw status` runs
- **THEN** its output SHALL be byte-identical to the output before this requirement
- **AND** `git paw status --json` SHALL report `backend` `"tmux"` at the top level and on every entry, and `activity` and `remote` `null` on every entry

#### Scenario: Human status of a mixed session

- **GIVEN** a live tmux session with tmux worker `feat-a` and remote worker `feat-r`, whose vendor session is working at `https://vendor.example/s/abc123` and is followed
- **WHEN** `git paw status` runs
- **THEN** the line for `feat-r` SHALL name the remote backend, the working state, that it is followed, and `https://vendor.example/s/abc123`
- **AND** the line for `feat-a` SHALL be as it was before this requirement

#### Scenario: JSON status of a mixed session

- **GIVEN** the same session, for which git-paw has created one vendor session for `feat-r`
- **WHEN** `git paw status --json` runs
- **THEN** the `feat-r` entry SHALL carry `"backend": "remote"` and a `remote` object whose `url` is `https://vendor.example/s/abc123`, whose `following` is `true` and whose `tasks` is `1`
- **AND** the `feat-a` entry SHALL carry `"backend": "tmux"` and `"remote": null`

### Requirement: Liveness reflects dead agent panes, not just session existence

The tmux-liveness probe that feeds `effective_status` SHALL report a session as not
alive when the tmux session object still exists but its agent panes' CLI processes
have all exited and no headless or remote worker of the session is alive (the
`is_tmux_alive` input becomes false) — so a session whose panes died shortly after
launch is reported `Stopped`, not `Active`. Liveness SHALL NOT be satisfied by the
mere existence of the named tmux session; it SHALL require at least one live worker:
an agent pane whose launched CLI process is still running, a headless worker whose
surface its backend reports alive, or a remote worker whose vendor session git-paw
still follows. This ensures `git paw status` and `git paw doctor` never report a
session healthy after its workers have all died (the full-auto boot-exit failure
mode). The probe SHALL tolerate the brief launch window in which panes are still
starting their CLIs (it SHALL NOT flap a just-launched session to `Stopped`).

The probe SHALL report "agent panes exited" as a separate liveness outcome from "tmux
session absent", and SHALL report it only when, in addition, no headless or remote
worker of the session is alive. `🔴 stale` and the JSON `"stale"` value SHALL mean only
that the tmux session is absent. `git paw status` SHALL NOT report a session as stale
when its tmux session exists and all its agent panes have exited. For such a session it SHALL:

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

#### Scenario: A followed remote worker keeps a tmux session Active

- **GIVEN** a tmux session recorded `Active`, past the launch grace period, whose only agent pane runs a bare shell and whose remote worker's vendor session git-paw still follows
- **WHEN** its effective status is resolved
- **THEN** the liveness probe SHALL report the session alive
- **AND** the effective status SHALL be `Active`
