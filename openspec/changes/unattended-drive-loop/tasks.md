## 1. CLI surface

- [ ] 1.1 Add `unattended: bool` to `StartArgs` in `src/cli.rs` with `#[arg(long)]` and `help` text describing the use case
- [ ] 1.2 Add a clap conflict between `--unattended` and `--no-supervisor` (mutually exclusive)
- [ ] 1.3 Unit tests: `--unattended` sets the flag; absent → false; combines with `--from-specs`/`--cli`; `--unattended --no-supervisor` is rejected
- [ ] 1.4 CLI binary test: `git paw start --help` output contains `--unattended` and its description

## 2. Dispatch wiring

- [ ] 2.1 In the `start` dispatch (`src/main.rs`), make `--unattended` resolve supervisor mode active (route to `cmd_supervisor`) per the cli-parsing delta
- [ ] 2.2 Thread the `unattended` flag into `cmd_supervisor` (and recovery path) so step 15 can branch on it
- [ ] 2.3 Test: `--unattended --branches a,b` routes to the supervisor launch path

## 3. Drive loop module (`src/supervisor/drive.rs`)

- [ ] 3.1 Create the module with a `run_drive_loop(...)` entry that blocks until an exit condition and returns a summary/exit status
- [ ] 3.2 Implement the ~15s poll cadence and the per-iteration pane sweep
- [ ] 3.3 Pure helper: `is_live_prompt(capture: &str) -> bool` — footer in the last ~4 non-blank lines (scrollback ignored); unit-test both directions
- [ ] 3.4 Per-pane explicit `tmux capture-pane` (one spawn per pane); assert no shell `for`-loop capture path
- [ ] 3.5 Pure helper: resolve pane→agent via `pane_current_path` matched against session `worktree_path` (pane 0/1 → repo root); unit-test with non-alphabetical, non-arg-order pane indices

## 4. Approval + escalation

- [ ] 4.1 Call the `auto-approve-classifier` per live prompt; on safe → send documented approve-and-remember sequence (each key separate); log to broker before keystrokes
- [ ] 4.2 2-option Yes/No prompts → send `"1"` then a separate `Enter` (not Down+Enter)
- [ ] 4.3 Cover pane 0 (supervisor) in the sweep (W15-3); minimal-keystroke-only approval that cannot leave stray input (W15-13); no keystrokes when pane 0 has no live prompt
- [ ] 4.4 Non-blocking escalation: `danger`/`unknown` prompts surfaced (broker + summary) without freezing the wave; the rest of the wave keeps progressing
- [ ] 4.5 send-keys nudges send text then a separate follow-up `Enter`

## 5. Dedup + stall + cycle tolerance

- [ ] 5.1 Pure helper: derive dedup `shape` from command/agent identity (or wait-for-clear token), NEVER from boilerplate text (W15-19); 5-minute `(agent_id, shape)` window
- [ ] 5.2 Unit-test: repeated identical alert → one alert/window; two distinct prompts sharing boilerplate → two alerts
- [ ] 5.3 Pane-keyed stall iteration (W15-7): feed every pane to `supervisor-stuck-bloat-detection`, including panes with no broker record yet
- [ ] 5.4 Tolerate N feedback→fix→re-verify cycles; never flag an iterating agent as stuck on cycle count

## 6. Completion, heartbeat, summary

- [ ] 6.1 Completion detection: terminal PASS/FAIL verdict OR all agents' tasks checked
- [ ] 6.2 ~25-minute heartbeat: surface a status summary instead of running forever
- [ ] 6.3 Render the exit summary: outcome, per-agent final state, deduped escalations awaiting review, pointer to broker log + learnings; unit-test the renderer
- [ ] 6.4 Disable the dashboard auto-approve thread for `--unattended` sessions so the in-process loop is the sole approver

## 7. Self-captured learnings

- [ ] 7.1 Opportunistic capture during the run via `sweep.sh learn` (no raw curl) when the loop absorbs friction
- [ ] 7.2 Wind-down synthesis pass via `sweep.sh learn`, deduped against in-session learnings

## 8. Tests (E2E via selftest harness)

- [ ] 8.1 E2E: `--unattended` against the selftest isolation harness (dummy CLI emitting a safe prompt + completion signal) auto-approves, detects completion, exits with a summary, no real LLM
- [ ] 8.2 E2E: a `danger` prompt is escalated without blocking the other agent
- [ ] 8.3 E2E: supervisor pane-0 safe prompt is approved; pane 0 with no prompt is left untouched
- [ ] 8.4 Map every `unattended-operation` scenario to at least one test (unit or E2E)

## 9. Cleanup + docs

- [ ] 9.1 Remove the `.git-paw/scripts/wave*-monitor.sh` pane-scrapers as the dogfood monitoring path
- [ ] 9.2 Update README CLI table and mdBook user guide with `--unattended`; `mdbook build docs/` succeeds
- [ ] 9.3 `just check` + `just deny` pass; no `unwrap()`/`expect()` in non-test code; doc comments on all new public items
