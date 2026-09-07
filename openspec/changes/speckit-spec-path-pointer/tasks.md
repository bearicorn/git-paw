## 1. Spec Kit doctrine arm

- [x] 1.1 Add a `SpecBackendKind::SpecKit` arm to `render_spec_path_doctrine` in `src/skills.rs` naming the Spec Kit spec location (`specs/<feature>/` or the injected sidecar), sourced from what the SpecKit backend already resolves — already present (naming `.specify/specs/<feature>/`), predating this change; no code change needed, only test strengthening (1.3)
- [x] 1.2 Grep `SpecBackendKind::SpecKit` across `src/` (per the variant-ripple checklist) to confirm no other consumer equates SpecKit with the OpenSpec path — found the actual bug: `build_task_prompt` (`src/main.rs`) grouped `SpecKit` with `Markdown` and pointed at `openspec/changes/<id>/`; `mcp/query/specs.rs` and `specs/mod.rs::backend_for_type` already handle SpecKit distinctly
- [x] 1.3 Tests: the doctrine rendered for SpecKit names the Spec Kit location and does NOT contain `openspec/changes/`; the OpenSpec doctrine still names `openspec/changes/`
- [x] 1.4 Fix `build_task_prompt` (`src/main.rs`) — give `SpecBackendKind::SpecKit` its own arm pointing only at the sidecar (no `openspec/changes/<id>/` claim); add a test asserting the SpecKit task prompt does not contain `openspec/changes/`

## 2. Gates

- [x] 2.1 `just check` green — `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` clean; full `cargo test --no-fail-fast` green except `worktree_env_provisioning::provisioned_files_are_present_before_the_agent_cli_starts`, which is unrelated to this change (exercises `build_task_prompt(None)`, untouched here) and reproduces identically across 3 isolated re-runs under the heavy concurrent load of 3 sibling dogfood agents also running full test suites on this machine at the time — the test's own comments document a 45s "generous" ceiling for exactly this load-sensitivity
- [x] 2.2 Every spec scenario maps to a test
