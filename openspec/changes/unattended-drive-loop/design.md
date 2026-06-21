## Context

git-paw's supervisor mode (v0.5.0) launches a multi-agent tmux session — supervisor pane (pane 0), dashboard pane (pane 1), and a grid of coding-agent panes — then `cmd_supervisor` **returns immediately** with an attach hint. The drive loop that keeps the wave moving (clearing safe permission prompts, escalating risky ones, noticing when work is done) is today either a human watching `tmux attach`, or a throwaway `.git-paw/scripts/wave*-monitor.sh` pane-scraper hand-written per dogfood session.

The auto-approve subsystem (`automatic-approval`, `safe-command-classification`) already runs inside the `__dashboard` subprocess and clears classifier-safe prompts on **coding-agent** panes. But the v0.6.0 dogfood (findings W15-*) proved it cannot drive a wave unattended:

- **W15-3** — the approver watches coding agents only. The supervisor's own pane (pane 0) is an agent too; it hits permission prompts (running `sweep.sh`, `cargo test`, `git`); nothing clears them, so an unattended supervisor stalls on its first non-allowlisted prompt.
- **W15-13** — but pane 0 is special: clearing its prompt vs polluting its conversation context are different failure modes, so a send-keys approver must NOT blindly type into pane 0.
- **W15-7** — the stall detector keyed off broker agent records, so it was blind to a pane that has booted but not yet published its first `agent.status` (the boot chicken-and-egg). It must be **pane-keyed**.
- **W15-19** — prompt dedup that keyed on the prompt's boilerplate text ate repeated-but-distinct prompts (v0.6.0 dogfood bug 9). Dedup must key on command/agent identity or wait-for-clear.
- Multiple feedback→fix→re-verify cycles per agent are normal; "not verified after N cycles" is NOT a stuck signal.

This change packages the perishable operator-loop heuristics as a first-class in-tool drive loop behind `git paw start --unattended`.

This design depends on four sibling v0.9.0 changes (all ship in the same release):
- `auto-approve-classifier` (#5) — supplies the safe/danger/unknown classifier the loop calls per captured prompt.
- `supervisor-stuck-bloat-detection` (#6) — supplies the stuck/bloat signal the loop treats as an exit/re-engage condition.
- `learnings-supervisor-observation-channel` (#12) — supplies `sweep.sh learn`, the self-capture path the loop uses since no human writes findings.
- `selftest-harness` (#2) — supplies the isolated dummy-CLI lifecycle harness so the loop is E2E-testable with no real LLM backend.

## Goals / Non-Goals

**Goals:**
- One flag — `git paw start --unattended` — runs a supervisor wave to completion with no human in the seat.
- Auto-approve classifier-safe prompts on EVERY agent pane, including the supervisor's own pane 0.
- Escalate risky/unknown prompts WITHOUT blocking the wave — surface for later human review and keep the remaining agents moving.
- Detect completion (PASS/FAIL signal, or all tasks checked) and exit with a human-readable summary.
- Encode the battle-tested heuristics (LIVE-prompt-only, explicit per-pane capture, `pane_current_path` resolution, follow-up Enter, `"1" Enter` for Yes/No, identity-keyed dedup) as normative requirements so they survive in-tool.
- Self-capture qualitative learnings during the run (no human to hand-write findings).
- Be E2E-testable with a dummy CLI via the selftest harness — no real LLM, no interactive terminal.

**Non-Goals:**
- Replacing the supervisor *agent's* judgement on substantive decisions (merge conflicts, design choices). The loop clears mechanical prompts and escalates the rest; it does not adjudicate.
- Adding a new classifier. The classifier is `auto-approve-classifier` (#5); this loop is a consumer.
- Driving non-supervisor (`cmd_start`) sessions unattended. `--unattended` requires supervisor mode.
- A configurable pane layout or >25 agents (still deferred to v1.0.0 per `supervisor-launch`).
- Windows-native support (tmux is Unix-only, unchanged).

## Decisions

### D1: The loop runs in-process from `cmd_supervisor`, not inside `__dashboard`

The existing coding-agent auto-approve runs as a thread inside the long-lived `__dashboard` subprocess (so it dies when the dashboard pane closes). The unattended drive loop instead runs **in the foreground `cmd_supervisor` process** and blocks until completion.

- **Why:** `--unattended` is fundamentally "this invocation drives the wave to done and then exits." The exit code + summary belong to the `git paw start --unattended` process. A loop buried in `__dashboard` cannot own the parent's exit status, and the dashboard pane may not even be attached.
- **Interaction with the existing dashboard approver:** when `--unattended` is set, the in-process drive loop is authoritative. To avoid two approvers racing on the same pane, the dashboard's auto-approve thread SHALL be disabled for unattended sessions (the drive loop covers all panes, including the ones the dashboard thread used to cover). This is a normative requirement in the spec.
- **Alternative considered:** keep the loop in `__dashboard` and have `cmd_supervisor` poll the broker for a completion message. Rejected — it splits ownership of the exit decision across two processes and reintroduces the W15-7 chicken-and-egg (the broker may not know about a pane yet).

### D2: Pane→agent resolution via `pane_current_path` (never index, never CLI-arg order)

Each poll iteration enumerates panes with `tmux list-panes -t <session> -F '#{pane_id} #{pane_current_path}'` and resolves each pane to its agent by matching `pane_current_path` against the session JSON's per-agent `worktree_path`. Pane 0 (supervisor) and pane 1 (dashboard) resolve to the repo root.

- **Why:** dogfood (memory `feedback_verify_pane_to_agent_mapping`) confirmed pane indices are NOT alphabetical and NOT CLI-arg order; the only stable key is the working directory. The supervisor-launch spec already pins agent panes to `-c <worktree>`, so `pane_current_path` is authoritative.
- **Alternative considered:** trust the pane index from session JSON. Rejected — tmux renumbers panes on layout changes and the dogfood proved index drift.

### D3: Per-pane explicit capture — no shell `for` loops

The loop captures each pane with an explicit `tmux capture-pane -p -t <pane_id>` invocation (one process spawn per pane), never a single `for p in $(...)` shell loop.

- **Why:** memory `feedback_avoid_loop_pane_capture` — `$p` expansion trips the simple-expansion approval gate, and a loop hides which pane produced which capture. Explicit per-pane capture keeps the captures attributable and the command allowlist-clean.

### D4: Act only on a LIVE prompt (footer in last ~4 non-blank lines)

A captured pane is treated as showing an actionable prompt ONLY when a recognized prompt footer appears within the last ~4 non-blank lines of the capture. Prompt-like text scrolled up in history is ignored.

- **Why:** a CLI's scrollback contains every past prompt; acting on a historical prompt would re-approve something already resolved or type into a pane that has moved on. The footer-in-tail rule is the operator heuristic that distinguishes a live prompt from scrollback.

### D5: Approve via the CLI's documented keystroke sequence; pane 0 via a no-pollution path

For a coding-agent pane with a classifier-**safe** prompt, the loop sends the CLI's documented "approve and remember" sequence (`automatic-approval`: `BTab Down Enter`, each key as a separate `send-keys`). For a 2-option Yes/No prompt it sends `"1" Enter` directly (memory `feedback_sweep_approve_interface` — Down+Enter picks the wrong option on 2-option prompts).

Pane 0 (supervisor) is approved through a path that **cannot leave stray input in the supervisor's conversation** (W15-13): the loop sends ONLY the minimal approval keystrokes that the live prompt consumes, and never sends free-text or a trailing newline that would land in the supervisor's prompt box. If the pane-0 capture does not show a live, recognized permission prompt, the loop does nothing to pane 0.

- **Alternative considered:** route pane-0 approvals through a broker round-trip. Rejected for v0.9.0 as over-engineered; the minimal-keystroke rule is sufficient and is itself a normative spec requirement so it can't regress.

### D6: send-keys nudges need a follow-up Enter

Any nudge the loop sends (e.g. re-prompting a stalled agent) sends the text, then a SEPARATE `Enter`, because the first Enter buffers rather than submits on paste-aware CLIs (memory `feedback_sendkeys_nudge_needs_followup_enter`).

### D7: Dedup keys on (agent identity, prompt shape) within 5 minutes — NEVER on boilerplate text

When the loop escalates or records an alert, it dedups on `(agent_id, shape)` within a 5-minute window, where `shape` is derived from the **command/agent identity** of the prompt (e.g. the program being approved, or a wait-for-clear token), NOT the prompt's boilerplate sentence (W15-19, memory `feedback_inbox_dedup_unknown_classification`). The same risky prompt re-observed every poll produces ONE escalation per window; two genuinely-distinct prompts from the same agent are NOT collapsed just because they share boilerplate.

### D8: Pane-keyed stall detection (not agent-record-keyed)

The stall/stuck detector keys on the tmux pane, not on a broker agent record (W15-7). A pane that has booted but never published an `agent.status` (boot chicken-and-egg) is still watched. The stuck signal itself comes from `supervisor-stuck-bloat-detection` (#6); this loop supplies the pane-keyed iteration so that change's detector sees every pane.

### D9: Tolerate N feedback→fix→re-verify cycles

An agent that has received feedback and is iterating (fix → re-verify → fix) is NOT stuck. The loop SHALL NOT treat "not yet verified after N cycles" as a stuck/exit signal. Only the explicit stuck/bloat signal (#6), a genuinely risky prompt, a completion signal, or a ~25-minute heartbeat are exit/re-engage conditions.

### D10: Exit / re-engage conditions and the summary

The loop polls on a ~15s cadence and runs until one of:
1. **Completion** — a PASS/FAIL signal (e.g. supervisor publishes a terminal verdict) or all agents' tasks are checked complete.
2. **Risky prompt** — a classifier-`danger`/`unknown` prompt that needs a human decision (escalated, non-blocking; the wave continues, but a risky prompt is also recorded for the human to review at wind-down).
3. **Stuck / bloat** — the `supervisor-stuck-bloat-detection` (#6) signal fires.
4. **Heartbeat** — ~25 minutes elapse with no completion, so the human is re-engaged with a status summary rather than the loop running forever silently.

On exit the loop prints a summary: outcome (completed / escalated-for-review / stuck / heartbeat), per-agent final state, the list of escalated prompts awaiting human review (deduped), and a pointer to the broker log + captured learnings.

- **Why non-blocking escalation:** a single risky prompt on one agent must not freeze the other agents' progress. Escalations are queued for human review at the summary, while the rest of the wave runs. This is the key difference from the old "block on the prompt forever" monitor scripts.

### D11: Self-captured learnings

Because no human writes a findings file in an unattended wave, the loop records qualitative learnings through `sweep.sh learn` (`learnings-supervisor-observation-channel`, #12) — opportunistically when it absorbs friction (e.g. a prompt it had to escalate, a detector that over-fired) and once at wind-down (synthesis pass). The loop does NOT hand-roll raw curl for `agent.learning` (G4 anti-pattern).

### D12: Testability via the selftest harness

The loop is exercised end-to-end by the `selftest-harness` (#2): a private tmux socket, ephemeral broker port, throwaway repo, and a dummy CLI scripted to emit a fake permission prompt and a fake completion signal. This lets the completion/escalation/dedup/pane-resolution scenarios run in CI with no real LLM and no interactive terminal. Pure helpers (LIVE-prompt detection, dedup-key derivation, pane→agent resolution, summary rendering) are unit-tested directly without tmux.

## Risks / Trade-offs

- **[A wrong auto-approval is irreversible mid-wave]** → The loop only auto-approves classifier-**safe** prompts; everything else escalates. The classifier (#5) owns the safe set; the loop never widens it. Pane-0 approvals are minimal-keystroke-only (D5).
- **[Two approvers race (in-process loop + dashboard thread)]** → The dashboard auto-approve thread is disabled for `--unattended` sessions (D1); the in-process loop is the sole approver.
- **[Loop runs forever if completion is never signalled]** → The ~25-minute heartbeat (D10) guarantees the human is re-engaged with a summary rather than an indefinite silent run.
- **[Polluting the supervisor's context by typing into pane 0]** → D5 minimal-keystroke rule + the "do nothing unless pane 0 shows a live recognized prompt" rule; both are normative spec requirements with scenarios.
- **[Dedup eats a distinct repeat prompt]** → D7 keys on command/agent identity, never boilerplate (W15-19); scenario asserts two distinct prompts from one agent are not collapsed.
- **[Stall detector blind to un-booted pane]** → D8 pane-keyed iteration (W15-7).
- **[Treating a normal feedback cycle as stuck]** → D9 explicit tolerance requirement with a scenario for N cycles.
- **[Non-TTY / detached runs]** → `--unattended` is designed for detached operation; it does not require an attached terminal and does not call `tmux::attach`. It blocks in-process and exits with a status code, which is what CI and the selftest harness consume.

## Migration Plan

- `--unattended` is opt-in and default `false`; omitting it preserves the exact v0.5.0 `cmd_supervisor` returns-immediately-with-attach-hint behaviour. No existing config or session JSON changes.
- The `.git-paw/scripts/wave*-monitor.sh` pane-scrapers are no longer the dogfood monitoring path; they can be deleted once `--unattended` lands. They were never tracked as a product surface, so there is nothing to deprecate in specs.
- Rollback: drop the `--unattended` flag handling; the loop module is dead code with the flag gone.

## Open Questions

- Exact wire shape of the "completion" signal: a dedicated terminal `agent.status` phase vs. an `agent.learning`/verdict message. Resolved against `supervisor-stuck-bloat-detection` (#6) and `supervisor-introspection` phase enum during apply; the spec states the behaviour (PASS/FAIL or all-tasks-checked) without pinning the wire field so #6 can own it.
- Whether the heartbeat interval (~25 min) and poll cadence (~15 s) should be configurable under `[supervisor]`. v0.9.0 ships fixed defaults; configurability deferred unless dogfood demands it.
