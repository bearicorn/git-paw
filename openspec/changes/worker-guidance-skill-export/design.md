## Context

git-paw exports `coordination.md` (worker) and `supervisor.md` (supervisor) skills via
the existing render/inject machinery. The peer-4-poker dogfood showed two gaps git-paw
currently forces consumers to patch by hand in their `AGENTS.md`: workers waste budget
on expected sandbox-policy conditions, and the safe-command policy (tier 2 of the
permission ladder) is invisible to the LLM supervisor. This change exports both as
agnostic skill content. It is the reframe of GP-01 (away from allowlist-seeding).

## Goals / Non-Goals

**Goals:** a worker recognises FS-confinement / missing install artifacts / setuid-under-
sandbox as expected policy and adapts instead of probing; the supervisor applies an
explicit tiered permission model anchored on a single-source classifier; every addition
is export-agnostic.

**Non-Goals:** removing the inert `allowed_bash_prefixes` / dev / curl allowlist seeding
(redundant, but its removal ripples across `approval-command-safety` /
`core-configuration` / `broker-agent-helper` — a separate decision); building a new
sandbox mechanism (v0.15 is docs-only); per-CLI skill templates (v0.18).

## Decisions

- **Extend the existing embedded skills, not new skill files.** The worker orientation
  goes in `coordination.md`, the permission model in `supervisor.md`, reusing the
  render/inject + per-CLI resolution + skill-content test machinery. Alternative
  rejected: standalone skill files (more injection wiring now, and v0.18's skills-
  packaging will restructure this anyway).
- **Safe-command policy defers to `git paw __classify` (single source).** The skill
  carries orientation prose and the policy *classes*, but directs the supervisor to
  `git paw __classify` as the authoritative check — it never enumerates a parallel
  allowlist that could drift from the Rust classifier (the drift hazard v0.14's
  `classifier-single-source` closed for the shell mirror). Alternative rejected:
  generating the safe list into the skill (build complexity + a second source to keep
  in sync).
- **Export-agnostic phrasing, enforced by conformance.** Stack steps are generic
  ("your stack's install step") or resolved-stack-sourced; the sandbox is conditional
  ("if your operator configured one"); only git-paw's own surface (`on_create`, the
  sandbox, `git paw __classify`, `.git-paw/…`) is named. Guarded by
  `tests/agent_skills_conform.rs` and the `export-agnosticism` dev skill.

## Risks / Trade-offs

- **Skill prose drifts from the classifier** → deferral to `git paw __classify` means
  there is no parallel list to drift; the danger/safe *classes* are stated as
  orientation only. Skill-content tests pin the key phrases.
- **A hard-coded stack verb slips in** → the export-agnosticism conformance test fails
  the build; the audit is part of the change checklist.
- **Skill bloat** → keep the orientation tight and bulleted; `coordination.md` already
  carries budget/compaction sections, so structure is established.

## Migration Plan

Additive skill content; no config or state change. Rendered into worktrees at launch;
existing sessions are unaffected until relaunch. Rollback is a straight revert.

## Open Questions

- Whether the worker orientation eventually moves to a dedicated worker-environment
  skill package — deferred to v0.18's skills-packaging; folded into `coordination.md`
  for now.
