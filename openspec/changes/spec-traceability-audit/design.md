# Design — spec-traceability-audit

## Context

Full audit findings: `.git-paw/v0.13.0-spec-traceability-audit.md` — 0 `no-impl`, ~18
`contradicts`, ~40 `no-test` (mostly coverage-exempt), ~10 `orphan`, + 7 proposed guards. This
change reconciles the contradictions, fills the genuine gaps, hardens the guards, and maintains
the requirement→test map. It runs **first** in the specs-before-tests-before-code order.

## Decisions

### D1 — Reconcile by amending the spec to match tested code (default)

The contradictions are overwhelmingly "the spec lags an intentional, tested code change." The code
is the tested truth, so the default reconciliation is to **amend the spec**, not change the code.
Each amendment is a MODIFIED delta; the code is already covered by tests, so no behavior moves.

### D2 — One reconciliation is a real decision: the `cli-parsing` Stop prompt

`cli-parsing` says `cmd_stop` renders a confirmation prompt when `--force` is omitted and stdin is
a TTY; the code (`cmd_stop(_force)`) renders **none** and `--force` is **inert**. Two valid fixes:
- **(A) amend the spec to match code** — stop never prompts; `--force` is accepted but has no
  gating effect. Consistent with the already-corrected `cli-interaction-e2e` scenario. *Smell:* a
  frozen 1.0 CLI shipping an inert flag.
- **(B) fix the code to match the spec** — implement the prompt so `--force` gates it. A
  behavior-changing add (its own reproducing-test-gated change), and a safety improvement for a
  destructive verb.
This is the one drift where the reconciliation direction is a judgment call — **surface it to the
owner**; do not bury it in a silent amendment. (Stop is recoverable via `git paw start`, which
argues (A) is acceptable; the inert-flag smell argues (B).)

**RESOLVED (A) — owner, 2026-07-25.** `cmd_stop` kills the CLI processes but preserves worktrees +
session state and is recoverable via `git paw start`, so it is non-destructive (the severity ladder
is `pause` soft → `stop` recoverable → `purge` destructive-and-prompts). No prompt. `--force` is
kept accepted-but-inert and **documented as a reserved no-op** rather than removed, because
`stop --force` already parses today and dropping it would break existing scripts (back-compat NFR).
Delta authored (`specs/cli-parsing/spec.md`); consistent with the `cli-interaction-e2e` scenario.

### D3 — Standing guards, modeled on `spec_purpose_backfilled.rs`

Where a closed gap can silently reopen, add a cheap guard test: `KNOWN_CLIS`↔spec-roster sync,
`--specs-format` value-list completeness (all `SpecBackendKind` incl. `superpowers`), a
no-`.specify/`-auto-detection prose guard, a gemini-as-current-CLI guard (arms the agy swap).

### D4 — The map is the consolidation safety net

The requirement→test map is produced here and run **before and after each `spec-consolidation`
merge**: if a scenario loses its covering test across a merge, a move dropped coverage — restore
it. This is why traceability sequences first / alongside consolidation, not after.

### D5 — Don't duplicate sibling-owned fixes

`cli-detection` roster (7→17 + agy) is owned by `gemini-to-antigravity-cli`; phantom `git paw
resume` and the `learnings-mode` negative are owned by `spec-consolidation`. This change verifies
they're closed, not re-authors them.

## The drift inventory (reconciliation tasks)

| Drift | Capability | Direction |
|---|---|---|
| `safe` preset "defaults only" vs the composed dev-allowlist model | `approval-configuration` | amend spec **(seeded)** |
| `cmd_stop` prompt claimed but not rendered; `--force` inert | `cli-parsing` | **DECISION (A/D2)** |
| `build_task_prompt` prepends `.git-paw/AGENTS.local.md` pointer; 3 scenarios assert the old verbatim strings | `supervisor-launch` | amend spec |
| Verified/Feedback/Question excluded from roster upsert (W15-16 fix) vs "sender record updated" ×3 | `message-delivery` | amend spec |
| empty-poll `last_seq=0` vs monotonic cursor held at `since` (two specs disagree) | `broker-endpoints` + `message-delivery` | reconcile both to code |
| `--specs-format` help + `resolve_dir` still reference removed `.specify/` auto-detection; help omits `superpowers` | `cli-parsing` / `mcp-server` | scrub/align |

## Risks

- **Low** for the amend-to-match-code reconciliations (code already tested). The Stop decision
  (D2) is the only behavior-affecting fork. Amending specs that `spec-consolidation` also merges is
  fine — apply the amendment first, then the merge carries the corrected text.
