## Context

Two small additive CLI conveniences (`attach`, `completions`) plus a docs-consistency
cleanup (strike the never-implemented `resume`). All reuse existing plumbing.

## Goals / Non-Goals

**Goals:** a first-class `git paw attach`; installable shell completions; docs that
reference only real subcommands.

**Non-Goals:** GP-13's "start reattaches instead of forking" (change
`start-launch-correctness`); `attach` is the explicit reattach path, `start` keeps its
own resolution. No new session semantics.

## Decisions

- **`attach` reuses `session-state` resolution + `tmux::attach()`.** The command
  resolves the repository's session (find-by-repo-path, as `status` does) and calls
  the existing low-level attach; it adds no new tmux capability, only the command
  surface. Errors actionably when nothing is running.
- **`completions` is a subcommand, not a hidden flag.** `git paw completions <shell>`
  prints to stdout via `clap_complete::generate`, matching the common convention and
  keeping generation out of the hot `start` path.
- **No `resume` verb.** Reattach and revival are distinct existing paths (`attach`,
  `start`); the phantom `resume` is a docs artefact, removed rather than implemented.

## Risks / Trade-offs

- **New dependency `clap_complete`** → first-party clap companion, permissive licence,
  low supply-chain risk; recorded in the approved dependency table and checked by
  `just deny`.

## Migration Plan

Additive; no migration. Rollback is a straight revert.

## Open Questions

None.
