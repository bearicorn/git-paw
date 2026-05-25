## Context

Three skill-content drifts captured during Batch 2 dogfood. Each is a one-paragraph rule the bundled skill needs to teach; bundling into one change avoids three micro-PRs.

## Decisions

### D1 — Commit cadence is per task GROUP, not per task

The drift observation was agents accumulating work across many tasks. Two grain levels were considered:

- **D1a (chosen): per task GROUP (e.g. `## 1.`, `## 2.`).** A group typically corresponds to a coherent unit (StatusPayload field rollout, dashboard prompt-inbox removal, etc.). Per-group commits map naturally to conventional-commit messages and the project's existing release-notes flow. Bounds: a group SHALL fit in roughly 10 files; if larger, the agent splits with `(part N of M)`.

- **D1b (rejected): per individual task (`- [ ] 1.1`).** Too granular; produces 20-50 commits per change with low-information messages. The project's existing v0.5.0 cycle commits per group, not per task.

The skill SHOULD recommend the GROUP grain but allow per-task commits for very small tasks. The threshold "~10 uncommitted files" is a soft cap, not a hard limit.

### D2 — Pane resolution is `pane_current_path`, not session JSON lookup

Two ways to map a pane to an agent:

- **D2a (chosen): `tmux display-message -t <session>:0.<pane> -p '#{pane_current_path}'`.** Tmux is the source of truth for what's running where. The output is the worktree path, which trivially yields the branch name via `basename`. Works during a session without external state.

- **D2b (rejected): read `<repo>/.git-paw/sessions/*.json` and cross-reference the worktrees list against pane indices**. The session JSON lists worktrees but doesn't record pane indices — there's no mapping to inspect. Even if the launcher were modified to persist `pane_index: u32` per worktree (it isn't), tmux remains the ground truth in case the user manually swapped panes.

The bundled `sweep.sh` (drift 68 §8c) implements this; teaching the underlying tmux command in the skill lets the supervisor agent fall back to direct invocation if the helper script is missing.

### D3 — `/opsx:verify` and `/opsx:archive` are off-limits to coding agents

Both opsx skills exist (per the available-skills list) and are intended for the supervisor's verification + merge flow. The coding agent's role ends at `agent.artifact{status:done}` (or the implicit committed via post-commit hook).

Why explicit prohibition rather than silence:

- Silence didn't work. Both Batch-2 coding agents typed `/opsx:verify <change-id>` themselves; one typed `/opsx:archive <change-id>` and was caught only by my intervention.
- The opsx skills are documented by openspec upstream as "verify implementation matches change artifacts" — that phrasing reads (to a coding agent) like a natural step in finishing their own work.
- The bundled coordination skill is the right place to disambiguate ownership.

The prohibition is paw-specific. In repos where the coding agent IS the verifier (single-agent workflow), this rule does not apply — but those repos don't use git-paw's bundled coordination skill either.

## Risks / Trade-offs

- **Skill verbosity.** Three more subsections add ~30-40 lines to the coordination + supervisor skills. The skills are already large; adding more doctrine risks dilution. Mitigation: each new section is short (≤10 lines) and uses bullet lists for scannability.

- **Test surface inflation.** Each new subsection adds 1-2 skill-content tests. Acceptable cost.

- **`/opsx:verify` for the agent IS valid in other workflows.** A user adopting paw for single-agent solo work might want the agent to self-verify. The prohibition lives in the bundled coordination skill, which the user can override per the existing skill-resolution-order requirement (`<config_dir>/git-paw/agent-skills/coordination.md` user override wins).

## Migration / Rollout

- Fully additive — no breaking changes.
- Existing skill content untouched; new subsections appended.
- No version bump beyond v0.5.0; part of the release prep.
- User overrides at the standard skill paths continue to work.
