## Why

`git paw remove` on a just-started, otherwise-clean agent worktree intermittently failed as "uncommitted changes: `**WARNING:`" — a phantom entry that was actually a newline-bearing fragment of git-paw's own injected coordination block, misread as a file path. Root cause: `git::uncommitted_files` parsed `git status` output by splitting on newlines, so an entry whose content/path contained a newline (or the multi-line injected block) bled across lines into a bogus "path." Load-dependent (worse under `cargo llvm-cov`), it flaked two remove e2e tests. This fix (already built on `fix/remove-dirty-check-flake` @ `38918e2`) makes the dirty-check parse robustly.

## What Changes

- The uncommitted-work check parses `git status --porcelain -z` (NUL-delimited) instead of splitting on newlines, so a path or entry containing whitespace/newlines can never be misparsed into a phantom "changed file."
- Rename/copy status entries (which carry a second NUL-delimited path) are handled correctly.
- git-paw's own `.git-paw/` subtree is classified as managed by the path check, alongside the already-specified sidecar and managed `AGENTS.md` block.

## Capabilities

### New Capabilities
<!-- none -->

### Modified Capabilities
- `remove-branch`: ADD a **Robust uncommitted-work detection** requirement — the dirty-check parses git status NUL-delimited (never newline-split) and treats the `.git-paw/` subtree as git-paw-managed. This refines the existing **Uncommitted-work safety** requirement's reliability without changing its user-visible refuse/allow contract.

## Impact

- `src/git.rs` (`uncommitted_files`): `["status", "--porcelain", "-z"]` + NUL-split parse with rename/copy handling. `src/agents.rs` (`is_managed_path`): `.git-paw/` subtree classification. Already implemented on `fix/remove-dirty-check-flake` @ `38918e2` (+ a regression test).
- No config, CLI, or dependency change.
