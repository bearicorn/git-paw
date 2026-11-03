## 0. Prerequisite

- [ ] 0.1 Confirm `feat/spec-backend-strategy` (the `SpecBackend` methods `title_artifact`, `artifacts`, `parse_tasks` and `src/specs/backend_pin_tests.rs`) is merged into the release branch before starting

## 1. Backend location

- [ ] 1.1 Add the location method to `SpecBackend` in `src/specs/mod.rs` (path, tasks file, title file), per design.md decision 1; doc-comment it
- [ ] 1.2 OpenSpec: `<dir>/<id>/`, tasks `tasks.md`, title `proposal.md`
- [ ] 1.3 Spec Kit: resolve the feature directory from a `<feature>-T<n>` or `<feature>-phase-<n>` id; tasks `tasks.md`, title `spec.md`
- [ ] 1.4 Markdown: the spec file the scan read, including an id set by `paw_branch`; tasks and title from that file
- [ ] 1.5 Superpowers: `<dir>/<stem>.md`; tasks and title from that file
- [ ] 1.6 Unit tests per backend for the `spec-providers` scenarios "Directory formats locate a directory", "Single-file formats locate the file", "A Spec Kit phase entry locates its feature", plus two Spec Kit features whose names are prefixes of each other

## 2. MCP read tools

- [ ] 2.1 `src/mcp/query/specs.rs`: `list_specs`, `get_spec`, `get_tasks`, `derive_title` and `dependency_graph` use the backend's location; remove every `<dir>/<id>` join
- [ ] 2.2 Report `path` as the location relative to the repository root
- [ ] 2.3 Tests for each `mcp-server` scenario under "Spec read tools serve each format's own files" (Spec Kit, Markdown, Superpowers, OpenSpec, every reported path exists)

## 3. Regression pin

- [ ] 3.1 Update the expected `list:`, `get_spec:` and `task:` lines in `src/specs/backend_pin_tests.rs` for Spec Kit, Markdown and Superpowers to the corrected values; leave OpenSpec unchanged
- [ ] 3.2 Make the pin assert that each reported path exists (the tightened "MCP spec paths use the scan's directory" scenario), and name this change in its doc comment

## 4. Docs

- [ ] 4.1 `docs/src/user-guide/mcp.md`: per-format `path` (directory or `.md` file) and where `get_tasks` reads tasks for each format

## 5. Quality gates

- [ ] 5.1 `cargo fmt`, then `just lint`
- [ ] 5.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 5.3 `just deny`
- [ ] 5.4 `mdbook build docs/`
- [ ] 5.5 `openspec validate mcp-spec-artifacts-per-format --strict`
