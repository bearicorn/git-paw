## Context

The MCP read surface has no source-browsing tools; clients can't explore `src/`. The git-context tools already shell to `git` via `std::process::Command`, and `query/docs.rs::read_doc` established the canonicalize + `starts_with` repo-confinement guard. Reusing git plumbing gives gitignore handling and tracked-set semantics for free.

## Goals / Non-Goals

**Goals:**
- list → search → read trio so a client can trace logic across files over the LOCAL working tree.
- Exclude gitignored paths (secrets, build artifacts); confine to repo root.

**Non-Goals:**
- No write surface. No LSP-grade symbol navigation (go-to-def/find-refs) — grep + read covers tracing without new deps; symbol intelligence is a possible future change.
- No reading gitignored files (even within the repo root).

## Decisions

- **Listing:** `git ls-files --cached --others --exclude-standard [-- <subpath>]` — tracked + untracked-not-ignored, gitignored excluded, optional subpath scope.
- **Search:** `git grep -n -I -e <query> [-- <subpath>]` over the same working-tree set (`--cached --others` via `git grep --untracked`); return `{ path, line_number, line }`. `-I` skips binary files. Cap result count (e.g. first N matches) and `log()`/note truncation rather than silently dropping.
- **Read confinement:** resolve `path` under repo root, canonicalize, verify `starts_with` repo root (reuse the `read_doc` guard); additionally run `git check-ignore` (or rely on the working-tree set) to refuse gitignored paths. Reads the on-disk working-tree content (so uncommitted/branch state shows).
- **Degradation:** non-git dir / no matches → empty results, never a transport error (matches the rest of the surface).

## Risks / Trade-offs

- **Exposure surface:** broader than docs/specs — full source read. Mitigated: own machine, client-spawned, read-only, gitignore-excluded, repo-confined. Documented.
- **Search cost on huge repos:** `git grep` is fast; cap matches to bound response size.
- **Binary/large files:** `-I` skips binaries; `read_file` returns text — note/skip non-UTF8.
