## Why

GP-09 (peer-4-poker dogfood, 2026-08-31): with N verified branches cut from one base, fast-forward-only
integration serialises the wave tail on a directive → agent rebase → watch → merge cycle **per branch**,
bottlenecked on agent latency. Most of those rebases are mechanical — the branches touch disjoint
files. After v0.18 there is no supervisor pane to "rebase them itself"; the orchestrator is an external
session and merges are its decision. What it lacks is one command that integrates a batch of verified
branches, rebasing the provably-disjoint ones mechanically and handing only the real conflicts back to
agents — in one pass, with one batch of rebase directives.

## What Changes

- `git paw finish` accepts several branches and integrates them **in the given order** as a queue;
  `--verified` selects every session agent whose broker status is `verified`, in session order.
- New `--rebase`: a branch that is not fast-forwardable is rebased onto the advanced target **by
  git-paw** only when it is *provably disjoint* — no path changed on both sides since the merge base
  AND `git merge-tree` reports no conflict — its worktree is clean, its agent surface is idle and claimed,
  and a test command is configured. The rebased worktree is re-tested before the target moves; a failed
  rebase or test restores the branch to its pre-rebase tip.
- Every branch the queue cannot integrate is **skipped** (never forced), and its live agent receives
  **one** `agent.feedback` rebase directive, all published together after the pass.
- Queue report: one line per branch (integrated / integrated after rebase / skipped + reason). Exit 0
  when every branch integrated, 3 when any was skipped by policy, 1 on an error (the queue stops).
- `--json` (the `cli-machine-output` document `finish-command` defines) gains queue entries, an
  `integrated_after_rebase` outcome, `directives` and `error` — additive within schema version 1; a
  queue that stopped after integrating something still prints its document on exit 1.
- The exported orchestrator skill's merge-orchestration procedure calls the queue instead of raw
  `git checkout` + `git merge --ff-only` + `git reset --hard`.
- The drive loop does **not** integrate on its own: merging stays the orchestrator's decision.

## Capabilities

### New Capabilities

<!-- None — extends `git-branch-finish`, introduced by `finish-command`. -->

### Modified Capabilities

- `git-branch-finish` (from `finish-command`): ADDED queue, mechanical rebase, batched directives,
  queue exit status, queue JSON document.
- `skill-agent`: **Supervisor skill — Merge orchestration** uses the queue command.
- `supervisor-launch`: **`docs/src/user-guide/supervisor.md` SHALL consolidate the supervisor
  surfaces** — the merge-orchestration mirror names the queue, not `git merge --ff-only`. Written against
  v0.17 `classifier-retirement`'s version of this requirement (its item 2 *Agent permissions* replaces
  the dev-command allowlist section, kept verbatim here) with v0.18 `orchestrator-skill-export`'s
  changes applied (it also MODIFIES it); rebase over both.

## Impact

- **Code:** `src/commands/finish.rs` (queue loop over the `finish-command` core), a pure
  `decide_rebase(facts) -> RebasePlan` (disjointness from `git diff --name-only <base>..<side>` sets,
  merge-tree result, cleanliness, surface idleness/claim, test-command presence); `src/git.rs`
  (`changed_paths_since_with`, rebase-onto-target generalising `rebase_branch_onto_default_with`,
  restore-to-tip); reuse `supervisor::claim` (surface claim) and the idle check used by
  `supervisor-branch-refresh`; the configured `[supervisor] test_command`; broker publish of
  `agent.feedback`; `src/cli.rs` (`branch` becomes `branches: Vec<String>`, `--verified`, `--rebase`);
  `assets/agent-skills/orchestrator.md` merge section (formerly `supervisor.md`, relocated by v0.18).
- **Enum-variant ripple:** none — `agent.feedback` and `agent.advanced-main` already exist.
- **Docs:** `--help`, `docs/src/cli-reference.md`, `docs/src/user-guide/session-lifecycle.md` (merging a
  wave), the orchestrator workflow chapter written in v0.18, README row.
- **Cross-version dependencies:** builds on `finish-command` (this release; must land first — its
  requirements are extended, not modified, here). v0.17 `classifier-retirement` (its *Agent
  permissions* item in the supervisor-chapter requirement is carried unchanged; git-paw approves no
  permission prompt, so an agent's `git paw finish` prompt reaches a human). v0.18 `broker-verdict-authority` (`agent.feedback` is
  a root-only verb; finish runs from the main working tree and uses the root credential),
  `orchestrator-skill-export` (relocates the supervisor skill; the merge section edited here is the
  relocated copy), `orchestrator-escalation-handoff` (the drive loop's merge hand-off text may point at
  the queue command — wording only). `supervisor-branch-refresh` is unchanged: its "verified branch is
  never refreshed" rule governs the drive loop; the queue's integration-time rebase is a different,
  stricter path (design D2).
- **Security / safety:** rewrites a verified branch only when its own patch provably survives unchanged
  and the configured tests pass on the result; never touches a dirty or busy worktree; never forces.
  See design.md.

## Rebase note (2026-10-19)

This change uses the pre-v0.21 supervisor vocabulary. If it archives after
`supervisor-vocabulary-split`, the user-facing text it adds (flags, config keys, help, docs, skill
text, broker wire values) SHALL use the new names (`--coordinated` / `--bare`, `[orchestrator]`
(+ `[orchestrator.gates]`) / `[workers]` / `[drive]`, `default_mode`, root identity `orchestrator`).
Its deltas are not rewritten: they archive against today's names, which that change's reading rule
maps and v1.0.0's `remove-supervisor-vocabulary` sweep renames (including the headers *Supervisor
skill — Merge orchestration* and *`docs/src/user-guide/supervisor.md` SHALL consolidate the
supervisor surfaces* and the `supervisor-launch` capability). Old terms used here:

- `[supervisor] test_command` (proposal, design, tasks, git-branch-finish scenario) →
  `[orchestrator.gates] test_command`
- `{from: "supervisor", …}` rebase directives (design) → `from: "orchestrator"`
- `broker.sh --agent supervisor` and `agent_id = "supervisor"` (skill-agent copy) →
  `--agent orchestrator`, `agent_id = "orchestrator"`
- `[supervisor] agent_approval` / `approval` (supervisor-launch copy, *Agent permissions*) →
  `[workers] approval` / `[orchestrator] approval`; `[supervisor.auto_approve]` and
  `[supervisor.common_dev_allowlist]` keep their names there — they are deprecated with no effect
  and version 2 drops them rather than renaming them
- the `[supervisor]` gate-command keys (supervisor-launch copy) → `[orchestrator.gates]`
- "Supervisor skill" / `supervisor.md` → the `orchestrator` skill (the `supervisor` name warns)
- "supervisor chapter" `docs/src/user-guide/supervisor.md` → same path, retitled "Coordinated
  mode" (renamed at 1.0.0)
- Kept (not user-visible): `supervisor::claim`, `src/supervisor/` paths; capability name
  `supervisor-branch-refresh` until the 1.0.0 rename.
