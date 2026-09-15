## Why

The unattended drive loop's exit summary can claim success having merged nothing. In the 0.16.0
external dogfood (peer-4-poker, 2026-09-14) the summary printed `completed`, `2/2 branches merged`,
`Escalations awaiting human review: 0` while `main` was unchanged from its baseline, no worker commit
was an ancestor of it, and both worker branches were still ahead. The supervisor CLI had hit
`You've hit your session limit` about three minutes in, so nobody was ever going to merge — and the
summary is the only thing an unattended operator reads before running `git paw purge`, which makes
this the first link of a data-loss chain (see `purge-unmerged-branch-safety`, GP-20).

The cause is in `src/supervisor/drive.rs::drive_loop`. The `N/M branches merged` line (added by
0.16.0's GP-05 fix) counts coding agents whose most-progressed `WorkerPhase` reached rank 2 —
`verified` or `done` — and *labels that count "merged"*. Nothing in the loop consults a git ref.
`detect_completion` likewise fires `AllTasksChecked` when every agent's `agent.status` is `verified`
/ `done`, so a wave whose workers finished their own tasks reads as a completed, fully merged wave
regardless of what happened on `main`. GP-05 fixed a real bug (a finished worker's later `working`
heartbeat used to regress its final state) and its monotonic phase tracking is kept intact here;
what this change corrects is the over-reach that equated "worker finished" with "branch merged".

The second half of the finding has no code at all today: no marker set, detector, or outcome exists
for a CLI that has stopped serving its pane (a usage/rate limit, an expired or missing login). Such
a supervisor pane is silently swept like any idle pane until the workers finish or the 25-minute
heartbeat fires. That is exactly the case in which the summary must not claim success.

## What Changes

- **Merged-ness comes from refs, not loop state.** At exit the loop asks git, once per coding
  branch, whether the branch tip is an ancestor of the default branch
  (`git merge-base --is-ancestor <branch> <default>`, default resolved through the single
  default-branch resolver). Only an ancestor counts as merged. A comparison git cannot make (a
  deleted branch, a bad ref) counts as **unmerged** — the fail-safe direction, because the summary
  feeds a destructive `purge`.
- **The summary tells the whole truth.** The `N/M branches merged` line uses the ref-derived
  count; every unmerged branch is listed with its unmerged-commit count and tip SHA; each agent's
  line carries its resolved phase *and* its merged/unmerged state. The GP-05 phase resolution
  (never regress a finished worker to `working`) is unchanged.
- **`completed` means merged.** A wave with any unmerged branch at exit is reported
  `escalated-for-review`, with one `[unmerged]` escalation entry per branch, so the escalation
  count can no longer read `0` while work sits un-integrated. These entries are appended to the
  summary directly — they are never typed into the orchestrator pane (wind-down rule, GP-16).
- **CLI unavailability is an explicit non-success exit.** The CLI prompt-shape profile gains a
  `cli_unavailable_markers` list (the embedded Claude Code default carries its session/usage-limit
  and authentication-failure phrasings). When the orchestrator pane's capture *tail* shows one, the
  loop exits at once with the new outcome `cli-unavailable`, the summary names the pane and quotes
  the matching line, and the `--unattended` process exits non-zero. When a coding agent's pane
  shows one, the loop records a deduped `cli-unavailable` escalation for that agent and keeps
  driving the others.
- `AgentPane` carries the agent's branch name (needed for the ref check); `DriveDeps` gains an
  injected merge-check seam so the loop stays testable without a real repository.

Not changed: completion detection itself (a wave still *ends* when the workers finish or the
supervisor publishes a verdict), the heartbeat, the correction loop, and exit status for every
pre-existing outcome.

## Capabilities

### New Capabilities

<!-- None — both fixes land in the existing unattended-operation and prompt-profile capabilities. -->

### Modified Capabilities

- `supervisor-unattended-operation`: **Exit summary** derives merged/unmerged from refs, lists
  unmerged branches, and withholds `completed` when any branch is unmerged; new requirement —
  CLI unavailability (rate limit / auth loss) is an explicit non-success exit reason.
- `cli-prompt-profiles`: the profile's minimum field set gains `cli_unavailable_markers`; the
  embedded Claude Code default carries the session-limit and login-failure markers.

## Impact

- **Code — offending sites:** `src/supervisor/drive.rs` — `drive_loop` (the `merged_summary`
  closure counting `resolved_phase.rank() == 2`, and the `winding_down` → `DriveOutcome::Completed`
  break), `resolved_agent_states`, `DriveSummary::render`, `DriveOutcome` + `label()`,
  `AgentPane` (add `branch`), `DriveDeps` (add a merge-check seam), `sweep_pane` (marker check on
  the capture tail). `src/commands/supervisor.rs::cmd_supervisor` (populate `AgentPane.branch`
  from the session's `WorktreeEntry.branch`; map `cli-unavailable` to a non-zero exit after the
  summary is printed). `src/git.rs` — a `branch_is_merged_into_with` helper beside the existing
  `branch_is_behind_default_with` (same `CommandRunner` seam, opposite `--is-ancestor` direction).
- **Prompt profile ripple:** `src/config/prompt_profile.rs` (`CliPromptProfile`,
  `CliPromptProfileOverride`, the per-field resolver, the embedded Claude literals),
  `src/main.rs::cmd_prompt_profile` (`__prompt-profile` field dispatch, so `sweep.sh` can read the
  new field — "Rust and the supervisor helper script read one source"),
  `tests/prompt_profile_security_boundary.rs` (the adversarial override must name the new field),
  `docs/src/user-guide/cli-prompt-profiles.md` field table.
- **Enum-variant ripple:** `DriveOutcome` gains `CliUnavailable`; it is matched only in
  `src/supervisor/drive.rs` (`label()`, tests) and documented in
  `docs/src/user-guide/supervisor.md`. `BrokerMessage` and `SpecBackendKind` are untouched.
- **Docs:** `docs/src/user-guide/supervisor.md` (exit-condition list, outcome labels, the summary
  paragraph that currently describes the `N/M branches merged` roll-up as phase-derived);
  `docs/src/user-guide/cli-prompt-profiles.md`; configuration reference only if the table
  enumerates profile fields.
- **Behaviour an existing user notices:** a wave that previously ended `completed` /
  `N/N branches merged` with nothing merged now ends `escalated-for-review` / `0/N branches
  merged` with `[unmerged]` entries — the intended correction. Exit status changes only for the
  new `cli-unavailable` outcome; `completed`, `escalated-for-review`, `heartbeat`, `stuck`, and
  `session-torn-down` keep their current status.
- **Security / safety:** read-only git (`merge-base`, `rev-parse`, `rev-list --count`) on the
  repo root; branch names come from the session file and are passed as discrete arguments, never
  interpolated into a shell. The profile field is prompt *shape* only — it can widen no gate
  ("Security decisions are never profile-sourced" holds). The change only ever makes the summary
  *less* optimistic and the exit *louder*; it deletes nothing and sends no new keystrokes.
- **Dependencies:** none.
