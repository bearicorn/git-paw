# v0.5.x release-notes bullets (coordination-skill-followups-2)

These bullets are intended to be copied into the v0.5.x release-prep commit's
release notes / archive plan. They cover three skill-content drifts captured
during the Batch-2 dogfood and surfaced into the bundled coordination and
supervisor skills.

## Highlights

- **Coordination skill teaches per-group commit cadence.** The bundled
  `coordination.md` now instructs the coding agent to commit after each
  numbered task group (`## 1.`, `## 2.`, ...), keep uncommitted work below a
  ~10-file soft cap, split overlong groups with a `(part N of M)` suffix,
  and use a conventional-commit prefix (`feat(<scope>):`, `fix(<scope>):`,
  `docs(<scope>):`, `test(<scope>):`, `chore(<scope>):`) per group.
- **Coordination skill forbids the coding agent from running `/opsx:verify`
  and `/opsx:archive`.** Both are explicitly named as off-limits — they are
  the supervisor's job. Verification runs the supervisor's five-gate
  framework against the committed branch; archive happens on the release
  branch during the supervisor's cherry-pick + merge flow. The coding
  agent's terminal action is a commit (auto-published as
  `agent.artifact { status: "committed" }`) or, for code-less tasks, a
  manual `agent.artifact { status: "done" }`.
- **Supervisor skill teaches `pane_current_path` as the canonical
  pane→agent resolution.** `tmux display-message -t paw-<project>:0.<pane>
  -p '#{pane_current_path}'` returns the pane's worktree path; its basename
  ends in `<project>-feat-<branch>`, yielding the authoritative `agent_id`.
  Pane indices are NOT alphabetical by `agent_id` and NOT in CLI-argument
  order — `git paw status` and dashboard row ordering MUST NOT be used as
  the mapping source.

## Follow-up

- `.git-paw/scripts/sweep.sh` already invokes the `tmux display-message`
  resolution per iteration (drift 68 §8c). No follow-up code change is
  required; the skill addition closes the doctrine half of that drift.
