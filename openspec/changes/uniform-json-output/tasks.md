## 1. Output seam (no key changes)

- [ ] 1.1 Add `src/output.rs` with `print_json<T: Serialize>(&T)`, which writes one document plus a newline to stdout, and with typed DTOs reproducing today's `status`, `approvals` and `doctor` documents key for key, keeping their current compact or pretty formatting
- [ ] 1.2 Switch `commands/status.rs`, `commands/approvals.rs` and `doctor::render_json` to the DTOs; gate: `tests/status_json_cli_session_id.rs`, `tests/session_stale_hygiene.rs`, `tests/approvals_integration.rs`, `tests/doctor_e2e.rs` and `src/doctor_tests.rs` pass unchanged

## 2. Versioned documents

- [ ] 2.1 Add `schema_version: 1` to every DTO; the no-session status document becomes `{"schema_version":1,"session":null}`
- [ ] 2.2 `status`: add `terminal_alive` (same value as `tmux_running`, which is kept) and `broker` (`null`, or `{url, state}` with `running|paused|not_responding|foreign` from the same probe the human output uses)
- [ ] 2.3 `doctor`: add a stable `id` to `CheckResult`, declared beside each check constructor (`<group-slug>.<check-slug>`), never derived from display text

## 3. New --json flags

- [ ] 3.1 `src/cli.rs`: `--json` on `Add`, `Remove` and `ListClis`, with the same help string as the existing flags; update the `long_about` examples
- [ ] 3.2 `cmd_add` renders `AddOutput {session, branch, agent_id, cli, worktree_path, paused}` in JSON mode, with no pane index, the success line suppressed, and warnings still on stderr
- [ ] 3.3 `cmd_remove` renders `RemoveOutput {session, branch, worktree_path, worktree_kept}`; the "Keeping worktree on disk" line is suppressed in JSON mode; the safety refusal is unchanged
- [ ] 3.4 `cmd_list_clis` renders `ClisOutput {clis: [{name, display_name, source, path}]}`; an empty roster is `[]` with exit 0

## 4. Tests (one or more per scenario)

- [ ] 4.1 `tests/json_output_contract.rs`: walk `Cli::command()` for subcommands with `--json`, assert the set equals the roster (including `config check`, which needs `config-check-command` to have landed; until it has, the guard's expected set omits it with a comment naming that change), and assert the CLI reference documents each one; run each roster command in a temp repo (HOME overridden) and assert one object, an integer `schema_version` and no `0x1B` byte, including with `CLICOLOR_FORCE=1`
- [ ] 4.2 Failure semantics: `status --json` outside a repo leaves stdout empty with the same stderr and exit code as human mode; `doctor --json` with a failing check prints the full document and exits non-zero
- [ ] 4.3 Golden key-set test per roster document (names and JSON types)
- [ ] 4.4 Status: the no-session document, both liveness keys true when alive, agents exited with the terminal session still up gives `status` `stopped` and `terminal_alive` true, stale gives `terminal_alive` false, broker `running` with the recorded URL, broker `null` when disabled, existing keys unchanged
- [ ] 4.5 Doctor: ids are well-formed, unique, and identical across two runs; `schema_version` and the existing fields are present
- [ ] 4.6 Approvals: versioned document with entry keys; an empty log gives `[]` and exit 0
- [ ] 4.7 `add --json` (active and paused session), with a failed `add --json` leaving stdout empty; `remove --json` (default and `--keep-worktree`), with the uncommitted-work refusal leaving stdout empty
- [ ] 4.8 `cli.rs` parse tests: `status` / `status --json`, `list-clis` / `list-clis --json`; `list-clis --json` e2e (custom CLI listed as `custom`, empty roster gives `[]`)

## 5. Docs

- [ ] 5.1 `docs/src/cli-reference.md`: a "Machine-readable output" section (the contract, `schema_version`, additive rule, consumers ignore unknown keys, `tmux_running` deprecated in favor of `terminal_alive`) and each command's document
- [ ] 5.2 `--help` / `long_about` for `add`, `remove` and `list-clis` show a `--json` example; README CLI table updated if it lists flags
- [ ] 5.3 After archive: replace the generated `## Purpose` of `openspec/specs/cli-machine-output/spec.md` with the Purpose paragraph from this change's delta if the archive did not carry it over, and add the capability to the docs Specifications index (`tests/specifications_page_lists_every_capability.rs`)

## 6. Gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 6.3 `just deny`
- [ ] 6.4 `mdbook build docs/`
- [ ] 6.5 `openspec validate uniform-json-output --strict`
