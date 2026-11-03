## 1. cli-provider follow-ups

- [ ] 1.1 Reference "The entry's args precede the approval flags and the headless form" from `the_entry_args_precede_the_approval_flags_and_headless_form` in `src/supervisor/escalation.rs`; add a test for "A CLI without args keeps its headless form" if none covers it
- [ ] 1.2 Unit test for "The launch command's leading token is never the id" (`[clis.foo] command = "claude"` → id `foo`, family `generic`)
- [ ] 1.3 End-to-end test for "A declared family reaches the worker's boot prompt end to end", modelled on `tests/cli_launch_identity_e2e.rs::mcp_opt_in_reaches_worker_boot_prompt` with `family = "claude"` and no `mcp`
- [ ] 1.4 `docs/src/supported-clis.md`: `--cli "claude --model x"` resolves the base CLI's record, so it gets `claude`'s approval flags and session ids; one line that `family` and `args` are set in the config file (`git paw add-cli` does not take them)
- [ ] 1.5 `src/cli.rs` `start --dry-run` help and `docs/src/cli-reference.md`: the plan prints a `runs:` line with each worker's exact pane command

## 2. cli-prompt-profiles follow-ups

- [ ] 2.1 Reference the three new requirements' scenarios from `an_empty_marker_fails_the_load_naming_the_field`, `a_shell_matched_pattern_outside_grep_e_fails_the_load` and `prompt_profile_subcommand_fails_on_an_invalid_config` in `tests/prompt_profile_threading.rs`
- [ ] 2.2 Test for "A CLI swap on a kept worktree re-resolves the profile" (session-state record after `remove --keep-worktree` + `add --cli mycli`, then loop and helper resolution)
- [ ] 2.3 Test for "A pane with no recorded CLI resolves the session CLI in the helper"
- [ ] 2.4 `docs/src/user-guide/cli-prompt-profiles.md`: configs with empty markers or non-ERE constructs in `option_line_pattern` / `context_bloat_pattern` / `paste_buffer_pattern` now fail to load (breaking for those configs since v0.19); a CLI swap re-resolves the profile; the helper and loop differ for a pane with no recorded CLI
- [ ] 2.5 `docs/src/configuration/README.md`: link the `prompt_profile` row to the `grep -E` subset in the user guide

## 3. worker-sandbox-mode follow-up

- [ ] 3.1 `src/commands/launch.rs` `dry_run_launch_line`: apply the native-sandbox refusal before formatting the `runs:` line, so the refusal is printed in its place
- [ ] 3.2 Test for "A dry run reports the refusal instead of a command"
- [ ] 3.3 `docs/src/user-guide/sandbox.md`: a dry run shows the refusal for a refused native-sandbox worker

## 4. Quality gates

- [ ] 4.1 `cargo fmt`, then `just lint`
- [ ] 4.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 4.3 `just deny`
- [ ] 4.4 `mdbook build docs/`
- [ ] 4.5 `openspec validate v019-review-followups --strict`
