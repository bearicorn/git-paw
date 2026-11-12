## Why

Claude Code shows a greyed prompt suggestion (a predicted next user message) in an idle input box.
In the v0.19.0 peer-4-poker run and the v0.20.0 dogfood, idle workers received messages nobody
sent ("do T012 … next", "poll again", "go with option 2"): no broker event or drive-log line
recorded them (PK-12, PK-18). The likely path is git-paw itself sending a lone Enter to an idle
pane (buffered-input recovery, `sweep.sh tell`, an approval) while a suggestion is showing. A
worker that acts on a message its orchestrator never sent breaks the stand-by-after-commit
protocol and drives scope creep.

## What Changes

- The `claude` operator-isolation row gains `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION=false`, the
  documented per-session switch (Claude Code 2.1.238+; it takes precedence over the
  `promptSuggestionEnabled` setting). Every worker launch path gets it through the one builder.
- `inherit_operator_config = true` leaves suggestions as the operator configured them.
- Not in scope: the operator's own session and the orchestrator one-shot (unchanged, as for the
  rest of operator isolation).

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `cli-provider`: ADDED — claude-family workers launch without prompt suggestions; MODIFIED — the
  exact launch-line and environment scenarios of *Each agent CLI resolves into one record*, *The
  ambiguous shim is reported before a launch*, *Operator isolation is a provider facet* and
  *Operator isolation is placed by the launch builder* gain the variable.

## Impact

- **Code:** `src/agent_cli/mod.rs` (the `claude` provider's `operator_isolation()` env list).
- **Tests:** the exact launch-line assertions in `src/agent_cli/tests.rs`, `src/commands/launch.rs`,
  `tests/cli_provider_dry_run.rs` and `tests/worker_operator_isolation_e2e.rs` gain the variable.
- **Docs:** `docs/src/supported-clis.md` (operator isolation table and example line),
  `docs/src/user-guide/security-posture.md` (what a worker inherits).
- **Fact gate:** confirm with Claude Code 2.1.322 that the variable hides the suggestion in an
  interactive session (capture an idle pane before and after).
