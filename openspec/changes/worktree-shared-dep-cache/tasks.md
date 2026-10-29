## 0. Preconditions

- [ ] 0.1 Verify the forbidden-output facts marked "to verify" in design.md D4 (`target`, `node_modules`, `.venv`, `venv`; nothing for Go) and the user-global default caches the proposal relies on (cargo, pip/uv, Go modules, pnpm) against each tool's documentation; correct any entry that does not hold
- [ ] 0.2 Rebase over v0.17 `purge-unmerged-branch-safety` (`detach_worktree`) and v0.17 `classifier-retirement` (no `src/supervisor/dev_allowlist.rs`, no stack presets)

## 1. Config and forbidden outputs

- [ ] 1.1 `WorktreeSharedDepsConfig { links: Vec<String> }` under `WorktreeConfig.shared_deps` (`serde(default, skip_serializing_if)`), included in `is_inert`; old configs load; round-trip test
- [ ] 1.2 Built-in multi-stack forbidden-output list in `src/worktree_provision.rs` (refuse-only, applied to every entry); unit tests (each listed output refused as equal, inside and containing; an unrelated path allowed)
- [ ] 1.3 Commented, multi-stack `[worktree.shared_deps]` example in `generate_default_config`; template test asserts every line is commented and more than one stack is shown

## 2. Provisioning (pure plan, then apply)

- [ ] 2.1 `src/git.rs`: `check_ignore_with`, `tracked_files_under_with` through `CommandRunner`
- [ ] 2.2 Pure `plan_shared_links(entries, facts) -> Vec<LinkDecision>` (`Link`, `CreateThenLink`, `Skip(reason)`) reusing `contained_relative_path`; table tests for every refusal (escape, not ignored, tracked, forbidden equal/inside/containing, existing path)
- [ ] 2.3 Apply step in `provision_worktree` after env/ports and before `run_hook`; warnings flow into `Provisioned.warnings`

## 3. Teardown

- [ ] 3.1 `detach_worktree`: unlink every shared-cache symlink pointing into the main working tree before the `on_remove` hook and `git worktree remove --force`

## 4. Skill

- [ ] 4.1 `assets/agent-skills/coordination.md` orientation section: shared-cache sentence (no stack or directory names); update the coordination-skill content test; `tests/lang_agnostic_skill_audit.rs` green

## 5. Tests per scenario (`tests/worktree_shared_deps.rs`, real repos via `tempfile`)

- [ ] 5.1 Declared gitignored cache linked before `on_create` (hook records `readlink`); missing main-tree dir created empty; absent table → no symlink, worktree identical
- [ ] 5.2 Effective entries are exactly `links` ("Only declared links are shared"); a table with no `links` creates no link ("No links, no entries")
- [ ] 5.3 Refusals: `node_modules`, `target/debug`, tracked `vendor/`, `../shared`, pre-existing directory — each skipped with a warning, nothing replaced
- [ ] 5.4 `git paw remove` and `git paw purge --force` keep the main tree's cache contents byte-identical

## 6. Docs

- [ ] 6.1 `docs/src/user-guide/runtime-isolation.md` (shared caches vs copied env files; when it helps; never build output), configuration reference `[worktree.shared_deps]`, `docs/src/user-guide/sandbox.md` (add the main-tree cache path to the writable set), `docs/src/user-guide/security-posture.md` (shared-cache poisoning note)

## 7. Gates

- [ ] 7.1 `cargo fmt`; `just check`; `just deny`; `mdbook build docs/`
