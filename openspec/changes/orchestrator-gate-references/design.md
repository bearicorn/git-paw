## Context

MILESTONE v0.20.0 (subagent half): give the orchestrator context isolation — "delegate bounded,
context-heavy work to subagents that each run in their own context window and report back a
compressed summary"; personas defined per CLI ("for Claude-family orchestrators, in
`.claude/agents/`") with their own role, tools and model: an **explorer** (read-only sweeps,
returns a map) and **gate auditors** (spec-audit, doc-audit, security). Since v0.18.0 the
orchestrator is an external agent CLI that has subagents natively, so persona definition is no
longer git-paw's job; what remains is shipping the gate-auditor content inside the orchestrator
skill set. v0.18.0's `orchestrator-skill-export` shipped without gate-auditor skills and left them
to v0.20.0, so this change owns them.

The bundled orchestrator skill (`assets/agent-skills/orchestrator.md`, ~1,970 lines; renamed from
`supervisor` by v0.18.0) carries the gates in its Workflow: step 4 Testing, 5 Regression,
6 Spec audit (→ "Spec Audit Procedure" section, with the governance-verification sub-step),
6a Doc audit, 6b Security audit, 7 Verify-or-feedback via `sweep.sh verified` / `feedback-gate`.
Requirements pinning this content: `skill-agent`'s *Supervisor skill — five verification gates*,
*Embedded supervisor skill* (Spec Audit Procedure), *Supervisor skill — Governance verification
sub-step*, and the five-gate framework in `AGENTS.md`.

## Goals / Non-Goals

**Goals:**
- Make the decision on personas explicit and justified.
- Keep what survives CLI-agnostic: per-gate briefs + a verdict contract, usable by any orchestrator
  CLI with or without subagents.
- Shrink the always-loaded orchestrator skill without losing a sentence.

**Non-Goals:**
- Persona/agent definition files for any CLI.
- Changing what any gate checks, the gate order, or who may publish a verdict.
- Splitting testing/regression or non-gate sections into references.
- Per-CLI orchestrator variants (git-paw does not know the operator's CLI).

## Decisions

### D1 — No vendor persona files; personas are config-defined profiles (the subagent decision)

**Superseded in part by the user decision of 2026-10-19:** a persona is not a vendor subagent file but
a configuration entry — a CLI pinned to a model plus a role skill, with an explicit purpose. That is
specced by the follow-on v0.20.0 change `agent-profiles` (delegate profiles run headless by git-paw;
bundled `explorer` and auditor role skills; delegate profiles first in this change's delegation
section). The reasoning below still rules out *vendor files*; it no longer rules out personas.

Persona files would be vendor formats. The only one the codebase knows of is Claude's
`.claude/agents/`; Codex's and Antigravity's equivalents are unknown here (**to verify**), and
v0.18.0 already decided git-paw does not install into any vendor's directory. Each persona's three
attributes all belong to the operator, not git-paw: *role* is the gate procedure (git-paw content
— shipped below as references), *tool access* is the CLI's own permission model, and *model* is a
cost choice the operator makes for their own session. The explorer has no git-paw-specific content
at all — every subagent-capable CLI ships one. So the persona layer is dropped; its git-paw-specific
residue (the gate briefs and the verdict shape) ships as skill-package content.
*Alternatives:* (a) ship per-CLI persona files as package assets for orchestrators whose CLI
supports subagents — rejected: vendor-specific export surface, facts unverifiable for 2 of 3 named
CLIs, duplicates gate content into a second place, and contradicts v0.18.0's no-vendor-dir stance;
(b) ship nothing in v0.20.0 and leave gate delegation to v0.18.0 — moot: v0.18.0 shipped without
it, and references need v0.20.0's package machinery. **Listed as a decision for the user.**

### D2 — Move, never copy: three audit gates become references

`references/gates/spec-audit.md` (the Spec Audit Procedure + governance sub-step + `[spec audit]`
format), `doc-audit.md`, `security-audit.md`. Each states its inputs (re-resolved tip, merge-base
diff, change id, its rendered gate command or `(not configured)`) so it reads correctly with no
other context — that is what makes it a subagent brief. The Workflow keeps one heading per gate with
a one-line purpose and the reference's installed path, which keeps *five verification gates*'
"each gate has its own heading" true. Shared rules (re-resolve the tip, merge-base diffing, the
bracketed prefixes) stay in `SKILL.md` *and* are restated in each reference's inputs paragraph —
the only intentional duplication, because a delegated context sees only its reference.

### D3 — The verdict contract

A delegated gate returns: gate name; `pass` or `fail`; zero or more findings, each one line
`[<gate prefix>] <location>: <issue>` — the exact string `feedback-gate` takes. Raw tool output stays
in the subagent's context. The orchestrator alone publishes: `agent.verified` only after testing and
regression pass and all three audit verdicts are `pass`; otherwise one `feedback-gate` per finding.
This keeps v0.18.0's verdict authority (only the root session publishes) and the five-gate order.

### D4 — Package-wide content satisfaction

Requirements written as "the orchestrator skill SHALL include …" are satisfied by the rendered
package (SKILL.md + the references it names). Tests render the package once and assert on that.

### D5 — MCP `get_skill` returns the package

`content` = rendered `SKILL.md`, then each rendered reference under a `## Reference: <path>`
heading. Simpler than a new parameter, and no MCP client silently loses the gates.

## Risks / Trade-offs

- [The orchestrator skips reading a reference] → each gate heading in the Workflow says to read its
  reference before running it; the orchestrator is a capable model, unlike cheap workers.
- [Restating shared rules in each reference drifts] → a test asserts each reference states the
  tip/merge-base/prefix rules.
- [Headless escalation (v0.18.0 `orchestrator-escalation-handoff`) loads only `SKILL.md`] →
  escalations are judgment calls, not gate runs; unaffected.
- [Concurrent audit subagents contend for one verify worktree] → they only read it; the reference
  says read-only, and testing (the only writer of build artefacts) runs first, in `SKILL.md`.

## Migration Plan

1. Package-wide render helper; re-point orchestrator content tests to it (green, no content moved).
2. Move the three gates into references; Workflow pointers; delegation section.
3. `get_skill` package content; docs.
Rollback: revert — content returns to `SKILL.md`.

## Open Questions

- ~~Decision for the user: D1~~ — decided 2026-10-19: no vendor persona files; personas are
  config-defined profiles, specced in `agent-profiles`. This change keeps the gate references and the
  vendor-neutral delegation contract that `agent-profiles` builds on.
- ~~Should v0.18.0's `orchestrator-skill-export` take the delegation contract (D3) as prose?~~ —
  closed: v0.18.0 shipped without it, so this change adds the delegation contract.
