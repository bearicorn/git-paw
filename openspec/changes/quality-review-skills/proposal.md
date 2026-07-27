## Why

The standards-as-skills suite has `test-strategy` (Gate 1), `code-standards` (code), and
`export-agnosticism` (cross-cutting) — but the **doc audit** (Gate 4) and the **security** (Gate 5)
concerns have no codifying skill, the **safety** of the agents git-paw *runs* is nowhere captured,
and nothing ties "is this change actually complete?" together. Fill those gaps so the worker builds
to each dimension and the supervisor gate confirms none was skipped.

Crucially, a tool that **orchestrates agents with real, auto-approved execution power** has a
two-part threat model: external **bad actors** (security) and a **rogue or mistaken agent it itself
runs** (safety — deleting an OS folder, planting a backdoor, destructive git, exfiltration). The
planned FS-scoped sandbox (roadmap v0.15.0) is the eventual structural containment for safety; until
then a continuous safety review is the guard.

## What Changes

Four new repo-local dev skills at `.agents/skills/`:
- **`doc-completeness`** — Gate 4: the four doc layers, per-change-type layer mapping, `mdbook build`.
- **`security-review`** — Gate 5, external bad actors: least-privilege, injection, secrets, deps.
- **`safety-review`** — rogue/mistaken-agent blast radius: out-of-worktree actions, irreversible git,
  persistence/backdoors, exfiltration; the containment a change must not weaken; interim until the
  v0.15.0 sandbox.
- **`definition-of-done`** — the completeness meta-skill: a change is done only when spec + code +
  tests + docs + security + safety + (export-agnosticism if it touches an export) are all satisfied.

Cross-linked from `code-standards`; `definition-of-done` references every dimension skill. Repo-local
dev tooling — **not** exports; conformance auto-covered by `tests/agent_skills_conform.rs`.

## Capabilities

### New Capabilities
- `quality-review-skills`: the four review/completeness dev skills, their agentskills.io conformance,
  and the code-standards cross-link.

### Modified Capabilities
_None._ No product behavior, CLI, config, or wire change.

## Impact

- **New:** `.agents/skills/{doc-completeness,security-review,safety-review,definition-of-done}/SKILL.md`;
  a cross-link edit in `.agents/skills/code-standards/SKILL.md`.
- Consumed by the worker + supervisor via `standards-skill-integration`; the conformance guard now
  validates 7 skills.
- Not exported (`assets/agent-skills/` untouched); no code/behavior change.
