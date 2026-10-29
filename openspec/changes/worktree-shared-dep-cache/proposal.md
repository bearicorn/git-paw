## Why

Each git-paw worktree is a fresh checkout, so a dependency cache that a project keeps **inside the
repository** (gitignored) is empty in every new worktree, and N parallel agents each re-download it
(dogfood tip 2026-08-06). MILESTONE's caveat shapes the whole design: share only a content-addressed,
read-mostly **download cache**; never share a per-project **build/install output** (`target/`,
`node_modules/`, a per-project `.venv`) that agents write concurrently.

Verifying the premise narrowed it: the default download caches of most toolchains already live in a
user-global location (the cargo registry under `CARGO_HOME`, the pip / uv caches, the Go module cache,
the pnpm store — to verify per tool), so worktrees *already* share them and nothing re-downloads. What
remains per worktree is (a) repo-local caches a project opts into (for example an offline package cache
kept inside the repo and gitignored), which really are empty in each worktree, and (b) build and install
output, which must stay per worktree. This change serves (a) and refuses (b).

## What Changes

- New opt-in `[worktree.shared_deps]` table with `links` (repo-relative cache directories). Only the
  listed entries are shared; there are no stack presets.
- At worktree creation, before `on_create`, each entry is symlinked from the worktree to the same path
  in the **main working tree**, which holds the already-populated cache (created empty if missing).
- Safety rules, checked per entry, every time: repo-relative and contained; gitignored and holding no
  tracked file; never a build/install output on git-paw's built-in forbidden-output list, nor inside or
  around one; never replacing anything that already exists in the worktree. A failing entry is skipped with a
  warning; worktree creation never fails because of it.
- Teardown removes the links before the worktree is removed, so the shared cache is never deleted with a
  worktree.
- The forbidden-output list is a small built-in list covering several stacks (never git-paw's own
  toolchain alone), kept in `src/worktree_provision.rs` and applied to every entry whatever the
  project's stack. It only refuses; it never adds an entry. (The draft's stack presets read
  `[supervisor.common_dev_allowlist] stacks`; v0.17.0 deprecated that table with no effect and
  deleted the stack presets, so they are dropped.)
- The exported coordination skill tells workers that a linked cache is shared and read-mostly: use it,
  never delete, clean or replace it.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `session-runtime-isolation`: ADDED shared-cache linking, its safety rules, its teardown, and its
  explicit, export-agnostic declaration.
- `skill-agent`: **Coordination skill — worktree environment orientation** gains the shared-cache rule.

## Impact

- **Code:** `src/config/mod.rs` (`WorktreeConfig.shared_deps: Option<WorktreeSharedDepsConfig { links
  }>`, included in `is_inert`; template comment), `src/worktree_provision.rs` (pure
  `plan_shared_links(entries, facts) -> Vec<LinkDecision>` + the symlink step before `run_hook`, reusing
  `contained_relative_path`, and the built-in forbidden-output list), `src/main.rs::detach_worktree`
  (unlink before `git worktree remove`), `src/git.rs` (`check_ignore_with`, `ls_files_under_with`),
  `assets/agent-skills/coordination.md` (shared-cache sentence).
- **Enum-variant ripple:** none.
- **Docs:** `docs/src/user-guide/runtime-isolation.md` (shared caches vs copied env files; when it helps),
  configuration reference `[worktree.shared_deps]`, `docs/src/user-guide/sandbox.md` (a linked cache is
  outside the worktree: add the main-working-tree cache path to the writable set, or the worker's cache
  writes are denied), init default config (commented table, multi-stack `# or:` examples).
- **Cross-version dependencies:** v0.17 `purge-unmerged-branch-safety` changes `detach_worktree` (branch
  retention) — the unlink step is added at the start of the same function; rebase over it. v0.18
  `cmd-library-surface` moves `detach_worktree` under `src/commands/`. This release's
  `config-schema-versioning` (other half) — the new table joins the versioned config wire type;
  `config-check-command` (this half) — forbidden entries are also reported by `git paw config check`.
  v0.17.0's `classifier-retirement` deprecates `[supervisor.common_dev_allowlist]` with no effect and
  deletes `src/supervisor/dev_allowlist.rs` and its stack presets, so this change reads no stack list
  and keeps its forbidden-output list in `worktree_provision`; `supervisor-vocabulary-split` drops that
  table in config version 2.
- **Security / safety:** a symlink is a path out of the worktree. Entries are contained, gitignored,
  never outputs, never replace existing files, and are unlinked before any recursive delete. See
  design.md.

## Rebase note (2026-10-19)

If this change archives after `supervisor-vocabulary-split`, the user-facing text it adds (config
keys, the init template comment, the configuration reference, user-guide pages) SHALL use the new
names (`--coordinated` / `--bare`, `[orchestrator]` / `[workers]` / `[drive]`, `default_mode`). No
old term remains in its deltas: the only one, `[supervisor.common_dev_allowlist] stacks`, went with
the dropped presets (2026-10-20, after v0.17.0's `classifier-retirement`).
