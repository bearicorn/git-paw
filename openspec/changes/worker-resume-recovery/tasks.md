## 1. Seams (behaviour-preserving)

- [ ] 1.1 Extract `resume_decision(entry, profile, probe) -> Resume(id) | FreshAssign | FreshNone` from `recover.rs::cli_command_for_recovery` (coordinate with `start-resume-summary`'s `resume_mode`: one function, both callers); recovery tests unchanged and green
- [ ] 1.2 `Clock::wall_now()` on the drive loop's clock (system and fake); no behaviour change

## 2. Conversation facts

- [ ] 2.1 `WorktreeEntry.conversation_started: Option<bool>` (`serde(default, skip_serializing_if = "Option::is_none")`); round-trip and old-file tests
- [ ] 2.2 `attach_agent` sets `Some(false)` with the id; boot-prompt submission (start, spec start, coordinated start, `add`, `submit_held_boot_prompts`) sets `Some(true)` through the session store
- [ ] 2.3 `CliProvider::conversation_exists` (default `None`); Claude row over `usage.rs::claude_transcripts`; tests with a temp config dir (present, absent, no dir)

## 3. Recovery

- [ ] 3.1 Apply the decision table; fresh relaunch assigns and records a new id (one store update after the panes are built)
- [ ] 3.2 Post-launch gate per worker with the fresh command as the relaunch command; record the fallback id and warn
- [ ] 3.3 Deliver per worker: resume message (bundled `assets/resume-nudge.md`, `[workers] resume_nudge` / `resume_message`) or launch prompt (held prompt, else `compose_launch_prompt` from boot block + spec prose by `spec_id` + worktree rule); set the marker; warn on blocked panes and keep undelivered prompts held
- [ ] 3.4 Config keys and docs comments; with `supervisor-vocabulary-split` absent, place under `[supervisor]` and list them in its migration table

## 4. Drive loop

- [ ] 4.1 Pure `detect_wake(prev, now, gap) -> Option<Duration>`; `[drive] wake_gap_seconds` (default 120, 0 off)
- [ ] 4.2 `decide_wake_reengagement(panes) -> Vec<DriveEffect>` (relaunch exited, nudge idle via `assets/wake-nudge.md`, escalate busy); new `DriveEffect` and escalation kind applied by the shell

## 5. Tests per scenario

- [ ] 5.1 Unit: decision table (probe yes/no/unknown × marker true/false/unset × template present/absent)
- [ ] 5.2 E2E (isolated `HOME`/`XDG_DATA_HOME`, a fake CLI script with `session_id_args`/`resume_args` that exits on an unknown resume id): stop before first prompt → `start` relaunches fresh with a new id; fallback after an exit; resumed panes get the message once; opt-out and custom text; paused restart sends nothing; held prompt delivered on recovery; blocked pane warned
- [ ] 5.3 E2E: `start --unattended` on a stopped session nudges before the first tick
- [ ] 5.4 Drive loop with the fake clock: 6 h wall gap detected; slow tick not; disabled; idle nudged once, exited relaunched, busy escalated with no keystroke

## 6. Docs

- [ ] 6.1 `docs/src/user-guide/session-lifecycle.md` (resume vs fresh, the nudge, host sleep in unattended mode, attended sleep is manual)
- [ ] 6.2 Configuration reference: `resume_nudge`, `resume_message`, `wake_gap_seconds`
- [ ] 6.3 Orchestrator skill: drop the "type continue into each pane after recovery" advice; describe the wake escalation

## 7. Gates

- [ ] 7.1 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
- [ ] 7.2 `openspec validate worker-resume-recovery --strict`
