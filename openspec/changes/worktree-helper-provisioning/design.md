## Context

Helpers are bundled in the binary (`assets/scripts/*.sh`, via `include_str!` / installed by `git paw init` into the repo's gitignored `.git-paw/scripts/`). Agents run inside per-branch worktrees whose fresh checkout has no `.git-paw/scripts/`, so the agent's relative `.git-paw/scripts/broker.sh` invocation fails until it hand-copies from `assets/`. `attach_agent` (shared by `start` and `add`) is the single per-worktree setup seam.

## Goals / Non-Goals

**Goals:** every agent worktree has the current bundled helpers, executable, before boot — zero manual `cp`, zero approval prompt for provisioning.
**Non-Goals:** no change to helper behavior; no per-CLI allowlist seeding (v1.0.0); no attempt to share one physical copy across worktrees (each worktree gets its own).

## Decisions

- **D1 — Provision in `attach_agent`, from the same bundled source as `init`.** Since `attach_agent` is shared by `start` and `add`, writing the helpers there guarantees start-time and added agents are byte-identical (the existing design invariant). Source the content from the embedded assets so a worktree's helper matches the installed binary's version — not from the repo's possibly-stale `.git-paw/scripts/`. *Alternative:* symlink the repo's copy into the worktree — rejected: brittle across worktree relocation and the child/sibling placement modes, and breaks if the repo copy is absent.
- **D2 — Idempotent overwrite.** Always (re)write the scripts + `chmod +x` on setup; attaching to a reused worktree refreshes them. Rationale: cheap, and keeps a worktree's helpers current if the binary was upgraded mid-cycle.
- **D3 — Only provision the helpers the session uses.** broker.sh whenever broker is enabled; docs-fetch.sh whenever `docs_base_url` is configured (mirrors the skill's gate); sweep.sh is supervisor-side (repo root), not per-agent-worktree, so it is out of scope here unless a helper an agent invokes needs it.

## Risks / Trade-offs

- Writing scripts into every worktree adds a few files per worktree → negligible; they are gitignored and small.
- A worktree's helper could drift from the repo's if the binary changes without re-running `start` → acceptable: `start`/`add` refresh on every attach (D2), matching the binary that launched the session.
