## Context

Two approvers exist and exactly one is active per session: the dashboard's auto-approve
poll loop (attended) and the in-process drive loop (`--unattended`). Only the poll loop
holds a `ManualDecisionRecorder`; it calls `record_forwarded(agent_id, captured)` from its
`QuestionForwarder`, i.e. once per forwarded prompt, and publishes the first-seen
`permission_pattern` learning the recorder returns. The drive loop's equivalent moment is
the Danger/Unknown branch of `sweep_pane`, after `DedupWindow::should_emit`.

## Goals / Non-Goals

**Goals:**
- An unattended run produces the same manual-decision log and first-seen learnings an
  attended run would.
- One escalation = one record; the opt-out behaves identically for both approvers.

**Non-Goals:**
- Recording CLI-unavailability escalations (not permission prompts).
- Changing the log format, `git paw approvals`, the dedup window, or escalation behaviour.
- Recording human decisions (neither loop can observe the keystroke — design D2 of the
  original capability still holds).

## Decisions

### D1 — A `ManualDecisionSink` trait on `DriveDeps`

`pub trait ManualDecisionSink { fn record_forwarded(&mut self, agent_id: &str, captured:
&str); }` and `DriveDeps.manual_decisions: &'a mut dyn ManualDecisionSink`, matching how
every other side effect of the loop (`AlertSink`, `LearningSink`, `BranchRefresher`) is
injected. Tests substitute a recording sink; production uses
`RecorderManualDecisionSink { recorder: ManualDecisionRecorder, broker_url: Option<String> }`,
which calls `recorder.record_forwarded` and publishes a returned learning with
`publish_to_broker_http` when `broker_url` is `Some` (warning on stderr on failure, as the
poll loop does). The opt-out lives in the recorder (`enabled = false` → no write, no
learning), so the sink needs no flag.

*Alternative rejected:* putting `ManualDecisionRecorder` directly on `DriveDeps`. The
learning publish needs the broker URL and HTTP, which the loop's pure core keeps behind
traits; a concrete recorder would either drop the learning or pull I/O into `sweep_pane`.

### D2 — Call site: after the dedup check, before `record_escalation`

```text
PromptVerdict::Danger | PromptVerdict::Unknown =>
    if !dedup.should_emit(..) { return None }      // suppressed: no record
    deps.manual_decisions.record_forwarded(agent_id, &capture)
    record_escalation(..); learnings; Escalated(..)
```

This gives "one escalation = one record" by construction. The CLI-unavailable branch
(earlier in `sweep_pane`) is untouched, so banners are never logged.

### D3 — One constructor for both approvers

`ManualDecisionRecorder::for_session(supervisor: Option<&SupervisorConfig>, repo_root:
&Path, session_name: &str, project_name: String) -> Self` resolves
`manual_approvals_log_enabled` (default on), `learnings`, and `cli` exactly as
`cmd_dashboard` does today, and uses `log_path(repo_root, session_name)`. `cmd_dashboard`
and `drive_unattended_loop` both call it, so the opt-out cannot drift between approvers.
`DriveRunOptions` carries the built recorder; `run_drive_loop` wraps it in the production
sink with the options' `broker_url`.

## Risks / Trade-offs

- [~28 `DriveDeps { .. }` literals in `drive.rs` tests gain a field] → add one
  `NoopManualDecisions` test helper; mechanical.
- [Merge conflict with `cli-unavailable-detection-gaps`, which edits `sweep_pane`] → land
  after it (proposal Sequencing); the insertion is a single line in the Danger/Unknown arm.
- [Supervisor-pane (pane 0) escalations are logged with `agent_id = "supervisor"`] →
  accepted: they are manual decisions too, and `git paw approvals` groups by pattern.

## Migration Plan

None. Existing logs and attended sessions are unchanged. Rollback is a plain revert.
