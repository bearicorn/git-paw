## Context

Worker guidance about peer dependencies is authored in two places that must agree:

- **`src/agents.rs` `build_inter_agent_rules`** (`:147-181`) — compiled prose assembled
  per wave and injected into each worker's gitignored sidecar under
  `## Inter-Agent Rules`. `generate_worktree_section` (`:219`) copies the string
  **verbatim**; unlike `skill_content`, it never passes through the placeholder
  renderer.
- **`assets/agent-skills/coordination.md`** — the bundled, `include_str!`-embedded
  coordination skill, rendered through `skills::render()` (so its `{{…}}` placeholders
  *are* substituted).

Both currently tell a blocked worker to cherry-pick a peer's commit. The skill goes
further and supplies the command (`### Cherry-pick peer commits`, ~`:511-527`), framed
explicitly as "rather than waiting for the supervisor to merge".

A third, *correct* mechanism already exists and is specified: `agent.advanced-main`
(`coordination.md` → *When main advances*). The supervisor publishes it on every merge
into the default branch; the worker fetches, inspects `git log HEAD..origin/<base>`, and
deliberately chooses rebase / merge / wait. That path takes on peer work **through the
base branch after it has landed**, which is why it does not duplicate commits. The
cherry-pick guidance is effectively a shortcut around it that trades mergeability for
immediacy.

## Goals / Non-Goals

**Goals:**

- Remove the instruction to graft a peer's commit onto the worker's own branch, from
  both authoring sites, so the two agree.
- Leave a blocked worker with an explicit, specified recourse rather than a gap.
- Keep supervisor-side integration language intact — the supervisor *does* cherry-pick
  and merge worker branches onto the release line, and that statement is correct.
- Resolve the unsubstituted `{{BRANCH_ID}}` in the adjacent peer bullet, since those
  lines are being rewritten anyway.

**Non-Goals:**

- An automatic supervisor-side mid-session rebase of live worker branches. Separate
  change; it carries its own design question (when it is safe to rewrite a live
  worker's history) and must not be pre-empted here.
- Changing `agent.advanced-main` semantics, payload, or the worker-side judgment
  discipline that surrounds it. This change *points at* that path; it does not alter it.
- Moving `build_inter_agent_rules` prose into an asset. That is a separate
  export-agnosticism change; doing it here would couple a correctness fix to a
  refactor.
- Removing peer `exports` advertisement or the `agent.artifact` manual escape hatch.

## Decisions

**D1 — Escalate-and-continue, not escalate-and-block.** The replacement rule is
"publish `agent.blocked` naming the peer and what you need, then continue on unblocked
work or wait." *Considered and rejected:* "publish `agent.blocked` and stop." That would
idle a worker that usually has other tasks in its slice, and it converts a soft
dependency into a hard stall — the opposite of what the coordination skill teaches
elsewhere (agents MUST NOT block on broker silence).

**D2 — Point at `agent.advanced-main` rather than invent a new mechanism.** The worker
already has a specified, judgment-gated way to absorb peer work once it lands. Naming it
in the replacement text costs nothing and prevents the removal from reading as "you have
no options." *Considered and rejected:* adding a new `agent.needs-rebase` message —
unnecessary; it would duplicate `agent.blocked` plus the existing event.

**D3 — Edit both sites in one change.** *Considered and rejected:* fix
`build_inter_agent_rules` now and the skill later. The two are injected into the same
sidecar; leaving the skill's ready-made `git cherry-pick` command in place would let a
worker follow it while the rules bullet forbids it — strictly worse than either
consistent state.

**D4 — Assert the *absence* of `git cherry-pick` in the skill, not merely the presence
of the new section.** The failure mode being closed is guidance the worker can act on,
so the test must pin absence. The supervisor-side sentence (~`:471`) says
"the supervisor cherry-picks", not `git cherry-pick`, so an absence assertion on the
exact substring `git cherry-pick` is safe and does not force deleting correct prose. A
companion scenario pins that the supervisor-side language survives, so an implementer
cannot satisfy the first assertion by blanket-deleting every mention.

**D5 — Substitute `{{BRANCH_ID}}` at the source rather than routing
`inter_agent_rules` through the renderer.** `build_inter_agent_rules` already receives
the peer branch list, so it can interpolate the agent's own id directly. *Considered and
rejected:* passing `inter_agent_rules` through `skills::render()` — that would widen the
renderer's contract to a second, differently-shaped input and risks substituting
placeholders in wave-specific text that is not a skill template.

## Risks / Trade-offs

- **A worker genuinely needs a peer's code to proceed and now waits longer.** →
  Mitigated by D1 (continue on unblocked work) and by `agent.advanced-main` firing on
  every merge, so the wait is bounded by the supervisor's merge cadence rather than
  open-ended. The trade is deliberate: a slower worker beats an unmergeable branch.
- **Test churn is broad but shallow.** The `Cherry-pick peer commits` heading and the
  `git cherry-pick` substring are pinned in `src/skills.rs`, `src/agents.rs:1866`, and
  the `tests/*_skill_content.rs` parity tests. → Grep the two literals across `src/` and
  `tests/` before finishing; every hit is either inverted or removed.
- **Docs may `{{#include}}` the edited skill region.** → Re-run `mdbook build docs/`
  and check the coordination-skill chapter after the asset edit; a stale include is a
  silent doc regression.
- **Removing guidance without the rebase change landing leaves a real gap for a worker
  blocked late in a wave**, when the supervisor may not merge again soon. → Accepted
  for this change; `agent.blocked` surfaces the worker to the supervisor, which is the
  intended escalation. The mid-session rebase change closes the residual gap.
