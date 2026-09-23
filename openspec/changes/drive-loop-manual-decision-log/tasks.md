## 0. Sequencing

- [ ] 0.1 Rebase onto `cli-unavailable-detection-gaps` once it has merged (both edit `sweep_pane` and the `DriveDeps` literals); confirm with `git merge-tree --write-tree HEAD <branch>` before starting if it has not

## 1. Failing tests first (`src/supervisor/drive.rs` tests)

- [ ] 1.1 Add a `RecordingManualDecisions` test sink and a `NoopManualDecisions` helper; a sweep over a pane showing a live unknown/danger prompt calls the sink exactly once with that agent id and capture (*A drive-loop escalation appends a JSONL line* — sink level) — fails to compile/assert before the fix
- [ ] 1.2 Two consecutive sweeps of the same unresolved prompt within the dedup window → exactly one sink call (*A deduplicated drive-loop escalation is logged once*)
- [ ] 1.3 A sweep that auto-approves a safe live prompt → zero sink calls (*Drive-loop auto-approvals are not logged*); a CLI-unavailability banner escalation → zero sink calls
- [ ] 1.4 Cross-module: `RecorderManualDecisionSink` around a real `ManualDecisionRecorder` writing to a tempdir log, driven by one escalating sweep → the JSONL file has one line with `timestamp`, `agent_id`, `pattern`, `first_seen: true` (*A drive-loop escalation appends a JSONL line* — file level)
- [ ] 1.5 Same with the recorder built from `manual_approvals_log = false` (and `learnings = true`, broker URL `None`) → no file written and no learning returned (*The opt-out applies to the drive loop*)
- [ ] 1.6 `src/supervisor/manual_approvals.rs`: `ManualDecisionRecorder::for_session` — default config → enabled; `manual_approvals_log = false` → disabled; `learnings = true` → learning returned on first sighting only

## 2. Recorder construction

- [ ] 2.1 `src/supervisor/manual_approvals.rs`: add `pub fn for_session(supervisor: Option<&SupervisorConfig>, repo_root: &Path, session_name: &str, project_name: String) -> Self` (resolution copied from `cmd_dashboard`: `is_none_or(manual_approvals_log_enabled)`, `is_some_and(learnings)`, `supervisor.cli`)
- [ ] 2.2 `src/main.rs` `cmd_dashboard`: build the poll loop's recorder with `for_session` (behaviour unchanged)

## 3. Drive-loop wiring

- [ ] 3.1 `src/supervisor/drive.rs`: add `pub trait ManualDecisionSink` and `DriveDeps.manual_decisions: &'a mut dyn ManualDecisionSink` (doc comments); add `manual_decisions` to every `DriveDeps { .. }` literal (`rtk grep "DriveDeps {" src`)
- [ ] 3.2 `sweep_pane`, Danger/Unknown arm: call `deps.manual_decisions.record_forwarded(agent_id, &capture)` after `dedup.should_emit` returns `true` and before `record_escalation`; leave the CLI-unavailable branch untouched
- [ ] 3.3 Production `RecorderManualDecisionSink { recorder, broker_url }`: record, then publish a returned learning via `publish_to_broker_http` when `broker_url` is `Some`, warning on stderr on failure
- [ ] 3.4 `DriveRunOptions.manual_decisions: ManualDecisionRecorder`; `run_drive_loop` wraps it in the production sink with the options' `broker_url`
- [ ] 3.5 `src/commands/supervisor.rs` `drive_unattended_loop`: build the recorder with `ManualDecisionRecorder::for_session(Some(supervisor_cfg), repo_root, session_name, state.project_name.clone())` and pass it in `DriveRunOptions`

## 4. Docs

- [ ] 4.1 `docs/src/user-guide/supervisor.md` *Manual approvals → What gets recorded*: the log is written by whichever approver runs the session — the poll loop when attended, the drive loop under `--unattended` (one line per escalation; deduplicated re-sightings are not re-logged)
- [ ] 4.2 `docs/src/configuration/README.md` `manual_approvals_log` row: state it applies to attended and `--unattended` sessions alike
- [ ] 4.3 `--help` for `git paw approvals` names no loop — confirm no change needed
- [ ] 4.4 Verify every scenario in `specs/approval-pattern-surfacing/spec.md` maps to a test (the three pre-existing poll-loop scenarios keep their existing tests)

## 5. Gates

- [ ] 5.1 `cargo fmt`, then `just lint`
- [ ] 5.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > target/test.log 2>&1; echo $?` — exit code must be 0 (never trust a piped status)
- [ ] 5.3 `mdbook build docs/`
- [ ] 5.4 `openspec validate drive-loop-manual-decision-log --strict`
