## 1. Boot blocks and MCP publish tools

- [x] 1.1 Reword the `exports` description in `assets/boot-block-template.md` and `assets/boot-block-mcp-template.md` without cherry-picking
- [x] 1.2 Reword the `exports` field doc and tool description in `src/mcp/tools/publish.rs` the same way
- [x] 1.3 Add content assertions that neither template nor the publish-tool schema invites cherry-picking

## 2. Task prompts

- [x] 2.1 Replace "Publish `agent.done`" in `assets/task-prompts/speckit/consolidated-footer.md` and `assets/task-prompts/superpowers/execution.md` with the real completion mechanism (commit; the post-commit hook publishes the artifact)
- [x] 2.2 Remove "full enforcement is planned for v1.0.0" from `assets/task-prompts/speckit/validation-criteria.md`
- [x] 2.3 Add assertions over every bundled task prompt: no `agent.done`, no future-version promise

## 3. Error text

- [x] 3.1 Correct the bare-session `add`/`remove` error in `src/commands/helpers.rs`: drop "the default" and "in v0.6.0", state the limitation plainly
- [x] 3.2 Use the `session_name` parameter in the error message (per spec requirement)
- [x] 3.3 Add test for add command error: asserts session name, mentions supervisor, no stale claims
- [x] 3.4 Add test for remove command error: same assertions as add test

## 4. Gates

- [x] 4.1 Review every edited asset against the export-agnosticism checklist
- [x] 4.2 Verify every scenario in `specs/` maps to a test
- [x] 4.3 `cargo fmt`, then `just lint`
- [x] 4.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [x] 4.5 `openspec validate shipped-text-corrections --strict`
