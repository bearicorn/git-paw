## Why

The supervisor's original raison d'être was to run a multi-agent wave to completion with no human babysitting — yet today every wave still needs a human (or a throwaway `.git-paw/scripts/wave*-monitor.sh` pane-scraper) to watch panes, clear safe prompts, decide what is risky, and notice when the wave is done. The babysitting heuristics are battle-tested but perishable: they live as ad-hoc shell incantations and supervisor-skill prose, and the v0.6.0 dogfood (findings W15-3/7/13/19) proved the current auto-approve loop literally cannot drive itself unattended — it watches coding agents only, so the supervisor stalls on its own first non-allowlisted prompt. This change makes the drive loop a first-class, in-tool mechanism behind `git paw start --unattended`, so a wave can run to PASS/FAIL with no human in the seat.

## What Changes

- Add a `git paw start --unattended` flag that engages an in-tool **drive loop**: a poll loop (~15s) that sweeps the supervisor pane and every agent pane, auto-approves classifier-safe prompts, escalates the rest for later human review without blocking the wave, detects completion, and exits with a summary.
- Encode the battle-tested operator-loop heuristics as normative requirements: act only on a LIVE prompt (footer in the last ~4 non-blank lines); capture each pane EXPLICITLY (no `for p in …` shell loops); resolve pane→agent via `pane_current_path` (NOT pane index or CLI-arg order); send-keys nudges need a FOLLOW-UP Enter; for 2-option Yes/No prompts send `"1" Enter` directly; dedup repeated alerts by `(agent_id, shape)` within a 5-minute window.
- Encode the v0.6.0 hard constraints: the approver MUST cover the supervisor's OWN pane (pane 0) but MUST NOT blindly type into it (W15-3, W15-13); the stall detector MUST be pane-keyed not agent-record-keyed so it sees panes with no broker presence yet (W15-7); prompt dedup MUST key on command/agent identity or wait-for-clear, NEVER on prompt boilerplate (W15-19); and the loop MUST tolerate multiple feedback→fix→re-verify cycles per agent rather than calling "not yet verified after N cycles" a stuck signal.
- Escalation is non-blocking: a risky/unknown prompt is surfaced (broker + summary) for LATER human review; the wave continues with the remaining agents rather than blocking forever.
- The loop self-captures qualitative learnings via the supervisor observation channel (there is no human to hand-write findings in an unattended wave).
- Replace the external `.git-paw/scripts/wave*-monitor.sh` pane-scraping monitors with this in-tool loop.

## Capabilities

### New Capabilities
- `unattended-operation`: the `--unattended` drive loop — its poll/sweep cycle, the LIVE-prompt and pane→agent-resolution heuristics, the supervisor-pane-0 coverage rules, the pane-keyed stall detector, identity-keyed dedup, non-blocking escalation, completion detection, the exit summary, the feedback-cycle tolerance, and the self-captured-learnings wiring.

### Modified Capabilities
- `cli-parsing`: add the `--unattended` flag to `git paw start` (boolean, default `false`), its `--help` text, and its interaction with the supervisor-mode resolution chain.
- `supervisor-launch`: when `--unattended` is set, the launch path SHALL drive the loop in-process to completion instead of returning immediately with an attach hint (the v0.5.0 `cmd_supervisor` returns-immediately behaviour is unchanged when `--unattended` is absent).

## Impact

- New module (e.g. `src/supervisor/drive.rs`) implementing the poll loop, pane-sweep, classifier dispatch, escalation, completion detection, and summary. Reuses `safe-command-classification` (`is_safe_command`), `automatic-approval` keystroke discipline, `stuck-prompt-detection` (pane-keyed stall), and the broker publish path.
- `src/cli.rs` (`StartArgs.unattended: bool`) + dispatch in `src/main.rs` (routes through `cmd_supervisor`, then drives the loop when `--unattended`).
- **Depends on (ships same release, v0.9.0):** `auto-approve-classifier` (#5 — the safe/danger classifier the loop calls), `supervisor-stuck-bloat-detection` (#6 — the stuck/bloat signal the loop treats as an exit condition), `learnings-supervisor-observation-channel` (#12 — the self-capture path for learnings), and `selftest-harness` (#2 — lets the loop be E2E-tested with a dummy CLI, no real LLM).
- Removes `.git-paw/scripts/wave*-monitor.sh` as the dogfood monitoring path (superseded by the in-tool loop).
- `--help`, README CLI table, mdBook user guide gain `--unattended`.
- No new third-party dependencies; uses existing tmux/broker/classifier/session modules.
