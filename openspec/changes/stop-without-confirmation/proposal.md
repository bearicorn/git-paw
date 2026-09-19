## Why

`git paw stop`'s documented contract and its behaviour disagree. The `cli-parsing` and
`broker-runtime` specs and the `stop --help` text say that `stop` prompts for confirmation
on a TTY and that `--force` skips the prompt. The code never prompts — `cmd_stop(_force)`
ignores its argument — and `tests/cli_prompt_matrix.rs` asserts that no prompt appears.

Decision (maintainer): **the code is right.** Stopping is recoverable — worktrees, branches
and session state survive, and `git paw start` recovers the session — so a confirmation
prompt protects nothing. With no prompt, `--force` has nothing to skip, so it is removed
rather than kept as a silent no-op.

## What Changes

- **BREAKING:** `git paw stop` no longer accepts `--force`. A script passing it now fails
  with clap's unexpected-argument error. Remove the flag from the invocation; `stop` already
  behaved as if it were always set.
- The `cli-parsing` and `broker-runtime` requirements stop describing a prompt and the flag.
- `stop --help` and the CLI reference no longer claim a prompt or document `--force`.
- `cmd_stop` loses its ignored parameter.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-parsing`: *Stop subcommand* — no prompt, no `--force`.
- `broker-runtime`: *Stop flow shuts down broker via pane 0 exit* — no prompt in any context.

## Impact

- `src/cli.rs` — the `Stop` variant loses `force`; the `long_about` drops the prompt and
  `--force` example; the parse test at ~:1406 is rewritten.
- `src/main.rs` — the `Command::Stop` dispatch; `src/commands/stop.rs` — `cmd_stop` signature.
- `tests/stop_confirmation_test.rs`, `tests/pause_e2e.rs` — invocations of `stop --force`.
- `docs/src/cli-reference.md` stop section (the "v0.5.0 change … now prompts" note and the
  `stop --force` example); any other `stop --force` mention in docs or README.
- **Breaking change**: commit with `!` and a `BREAKING CHANGE:` footer so it surfaces in the
  changelog.
- **Delta coordination**: `broker-repo-identity` modifies three *other* `broker-runtime`
  requirements and renames the broker log; this delta refers to "the session's broker log"
  so the two read consistently once both are archived. No other change touches `cli-parsing`.
