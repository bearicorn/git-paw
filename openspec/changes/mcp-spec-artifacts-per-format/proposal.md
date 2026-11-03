## Why

The MCP spec read tools (`get_specs`, `get_spec`, `get_tasks`) return wrong data for three of the four spec formats. Every format's spec is assumed to be a directory named `<dir>/<id>/` with a `tasks.md` inside it, which holds only for OpenSpec. The `per_format_outputs_are_pinned` regression pin in `src/specs/backend_pin_tests.rs` records the wrong output, and it passes on code from before v0.19, so the bug predates `spec-backend-strategy`; the strategy move only preserved it.

| Format | Wrong today |
|---|---|
| Spec Kit | `path` is `specs/001-login-T002` (the entry id, with a task-decomposition suffix, names no directory); `get_spec` returns no artifacts; `get_tasks` returns no tasks |
| Markdown | `path` is `specs/fix-bug` while the file is `specs/fix-bug.md`; `get_tasks` returns nothing although the spec has `- [ ] reproduce` |
| Superpowers | `path` is `plans/2026-07-20-add-auth` without `.md`; `title` is the file stem, not the plan's `# Add auth Implementation Plan` heading; `get_tasks` returns nothing |

The spec scenario that should have caught this, "MCP spec paths use the scan's directory", only checks the path's prefix.

## What Changes

- Each spec format decides where its spec lives: a directory (OpenSpec, the Spec Kit feature directory) or a single file (Markdown, Superpowers). That decision moves into the format's `SpecBackend`; `src/mcp/query/specs.rs` stops joining `<dir>/<id>` itself.
- `get_specs` and `get_spec` report a `path` that exists in the repository: the feature directory for a Spec Kit entry, the `.md` file for Markdown and Superpowers.
- `get_spec` returns a Spec Kit feature's `spec`, `plan`, `tasks` and checklist artifacts for every entry decomposed from that feature.
- `get_tasks` parses each format's own task list: the feature's `tasks.md` for Spec Kit, and the spec file itself for Markdown and Superpowers.
- A Superpowers title is the plan's first `# ` heading.
- The "MCP spec paths use the scan's directory" scenario is tightened: the path also has to resolve to an existing file or directory.
- The pin test's expected outputs are updated on purpose. The updated pin stays the regression guard.

No MCP tool, parameter or response field is added or removed; only the values change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `spec-providers`: the `SpecBackend` trait also decides a spec's location and the file its tasks and title are read from. "Per-format outputs are unchanged by the move" now excepts the outputs this change corrects. "MCP spec paths use the scan's directory" now requires the path to exist.
- `mcp-server`: a new requirement states what each format's spec read tools return: path, title, artifacts and tasks.

## Impact

- **Code**: `src/specs/mod.rs` (the `SpecBackend` trait), the four implementors `src/specs/{openspec,markdown,speckit,superpowers}.rs`, `src/mcp/query/specs.rs` (`list_specs`, `get_spec`, `get_tasks`, `derive_title`, `dependency_graph`), `src/specs/backend_pin_tests.rs` (expected outputs).
- **Trait ripple**: adding a required trait method breaks every `impl SpecBackend for`. On `feat/spec-backend-strategy` there are exactly four, one per backend file above; no test double implements the trait.
- **No struct-literal ripple**: `SpecEntry` gains no field. The 27 `SpecEntry {` literal sites (`src/commands/launch.rs` 9, `src/interactive/resolver.rs` 4, `src/specs/mod.rs` 4, `src/git.rs` 2, `src/specs/resolve.rs` 2, `src/specs/speckit.rs` 2, `src/specs/superpowers.rs` 2, `src/specs/markdown.rs` 1, `src/specs/openspec.rs` 1) stay untouched; see design.md.
- **No enum ripple**: no `SpecBackendKind` or `BrokerMessage` variant changes.
- **Dependency on unmerged code**: the trait methods this change extends (`title_artifact`, `artifacts`, `parse_tasks`) and the pin test exist on `feat/spec-backend-strategy`, whose specs are archived into `openspec/specs/` but whose code is not yet on `feat/v0.19.0-specs`. Implementation starts after that branch is merged.
- **Docs**: `docs/src/user-guide/mcp.md`, where it documents `get_specs` / `get_spec` / `get_tasks` per format.
- **Compatibility**: MCP clients that worked around the wrong paths (for example by appending `.md`) receive a path that already carries the suffix.
