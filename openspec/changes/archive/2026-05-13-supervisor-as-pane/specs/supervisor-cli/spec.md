## REMOVED Requirements

### Requirement: Merge ordering from dependency signals

**Reason**: merge orchestration moves from a Rust subsystem (the `run_merge_loop` function called from `cmd_supervisor` after the foreground supervisor CLI exits) to a supervisor-skill responsibility. v0.4's Rust implementation built a dependency graph from `agent.blocked` messages, computed a topological sort, and ran `git merge` + the configured test command per branch.

In v0.5.0 with `supervisor-as-pane`, `cmd_supervisor` returns immediately after launching the session — it never reaches a point where it could call `run_merge_loop`. Merge orchestration becomes the supervisor agent's job, performed via the existing skill mechanisms (curl `/messages/supervisor` to read events, shell for `git`, the configured `test_command`, curl `/publish` to report results). See the `agent-skills` capability and the new "Merge orchestration" requirement on the supervisor skill.

**Migration**: existing v0.4 deployments that relied on the auto-merge-after-supervisor-CLI-exit behaviour need to either:
- Trust the supervisor agent to perform merge orchestration per its skill (the autonomous loop instructs it to merge once all expected agents have published `agent.verified`); OR
- Merge manually by checking out branches and running `git merge`/test command outside the supervisor session.

The Rust functions `run_merge_loop`, `MergeResult`, `MergeResults` (and any helpers) are deleted. No equivalent Rust API replaces them.

The other supervisor-cli requirements (Supervisor mode resolution chain, Validate specs are committed before launching, Purge warns about unmerged commits) are unchanged by this change.
