## Context

`drive_loop` (`src/supervisor/drive.rs`) already tracks the most-progressed `WorkerPhase` per agent
(GP-05) and prints an `N/M branches merged` line for a `Completed` wave. That line counts agents at
phase rank 2 (`verified` / `done`); no ref is consulted anywhere in the loop, and `AgentPane` does
not even carry the branch name. Completion (`detect_completion`) fires when every worker reports
`verified` / `done` or the supervisor row carries a verdict status — so a supervisor that died three
minutes in (session limit) still yields `completed`, `N/N branches merged`, `0 escalations` once the
workers finish their own tasks. The 0.16.0 dogfood read that summary and ran `git paw purge`.

Git already knows the truth in one call per branch: `git merge-base --is-ancestor <branch> <default>`.
`src/git.rs` has the mirror-image helper (`branch_is_behind_default_with`, used by branch refresh)
and the single default-branch resolver (`git::default_branch`, GP-12). No detector exists for a CLI
that has stopped serving its pane; `CliPromptProfile` is where every other prompt-shape literal lives
and `sweep.sh` reads it through `git paw __prompt-profile`.

## Goals / Non-Goals

**Goals:**
- The exit summary's merged/unmerged claims are true by construction (refs), and `completed` implies
  every branch is merged.
- A rate-limited or logged-out CLI is a loud, explicit, non-success exit — not a silent idle pane.
- Keep GP-05's phase resolution intact (never regress a finished worker to `working`).
- Smallest possible change: no `drive.rs` split, no new module, one new outcome variant, one new
  profile field.

**Non-Goals:**
- Changing *when* the wave ends (completion detection, heartbeat) — only what the exit says.
- Merging anything on the operator's behalf, or retrying a rate-limited CLI.
- Exit-status changes for pre-existing outcomes (`heartbeat`, `escalated-for-review`, …).
- Detecting rate limits from broker messages or CLI log files — pane tail only, via the profile.

## Decisions

### D1 — Refs are the merge truth; status stays the progress truth

`merged` is answered by `git merge-base --is-ancestor <branch> <default>` at exit, once per coding
branch. The default branch comes from `git::default_branch` (the single resolver), not a local
re-implementation. A new `branch_is_merged_into_with(runner, repo_root, branch, default)` sits beside
`branch_is_behind_default_with` in `src/git.rs` with the same `CommandRunner` seam and the same
exit-code contract (`0` → merged, `1` → not, anything else → error). The unmerged-commit count and
tip come from `git rev-list --count <default>..<branch>` and `git rev-parse --short <branch>`.

The loop reaches git through a small injected trait on `DriveDeps` (`MergeCheck`, one method returning
`Result<MergeState, PawError>` with `{ merged: bool, ahead: usize, tip: String }`), mirroring how
`refresher` and `status` are injected — the existing seam pattern, not a new abstraction. Tests use
a fake; `run_drive_loop` wires the git-backed implementation.

`AgentPane` gains `branch: String`; `cmd_supervisor` fills it from `WorktreeEntry.branch` (the same
entry whose slug already forms `agent_id`).

### D2 — Fail-safe: undeterminable means unmerged

If the comparison errors (branch deleted mid-wave, unreadable repo), the branch is reported unmerged
with the reason. The summary is the input to a destructive `purge`; the only acceptable failure
direction is pessimistic. This is the safety decision of the change.

### D3 — `completed` requires all-merged; otherwise `escalated-for-review` with `[unmerged]` entries

Rather than a new "completed-but-unmerged" outcome, an unmerged branch is exactly what the existing
escalation list is for: work a human must act on. One `Escalation { verdict: "unmerged", agent_id,
command: "<branch>: N commit(s) not in <default> (tip <sha>)" }` per unmerged branch is pushed onto
the loop's `escalations` **after** the loop breaks, then the outcome is derived as today
(`escalations.is_empty()` → `Completed`, else `EscalatedForReview`). Pushing directly — not via
`record_escalation` — is deliberate: `record_escalation` hands text to the orchestrator pane, and
nothing may be typed into a pane during wind-down (GP-16). The wind-down learning synthesis sees the
entries and records them, which is desirable.

Per-agent lines become `<agent>: <phase> · merged` / `<agent>: <phase> · unmerged (N ahead of
<default>, tip <sha>)`; `merged_summary` becomes the ref-derived `N/M branches merged` and is printed
for every outcome that has coding agents, not only `Completed` — a `heartbeat` exit benefits from the
same truth.

### D4 — CLI unavailability: profile markers, tail-only, asymmetric response

- **Where the literals live:** `CliPromptProfile.cli_unavailable_markers` (+ the override struct, the
  per-field resolver, the embedded Claude default, the `__prompt-profile` field dispatch, the docs
  table). Profiles are the single source of prompt-shape literals for both the compiled loop and
  `sweep.sh`; a compiled constant in `drive.rs` would re-scatter them. The field is shape only and
  can widen no gate.
- **Claude default:** the session/usage-limit status line (`You've hit your session limit`, the
  usage-limit variant) and the login-loss phrasings (`Please run /login`, invalid/expired
  credential text). The implementer confirms the exact strings against the current Claude Code
  build before pinning them; the spec pins only the one observed in the field.
- **Tail-only:** matched in the same capture tail the live-prompt scan uses, so a marker that has
  scrolled into history — or that an agent merely printed while reading docs — does not fire.
- **Orchestrator pane → exit now.** With the brain gone, approving worker prompts blindly is not
  progress; the heartbeat would otherwise hide the failure for up to 25 minutes, and the human
  needs to know *now* (the limit resets on a clock; with `--resume` the workers come back with
  context). Outcome `CliUnavailable` (label `cli-unavailable`), summary quotes the matching line,
  and `cmd_supervisor` returns `Err(PawError::SessionError(...))` *after* printing the summary so
  the process exits non-zero — the one exit-status change in this release.
- **Coding pane → escalate and continue.** The other workers still benefit from approvals and the
  orchestrator may still merge them; the dead worker's branch will show as unmerged at exit anyway.
  The escalation reuses `sweep_pane`'s dedup path (`(agent_id, shape)`), so it does not re-fire
  every 15 s.

### D5 — What the unhappy paths look like after this change

- **Rate-limited supervisor, attended terminal:** within one poll the process prints
  `Unattended drive loop exited: cli-unavailable`, the quoted limit line, the ref-true
  `0/N branches merged`, per-branch tips, and exits `1`. Nothing is killed: the tmux session and
  worker panes are left running for the operator to attach to or resume later.
- **Detached run (`nohup … &`):** identical output lands in the redirected log; the non-zero
  status is visible to the wrapper. No branch is touched, so nothing is lost while nobody watches.
- **Workers finish, nothing merged, supervisor fine but idle:** `escalated-for-review`,
  `0/N branches merged`, `[unmerged]` entries — the operator sees exactly which branches need
  integration before considering `purge`.

## Risks / Trade-offs

- **Marker false positive on the orchestrator pane** ends the run early. Bounded by tail-only
  matching and by the response being loud and non-destructive (nothing deleted, nothing typed);
  the operator simply re-runs. A false *negative* (new CLI phrasing) degrades to today's behaviour,
  with the ref-derived summary still telling the truth about merges.
- **Waves that intentionally leave merging to a human** now end `escalated-for-review` with
  `[unmerged]` entries instead of `completed`. This is the honest report; the milestone treats a
  false `completed` as the critical defect.
- **One extra git spawn per coding branch at exit** — negligible; runs once.
- **Profile field ripple** touches five files plus a doc table; all compile-forcing except the docs,
  which the doc-completeness gate covers.
