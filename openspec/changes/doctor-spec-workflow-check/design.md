## Context

`git paw doctor` is already a functional core: `collect_probes` gathers `Probes`, and each `check_*`
is a pure function returning `CheckResult`s (`Ok` / `Warn` / `Fail`, remedy required for non-Ok).
`preflight-diagnostics` already has "Doctor surfaces the effective spec-driven worker CLI", so the
probe can reuse its resolution: `SpecCliProbe.resolved_clis` (`probe_spec_cli`), a deduplicated
`Vec` — one entry per distinct CLI the discovered specs resolve to (a spec's own `paw_cli`, else
`default_spec_cli`, else `default_cli`), empty when no specs are discovered. Placement is already
probed as `ConfigProbe.placement` from `PawConfig::worktree_placement()` (`Sibling` when unset).
`build_task_prompt` dispatches on `SpecBackendKind`; only the OpenSpec arm
names a slash command (`/opsx:apply`); Markdown and Spec Kit embed the assignment in the sidecar, and
Superpowers works a plan file.

## Goals / Non-Goals

**Goals:**
- Warn when the boot prompt's instructed workflow cannot be found from where worktrees are placed.
- Stay a pure check over a probe; no behavior change to any launch.

**Non-Goals:**
- Changing the boot prompt, provisioning workflow definitions into worktrees, or making placement
  decisions — the roadmap scopes this to a diagnostic.
- Knowing every agent CLI's command-definition layout (see D2).

## Decisions

### D1 — Reachability rule

A worktree sees a root file if git checks it out (it is tracked) or if the worktree is under the root
and the CLI searches upward (child placement). So: tracked → Ok; child + present at root → Ok; sibling +
present only untracked → Warn; not present anywhere → Warn; format names no command → Ok (not
applicable); CLI layout unknown → Ok with "could not verify".

The rule is applied **per resolved CLI**, then aggregated:
- zero resolved CLIs (no specs discovered, or none resolvable without the interactive picker) →
  Ok (not applicable) — there is no worker CLI to check;
- otherwise each CLI gets its own verdict; the check is Warn when any CLI with known locations is
  unreachable, and the detail names exactly those CLIs (with their locations checked); reachable
  CLIs pass silently; CLIs with unknown locations are listed as "could not verify" in the detail
  without warning. A mixed repository (one spec pinning `paw_cli = "codex"`, the rest `claude`)
  therefore warns only about the CLI that cannot reach the command.

### D2 — Per-CLI definition locations, verified rows only

The locations are a per-CLI fact and git-paw must not invent them. The table starts with the one row
verified against a real install (this repository's OpenSpec setup for Claude Code:
`.claude/commands/opsx/apply.md`, `.claude/skills/openspec-apply-change/SKILL.md`). Rows for other CLIs
are added only after checking the spec tool's generated layout for that CLI ("to verify" at
implementation); until then those CLIs get the informational "could not verify" result, never a false
warning. The table is a Strategy with a generic fallback, the shape the v0.19 per-CLI provider layer can
absorb.

### D3 — "Tracked" is asked of git

The probe runs `git ls-files --error-unmatch -- <path>` (discrete arguments via `CommandRunner`) for each
candidate; ignored or untracked files are "present only untracked". No `.gitignore` parsing.

## Risks / Trade-offs

- [False warning for a CLI whose layout changed upstream] → only verified rows warn; the remedy names
  the paths checked so the user can see why.
- [Consumers who intentionally keep agent-tool directories untracked] → the remedy offers child
  placement as the alternative; the check is a warning, never a failure.

## Migration Plan

1. Probe + pure check + table tests.
2. Wire into `collect_probes` / `run_checks`; temp-repo probe test.
3. Docs.

Rollback: revert; doctor output only.

## Open Questions

- Should `git paw start` print the same warning at launch for a spec-driven OpenSpec session with sibling
  placement? Recommendation: not in this change (the roadmap asked for doctor only); revisit if the
  dogfood shows users skip doctor.
