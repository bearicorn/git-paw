## 1. Boot blocks and MCP publish tools

- [ ] 1.1 Reword the `exports` description in `assets/boot-block-template.md` and `assets/boot-block-mcp-template.md` without cherry-picking
- [ ] 1.2 Reword the `exports` field doc and tool description in `src/mcp/tools/publish.rs` the same way
- [ ] 1.3 Add content assertions that neither template nor the publish-tool schema invites cherry-picking

## 2. Task prompts

- [ ] 2.1 Replace "Publish `agent.done`" in `assets/task-prompts/speckit/consolidated-footer.md` and `assets/task-prompts/superpowers/execution.md` with the real completion mechanism (commit; the post-commit hook publishes the artifact)
- [ ] 2.2 Remove "full enforcement is planned for v1.0.0" from `assets/task-prompts/speckit/validation-criteria.md`
- [ ] 2.3 Add assertions over every bundled task prompt: no `agent.done`, no future-version promise

## 3. Error text

- [ ] 3.1 Correct the bare-session `add`/`remove` error in `src/commands/helpers.rs`: drop "the default" and "in v0.6.0", state the limitation plainly

## 4. Gates

- [ ] 4.1 Review every edited asset against the export-agnosticism checklist
- [ ] 4.2 Verify every scenario in `specs/` maps to a test
- [ ] 4.3 `cargo fmt`, then `just lint`
- [ ] 4.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 4.5 `openspec validate shipped-text-corrections --strict`
