## ADDED Requirements

### Requirement: A drive loop can be bound to a live supervised session

When `git paw start --unattended` (not a dry run) finds a session of the current repository (per `session-state` find-by-repository) that was launched in supervised mode, the system SHALL bind a new drive loop to that session instance instead of refusing, attaching, or launching a new session, and SHALL then run that loop exactly as a fresh `--unattended` launch runs it: the same cadence, escalation, correction, heartbeat and exit summary, approving nothing. By the session's effective status:

- **Active:** the loop SHALL run against the session as it stands. Binding SHALL NOT create, rebuild or re-title any pane, SHALL NOT create or touch any worktree, and SHALL NOT inject any boot block, prompt or keystroke.
- **Paused:** the system SHALL first revive the session exactly as `git paw start` revives a paused session, then run the loop.
- **Stopped:** the system SHALL first recover the session exactly as `git paw start` recovers a stopped session, then run the loop.

A revival or recovery that fails SHALL fail the command without running a loop. In every case the system SHALL NOT attach the terminal, even when stdin is interactive, and SHALL print the session name and how to attach from another terminal. The loop SHALL drive the session's own coding-agent roster and SHALL reach the broker at the endpoint recorded for the session, not at the endpoint in the current configuration. Launch-selection flags given with the command (`--branches`, `--specs`, `--from-all-specs`, `--cli`) SHALL NOT be applied to the live session, so re-running the command that launched a session binds a new loop to it.

When the session was launched without supervised mode (a bare session), `start --unattended` SHALL refuse before changing any session, pane or worktree state, with an error that names the session, states that only a session launched in supervised mode can be driven unattended (a bare session leaves its prompts for the human to submit), and names `git paw attach` for driving it by hand. A dry run SHALL keep its current behaviour and bind nothing.

#### Scenario: Re-running the launch command after a heartbeat exit re-binds the loop

- **GIVEN** a supervised session `paw-x` launched with `git paw start --unattended --branches feat/a`, whose drive loop exited on `heartbeat` while the session stayed Active
- **WHEN** the same command runs again with no controlling terminal
- **THEN** a new drive loop SHALL run against `paw-x` and exit with a summary
- **AND** the session's pane count and pane titles SHALL be unchanged and no worktree SHALL be created
- **AND** no `paw-x-2` session SHALL exist

Test: `tests/unattended_drive_e2e.rs::unattended_rebinds_a_loop_to_a_live_session_after_heartbeat`.

#### Scenario: A paused session is revived, then driven

- **GIVEN** a paused supervised session `paw-x` with no drive loop running
- **WHEN** `git paw start --unattended` runs in its repository
- **THEN** the session SHALL be revived as `git paw start` revives it (status `active`)
- **AND** a drive loop SHALL then run against `paw-x`
- **AND** the terminal SHALL NOT be attached

Test: `tests/unattended_drive_e2e.rs::unattended_revives_a_paused_session_then_drives`.

#### Scenario: A stopped session is recovered, then driven

- **GIVEN** a stopped supervised session `paw-x`
- **WHEN** `git paw start --unattended` runs in its repository
- **THEN** the session SHALL be recovered as `git paw start` recovers it
- **AND** a drive loop SHALL then run against the recovered `paw-x`

Test: `tests/unattended_drive_e2e.rs::unattended_recovers_a_stopped_session_then_drives`.

#### Scenario: The re-bound loop uses the session's recorded broker

- **GIVEN** an Active supervised session `paw-x` whose recorded broker port is P
- **AND** `[broker] port` in the configuration changed to Q after launch
- **WHEN** `git paw start --unattended` binds a loop to `paw-x` and an agent shows a live permission prompt
- **THEN** the loop SHALL publish the escalation to the broker on port P

Test: `tests/unattended_drive_e2e.rs::unattended_rebind_uses_the_recorded_broker_endpoint`.

#### Scenario: A bare session is refused without being changed

- **GIVEN** an Active, Paused or Stopped session `paw-x` launched without supervised mode
- **WHEN** `git paw start --unattended` runs in its repository
- **THEN** the command SHALL exit non-zero with an error naming `paw-x`, saying only supervised sessions can be driven unattended, and naming `git paw attach`
- **AND** the session record, its panes and its worktrees SHALL be unchanged and no drive loop SHALL run

Test: `tests/unattended_drive_e2e.rs::unattended_refuses_a_bare_session`.

### Requirement: At most one drive loop drives a session instance

Every drive loop, whether started by a fresh `--unattended` launch or bound to a live session, SHALL hold a drive-loop binding naming its session instance (the session name and the instance token of *The drive loop is bound to a specific session instance*) and itself, SHALL refresh that binding every tick, and SHALL release it when it exits. At most one drive loop SHALL hold the binding of a session instance at a time:

- A `start --unattended` that finds an Active or Paused session's binding held by another loop that refreshed it within the staleness window SHALL refuse before changing any session, pane or worktree state and before running a loop. The error SHALL name the session and the holding process, state that only one drive loop may drive a session, and tell the user to wait for that loop to exit (or stop that process) and re-run the command.
- A binding not refreshed within the staleness window (its loop was killed without releasing it) SHALL NOT block a new loop. The window SHALL be the larger of four poll intervals and two minutes.
- A binding that does not name this session instance (left by a loop of a purged or replaced session, or unreadable) SHALL NOT block a new loop.
- When `start --unattended` recovers a Stopped session, the new loop SHALL supersede any binding left for that instance: the loop of a stopped instance is already required to stop acting.
- `git paw stop` SHALL revoke the session's binding.

At the start of every tick, before sweeping or sending anything, a loop SHALL confirm that it still holds the binding. A loop whose binding was revoked or taken over SHALL exit with outcome `session-torn-down` without acting on that tick, even when the session record still carries its instance token (recovery keeps the session's creation timestamp, so a session stopped and recovered within one poll interval looks unchanged to the record check alone).

#### Scenario: A second loop on a live session is refused

- **GIVEN** an Active supervised session `paw-x` driven by a running drive loop in process P
- **WHEN** `git paw start --unattended` runs again in its repository
- **THEN** the command SHALL exit non-zero with an error naming `paw-x` and P and saying only one drive loop may drive a session
- **AND** no second loop SHALL run and the session SHALL be unchanged
- **AND** the first loop SHALL keep driving `paw-x`

Test: `tests/unattended_drive_e2e.rs::second_unattended_loop_on_a_live_session_is_refused`.

#### Scenario: A released binding lets the next loop bind at once

- **GIVEN** a drive loop bound to `paw-x` that exited on `heartbeat`
- **WHEN** `git paw start --unattended` runs immediately afterwards
- **THEN** a new loop SHALL bind to `paw-x` without waiting for the staleness window

Test: `tests/unattended_drive_e2e.rs::unattended_rebinds_a_loop_to_a_live_session_after_heartbeat`.

#### Scenario: A killed loop's binding lapses

- **GIVEN** a binding for the live instance of `paw-x` last refreshed longer than the staleness window ago
- **WHEN** a new loop tries to bind to `paw-x`
- **THEN** it SHALL bind

Test: `supervisor::loop_binding::tests::a_binding_not_refreshed_within_the_window_lapses` (backdated mtime).

#### Scenario: A binding from another session instance does not block

- **GIVEN** a fresh binding that names an earlier, purged instance of `paw-x`
- **WHEN** a loop binds to the new live instance of `paw-x`
- **THEN** it SHALL bind

Test: `supervisor::loop_binding::tests::a_binding_for_another_instance_does_not_block`.

#### Scenario: Stop revokes the binding before a quick recovery

- **GIVEN** a running drive loop bound to `paw-x`
- **WHEN** `git paw stop` runs and `git paw start` recovers `paw-x` before the loop's next tick
- **THEN** the loop SHALL exit with outcome `session-torn-down` on its next tick
- **AND** it SHALL send no keystroke to the recovered session

Test: `tests/e2e_supervisor_stop.rs::stop_revokes_the_drive_loop_binding`; `supervisor::drive::tests::lost_binding_ends_the_loop_before_it_acts`.

#### Scenario: Recovering a stopped session supersedes a leftover binding

- **GIVEN** a stopped session `paw-x` whose binding is still held by a loop that has not yet observed the stop
- **WHEN** `git paw start --unattended` recovers `paw-x` and binds a new loop
- **THEN** the new loop SHALL drive `paw-x`
- **AND** the earlier loop SHALL exit with outcome `session-torn-down` on its next tick without acting

Test: `supervisor::loop_binding::tests::recovery_supersedes_a_leftover_binding`; `supervisor::drive::tests::lost_binding_ends_the_loop_before_it_acts`.
