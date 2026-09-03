## Why

git-paw tells every worker, in the `## Inter-Agent Rules` block injected into its
sidecar, to cherry-pick a peer's commit when blocked and to "not wait for the
supervisor to merge" (`src/agents.rs:172-175`), and the bundled coordination skill
repeats the instruction with a ready-made command (`### Cherry-pick peer commits`).
Following that advice manufactures same-content/different-SHA duplicate commits: when
the supervisor later merges the peer's *own* branch into the default branch, the
cherry-picking worker's branch is divergent and unmergeable, and its work is discarded.

The peer-4-poker dogfood (2026-08-31, GP-18) hit exactly this — a `[P]` worker branched
at `939dc1c` and ended up carrying divergent copies of T013, T014 and T015–T021. This is
systemic rather than incidental: the instruction ships to *every* worker, so it fires on
any wave where a worker blocks on a peer. git-paw manufactures the unmergeable branch
through its own guidance.

## What Changes

- The inter-agent rules SHALL NOT instruct a worker to cherry-pick a peer's commit into
  its own branch. The rule is replaced with: publish `agent.blocked` naming the peer and
  what you need, then continue on unblocked work or wait — the supervisor owns
  integration.
- The bundled coordination skill's `### Cherry-pick peer commits` section is replaced by
  a peer-dependency section carrying the same escalation discipline: an arriving peer
  `agent.artifact` is a *signal*, not an instruction to graft the peer's commit.
- **Worker recourse is preserved, not removed.** A blocked worker keeps two existing
  paths: `agent.blocked` escalation, and the already-specified `agent.advanced-main`
  discipline (`coordination.md` → *When main advances*), under which a worker
  deliberately fetches and rebases onto the base **after** the supervisor has merged.
  That path integrates peer work through the base branch, where it does not duplicate
  commits.
- Peer `exports` advertisement stays — announcing named exports remains useful; only the
  "graft their commit onto your branch" action is withdrawn.
- **Adjacent defect fixed in the same lines:** the *Watch peer status* bullet
  (`src/agents.rs:171`) emits a literal `{{BRANCH_ID}}` that nothing substitutes —
  `generate_worktree_section` copies `inter_agent_rules` verbatim (`src/agents.rs:219`),
  and unlike `skill_content` it never passes through the placeholder renderer. Every
  worker's sidecar therefore ships an unrendered placeholder in the poll URL it is told
  to use. The two peer bullets are being rewritten together, so the placeholder is
  resolved here rather than left broken in freshly-edited lines.

**Explicitly out of scope:** an automatic supervisor-side mid-session rebase of live
worker branches. That is a separate change with its own design question (when it is safe
to rewrite a live worker's history); this change deliberately routes a blocked worker to
the supervisor rather than pre-empting that decision.

**Not breaking:** guidance-only. No CLI surface, config field, wire message, or state
shape changes.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `skill-agent`: the *Embedded coordination skill* requirement drops the mandated
  `### Cherry-pick peer commits` section (item 5) and instead requires a peer-dependency
  section that directs a blocked worker to publish `agent.blocked` and defer integration
  to the supervisor.
- `boot-agents-md`: the *Generate worktree assignment section* requirement no longer
  describes the supervisor-populated inter-agent rules as carrying "cherry-pick
  instructions"; the rules carry peer-dependency escalation instead. The rendering
  contract (verbatim, marker placement, `None` behaviour) is unchanged.

## Impact

- **Code:** `src/agents.rs` `build_inter_agent_rules` — replace the *Cherry-pick peer
  artifacts* bullet (`:172-175`). The surrounding bullets (file ownership, commit-never-
  push, automatic status publishing, watch peer status) are unchanged.
- **Assets:** `assets/agent-skills/coordination.md` — replace the
  `### Cherry-pick peer commits` section (~`:511-527`) and reword the two
  "peers should cherry-pick" mentions in the `agent.artifact` / exports guidance
  (~`:438`, `:457`).
  **Do NOT blanket-remove every `cherry-pick` mention:** the statement that *the
  supervisor* cherry-picks and merges the worker's branch onto the release line
  (~`:471`) describes correct supervisor behaviour and SHALL remain.
- **Tests:** three sites pin the literals and must invert (verified by grep; there are
  **no** `cherry` hits under `tests/`):
  `src/agents.rs:1866` `build_inter_agent_rules_contains_cherry_pick_reference`,
  `src/agents.rs:1876` `embedded_coordination_contains_cherry_pick`, and
  `src/skills.rs:1092` `coordination_skill_contains_cherry_pick_instructions`.
  The doc comment at `src/agents.rs:135` also names cherry-pick and must be reworded.
- **MUST NOT be touched — a legitimate, unrelated use of the same word:**
  `src/agents.rs:518`, the branch-guard git hook's `agent.feedback` text, tells an agent
  that committed to the *wrong branch* to cherry-pick onto the expected branch and reset.
  That is self-correction, not peer grafting, and it SHALL remain.
- **Enum-variant ripple:** none — no `BrokerMessage` or `SpecBackendKind` variant is
  added or removed. `agent.blocked` and `agent.artifact` already exist.
- **Backward compatibility:** prose-only. Existing sessions, configs and session files
  load and behave unchanged; a worktree whose sidecar was generated by an earlier
  version keeps its old text until regenerated.
- **Docs:** `docs/src/user-guide/coordination.md:507` paraphrases the exports guidance as
  "announce named `exports` peers should cherry-pick" and must be reworded. Note this
  page is a hand-maintained **paraphrase**, not an `{{#include}}` of the skill — it
  drifts silently, so it needs an explicit edit rather than a rebuild.
  `docs/src/user-guide/coordination.md:517` and `docs/src/user-guide/supervisor.md:331`
  describe *supervisor-side* cherry-picking and SHALL remain unchanged.
  `mdbook build docs/` must still succeed.
