## 1. Git helper (`src/git.rs`)

- [ ] 1.1 Add `branch_is_merged_into_with(runner, repo_root, branch, default_branch) -> Result<bool, PawError>` beside `branch_is_behind_default_with` (`git merge-base --is-ancestor <branch> <default>`; `0` → merged, `1` → not, other → `Err`), plus a `RealCommandRunner` wrapper; add `unmerged_commit_count` / `branch_tip_short` helpers (`rev-list --count <default>..<branch>`, `rev-parse --short <branch>`) through the same seam
- [ ] 1.2 Unit tests with the fake runner for all three exit paths, and one real-repo integration test (a branch with one commit ahead → not merged; after fast-forwarding the default → merged)

## 2. Ref-derived exit summary (`src/supervisor/drive.rs`, `src/commands/supervisor.rs`)

- [ ] 2.1 Add `branch: String` to `AgentPane`; populate it in `cmd_supervisor` from `WorktreeEntry.branch`; update every test constructor
- [ ] 2.2 Add a `MergeCheck` trait to `DriveDeps` (`fn merge_state(&self, branch: &str) -> Result<MergeState, PawError>`), a git-backed impl wired in `run_drive_loop` (resolving the default branch once via `git::default_branch`), and a fake for tests
- [ ] 2.3 After the loop breaks: query every coding branch; an `Err` counts as unmerged with the reason; push one `Escalation { verdict: "unmerged", .. }` per unmerged branch directly onto `escalations` (never via `record_escalation`); derive `Completed` / `EscalatedForReview` from the resulting list; build `merged_summary` from the ref-derived count for every outcome with coding agents
- [ ] 2.4 Extend `resolved_agent_states` / `DriveSummary::render` so each agent line carries `merged` or `unmerged (N ahead of <default>, tip <sha>)` alongside its resolved phase; keep GP-05's monotonic phase behaviour and its existing tests green
- [ ] 2.5 Unit tests: two `done` agents / none merged → `0/2 branches merged`, `escalated-for-review`, two `unmerged` entries with tips; all merged → `completed`, `2/2`; comparison error → unmerged with reason; no keystrokes dispatched for the unmerged entries with an orchestrator pane present

## 3. CLI-unavailability markers (`src/config/prompt_profile.rs`, `src/main.rs`)

- [ ] 3.1 Add `cli_unavailable_markers: Vec<String>` to `CliPromptProfile`, `Option<Vec<String>>` to `CliPromptProfileOverride`, per-field fallback in the resolver; embedded Claude default carries `You've hit your session limit` plus the usage-limit and login-loss phrasings confirmed against the current Claude Code build
- [ ] 3.2 Add the field to `cmd_prompt_profile`'s `__prompt-profile` dispatch; add it to the adversarial override in `tests/prompt_profile_security_boundary.rs`
- [ ] 3.3 Unit tests: default resolves the session-limit marker; an override wins and the hidden command prints it; a partial override without the field falls back to the default

## 4. Detection and outcome (`src/supervisor/drive.rs`, `src/commands/supervisor.rs`)

- [ ] 4.1 Add `DriveOutcome::CliUnavailable` (label `cli-unavailable`); add a tail-only `cli_unavailable_line(capture) -> Option<String>` using the resolved profile's markers
- [ ] 4.2 In the sweep: orchestrator pane match → break the loop with `CliUnavailable`, recording the agent and quoted line for the summary; coding pane match → `Escalation { verdict: "cli-unavailable", .. }` through the existing dedup path, loop continues
- [ ] 4.3 `DriveSummary::render` prints the pane's agent and the quoted line for `cli-unavailable`; `cmd_supervisor` returns an error after printing when the outcome is `CliUnavailable` so the process exits non-zero; all other outcomes keep their exit status
- [ ] 4.4 Unit tests: orchestrator tail marker → `cli-unavailable`, summary quotes the line; worker tail marker → escalation, other worker still approved, final outcome not `completed`; marker only in scrolled-away history → no action; no marker → unchanged outcome
- [ ] 4.5 E2E (`tests/unattended_drive_e2e.rs`): a dummy worker publishes `done` with a commit ahead of the default branch → summary shows `0/1 branches merged`, an `unmerged` entry, outcome `escalated-for-review`, exit `0`; same with the branch fast-forwarded into the default → `completed`, `1/1`; an orchestrator dummy printing `You've hit your session limit` → `cli-unavailable` and non-zero exit

## 5. Docs and gates

- [ ] 5.1 `docs/src/user-guide/supervisor.md`: add `cli-unavailable` to the exit-condition list and outcome labels; rewrite the summary paragraph so the `N/M branches merged` roll-up is described as ref-derived and `completed` as all-merged; describe the `[unmerged]` entries and the non-zero exit
- [ ] 5.2 `docs/src/user-guide/cli-prompt-profiles.md`: add the `cli_unavailable_markers` row (consumer: the unattended drive loop); configuration reference if it enumerates profile fields
- [ ] 5.3 Every scenario maps to a named test; `cargo fmt`; `just check` + `just deny` green; `mdbook build docs/` succeeds

## 7. Amendment — connection-drop markers (folded in 2026-09-15)

- [ ] 7.1 Add `API Error` and `Connection closed mid-response` to the embedded Claude profile's `stream_error_markers` in `src/config/prompt_profile.rs` — NOT to `cli_unavailable_markers`
- [ ] 7.2 Add a unit test asserting a capture containing `API Error: Connection closed mid-response` matches `stream_error_markers` and does NOT match `cli_unavailable_markers`
- [ ] 7.3 Re-run `openspec validate drive-loop-exit-summary-truth --strict`
