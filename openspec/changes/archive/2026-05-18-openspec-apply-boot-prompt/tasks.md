## 1. `SpecBackendKind` enum

- [x] 1.1 In `src/specs/mod.rs`, define `pub enum SpecBackendKind { OpenSpec, Markdown }` with `#[derive(Debug, Clone, Copy, PartialEq, Eq)]`. Place the enum next to the `SpecBackend` trait so the two are visually adjacent.
- [x] 1.2 Add a module-level doc comment line explaining that `SpecBackendKind` is the per-entry tag a `SpecBackend` implementation sets on every `SpecEntry` it returns, and that downstream consumers (notably `build_task_prompt`) dispatch on it.
- [x] 1.3 ~~Do **not** add a `SpecKit` variant in this change~~ — **Superseded at apply time (2026-05-14):** `spec-kit-format` is already archived (2026-05-13) and `SpecKitBackend` constructs `SpecEntry` literals in production; once `SpecEntry.backend` is non-optional, those sites fail to compile without a `SpecKit` variant. This change adds the variant, populates it in `SpecKitBackend::scan`, and falls the `SpecKit` arm of `build_task_prompt` through to the generic AGENTS.md pointer (same shape as Markdown). See design.md §D3 for the full rationale. The `// NOTE:` comment on the enum records this divergence so future contributors understand the context.

## 2. Add `backend` field to `SpecEntry`

- [x] 2.1 In `src/specs/mod.rs`, add `pub backend: SpecBackendKind` as a required field on `SpecEntry`. Place it after `id` (the other identifying field) and before `branch`.
- [x] 2.2 Update the struct's doc comment to mention that `backend` identifies the `SpecBackend` implementation that produced the entry.

## 3. Populate `backend` in `OpenSpecBackend::scan`

- [x] 3.1 In `src/specs/openspec.rs`, set `backend: SpecBackendKind::OpenSpec` on every `SpecEntry` literal constructed inside `scan()`. There should be exactly one construction site after a successful `tasks.md` read.
- [x] 3.2 No other change to the OpenSpec backend's logic. Frontmatter parsing, spec-content concatenation, file-ownership extraction, and archive-directory filtering all remain unchanged.

## 4. Populate `backend` in `MarkdownBackend::scan`

- [x] 4.1 In `src/specs/markdown.rs`, set `backend: SpecBackendKind::Markdown` on every `SpecEntry` literal constructed inside `scan()`. There should be exactly one construction site per pending file.
- [x] 4.2 No other change to the Markdown backend's logic.

## 5. Update test fixtures that build `SpecEntry` literals

- [x] 5.1 In `src/specs/mod.rs::tests`, update the `spec_entry_all_fields` and `spec_entry_optional_fields_absent` tests' fixtures to populate the new `backend` field. Use `SpecBackendKind::OpenSpec` for `spec_entry_all_fields` (the test asserts populated fields; OpenSpec is the natural choice) and `SpecBackendKind::Markdown` for `spec_entry_optional_fields_absent`.
- [x] 5.2 In `src/main.rs::tests::make_spec_entry`, change the helper signature to accept an optional `backend: SpecBackendKind` parameter, OR keep the parameterless signature but default `backend = SpecBackendKind::Markdown` so existing call sites preserve their semantics. (The Markdown-as-default keeps the existing v0.5.0 regression tests on the generic-pointer path.) Additional fixture sites updated to default to `Markdown`: `src/interactive.rs::tests::spec`, `src/interactive.rs::tests::bare_spec`, `src/specs/resolve.rs::tests::entry`.

## 6. Branch `build_task_prompt` on the backend

- [x] 6.1 In `src/main.rs`, modify `build_task_prompt`'s match arm for `Some(s)` to dispatch on `s.backend`:
  - `SpecBackendKind::OpenSpec` → return `format!("/opsx:apply {id}", id = s.id)`.
  - `SpecBackendKind::Markdown` → return the existing v0.5.0 generic AGENTS.md pointer (the format! string with the `openspec/changes/{id}/` interpolation).
  - `SpecBackendKind::SpecKit` → same as Markdown (generic AGENTS.md pointer); see design.md §D3.
- [x] 6.2 The match arm SHALL be written so the Rust compiler enforces exhaustiveness (no `_ =>` catch-all). If `SpecBackendKind` gains a variant later, the compiler shall force a decision. (Markdown + SpecKit are combined into one `|`-OR'd arm; the match is still exhaustive.)
- [x] 6.3 Update `build_task_prompt`'s doc comment to describe the per-backend dispatch and to note that the OpenSpec branch returns the bare slash command (no surrounding prose) so paste-aware CLIs parse it as a command at the start of the agent's first turn.
- [x] 6.4 The `None` branch is unchanged; it returns `"Begin your assigned task as described in AGENTS.md."` verbatim.

## 7. Unit tests in `src/main.rs::tests`

- [x] 7.1 `task_prompt_openspec_backend_invokes_opsx_apply_slash_command`: construct a `SpecEntry` with `id = "my-change"`, `backend = SpecBackendKind::OpenSpec`. Assert `build_task_prompt(Some(&entry)) == "/opsx:apply my-change"` exactly (no leading or trailing whitespace, no surrounding prose). Also assert the result does not contain `AGENTS.md` and does not contain `openspec/changes/` (proves the prose pointer is suppressed for the OpenSpec branch).
- [x] 7.2 `task_prompt_markdown_backend_uses_generic_agents_md_pointer`: construct a `SpecEntry` with `id = "my-feature"`, `backend = SpecBackendKind::Markdown`. Assert the result contains `AGENTS.md`, contains `openspec/changes/my-feature`, and does not contain `/opsx:apply`.
- [x] 7.3 `task_prompt_without_spec_unchanged_after_backend_introduction`: regression test for the `None` branch. Assert `build_task_prompt(None) == "Begin your assigned task as described in AGENTS.md."` verbatim.
- [x] 7.4 Ensure the existing tests `task_prompt_with_spec_points_at_agents_md_and_includes_id` and `task_prompt_does_not_include_spec_body_first_line` continue to pass after the fixture's backend defaults to `Markdown`. (Both tests assert behaviour of the generic-pointer branch, which Markdown now exercises.)

## 8. Quality gates

- [ ] 8.1 `just check` (fmt + clippy + tests) passes on the change branch.
- [ ] 8.2 `just deny` passes (no new dependencies).
- [ ] 8.3 No `unwrap()`/`expect()` introduced in `build_task_prompt`, the backend scanners, or any new test fixture.
- [ ] 8.4 All public items (the new enum and the new field) have doc comments.
- [ ] 8.5 Confirm via `cargo clippy --all-targets -- -D warnings` that the match in `build_task_prompt` is exhaustive (no `non_exhaustive` warnings).

## 9. Docs

- [ ] 9.1 No `--help` text changes — there is no new flag, no new subcommand, no new config field.
- [ ] 9.2 No README changes — this is internal behaviour observable only to agents the launcher boots.
- [ ] 9.3 No mdBook chapter changes are required. If `docs/src/user-guide/spec-driven-launch.md` mentions the boot-prompt content explicitly for OpenSpec specs, update it to describe the slash-command form. (Otherwise leave docs untouched.)
- [ ] 9.4 No architecture doc changes — module boundaries are unchanged.

## 10. Dogfood verification

- [ ] 10.1 Build the binary on the change branch.
- [ ] 10.2 Launch a fresh supervisor session against an OpenSpec-backed repo (this repo itself qualifies). Confirm that each coding-agent pane receives `/opsx:apply <change-id>` as its task prompt — visible by inspecting `tmux capture-pane` immediately after launch, or by tailing broker `agent.status` messages.
- [ ] 10.3 Launch a supervisor session against a Markdown-backed repo (or a temp fixture). Confirm each agent receives the v0.5.0 generic AGENTS.md pointer with the spec id interpolated.

## 11. Follow-ups (out of scope; captured here for cross-change coordination)

- [ ] 11.1 The `spec-kit-format` change SHALL extend `SpecBackendKind` with a `SpecKit` variant and SHALL extend `build_task_prompt` with a matching arm. Whether that arm returns a slash command (e.g. `/speckit:apply <feature>`) or falls through to the generic pointer is owned by `spec-kit-format`.
- [ ] 11.2 If dogfood surfaces users who cannot run the `/opsx:apply` skill (stripped-down CLI configurations, non-Anthropic harness), add a `[supervisor]` config flag (e.g. `openspec_use_slash_command = true|false`) to opt back into the v0.5.0 generic pointer. Defer until evidence demands it.
- [ ] 11.3 The Markdown branch's pointer path string still says `openspec/changes/<id>/`, which is misleading for Markdown specs. A separate change can refine that wording; not blocking this one.
