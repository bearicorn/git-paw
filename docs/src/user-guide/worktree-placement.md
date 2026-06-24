# Worktree Placement

Every agent git-paw launches works in its own **git worktree** — an
independent checkout of a branch that shares the repository's object store.
`worktree_placement` controls *where* git-paw creates that directory.

## Child vs sibling

There are two layouts:

| Placement   | Worktree path                              | When it applies                                    |
|-------------|--------------------------------------------|----------------------------------------------------|
| `child`     | `<repo>/.git-paw/worktrees/<branch-slug>/` | New repos (`git paw init` writes it)               |
| `sibling`   | `../<project>-<branch-slug>/`              | The default when `worktree_placement` is absent    |

**Sibling** is the original (v0.7.0) layout: worktrees are created in the
repository's *parent* directory, beside the repo. Three agents on
`my-project` produce `../my-project-feat-a/`, `../my-project-feat-b/`, and
`../my-project-fix-c/` — scattered across the parent folder.

**Child** keeps worktrees *inside* the repository, under
`.git-paw/worktrees/`. The same three agents produce
`.git-paw/worktrees/feat-a/`, `.git-paw/worktrees/feat-b/`, and
`.git-paw/worktrees/fix-c/`. This matches how tools like Claude Code keep
worktrees within the project tree.

The child-layout slug comes from the branch name alone — `/` becomes `-` and
characters outside `[A-Za-z0-9._-]` are stripped — so `feat/auth-flow` →
`.git-paw/worktrees/feat-auth-flow/` and `fix/issue#42` →
`.git-paw/worktrees/fix-issue42/`. The project name is not prepended because
the directory already lives under that project's `.git-paw/worktrees/`.

## Why the contained layout

The motivation for `child` is a **project-scoped permission model**. An agent
CLI (or an MCP client, or a sandbox profile) scoped to the repository can
read and write `.git-paw/worktrees/` with a single grant. A sibling worktree
lives *outside* the repository the tool is scoped to, so it either falls
outside the grant or forces a broader, less precise one. Containing the
worktrees lets one grant for `.git-paw/worktrees/` cover every agent.

Because child worktrees live inside the repo tree, they must be ignored so
git never tries to stage them. `git paw init` adds `.git-paw/worktrees/` to
`.gitignore` for you. If you switch an existing repo to `child` by editing
the config manually, add that entry yourself. Git itself handles the nested
`.git` correctly — a linked worktree carries its own `.git` file pointing
back at the main repository.

## Existing sessions are never moved

Placement only governs **new** worktree creation. git-paw records the
concrete, absolute path of each worktree in the session it created, and
resume, status, and purge all operate on that **recorded path** — they never
re-derive it from the current `worktree_placement`. The consequences:

- A session created under `sibling` keeps resuming and purging from its
  sibling path even if you later flip the config to `child`.
- A session created under `child` keeps using its `.git-paw/worktrees/...`
  path.
- Flipping the config mid-project leaves a mixed layout (old worktrees stay
  put, new ones follow the new setting). Each worktree is still torn down
  correctly because the recorded path is the single source of truth at
  teardown time. git-paw does not migrate existing worktrees.

This makes the upgrade to v0.8.0 a no-op for existing repositories: with no
`worktree_placement` field, the effective placement is `sibling`, byte-for-byte
identical to v0.7.0.

## Setting it

Set it per-repo in `.git-paw/config.toml` or globally in
`~/.config/git-paw/config.toml` (repo wins on conflict):

```toml
worktree_placement = "child"   # or "sibling"
```

See the [Configuration reference](../configuration/README.md#worktree_placement)
for the full field description and merging rules.
