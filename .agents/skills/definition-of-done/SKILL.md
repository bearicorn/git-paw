---
name: definition-of-done
description: The completeness check for a git-paw change — a change is done only when every dimension is satisfied. Use at author-time to self-check before publishing done, and at the supervisor review gate to confirm nothing was skipped. Ties together spec, code, tests, docs, security, safety, and export-agnosticism via the per-dimension standards skills; it is the AGENTS.md Change Checklist and the five-gate framework as one skill.
license: MIT
compatibility: git-paw
---

# Definition of Done

A git-paw change is **done** only when every applicable dimension below is satisfied. The worker
self-checks this before publishing `done`; the supervisor confirms it at the review gate (nothing
skipped). Each dimension has its own skill — this is the index that ties them together.

## Dimensions (each links its standard)

- **Spec** — the behavior is specified (OpenSpec); every SHALL/MUST has a WHEN/THEN scenario. (the
  spec-driven / opsx workflow)
- **Code** — conforms to `code-standards` (patterns, no `unwrap`/`expect`, docs, frozen do-not-touch).
- **Tests** — behavioral, right-layer, and every scenario covered, per `test-strategy`.
- **Docs** — complete and consistent across the four layers, per `doc-completeness`; `mdbook build` passes.
- **Security** — passes `security-review` (external bad actors: least-privilege, injection, secrets, deps).
- **Safety** — passes `safety-review` (rogue-agent blast radius: worktree confinement, danger-list, no
  persistence / exfiltration).
- **Export-agnosticism** — if the change touches an export, it passes `export-agnosticism`.
- **Backward-compat** — existing configs / sessions / wire load unchanged; no breaking change (that is
  the v1.0.0 line).

## The done checklist

- [ ] Spec updated; every scenario mapped to a test.
- [ ] Code conforms (code-standards); `just check` green.
- [ ] Tests behavioral + covering (test-strategy); full suite green vs merge-base.
- [ ] Docs complete across all four layers (doc-completeness); `mdbook build` passes.
- [ ] Security reviewed (security-review).
- [ ] Safety / blast-radius reviewed (safety-review).
- [ ] Export-agnostic if it touches an export (export-agnosticism).
- [ ] Backward-compatible; no frozen surface broken.

A change missing any **applicable** dimension is NOT done — finish it, or explicitly scope it out with
a reason recorded in the change.

---

Repo-local dev skill; maps to git-paw's five-gate verification and the AGENTS.md Change Checklist.
