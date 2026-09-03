## Context

Three `git paw start` / launch correctness defects (peer-4-poker, 2026-09-03); GP-10 is
a security regression (workers silently unsandboxed). All touch launch-time resolution.

## Goals / Non-Goals

**Goals:** supervisor-mode spec workers launch the configured CLI; resume never fails
where first launch succeeded; a re-`start` never silently forks a parallel wave.

**Non-Goals:** the broader per-CLI provider / sandbox generalisation (v0.18); session
reconciliation (v0.20). `git paw attach` (its own change) is the explicit reattach
command; GP-13 is `start`'s implicit reattach guard.

## Decisions

- **Thread the existing resolution chain into the supervisor path (GP-10).** The
  `--supervisor` spec-launch dispatch reuses the same 5-level chain the non-supervisor
  path uses, rather than a separate resolution — so `default_spec_cli` applies
  uniformly. Doctor recomputes the effective spec CLI from config+specs and warns on a
  `default_spec_cli` mismatch. Alternative rejected: a supervisor-specific default,
  which is exactly what caused the silent divergence.
- **Collapse the two default-branch resolvers into one (GP-12).** `default_branch()`
  (git.rs:145, `symbolic-ref refs/remotes/origin/HEAD`) and the `symbolic-ref --short
  HEAD` path (git.rs:289) are reconciled onto a single resolver with an
  `origin/HEAD`-absent fallback: local `main` → local `master` → checked-out branch.
  Alternative rejected: making resume skip default-branch resolution — that would
  diverge first-launch and resume behaviour, the opposite of the fix.
- **Reattach guard before `-N` suffixing (GP-13).** Before `resolve_session_name`
  appends `-N`, look up an existing live session for the *current repository*
  (`session-state` find-by-repo); if found, reattach. `-N` is reserved for genuinely
  distinct sessions (cross-repo name collisions). A non-interactive re-`start` refuses
  with an actionable error rather than forking.

## Risks / Trade-offs

- **GP-12 fallback picks the wrong base branch** → prefer a real local `main`/`master`
  over the checked-out branch; document the precedence; `origin/HEAD` still wins when
  present (unchanged behaviour).
- **GP-13 refusal blocks an intentional parallel session** → if a real use case exists,
  add an explicit `--new`/opt-in flag; default stays safe (reattach/refuse).

## Migration Plan

Additive/behavioural; no config or state change. A remote-tracked repo resolves exactly
as before. Rollback is a straight revert.

## Open Questions

- Whether GP-13 warrants an explicit opt-in flag for a deliberate second same-repo
  session (deferred unless a use case surfaces).
