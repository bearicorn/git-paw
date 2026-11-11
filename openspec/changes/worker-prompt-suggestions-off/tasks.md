## 1. Fact gate

- [x] 1.1 With Claude Code 2.1.322, launch an interactive `claude` in a scratch repository under the worktree's git-ignored `target/`, once without and once with `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`; after one turn, capture the idle pane and confirm the suggestion shows only without the variable. Record the result in the commit body

## 2. Implementation

- [x] 2.1 Add `("CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION", "false")` to the `claude` provider's operator-isolation environment, after `CLAUDE_CODE_PLUGIN_DIRS`
- [x] 2.2 Update every exact launch-line and environment assertion (`src/agent_cli/tests.rs`, `src/commands/launch.rs` tests, `tests/cli_provider_dry_run.rs`, `tests/worker_operator_isolation_e2e.rs`); map the MODIFIED scenarios to them
- [x] 2.3 Tests "An isolated claude worker gets no prompt suggestions" and "An opted-out claude worker keeps the operator's setting" (`src/agent_cli/tests.rs`)

## 3. Docs and gates

- [x] 3.1 `docs/src/supported-clis.md` and `docs/src/user-guide/security-posture.md` list the variable and say why
- [x] 3.2 `just check`, `just deny`, `mdbook build docs/` and `openspec validate worker-prompt-suggestions-off --strict` pass (exit codes checked, not piped)
