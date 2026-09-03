## Context

A fresh `--unattended` run cannot reach its first tool call (peer-4-poker,
2026-08-31). This change fixes three of the four boot defects — the classifier
escalating git-paw's own boot commands (GP-02), non-deterministic launch flags plus
dead panes reporting healthy (GP-03), and conflict/`.git/` misclassification
(GP-04). GP-01 (agnostic allowlist seeding) is a sibling change. The work touches
the security-critical auto-approve classifier (`src/supervisor/auto_approve.rs`, with
a mirrored shell classifier reached via the hidden `git paw __classify` seam), the
per-CLI flag mapping (`src/supervisor/config.rs`), the tmux launch-readiness gate,
the session liveness probe, and the broker conflict detector.

## Goals / Non-Goals

**Goals:**
- A fresh Claude-family unattended run auto-resolves its own boot-time helper
  prompts instead of stalling on them.
- `Auto` and `FullAuto` produce deterministic, per-CLI launch behaviour; a session
  whose panes died is never reported healthy.
- Tighten the classifier so a `.git/` mutation can never be auto-approved, while
  loosening it only for git-paw's own authored helper scripts and for
  prefix-normalized forms of already-safe commands.
- Stop a shared untracked `.git-paw/` from fabricating conflicts.

**Non-Goals:**
- The agnostic per-CLI allowlist-*seeding* migration (GP-01) — sibling change
  `cli-native-allowlist-seeding`.
- Generalising the per-CLI hook/provider contract (v0.18) or running sandboxed
  workers in skip-permissions (v0.18). GP-03a's `acceptEdits` clears *edit*
  approvals for the Auto level as a side effect, but the pump's general
  edit-approval path stays out of scope.

## Decisions

- **Extend `normalize_command`, don't add a second normalizer (GP-02a).** The
  existing function already strips a trailing exit-probe/redirect; prepend a
  leading-prefix strip (a run of `NAME=value` assignments, then an `env`/`nohup`
  wrapper) so both compose. Strip conservatively — only tokens matching strict
  shell-assignment / known-wrapper shapes — so a value that merely contains `=` is
  not mistaken for an assignment. Alternative rejected: a broad regex that could
  swallow real arguments and hide a danger verb.
- **Reuse `is_managed_path()` for both GP-02b and GP-04a.** git-paw already owns the
  authoritative "is this one of my managed paths" check in `src/agents.rs`. GP-02b
  keys the safe-classification on the normalized leading verb resolving to a managed
  `.git-paw/scripts/*.sh` path (direct or interpreter-invoked); GP-04a filters
  managed paths out of each agent's modified-file set before overlap. Single source
  of truth, export-agnostic (git-paw authors these paths).
- **`.git/`-write danger mirrors the existing config-path danger rule (GP-04b).**
  Same precedence (before any safe rule), same canonicalize + fail-closed posture as
  the worktree-boundary and protected-path checks. It matches *file-path* targets
  only; ordinary `git` subcommands keep flowing through the git-verb rules, so
  `git commit`/`git add` are unaffected.
- **`Auto` → `acceptEdits`, kept in the per-CLI table (GP-03a).** The milestone's
  "`--permission-mode auto`" is not a real Claude value; the correct deterministic
  low-friction mode is `acceptEdits`. Encode it as a built-in table row, leaving the
  per-CLI `approval_args` override and the "verify upstream at implementation" clause
  intact, so the mapping stays agnostic (a CLI without such a mode keeps `""`).
- **Launch gate gains a dialog state, reusing the readiness-poll heuristic (GP-03b).**
  The poll already classifies bare-shell vs ready per-CLI; add a third recognised
  state — a first-run acceptance/trust dialog — handled by answer-or-fail-loud, with
  the same conservative fallback for unrecognised CLIs. Alternative rejected: a
  blind fixed keystroke, which would mis-fire on CLIs without the dialog.
- **Enrich the liveness probe, not the effective-status table (GP-03c).** The
  `effective_status` table over `is_tmux_alive` stays; the fix is that the probe
  computing `is_tmux_alive` requires ≥1 live agent-pane CLI process rather than mere
  session existence. No new `SessionStatus` variant (an all-dead session resolves to
  the existing `Stopped`).

## Risks / Trade-offs

- **Normalization hides a danger verb behind a crafted prefix** → strip only strict
  assignment/wrapper shapes; re-run the full danger-list on the normalized command;
  keep the "normalization never downgrades a danger command" scenarios as guards.
- **Shell/Rust classifier drift** → the shell classifier is the `git paw __classify`
  seam (single source since v0.14); verify the mirrored logic and any tracked
  `.git-paw/scripts/*.sh` copy after editing (known drift hazard).
- **Liveness probe flaps a just-launched session to Stopped** → require the probe to
  tolerate the launch window (panes still starting their CLI are not "dead");
  covered by a spec scenario.
- **`acceptEdits` is the wrong upstream token** → the flag-mapping requirement
  already mandates upstream verification at implementation; confirm against the live
  `--permission-mode` enumeration before landing.

## Migration Plan

Additive/behavioural; no config or state migration. Existing sessions and configs
load unchanged. Rollback is a straight revert (no persisted format change).

## Open Questions

- The exact per-CLI acceptance-dialog markers for non-Claude CLIs (agy, codex) — the
  conservative fallback covers unknowns, so this can be filled in incrementally.
