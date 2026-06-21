## Context

The `agent-skills` capability already carries two views of commit-message
format that now contradict each other:

- The "Embedded coordination skill" requirement (item 13) and its scenario
  "Coordination skill defers commit-message format to the project AGENTS.md"
  already say the bundled skill SHALL NOT mandate a Conventional-Commits format
  and SHALL defer to the host project's injected `AGENTS.md`.
- The "Coordination skill SHALL teach per-group commit cadence" requirement
  still says (item 3) the commit message "SHALL follow the project's
  conventional-commit pattern", and its scenario "Coordination skill names
  conventional-commit types" *requires* the skill to show a conventional-commit
  prefix example.

The shipped `assets/agent-skills/coordination.md` already implements the
de-opinionated behaviour: its "Commit cadence" section defers message format to
`AGENTS.md` and presents `feat(<scope>):` only as an illustrative example. So
the contradiction is purely a stale main-spec requirement, not a code gap.

## Goals / Non-Goals

**Goals:**
- Remove the mandatory-Conventional-Commits prescription from the "per-group
  commit cadence" requirement so the two `agent-skills` requirements agree.
- Make the spec/test contract match the already-shipped skill text.

**Non-Goals:**
- Changing the per-group cadence rules (commit per task group, ~10-file soft
  cap, `(part N of M)` split) — those stay.
- Changing the releasable-unit / amend-fixup discipline (its own requirement) —
  that stays.
- Touching `lang-agnostic-skills` — that capability governs language-leak and
  example rotation, not commit-message format.
- Any embedded-skill source edit — `coordination.md` is already de-opinionated.

## Decisions

**Decision: MODIFY the "Coordination skill SHALL teach per-group commit cadence"
requirement rather than REMOVE its conventional-commit scenario.**
The requirement is still valid in full — only item 3's wording and one scenario
need softening. A MODIFIED delta preserves the cadence content (items 1, 2, 4
and the first scenario) while reframing the format prescription. Removing the
scenario outright would lose the per-group/soft-cap coverage it shares context
with; reframing it keeps a positive assertion (the section defers to
`AGENTS.md`) that is directly testable.

Alternative considered: REMOVE the whole requirement and rely on the
"Embedded coordination skill" item 13. Rejected — item 13 covers only the
"Commit cadence" deferral prose, not the per-group grain / soft-cap behaviour
that this requirement uniquely tests. Removal would drop that coverage.

**Decision: keep `agent-skills` as the only modified capability.**
The contradiction lives entirely in `agent-skills`. `lang-agnostic-skills` has
no commit-format requirement, so it gets no delta.

## Risks / Trade-offs

- [The existing test "Coordination skill names conventional-commit types"
  asserts a prefix example is present] → After this change the spec no longer
  requires that, so the test is reframed to assert deferral to `AGENTS.md` and
  absence of a mandatory-format prescription. The shipped skill already contains
  the deferral prose, so the reframed test passes without a skill edit.
- [An illustrative `feat(<scope>):` example still appears in the skill] → This
  is allowed: item 13 already permits a Conventional-Commits prefix as an
  illustrative example, just not as a mandate. The reframed scenario asserts
  only that no MUST/SHALL-format language remains.

## Migration Plan

1. Apply the MODIFIED delta to `agent-skills`.
2. Reframe the affected skill-content test to match the new scenario.
3. `openspec validate commit-prefix-de-opinionation --strict`, then archive on
   the release branch (deltas sync into `openspec/specs/agent-skills/spec.md`).

No runtime migration: behaviour is unchanged for consumers; the bundled skill
already defers commit-message format to `AGENTS.md`.
