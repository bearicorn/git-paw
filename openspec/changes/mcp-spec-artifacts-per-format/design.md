## Context

`src/mcp/query/specs.rs` builds every spec's location as `<dir>/<entry.id>`, reads `title_artifact()` and `tasks.md` inside it, and passes it to `artifacts()`. That shape holds only for OpenSpec. A Markdown or Superpowers spec is a single `<dir>/<stem>.md` file, and a Spec Kit entry id (`001-login-T002`, `001-login-phase-2`) is the feature directory plus a decomposition suffix. `spec-backend-strategy` moved the title, artifact and task parsing into `SpecBackend` but kept the MCP join, so the backends receive a path that does not exist.

## Goals / Non-Goals

**Goals:**
- Report a `path` that exists for every format, and serve each format's real artifacts, tasks and title.
- Keep the decision in the backend (`spec-providers` "Per-format behaviour is reached only through the backend").

**Non-Goals:**
- Changing the MCP response shapes, `SpecEntry` ids, branch names or the scan's decomposition.
- Changing what `get_spec` returns for a single-file format (the one artifact named after the entry).

## Decisions

### 1. A backend method resolves the location; `SpecEntry` gains no field

Add one required method to `SpecBackend`, for example `fn spec_location(&self, specs_dir: &Path, entry: &SpecEntry) -> SpecLocation`, returning the spec's path (a directory or a file), its tasks file and its title file. `title_artifact()` is absorbed into it or kept as the helper the directory formats use.

- OpenSpec: `<dir>/<id>/`, tasks `tasks.md`, title `proposal.md`.
- Spec Kit: the feature directory. The backend recovers it by matching the entry id against the feature directories under `specs_dir`: the id is `<feature>-T<n>` or `<feature>-phase-<n>`, and the feature is the directory whose name plus that suffix equals the id. Tasks `tasks.md`, title `spec.md`.
- Markdown: `<dir>/<file>.md`, the file the scan read. Tasks and title from the same file. The backend finds it the way its scan does (the id is the file stem, or `paw_branch`; see below).
- Superpowers: `<dir>/<stem>.md`. Tasks and title from the same file.

**Alternative rejected: a `source` field on `SpecEntry`.** It is the most direct carrier, but `SpecEntry` has 27 struct-literal sites across 9 files (AGENTS.md §7, GP-40). The location is derivable from the specs directory and the entry, so the field would add that ripple with no gain.

**Markdown with `paw_branch`.** When the spec sets `paw_branch`, the id is not the file stem, so the location cannot be derived from the id alone. The backend re-reads the specs directory and picks the pending file whose derived id equals the entry id, the same derivation its scan uses. This is one more directory read per call, which is acceptable for an MCP read tool.

### 2. `get_tasks` parses the location's tasks file with the format's parser

Markdown and Superpowers task lists come from the spec file itself. Their existing `parse_tasks` (checkbox parsing) already handles `- [ ]` lines. No new parser is needed.

### 3. The pin is updated, not removed

`per_format_outputs_are_pinned` keeps guarding all four formats. The expected `list:`, `get_spec:` and `task:` lines change to the corrected values. Its doc comment names this change as the deliberate edit.

## Risks / Trade-offs

- [Spec Kit feature names that are prefixes of each other, such as `001-login` and `001-login-t`] → match the whole id as `<feature>` + a known suffix (`-T<digits>` or `-phase-<digits>`), not by string prefix alone. A unit test covers the ambiguous pair.
- [MCP clients that appended `.md` themselves] → they now get a doubled suffix. The proposal records this as the only compatibility note. git-paw ships no such client.
