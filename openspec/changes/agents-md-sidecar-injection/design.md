# Design — agents-md-sidecar-injection

## Context

During worktree setup, `setup_worktree_agents_md()` (`src/agents.rs:246`) reads
the root repo's tracked `AGENTS.md`, injects the managed
`<!-- git-paw:start --> … <!-- git-paw:end -->` block (per-worktree assignment +
skill section + inter-agent rules) into it via `inject_into_content()`, and
writes the combined content to the worktree's **tracked** `AGENTS.md`. It then
protects that file from being committed with two layers:

1. `exclude_from_git(worktree_root, "AGENTS.md")` — appends `AGENTS.md` to
   `.git/info/exclude` (`src/git.rs:600`).
2. `assume_unchanged(worktree_root, "AGENTS.md")` — runs
   `git update-index --assume-unchanged AGENTS.md` (`src/git.rs:668`,
   called at `src/agents.rs:281`).

Every supported coding CLI auto-loads `AGENTS.md` from the worktree root on
startup, so the agent sees the combined view "for free" — `build_task_prompt`
(`src/main.rs:332`) just points the agent at `AGENTS.md` without naming a path.

### The footgun (finding F10)

`git update-index --assume-unchanged AGENTS.md` is a **file-level** flag. Git
stops reporting *any* modification to that path — not just the ephemeral
injection git-paw wrote, but every legitimate edit too. The v0.7.0 dogfood hit
this twice:

- An agent had to commit a real `AGENTS.md` edit (adding the `rmcp` row to the
  approved-dependency table). `git status` reported the file unmodified and
  `git add -A` silently skipped it, so the commit was empty for that file. It
  took ~5 cycles to diagnose because nothing surfaced the assume-unchanged bit.
- The same bit recurred during the release cherry-pick, where the AGENTS.md
  change had to be reconstructed by hand.

This is general product behaviour: any project where an agent legitimately
edits the tracked `AGENTS.md` mid-session is silently blocked from committing
it. `assume-unchanged` is the wrong tool — it was chosen to stop `git add -A`
from staging the *injected* block, but it cannot distinguish injected content
from authored content because it operates on the whole file.

## Goals

- The tracked `AGENTS.md` in a worktree is a normal committable file: a hand
  edit shows in `git status` and stages/commits via `git add -A`.
- The managed git-paw block (assignment + skill + inter-agent rules) still
  reaches the agent on every `git paw start`.
- No `assume-unchanged` bit and no `.git/info/exclude` entry on the tracked
  `AGENTS.md`.
- Backward compatible: existing sessions recover on the next `git paw start`;
  nothing new is tracked by git.

## Non-Goals

- Changing the *content* of the managed block (markers, skill prose,
  inter-agent rules rendering) — the block is byte-identical to today, only its
  destination file changes.
- Changing how the agent is told to read its instructions beyond the file it
  points at — `build_task_prompt` prose is unchanged except for the target path
  if needed.
- Supporting CLIs that cannot be pointed at an additional instruction file
  (all currently supported CLIs auto-load the worktree-root instruction file).
- Removing `assume_unchanged`/`exclude_from_git` from `src/git.rs` — they remain
  general-purpose helpers; only the `AGENTS.md` *call sites* change.

## Decisions

### D1 — Inject into a gitignored sidecar, not the tracked AGENTS.md

The managed block is written to a per-worktree **sidecar** instruction file
that is gitignored, e.g. `.git-paw/AGENTS.local.md` (the `.git-paw/` directory
is already used for session learnings and helper scripts and is gitignored).
The tracked `AGENTS.md` is left untouched by git-paw.

Rationale: the injection's purpose is purely ephemeral per-session guidance for
the agent — it has no business living in version-controlled content. Moving it
to a gitignored sidecar removes the entire reason `assume-unchanged` existed.

The sidecar's content is the *combined view*: root `AGENTS.md` content followed
by the worktree assignment section (today's `inject_into_content` output),
exactly the bytes that used to be written to the worktree `AGENTS.md`. This
preserves the agent's experience — it reads one file that contains both the
project rules and its assignment.

### D2 — Point the CLI's instruction file at the combined sidecar

Supported CLIs auto-load the worktree-root instruction file. To have the agent
read the combined sidecar instead of (or in addition to) the tracked
`AGENTS.md`, git-paw points the CLI's instruction file at the sidecar. The
chosen mechanism is the lowest-friction one available per CLI:

- Where the CLI accepts an explicit instruction-file path/flag, git-paw passes
  the sidecar path.
- Otherwise git-paw ensures the worktree-root instruction file the CLI
  auto-loads resolves to the combined content — e.g. by placing the sidecar at
  the path the CLI reads and gitignoring that path, or symlinking. The tracked
  `AGENTS.md` is never the injection target regardless.

The invariant the spec pins down: the agent's effective instruction view SHALL
equal `tracked AGENTS.md content + managed block`, and the managed block SHALL
live only in the gitignored sidecar.

### D3 — Drop assume-unchanged and the exclude entry on the tracked AGENTS.md

`setup_worktree_agents_md` no longer calls `assume_unchanged(_, "AGENTS.md")`
and no longer calls `exclude_from_git(_, "AGENTS.md")`. The sidecar path is
gitignored instead (via `.gitignore`/`.git/info/exclude` on the sidecar path,
not the tracked file).

For repos that already have an assume-unchanged bit set on `AGENTS.md` from a
prior git-paw version, the next `git paw start` SHALL clear it
(`git update-index --no-assume-unchanged AGENTS.md`) so the tracked file
becomes committable again. This is the backward-compat recovery path.

### D4 — Gitignore the sidecar

The sidecar path (e.g. `.git-paw/AGENTS.local.md`) is added to the worktree's
ignore set so the ephemeral injection is never accidentally committed —
replacing the protection `assume-unchanged` used to provide, but scoped to a
file that contains *only* git-paw content.

## Risks

- **R1 — A CLI does not auto-load the sidecar path.** Mitigation: per-CLI the
  sidecar is placed at / pointed to via the path the CLI already reads (D2). If
  a future CLI supports neither a flag nor an auto-loaded path, that CLI needs
  explicit handling; out of scope here (all current CLIs are covered).
- **R2 — Stale assume-unchanged bit from an older git-paw.** Mitigation: D3's
  `--no-assume-unchanged` clear on every start makes the upgrade self-healing;
  no manual `git update-index` needed by the user.
- **R3 — Root AGENTS.md itself already contains a git-paw block** (e.g. a repo
  committed an old injected block). Mitigation: unchanged from today —
  `inject_into_content` replaces any existing `<!-- git-paw:start -->` block
  when building the combined sidecar content, so the block is never duplicated.
- **R4 — Confusion about which file to edit.** A contributor might still edit
  the sidecar by hand. Mitigation: the sidecar is gitignored, so such edits are
  local-only and discarded on the next re-inject; the skill prose directs
  agents to edit the tracked `AGENTS.md` for committable changes.
