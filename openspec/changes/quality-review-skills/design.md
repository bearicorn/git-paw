# Design — quality-review-skills

## Context

Fills the standing-skill gaps in the five-gate framework: the doc audit (Gate 4), security (Gate 5),
the agent-safety concern, and the change-completeness meta. Extends the standards-as-skills suite
(`test-strategy`, `code-standards`, `export-agnosticism`).

## Decisions

### D1 — Four repo-local skills at `.agents/skills/` (not exported)

agentskills.io format; git-paw-specific dev tooling (its doc layers, its containment posture). They
are NOT placed in `assets/agent-skills/` — a consumer applies its own review standards.

### D2 — Security ≠ Safety (the key split)

`security-review` = external **bad actors** (attackers against the tool / consumer). `safety-review`
= the blast radius of an agent git-paw *itself runs* (rogue or mistaken, with auto-approved execution
power). A tool that auto-approves agent commands needs both, and they trigger on different diffs. The
FS-scoped sandbox (roadmap v0.15.0) is the eventual **structural** containment for safety; the skill
is the **interim + ongoing behavioral** guard that also ensures no change weakens the existing
containment (worktree confinement, the classifier danger-list, the send-gate).

### D3 — `definition-of-done` is the tie-together

The AGENTS.md Change Checklist + the five-gate framework, expressed as one skill. It references every
dimension skill so "is this change complete?" has a single authoritative home; the worker self-checks
it, the supervisor confirms it at the gate.

### D4 — Enforcement + consumption already exist

Conformance is covered by `tests/agent_skills_conform.rs` (now 7 skills). The worker (author-time) and
supervisor (review gate) already consult the project's `.agents/skills/` standards via
`standards-skill-integration`. `code-standards` cross-links to `definition-of-done`.

## Non-goals

- Not exported; not shipped by `git paw init`.
- No new enforcement code and no product/CLI/config/wire change.
- No separate `spec-authoring` skill (opsx + the change-checklist own it) or `regression` skill (a
  command discipline).

## Risks

- **Very low** — dev-tooling skills. The main upkeep is keeping them in step as the gates / NFRs
  evolve, and folding the safety guard into the FS sandbox once it lands (then `safety-review` shifts
  from primary control to defence-in-depth).
