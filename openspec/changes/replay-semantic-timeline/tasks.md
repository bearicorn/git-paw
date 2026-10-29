## 1. Seams (behaviour-preserving)

- [ ] 1.1 Move `type_short`, `derive_summary` and the heartbeat key from `src/dashboard/broker_log.rs` to `src/broker/summary.rs`; re-export from the dashboard; existing dashboard tests pass unchanged; update the AGENTS.md §7 ripple list to the new file
- [ ] 1.2 Split `src/replay.rs` into `src/replay/` (`mod.rs`, `timeline.rs`); generalize `strip_ansi` into `sanitize_terminal_text` (CSI/OSC removal + control-character neutralisation); keep the existing strip tests and add "Control characters neutralised"

## 2. Timeline core (pure)

- [ ] 2.1 Domain types: `TimelineInputs`, `TimelineEntry` (`at`, `agent`, `source`, `kind`, `summary`, `repeat`), `Timeline` (entries + notes), `AgentFilter`
- [ ] 2.2 `build()`: manifest → session/agent entries; events → broker entries via `broker::summary`; commits and transcript turns → entries; every summary sanitized
- [ ] 2.3 Ordering and tie-break; tests "Sources interleave by time", "Ties keep source order"
- [ ] 2.4 Per-agent heartbeat collapse; tests "Identical heartbeats collapse", "A different entry ends the run"
- [ ] 2.5 Filter; core-level tests for "Timeline filtered to one agent" and "Hostile broker text cannot reach the terminal" (core output contains no ESC)
- [ ] 2.6 Pure record selection and agent matching over the `session-record-capture` enumeration; tests "Most recent record selected", "Explicit record", "Session name selects its latest record", "Specified session does not exist", "Match by branch name", "Match by agent id", "Match by sanitized name", "No matching agent"

## 3. Source adapters

- [ ] 3.1 Record reader: manifest + `events.jsonl` (line-by-line, skipped-line count); tests "Broker events become entries", "Malformed event line skipped", "Sparse record"
- [ ] 3.2 `src/git.rs`: `commits_in_range(repo, from, to)` via `CommandRunner` with `%x1f`-separated `log --reverse` output; fake-runner unit tests for parsing and for an unresolvable ref
- [ ] 3.3 Commit range resolution (`end_commit`, else branch, else note); real-git tests "Commits are bounded by the agent's start commit", "Merged and deleted branch keeps its commits", "Unresolvable commit range"

## 4. Transcript sources

- [ ] 4.1 Verify the Claude-format facts listed in design D5 against the shipped Claude Code version; capture a minimal, scrubbed transcript fixture under `tests/fixtures/`; record the verification in the change's design
- [ ] 4.2 `TranscriptSource` trait + `NoTranscript` Null Object + resolution from the resolved CLI profile (name fallback `claude` until the v0.19 provider facet exists)
- [ ] 4.3 Claude-format pure parser (user / assistant text first line, tool name + primary target, tool results dropped, skipped-line count); tests "Transcript rendered", "Tool results are not rendered", "Unparseable transcript lines are counted"
- [ ] 4.4 Claude-format locator: UUID-shape validation, candidate dirs (`settings_path` parent, `CLAUDE_CONFIG_DIR`, `~/.claude`) passed explicitly so tests never mutate the process env, one-level `projects/*/` listing, no symlink following; tests "Non-UUID session id is never used as a path", "Transcript no longer present", "CLI without a transcript source"

## 5. Command, CLI and rendering

- [ ] 5.1 `cli.rs` `Replay`: optional `<agent>`, `--list`, `--session`, `--json`, `--raw` (requires agent; conflicts with `--json`/`--list`), drop `--color`; `about`/`long_about` with examples and the `--raw` unsanitized note
- [ ] 5.2 Parse tests in `tests/cli_tests.rs`: "Replay with agent", "Replay with no arguments", "Replay with --list", "Replay with --session", "Replay with --json", "Replay with --raw", "--raw without an agent is rejected", "--raw with --json is rejected", "--color is no longer accepted", "Replay help text"
- [ ] 5.3 Library `replay(repo, config, options) -> ReplayOutcome` (no stdout, no `current_dir`, no `expect`); dispatch renders text (fixed-offset-testable) / JSON wire type `TimelineDoc` (`schema_version` 1, per `cli-machine-output`) / raw stream; notes → stderr in text mode only
- [ ] 5.4 `--list` text + JSON rendering; tests "Records available", "Legacy record marked", "Listing as JSON", "No sessions available"
- [ ] 5.5 Legacy handling + raw; tests "Timeline requested on a legacy record", "Raw bytes are unmodified", "Raw works on a legacy record", "No capture for the agent", "No logs directory"
- [ ] 5.6 End-to-end (`assert_cmd`, no tmux needed): build a record fixture (manifest, `events.jsonl`, a git repo with agent commits, a transcript fixture under a temp `CLAUDE_CONFIG_DIR`) and assert "Whole-session timeline", "Timeline filtered to one agent", "Notes go to stderr", "JSON document parses", "Notes inside the document"
- [ ] 5.7 `cli-machine-output`: add `replay` to the standing JSON-roster test and pin the golden key sets of both replay documents; tests "The flag set matches the roster", "Every roster command is documented", "Both replay documents carry the schema version"
- [ ] 5.8 `core-terminal-output`: replace the `replay --color` exception in the standing piped / `NO_COLOR=1` escape test with `replay <agent> --raw`; tests "The replay timeline is plain when the record holds escapes", "Raw replay output is passed through unmodified"

## 6. Docs

- [ ] 6.1 `docs/src/user-guide/session-logging.md` "Replaying": timeline (sources, what is and is not shown), `--json`, `--raw`, legacy records, the `--color` migration
- [ ] 6.2 `docs/src/cli-reference.md` `git paw replay` usage block and examples, and its timeline and `--list` JSON documents (with `schema_version`) in the JSON-output section; README CLI table row and the Session logging / Replay feature bullets
- [ ] 6.3 `docs/src/architecture.md` Replay row (`src/replay/`, sources, functional core); `docs/src/specifications/README.md` session-logging summary line
- [ ] 6.4 After archive: rewrite the `session-logging` `## Purpose` to describe the per-run record and the timeline / raw (/ cast) replay; add `Test:` pointers to the touched requirements

## 7. Quality gates

- [ ] 7.1 `just check` passes, verified by the real exit code
- [ ] 7.2 `just deny` passes; no new dependency
- [ ] 7.3 `mdbook build docs/` passes
