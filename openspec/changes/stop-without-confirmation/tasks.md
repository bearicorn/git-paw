## 1. CLI surface

- [ ] 1.1 Remove `force` from the `Stop` variant in `src/cli.rs`; passing `--force` must now be a clap unexpected-argument error
- [ ] 1.2 Rewrite `stop`'s `long_about`: keep the three-verb summary, drop the prompt description and the `git paw stop --force` example
- [ ] 1.3 Replace the parse test at `src/cli.rs` ~:1406 with: `stop` parses to `Command::Stop`, and `stop --force` is rejected naming the flag
- [ ] 1.4 Remove the ignored parameter from `cmd_stop` (`src/commands/stop.rs`) and update the dispatch in `src/main.rs`

## 2. Tests

- [ ] 2.1 Drop `--force` from the `stop` invocations in `tests/stop_confirmation_test.rs` and `tests/pause_e2e.rs`; keep their assertions about what stop does
- [ ] 2.2 Keep `tests/cli_prompt_matrix.rs`'s no-prompt assertion as the test for "Stop from a TTY does not prompt"
- [ ] 2.3 Add a `--help` assertion: no prompt or `--force` mention, three verbs named
- [ ] 2.4 Grep the tree for any remaining `stop --force` (docs, README, tests, scripts) and fix each

## 3. Docs

- [ ] 3.1 `docs/src/cli-reference.md` stop section: remove the "v0.5.0 change … now prompts" note and the `stop --force` example; state that stop never prompts and why
- [ ] 3.2 Confirm README and the pause chapter agree

## 4. Gates

- [ ] 4.1 Commit as a breaking change: `!` after the type/scope and a `BREAKING CHANGE:` footer stating that `stop --force` is removed and how to migrate
- [ ] 4.2 Verify every scenario maps to a test
- [ ] 4.3 `cargo fmt`, then `just lint`
- [ ] 4.4 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 4.5 `mdbook build docs/`
- [ ] 4.6 `openspec validate stop-without-confirmation --strict`
