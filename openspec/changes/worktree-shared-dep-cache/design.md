## Context

`worktree_provision::provision_worktree` runs, in order: env-file copy (`copy_env_files`, verbatim copy,
"never a symlink — a symlinked `.env` would be shared mutable state"), port block (`.env.local` managed
block), `on_create` hook. `contained_relative_path` rejects absolute and `..` paths. Teardown is
`main.rs::detach_worktree`: `on_remove` hook → `git worktree remove --force` → created-branch deletion.
v0.17.0 (`classifier-retirement`) deleted the only stack presets (`supervisor/dev_allowlist.rs`) and
left `[supervisor.common_dev_allowlist] stacks` parsed with no effect; config version 2
(`supervisor-vocabulary-split`) drops that table. There is no stack auto-detection, so git-paw has no
resolved stack to read. The coordination skill's orientation section says workers must not symlink
install artifacts and must run their stack's install step when artifacts are missing. The sandbox
(docs-only, operator-configured) allows writes only inside the worktree, shared `.git`, `TMPDIR`, the
CLI home and the toolchain cache.

## Goals / Non-Goals

**Goals:** let repo-local, content-addressed download caches be shared by every worktree; make
sharing build/install output impossible through this feature; keep it opt-in and stack-agnostic.

**Non-Goals:** sharing build output (`target/`) or install trees (`node_modules/`, `.venv/`) in any
form; managing user-global caches (already shared); exporting cache environment variables (default
locations are already shared; a consumer who relocates a cache sets the variable in their own shell or
`on_create` hook); a compile cache (a consumer can configure one; it is not a download cache).

## Decisions

- **D1 — Link to the main working tree's directory.** The cache the operator already populated there is
  the "already downloaded" store; no new storage location. Alternative — a git-paw store under
  `.git-paw/` that the main tree also links to — rejected: it would rewrite the operator's checkout.
- **D2 — Pure plan, then apply.** `plan_shared_links(entries, facts)` where facts are: contained?,
  ignored (`git check-ignore -q`)?, tracked files under it (`git ls-files -- <path>`)?, overlaps a
  forbidden output?, exists in worktree?, exists in main tree?. Decisions: `Link`, `CreateThenLink`,
  `Skip(reason)`. Table-tested without a filesystem; the shell creates links with
  `std::os::unix::fs::symlink` (Unix-only is the platform matrix).
- **D3 — One built-in forbidden-output list, applied to every entry.** It lives in
  `src/worktree_provision.rs` next to the planner, covers several stacks (never git-paw's toolchain
  alone) and is checked whatever the project's stack: refusing `node_modules` in a Rust project costs
  nothing, and there is no resolved stack to narrow it by. An entry equal to, inside, or containing a
  forbidden output is refused. The list only refuses; it never adds an entry.
- **D4 — Explicit `links` only; no presets.** The draft also had `presets = true`, adding per-stack
  shareable caches from `[supervisor.common_dev_allowlist] stacks`. v0.17.0 retired that table (no
  effect) and deleted the stack presets, so the presets are dropped rather than re-homed: an operator
  who keeps a repo-local cache names it in `links`. Forbidden-output content, kept small and marked to
  verify: `target`, `node_modules`, `.venv`, `venv` (Go — none built in; to verify). With no `links`
  the table is inert.
- **D5 — Unlink first on teardown.** `detach_worktree` removes every link it created (recorded as the
  set of configured entries that are symlinks into the main tree) before the `on_remove` hook and before
  `git worktree remove --force`, so no recursive delete can traverse into the shared cache. Whether
  `git worktree remove` would follow a symlink is irrelevant once the link is gone.
- **D6 — Order: after env/ports, before `on_create`.** The consumer's install hook then sees the shared
  cache.

## Security and safety review

- **Out-of-worktree reach (safety):** a link is a deliberate write path from the worktree into the main
  tree. Bounded to: contained repo-relative paths, gitignored, untracked, never outputs, never replacing
  files. A rogue agent could poison a shared cache (write a bad archive) that other worktrees then use —
  the same exposure as the user-global caches every worktree already shares; content-addressed caches
  verify checksums on use (tool-dependent — to verify). Documented in the security posture.
- **Destructive teardown:** D5 removes the only path by which `git worktree remove --force` could
  delete shared content.
- **Sandbox interaction:** under the documented sandbox the target is outside the writable set, so cache
  writes through the link are denied until the operator adds it — fail-closed; documented.
- **Injection:** entries are paths, never passed to a shell; git probes use discrete argv through
  `CommandRunner`.
- **Export-agnosticism:** the init template shows commented, multi-stack examples; no active default;
  no entry is derived from a stack; the forbidden-output list is multi-stack and refuse-only; the
  coordination-skill sentence names no stack, cache directory or package manager; covered by the
  existing `tests/lang_agnostic_skill_audit.rs` plus a new template test.

## Risks / Trade-offs

- [Few users have repo-local caches, so the feature helps a minority] → it is opt-in and small; the
  docs say when it helps and when it does not (most user-global caches are already shared).
- [Forbidden-output facts about third-party tools may be wrong or incomplete] → marked to verify;
  the list only refuses, and an operator never needs it to share a cache.

## Open Questions

- None.
