## Why

**DS-4.** An `--unattended` session writes no manual-decision log, so `git paw approvals`
has nothing to aggregate for exactly the runs where nobody watched the prompts.

- The only `ManualDecisionRecorder` is built in `cmd_dashboard` (`src/main.rs`), inside the
  dashboard auto-approve wiring — which `dashboard_should_auto_approve` disables when the
  session is unattended, so the drive loop is the sole approver.
- The drive loop's escalation branch (`sweep_pane`, `PromptVerdict::Danger |
  PromptVerdict::Unknown` in `src/supervisor/drive.rs`) records an escalation and a
  `tooling_friction` learning, and nothing else. `DriveDeps` has no recorder.

*Manual-decision log file* (`approval-pattern-surfacing`) is scoped to "the auto-approve
poll loop", so the gap is a spec gap as much as a wiring gap: the requirement never said the
drive loop must record.

## What Changes

- The drive loop records each prompt it escalates for a manual decision in
  `.git-paw/sessions/<session>.manual-approvals.jsonl` — one record per escalation, i.e.
  after the escalation dedup check, so a prompt re-seen within the dedup window is not
  logged twice.
- It honours `[supervisor] manual_approvals_log = false` and, when learnings are enabled and
  the broker is on, publishes the first-seen `permission_pattern` learning exactly as the
  poll loop does.
- Auto-approved prompts and CLI-unavailability escalations are not logged.
- No CLI, config, or log-format change.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `approval-pattern-surfacing`: *Manual-decision log file* covers both approvers — the
  dashboard poll loop and the `--unattended` drive loop — with drive-loop scenarios for
  logging, dedup, the auto-approved exclusion, and the opt-out.

## Impact

- `src/supervisor/drive.rs` — new `ManualDecisionSink` trait (mirrors `AlertSink` /
  `LearningSink`); `DriveDeps` gains `manual_decisions: &mut dyn ManualDecisionSink`;
  `sweep_pane`'s Danger/Unknown branch calls it after `dedup.should_emit`; production
  `RecorderManualDecisionSink` wraps `ManualDecisionRecorder` and publishes the returned
  learning; `DriveRunOptions` gains `manual_decisions: ManualDecisionRecorder`. Every
  `DriveDeps { .. }` literal in `drive.rs` tests (~28) gains the field.
- `src/supervisor/manual_approvals.rs` — new
  `ManualDecisionRecorder::for_session(supervisor: Option<&SupervisorConfig>, repo_root,
  session_name, project_name)` so both approvers resolve the opt-out and learnings flags
  identically.
- `src/main.rs` — `cmd_dashboard` builds its recorder with `for_session`.
- `src/commands/supervisor.rs` — `drive_unattended_loop` builds the recorder with
  `for_session` and passes it in `DriveRunOptions`.
- **Delta coordination** (grepped every pending change): no pending change touches
  `approval-pattern-surfacing`. No collision.
- **Sequencing**: land after `cli-unavailable-detection-gaps` (it edits `sweep_pane` and the
  `DriveDeps` call sites) — rebase onto it. `idle-pane-nudge-submission` is already archived.
  `approval-pane-map-refresh` also edits `drive_unattended_loop` (the roster line); the two
  edits are in different statements.
