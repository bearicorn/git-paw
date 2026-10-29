## 1. Pin today's behaviour

- [ ] 1.1 Add a table-driven test that, for a fixture spec of each format, records the task prompt, path doctrine, `list_specs` title and path, `get_spec` artifact names, `get_tasks` output, opsx-region gating and governance auto-wire result; this is the guard every later step must keep green
- [ ] 1.2 Re-grep `SpecBackendKind::` and every `"openspec" | "markdown" | "speckit" | "superpowers"` literal outside `src/specs/` and tests; confirm the 7 matches and 8 string sites in the proposal (plus any added by v0.17.0 / v0.18.0) and record the list in the PR

## 2. One format type

- [ ] 2.1 Add `SpecBackendKind::ALL`, `as_str` and `FromStr` (error message built from `ALL`, byte-identical to today's "unknown spec type" text)
- [ ] 2.2 Add `From<SpecsFormat> for SpecBackendKind`; replace `SpecsFormat::as_str` string plumbing in `main.rs` / `commands/{start,supervisor}.rs` with the kind; `--help` and the clap rejection message unchanged (existing `cli.rs` tests pass)
- [ ] 2.3 Parse `[specs] type` once in `resolve_specs_config`, returning the kind and the resolved directory; `default_dir` supplied only on the `--specs-format` path, as today

## 3. Move behaviour into the trait

- [ ] 3.1 Add the trait methods (design D2) with the shared defaults; implement them in the four backends by moving the bodies out of the call sites, assets unchanged
- [ ] 3.2 Add `SpecBackendKind::backend()`; replace `backend_for_type` with it
- [ ] 3.3 `skills::render_spec_path_doctrine` and the opsx region gate in `skills::render` → `path_doctrine` / `uses_opsx_workflow`
- [ ] 3.4 `build_task_prompt` → `task_prompt` (keeping the no-spec fallback where it is)
- [ ] 3.5 `mcp/query/specs.rs`: `backend_str` → `as_str`, `derive_title` → `title_artifact`, `get_spec` → `artifacts`, `get_tasks` → `parse_tasks`; delete `resolve_dir` in favour of the shared resolver
- [ ] 3.6 `mcp/server.rs` valid set and `mcp/tools/project.rs::get_skill` backend selection → `ALL` / `FromStr`
- [ ] 3.7 `main.rs::cmd_dashboard` role-gating check and `config::auto_wire_governance` → `uses_opsx_workflow` / `constitution`
- [ ] 3.8 `init::SPEC_SYSTEMS` → `ALL` + `default_dir` (init prompt order and written values unchanged; existing init test asserting the order passes)
- [ ] 3.9 Add a source-audit guard in `tests/source_audit.rs`: no spec-type string literal compared outside `src/specs/`, and no `match` on `SpecBackendKind` outside `backend()`

## 4. Markdown task prompt (behaviour change)

- [ ] 4.1 Rewrite `assets/boot-task-prompts/markdown.md` to point at the sidecar and state the spec file is the whole contract; no `openspec/changes/`
- [ ] 4.2 Test: no Markdown, Spec Kit or Superpowers task prompt contains `openspec/changes/`; update the pinned Markdown row from 1.1 in the same commit
- [ ] 4.3 Run the agnostic-asset audit (`tests/lang_agnostic_skill_audit.rs`) over the rewritten asset

## 5. Spec traceability

- [ ] 5.1 Map every scenario in `specs/spec-providers` and `specs/core-lang-agnostic` to a named test

## 6. Docs

- [ ] 6.1 `AGENTS.md` §7: the `SpecBackendKind` ripple list becomes the enum, `backend()`, `ALL` / `FromStr` and the new backend impl; drop the string-site list v0.17.0 adds
- [ ] 6.2 `docs/src/architecture.md`: the spec backend Strategy; `docs/src/contributing.md`: how to add a spec format
- [ ] 6.3 `docs/src/user-guide/spec-driven-launch.md`: Markdown workers read the sidecar; no OpenSpec path

## 7. Quality gates

- [ ] 7.1 `cargo fmt`, then `just lint`
- [ ] 7.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 7.3 `just deny`
- [ ] 7.4 `mdbook build docs/`
- [ ] 7.5 `openspec validate spec-backend-strategy --strict`
