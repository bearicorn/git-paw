## 0. Preconditions

- [ ] 0.1 Confirm `start-spec-launch-parity` and `broker-repo-identity` are merged into the branch first (both edit `src/commands/start.rs` / `src/commands/supervisor.rs` / `src/main.rs`); rebase onto them before starting
- [ ] 0.2 Remember the `git-paw` `[[bin]]` has `test = false`: `#[cfg(test)]` code in `src/main.rs` / `src/commands/*.rs` never runs — put bin-side scenario tests in `tests/`

## 1. Failing tests first

- [ ] 1.1 `src/config/tests.rs`: `launch_command_falls_back_to_name_without_entry`, `launch_command_quotes_path_with_space_as_one_shell_word` (build the command for `[clis.my-agent] command = "<tempdir>/My Tools/my-agent"` with an executable stub that prints a marker; run it via `sh -c` and assert the marker), `launch_command_preserves_trailing_text`, plus a case asserting a safe-set command (`claude`, `/usr/local/bin/claude`) is emitted unquoted
- [ ] 1.2 `src/config/tests.rs`: `custom_cli_mcp_field_round_trips_and_is_skipped_when_unset` (serialize with `mcp: None` → no `mcp` key; `Some(true)` round-trips)
- [ ] 1.3 `src/skills.rs` tests: `cli_speaks_mcp_honours_config_opt_in`, `cli_speaks_mcp_honours_config_opt_out`, `cli_speaks_mcp_unset_custom_cli_falls_back_to_helper`; update `cli_speaks_mcp_known_clis_true_unknown_clis_false` to the new signature and set (`cursor-agent` true; `cursor`, `aider` false)
- [ ] 1.4 `src/detect.rs` tests: `cursor_agent_detected_and_editor_launcher_not_known`; update `all_known_clis_detected_when_present`'s name list (`cursor` → `cursor-agent`)
- [ ] 1.5 `src/interactive/resolver.rs` tests: `explicit_cursor_cli_passes_through_without_detection` (bare `--cli cursor` with no `cursor` in the available list maps every branch to `cursor`)
- [ ] 1.6 `src/tmux/tests.rs`: `gate_pane_resolves_profile_by_identity_and_relaunches_command` — drive `gate_pane_generic` (or the identity/command split) with a fake capture and a recording relaunch; adapt `gate_pane_for_injection_uses_the_resolved_cli_profile` to the new signature
- [ ] 1.7 New `tests/cli_launch_identity_e2e.rs` (tmux; dedicated socket + `HOME`/`XDG_DATA_HOME` overrides per `core-ci-hygiene`; model on `tests/e2e_supervisor_approval_flags.rs`, using `echo`-style stubs so pane scrollback shows the launched command): `worker_pane_launches_configured_command_not_key`, `saved_session_records_cli_key`, `worker_overrides_resolve_by_key_and_launch_command`, `supervisor_pane_launches_configured_command`, `recovery_launches_configured_command_with_resume_args`, `mcp_opt_in_reaches_worker_boot_prompt`
- [ ] 1.8 New `tests/supported_clis_docs_sync.rs`: `readme_and_book_tables_match_known_cli_roster` (parse the backticked binary column of both tables; compare with `git_paw::detect::known_cli_names()`)
- [ ] 1.9 Run the new tests and confirm they fail for the right reason before implementing

## 2. Detection roster (DS-6)

- [ ] 2.1 `src/detect.rs`: replace `cursor` with `cursor-agent` in `KNOWN_CLIS`; add `"cursor-agent" => "Cursor Agent"` to `known_display_name`
- [ ] 2.2 `src/detect.rs`: add `pub fn known_cli_names() -> &'static [&'static str]` (doc-commented) returning `KNOWN_CLIS`

## 3. Launch command (GP-25)

- [ ] 3.1 `src/config/cli.rs`: add `pub fn launch_command(cli, &clis) -> String` per design D1/D2 (leading-token lookup; `domain::shell_quote` only when a char falls outside the safe set; remainder appended verbatim); re-export from `src/config/mod.rs` alongside the other resolvers
- [ ] 3.2 `src/main.rs` `attach_agent`: start `cli_command` from `config::launch_command(ctx.agent_cli, ctx.clis)`; keep `entry.cli = ctx.agent_cli` and every resolver keyed by `ctx.agent_cli`
- [ ] 3.3 `src/commands/start.rs` `cmd_start` and `cmd_start_with_specs`: build `PaneSpec.cli_command` from `launch_command(cli, &config.clis)` composed with the resolved approval flags (no trailing separator when flags are empty); keep `WorktreeEntry.cli = cli`
- [ ] 3.4 `src/commands/supervisor.rs`: `supervisor_cli_command` from `launch_command(&supervisor_cli, &config.clis)` + flags
- [ ] 3.5 `src/commands/recover.rs`: `cli_command_for_recovery` starts from `launch_command(&entry.cli, clis)` (resume args still resolved by `entry.cli`); `recover_supervisor_session` likewise for the supervisor pane
- [ ] 3.6 `src/tmux/readiness.rs` `gate_pane_for_injection`: add a `cli_id: &str` parameter used for `resolve_prompt_profile`; keep `cli_command` for the relaunch. Thread it through `src/main.rs` `gate_pane_or_fail_on_dialog` and its callers in `src/commands/supervisor.rs` (supervisor + agent loop) and `src/commands/add.rs`
- [ ] 3.7 Grep `src/` for every remaining `cli_command:` / `send-keys` construction from a bare CLI identity and fix or justify each

## 4. MCP opt-in (GP-29)

- [ ] 4.1 `src/config/cli.rs` `CustomCli`: add `pub mcp: Option<bool>` with `#[serde(default, skip_serializing_if = "Option::is_none")]` and a doc comment stating the stranding risk; update every `CustomCli { .. }` literal (grep `src/` and `tests/`)
- [ ] 4.2 `src/skills.rs`: `MCP_CAPABLE_CLIS = ["claude", "codex", "cursor-agent", "agy"]`; change `cli_speaks_mcp` to `cli_speaks_mcp<S: BuildHasher>(cli: &str, clis: &HashMap<String, CustomCli, S>) -> bool` (override first, then table, on the leading token); update its doc comment
- [ ] 4.3 Update the callers: `src/commands/start.rs` (both boot-inject loops), `src/main.rs` `attach_agent`, `src/commands/supervisor.rs` (supervisor boot block)

## 5. Docs

- [ ] 5.1 `README.md` Supported AI CLIs table: `Cursor Agent | \`cursor-agent\``
- [ ] 5.2 `docs/src/supported-clis.md`: row → `Cursor Agent | \`cursor-agent\` | Cursor's agent CLI`; add a short note that the `cursor` editor launcher is no longer auto-detected and how to keep using it (`--cli cursor` or a `[clis.cursor]` entry)
- [ ] 5.3 `docs/src/configuration/README.md` Custom CLIs: state that panes launch `command` while the `[clis.<name>]` key stays the identity for approval/session/profile/delay lookups; document `mcp = true|false` (type, default unset, override order, and the warning that `mcp = true` without git-paw's MCP server registered for that CLI strands the agent at boot); keep the `claude-variant` example and add `mcp` to it commented
- [ ] 5.4 `docs/src/user-guide/coordination.md` (*MCP tool form*) and `docs/src/user-guide/skill-templates.md`: list `cursor-agent` instead of Cursor/`cursor` and mention the `[clis.<name>].mcp` override
- [ ] 5.5 `--help`: no CLI surface change; confirm nothing in `src/cli.rs` names the `cursor` binary as a supported agent CLI

## 6. Spec traceability

- [ ] 6.1 Every scenario in `specs/cli-resolution/spec.md` and `specs/boot-block/spec.md` maps to a test that exists and runs under `cargo test` (none in `src/main.rs` / `src/commands/`)

## 7. Gates

- [ ] 7.1 `cargo fmt`, then `just lint`
- [ ] 7.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast > target/test.log 2>&1; echo $?` — confirm `0`, then read the log
- [ ] 7.3 `mdbook build docs/`
- [ ] 7.4 `openspec validate cli-launch-identity --strict`
