## Context

Agent worktrees are created as siblings of the repository at
`../<project>-<branch>/`. The sibling layout scatters worktrees across
the parent directory, pollutes the space outside the project root, and
makes per-project permission grants awkward: an agent CLI scoped to the
repository cannot reach a worktree that lives outside it. Tools such as
Claude Code keep worktrees inside the project tree.

v0.8.0's headline change moves agent worktrees inside the repository at
`<repo>/.git-paw/worktrees/<branch-slug>/`, controlled by a new
`worktree_placement` config field (`"child" | "sibling"`). New repos
default to the contained (`child`) layout via `git paw init`; existing
repos with no field continue to use the sibling layout so their
already-created worktrees and recorded session paths keep working.

The path resolution lives in `src/git.rs` (`create_worktree` and the
purge/remove teardown), the config field in `src/config.rs`, the init
default + gitignore seeding in `src/init.rs`, and the round-trip is
validated against `src/session.rs`, which already records concrete
worktree paths.

## Goals

- Add a `worktree_placement` config field selecting child vs sibling
  layout.
- Resolve the worktree target path from the configured placement in
  `create_worktree`, and resolve the same path in teardown.
- Make `git paw init` write `worktree_placement = "child"` for new
  repos and gitignore `.git-paw/worktrees/`.
- Keep both layouts fully round-trippable through the session JSON so
  resume/status/purge work regardless of placement.
- Preserve exact v0.7.0 behaviour when the field is absent.

## Non-Goals

- Migrating existing sibling worktrees into the child layout. Existing
  worktrees stay where they are; placement only governs *new*
  worktree creation.
- Changing the branch-to-directory sanitisation rules already used by
  `worktree_dir_name` (slash → dash, unsafe chars stripped).
- Changing how sessions are located or named.
- Per-CLI or per-preset placement overrides — placement is a single
  repo/global config scalar.

## Decisions

### Path resolution: child vs sibling

`create_worktree` resolves the absolute worktree path from the
configured placement before any git invocation:

- **`sibling`** (default-on-absent): `<repo_parent>/<project>-<slug>`,
  identical to v0.7.0. `<repo_parent>` is the parent of the repository
  root; `<project>` is `project_name()`; `<slug>` is the sanitised
  branch.
- **`child`**: `<repo_root>/.git-paw/worktrees/<branch-slug>`. The
  `.git-paw/worktrees/` directory is created if absent.

In both cases the rest of `create_worktree` (rebase-onto-main,
idempotent existence check, `git worktree add` fallback) is unchanged —
only the *target path* differs. Teardown (`remove_worktree`, invoked
from purge) operates on the concrete path recorded in the session JSON,
so it is placement-agnostic by construction.

### Slug derivation for `<branch-slug>` (child layout)

The `<branch-slug>` for the child layout reuses the existing branch
sanitisation: `/` is replaced with `-` and characters outside
`[A-Za-z0-9._-]` are stripped (the same rule `worktree_dir_name`
applies to the branch component). It does NOT prepend the project name,
because the directory already lives under that project's
`.git-paw/worktrees/`, so the project prefix would be redundant. Thus
branch `feat/auth-flow` → `.git-paw/worktrees/feat-auth-flow/`, and
`fix/issue#42` → `.git-paw/worktrees/fix-issue42/`.

### Session JSON records concrete paths (both layouts)

The session JSON already stores the concrete, absolute worktree path
per worktree entry. This change records whatever path `create_worktree`
produced — sibling or child — without adding a placement marker to the
session. Resume, status, and purge read the stored path directly and do
not re-derive it from config, so:

- A session created under `sibling` resumes/purges from its sibling
  path even if the config later flips to `child`.
- A session created under `child` resumes/purges from its
  `.git-paw/worktrees/...` path.

This keeps resume/purge correct across config changes and avoids a
migration: the stored path is the single source of truth at teardown
time.

### Absent-config = sibling (back-compat)

`worktree_placement` is `Option`-like with default-on-absent =
`sibling`. A pre-existing `.git-paw/config.toml` written before this
field loads with sibling placement, byte-identical to v0.7.0. New repos
get `child` only because `git paw init` writes the field explicitly.
The field is serialised with `skip_serializing_if` so a default value
does not appear in round-tripped configs, preserving existing-config
equality.

## Risks & Trade-offs

- **Config flip mid-project leaves mixed layouts.** If a user switches
  `sibling` → `child` partway through, old worktrees stay siblings and
  new ones go under `.git-paw/worktrees/`. Accepted: the session JSON
  records concrete paths, so each worktree is torn down correctly
  regardless. We do not migrate.
- **`.git-paw/worktrees/` must be gitignored** or child worktrees would
  be staged as part of the repo. `git paw init` seeds the ignore entry;
  the gitignore requirement covers it. Repos that opt into `child`
  manually (editing config without re-running init) must add the
  ignore entry themselves — documented in the configuration reference.
- **Nested `.git` inside the repo tree.** Child worktrees place a git
  worktree directory under the repo root. `git worktree add` handles
  this correctly (the worktree has its own `.git` file pointing back to
  the main repo), and the ignore entry prevents accidental staging.
- **Slug collisions** are no more likely than in the sibling layout —
  the same sanitisation maps the same branches to the same names; two
  branches that sanitise identically already collided under the sibling
  scheme.
