## Why

Three `git paw start` / launch correctness defects (peer-4-poker, 2026-09-03), one a
security regression:

- **GP-10 (HIGH, security)** — `default_spec_cli` is ignored when `--specs` meets
  `--supervisor`, silently launching workers with the wrong (unsandboxed) CLI while
  `git paw doctor` stays green — defeating the v0.15 sandbox invisibly.
- **GP-12** — `start` aborts without `origin/HEAD` once branches have commits: two
  default-branch resolvers disagree (`default_branch()` needs `origin/HEAD`; another
  path uses the checked-out branch), so first launch works and every *resume* fails.
- **GP-13** — `start` forks a `paw-<project>-2` session instead of reattaching to the
  repository's live session, contradicting its documented behaviour; combined with the
  scanner silently advancing phases, one command can fork a parallel wave with no
  confirmation.

## What Changes

- **GP-10** — the spec-driven CLI resolution chain (including `default_spec_cli`)
  applies identically under `--supervisor`; the `--supervisor` flag SHALL NOT bypass
  it. `doctor` surfaces the effective spec-worker CLI and warns on a config mismatch,
  so a mis-resolution is no longer silently green.
- **GP-12** — a single default-branch resolver, used by every launch path, that falls
  back deterministically when `origin/HEAD` is absent instead of aborting, so resume
  no longer fails.
- **GP-13** — `start` reattaches to the repository's existing live session instead of
  forking a suffixed parallel one; the `-N` suffix is reserved for genuinely distinct
  sessions; a non-interactive re-`start` refuses (or requires an explicit opt-in)
  rather than silently forking.

Additive/behavioural; existing single-session, remote-tracked flows unchanged. Not
breaking.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `cli-resolution`: the spec-driven CLI resolution chain applies identically under
  supervisor mode (GP-10).
- `preflight-diagnostics`: `doctor` surfaces the effective spec-worker CLI and warns
  on a mismatch with configured `default_spec_cli` (GP-10).
- `git-operations`: default-branch resolution is single-source and tolerant of a
  missing `origin/HEAD` (GP-12).
- `tmux-orchestration`: `start` reattaches to an existing live session for the
  repository instead of forking a suffixed one (GP-13).

## Impact

- **Code:** the supervisor spec-launch dispatch (thread the same CLI resolution chain
  as the non-supervisor path); `src/git.rs` (`default_branch` / `default_branch_with`
  — single resolver + `origin/HEAD`-absent fallback, reconcile with the
  `symbolic-ref --short HEAD` path at git.rs:289); the `start` dispatch (reattach guard
  via session-by-repository lookup before `resolve_session_name`); `doctor` checks.
- **Enum-variant ripple:** none (no `BrokerMessage` / `SpecBackendKind` change).
- **Security:** GP-10 is a security-regression fix — workers launch with the
  configured (sandboxed) CLI, and the doctor honesty check closes the blind spot.
  Gate-5 security review required.
- **Backward compatibility:** additive/behavioural; a repo with `origin/HEAD` resolves
  as before; the `-N` collision suffix still applies to genuinely distinct sessions;
  no config or state change.
- **Docs:** CLI reference / user guide note `start`'s reattach behaviour and the
  default-branch fallback; the doctor chapter notes the spec-CLI check.
