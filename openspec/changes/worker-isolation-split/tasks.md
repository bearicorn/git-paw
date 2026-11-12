# Tasks

## 1. Split the facet

- [x] 1.1 Split the claude isolation into worker-safety and operator-config parts (design D1); verify `agent_cli::tests::the_claude_family_declares_its_isolation` and that every default-launch exact-line test passes unchanged
- [x] 1.2 Make `CliProfile::operator_isolation` drop only the operator-config part on `inherit_operator_config = true`; replace `opting_out_restores_the_previous_launch` with `opting_out_keeps_worker_safety` and add `an_opted_out_claude_worker_still_gets_no_prompt_suggestions`
- [x] 1.3 Apply the same rule to delegates (design D3); update `commands::delegate_tests::a_delegate_gets_its_clis_operator_isolation_unless_opted_out`
- [x] 1.4 Add `tests/worker_operator_isolation_e2e.rs::an_opted_out_worker_keeps_auto_memory_off` covering start, add and recovery

## 2. Docs

- [x] 2.1 Write `docs/src/user-guide/customising-workers.md`: wrapper script (`CLAUDE_CONFIG_DIR=~/.claude-worker exec claude "$@"`), one-time login, installing plugins and skills there, the `[clis.claude-worker]` entry (`command`, `family = "claude"`, `settings_path`, `inherit_operator_config = true`), selecting it with `default_profile`, a worker profile or `--cli`; a codex `CODEX_HOME` example marked unverified; add it to `docs/src/SUMMARY.md`
- [x] 2.2 Update every page that says the opt-out restores the pre-v0.20.0 launch (`rg -n inherit_operator_config docs/src`): configuration README row, security-posture, supported-clis, cli-reference, agent-profiles, supervisor; link the new chapter
- [x] 2.3 The changelog entry comes from this change's commit message (git cliff generates CHANGELOG.md at release prep); `mdbook build docs` passes

## 3. Integration

- [x] 3.1 Run the project's full check (fmt, clippy, tests) and `openspec validate worker-isolation-split --strict`; both pass

## Workflow follow-up

- Archive the change, sync deltas into `openspec/specs/`, then continue the v0.20.0 release.
