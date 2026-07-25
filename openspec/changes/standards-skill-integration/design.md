# Design — standards-skill-integration

## Context

`test-strategy` and `code-standards` codify standards as skills. This change makes them *load-bearing*
by wiring both consumers to them: the worker at author-time, the supervisor at gate-time.

## Decisions

### D1 — Agnostic by deferral to the project's own `AGENTS.md` / `.agents/skills/`

A project's gate process is already defined by its own `AGENTS.md` (git-paw's gate commands are
templated from consumer config). So "consult the project's standards skills" imposes nothing —
git-paw ships the *mechanism*, the consumer supplies the *content*. git-paw shipping its own
`test-strategy`/`code-standards` skills is **not** an agnosticism concern: they live in `.agents/`
(repo dev tooling), not the exported `assets/agent-skills/`. The only rule is that the **exported
wording** stays generic ("the project's standards skills"), never git-paw's Rust specifics.

### D2 — One source of truth, two consumers, closed loop

The same skill defines the standard the worker builds to and the supervisor verifies against:
**skill → worker writes to it → supervisor gates against it → apply-changes conform the rest.** No
second copy of the standard, so they can't diverge.

### D3 — Skill-content test ripple, scoped up front

Editing `assets/agent-skills/{supervisor,coordination}.md` ripples into the `skills.rs` unit tests
and the `*_skill_content.rs` integration tests that pin skill prose. Update those in lockstep, and
**sync the tracked `.git-paw` skill/script copies** the dogfood executes (known drift hazard). Grep
`skills.rs` + `*_skill_content.rs` for the affected literals before editing.

### D4 — Agnosticism guard

Extend the `lang-agnostic-skills` no-language-leak audit so the new consult wording is covered — a
test asserting the exported consult step embeds no stack/language token.

### D5 — Backward-compatible no-op when absent

If a project has no `.agents/skills/` standards, the consult step does nothing and behavior is
identical to before — satisfying the "feature-disabled == previous version" compatibility rule.

## Non-goals

- Not authoring the standards themselves (`test-strategy`, `code-standards`).
- Not changing the five-gate framework's structure — only adding a "consult the project's standards"
  step to the review gate.
- No product code / CLI / config / wire change.

## Risks

- **Low-Med.** The real work is the skill-content test ripple (D3) — mechanical but easy to miss a
  pinned literal. Mitigated by grepping first and syncing the tracked copies.
