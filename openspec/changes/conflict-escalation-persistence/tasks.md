## 1. Reproduce the re-escalation

- [ ] 1.1 Add a detector test replaying the observed sequence: overlap established, escalation emitted, one agent publishes `modified_files = []`, then republishes the file, then the window elapses — assert NO second `agent.question` — SHALL fail before the fix
- [ ] 1.2 Add a test asserting `first_seen` is not reset by a transient empty `modified_files`: an overlap at `T < window_seconds` that blips empty SHALL still escalate at the original `T + window_seconds`, not later
- [ ] 1.3 Add a test asserting the overlap remains present in the tracker after acknowledgement

## 2. Separate decision from residency

- [ ] 2.1 Move the escalation decision and `first_seen` out of the tracker entry into a record keyed on `(min_agent, max_agent, file)`
- [ ] 2.2 Stop tracker removal from discarding that record; removal SHALL affect only the live-overlap view
- [ ] 2.3 Define and implement the disposal rule for decision records so they cannot accumulate unbounded for the session's life
- [ ] 2.4 Verify the existing "Escalation is emitted only once per triple" and "Conflict resolves when one agent stops touching the file" scenarios still pass unchanged

## 3. Acknowledgement

- [ ] 3.1 Decide the carrier. Check whether `agent.feedback` (already consumed by the detector) can carry the acknowledgement; only add a `BrokerMessage` variant if it cannot
- [ ] 3.2 If a new variant is unavoidable, scope every site in AGENTS.md §7 BEFORE implementing — `messages.rs` (`agent_id`/`status_label`/`validate`), `dashboard/broker_log.rs` (`message_bit`/`type_short`/`derive_summary`), `learnings.rs::observe()`, plus the non-compile-forcing routing in `delivery.rs`, `conflict.rs`, `server.rs`, `watcher.rs`, `dashboard.rs`
- [ ] 3.3 Implement acknowledgement: suppress further `agent.question` for the pair and file, without removing the overlap from the tracker
- [ ] 3.4 Invalidate the acknowledgement when the pair's region declarations for that file materially change; define "materially" in terms of the existing intersection rules rather than inventing a second notion
- [ ] 3.5 Add a test asserting a materially changed conflict can escalate again despite a prior acknowledgement

## 4. Make it reachable

- [ ] 4.1 Add an acknowledge subcommand to `assets/scripts/sweep.sh` so an orchestrator can record an adjudication, and sync the tracked `.git-paw/scripts/sweep.sh` copy
- [ ] 4.2 Run `bash -n` on both scripts after editing
- [ ] 4.3 Mention the verb in the bundled orchestration guidance so it is discoverable — keeping the wording CLI-agnostic and project-agnostic per the export-agnosticism principle

## 5. Spec traceability

- [ ] 5.1 Verify every scenario in `specs/broker-conflict-detection/spec.md` maps to at least one test
- [ ] 5.2 Confirm no previously-passing conflict-detection test changed meaning rather than merely being extended

## 6. Docs and gates

- [ ] 6.1 Update the Conflict Detection user-guide chapter: escalations persist across commits, and how to acknowledge one
- [ ] 6.2 `cargo fmt`, then `just lint`
- [ ] 6.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code rather than piped output
- [ ] 6.4 `mdbook build docs/`
- [ ] 6.5 `openspec validate conflict-escalation-persistence --strict`
