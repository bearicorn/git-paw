## Context

Retro-spec for a built fix (`fix/remove-dirty-check-flake` @ `38918e2`). The remove dirty-check misparsed `git status` (newline split), so git-paw's own injected multi-line block surfaced as a phantom `**WARNING:` changed path, flaking two remove e2e tests (load-dependent, worse under `llvm-cov`). The existing **Uncommitted-work safety** requirement already specifies *which* files are git-paw-managed and that a clean-but-only-managed worktree removes without `--force`; the bug was purely in *how* status was parsed.

## Goals / Non-Goals

**Goals:** the dirty-check never misparses status output; git-paw-managed paths (including the `.git-paw/` subtree) are reliably excluded.
**Non-Goals:** no change to the user-visible refuse/allow contract (already specified under **Uncommitted-work safety**).

## Decisions

- **D1 — Parse `git status --porcelain -z` (NUL-delimited).** `-z` separates records and paths with NUL, so no path or content can bleed across records; the previous newline split was the bug. Rename/copy entries carry a second NUL-delimited path and are consumed accordingly. *Alternative:* quote-aware line parsing of default porcelain — rejected as fragile versus `-z`.
- **D2 — Classify the `.git-paw/` subtree as managed in `is_managed_path`.** git-paw's own scratch/injected tree is never the user's uncommitted work.

## Risks / Trade-offs

- `-z` output shape differs from line porcelain (NUL separators, two-field rename) → covered by the parse logic and a regression test asserting a newline-bearing path stays a single record.
